//! タイムラインのページング取得と DB 格納（DESIGN §6.2 / §6.3）。
//!
//! - 差分同期: 先頭から取得し、既知 URI に当たるか上限ページで打ち切る。
//! - 初回同期: cursor を辿って cutoff（既定 30 日前）または上限ページまで遡る。
//! - 429 は Retry-After / RateLimit-Reset に従い、無ければ指数バックオフ + jitter。
//! - 401 は refreshSession を一度挟んで再試行。
//! - cancel フラグで各ページ境界・待機中に中断可能。

use crate::auth::SessionManager;
use crate::bsky::{client::TimelinePage, BskyClient};
use crate::db::{queries, Db};
use crate::error::{AppError, Result};
use crate::sync::{extractor, SyncPhase, SyncRuntime};
use chrono::Utc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// sync_state のキー。
const KEY: &str = "timeline";
/// 1 ページの取得件数。
const PAGE_LIMIT: u32 = 100;
/// OGP サムネの取り込み（Phase 5 で設定連動）。既定は取り込まない。
const INCLUDE_EXTERNAL: bool = false;
/// レート残量がこれ未満なら自発的に間引く。
const RATE_LOW_THRESHOLD: i64 = 20;
/// 429 の最大リトライ回数。
const MAX_RATELIMIT_RETRIES: u32 = 6;
/// ネットワークエラーの最大リトライ回数。
const MAX_NETWORK_RETRIES: u32 = 3;
/// バックオフ下限・上限。
const BACKOFF_BASE_SECS: u64 = 5;
const BACKOFF_MAX_SECS: u64 = 300;

#[derive(Clone, Copy)]
pub enum SyncMode {
    /// 差分同期。上限ページ数まで、既知 URI で打ち切り。
    Differential { max_pages: u32 },
    /// 初回同期。cutoff_ts（Unix 秒）より古くなるか上限ページまで遡る。
    Initial { cutoff_ts: i64, max_pages: u32 },
}

impl SyncMode {
    fn max_pages(&self) -> u32 {
        match self {
            SyncMode::Differential { max_pages } | SyncMode::Initial { max_pages, .. } => *max_pages,
        }
    }
    fn phase(&self) -> SyncPhase {
        match self {
            SyncMode::Differential { .. } => SyncPhase::Diff,
            SyncMode::Initial { .. } => SyncPhase::Initial,
        }
    }
}

/// 同期 1 回分の集約結果。
pub struct SyncOutcome {
    pub pages: u32,
    pub media_added: u32,
    /// 遡り到達点（タイムライン基準、Phase 3 でステータス表示に利用予定）。
    #[allow(dead_code)]
    pub oldest_indexed: Option<i64>,
    pub cancelled: bool,
}

/// poller が必要とする依存の束。
pub struct PollContext {
    pub db: Arc<Db>,
    pub session: Arc<SessionManager>,
    pub client: BskyClient,
    pub runtime: Arc<SyncRuntime>,
    pub app: AppHandle,
}

/// 同期本体。単一タスクからのみ呼ばれる前提（Syncer が直列性を保証）。
pub async fn run(ctx: &PollContext, mode: SyncMode) -> Result<SyncOutcome> {
    let max_pages = mode.max_pages();
    let phase = mode.phase();
    ctx.runtime.begin(phase);
    emit_progress(ctx);

    let started = Instant::now();
    match mode {
        SyncMode::Differential { .. } => {
            tracing::info!(max_pages, "差分同期を開始");
        }
        SyncMode::Initial { cutoff_ts, .. } => {
            tracing::info!(max_pages, cutoff_ts, "初回同期を開始（cutoff より新しい分を遡る）");
        }
    }

    // 初回同期は保存済み cursor から遡りを継続する（レジューム）。差分同期は常に先頭から。
    let mut cursor: Option<String> = None;
    if let SyncMode::Initial { cutoff_ts, .. } = mode {
        let conn = ctx.db.0.lock().unwrap();
        let oldest_seen = queries::get_oldest_seen(&conn, KEY)?;
        // 既に指定期間（cutoff）まで遡り済みなら何もしない。差分同期・ポーリングが先頭を追随する。
        if oldest_seen.map(|o| o <= cutoff_ts).unwrap_or(false) {
            drop(conn);
            tracing::info!(oldest_seen, cutoff_ts, "初回同期: 既に指定期間分を保有しているためスキップ");
            return Ok(SyncOutcome {
                pages: 0,
                media_added: 0,
                oldest_indexed: oldest_seen,
                cancelled: false,
            });
        }
        cursor = queries::get_sync_cursor(&conn, KEY)?;
        if cursor.is_some() {
            tracing::info!(oldest_seen, "初回同期: 保存済み cursor から遡りを継続（レジューム）");
        }
    }

    let mut pages = 0u32;
    let mut media_added = 0u32;
    // 遡りの基準はタイムラインの新しさ（indexed_at）。created_at はリポストで
    // 原投稿の古い日付を持つため cutoff には使わない（learnings.md L4）。
    let mut oldest_indexed: Option<i64> = None;

    loop {
        if ctx.runtime.is_cancelled() {
            tracing::info!(pages, media_added, "同期を中断しました");
            return Ok(SyncOutcome {
                pages,
                media_added,
                oldest_indexed,
                cancelled: true,
            });
        }
        if pages >= max_pages {
            break;
        }

        let page = fetch_page_with_retry(ctx, cursor.as_deref()).await?;
        pages += 1;
        let next_cursor = page.feed.cursor.clone();
        let feed = &page.feed.feed;
        let items = feed.len();
        let media_before = media_added;

        // --- DB 格納（この間は await しない） ---
        let mut hit_known = false;
        {
            let conn = ctx.db.0.lock().unwrap();
            let now = Utc::now().timestamp();
            for item in feed {
                if matches!(mode, SyncMode::Differential { .. })
                    && queries::is_post_known(&conn, &item.post.uri)?
                {
                    hit_known = true;
                    break;
                }
                if let Some(post) = extractor::extract(item, INCLUDE_EXTERNAL) {
                    let n = queries::insert_post_with_media(&conn, &post, now)?;
                    media_added += n as u32;
                    oldest_indexed = Some(match oldest_indexed {
                        Some(prev) => prev.min(post.indexed_at),
                        None => post.indexed_at,
                    });
                }
            }
            // cursor は初回（バックフィル）専用。差分同期は cursor を壊さない。
            match mode {
                SyncMode::Initial { .. } => {
                    queries::set_sync_state(&conn, KEY, next_cursor.as_deref(), now, oldest_indexed)?;
                }
                SyncMode::Differential { .. } => {
                    queries::touch_sync_state(&conn, KEY, now, oldest_indexed)?;
                }
            }
        }

        ctx.runtime.update(|s| {
            s.page = pages;
            s.media_added = media_added;
            s.oldest_indexed_at = oldest_indexed;
        });
        emit_progress(ctx);
        tracing::info!(
            page = pages,
            items,
            page_media = media_added - media_before,
            total_media = media_added,
            "ページ取得"
        );

        // --- 打ち切り判定 ---
        if hit_known {
            tracing::info!(page = pages, "差分同期: 既知 URI に到達したため打ち切り");
            break;
        }
        if next_cursor.is_none() || feed.is_empty() {
            tracing::info!(page = pages, "タイムライン終端に到達");
            break; // タイムライン終端
        }
        if let SyncMode::Initial { cutoff_ts, .. } = mode {
            if oldest_indexed.map(|oi| oi < cutoff_ts).unwrap_or(false) {
                tracing::info!(page = pages, oldest_indexed, cutoff_ts, "初回同期: 遡り到達点に到達したため打ち切り");
                break; // 遡り到達（タイムライン基準で cutoff 日数分）
            }
        }
        cursor = next_cursor;

        // レート残量が少なければ自発的に間引く
        if let Some(rem) = page.rate.remaining {
            if rem < RATE_LOW_THRESHOLD {
                sleep_cancelable(ctx, Duration::from_secs(2)).await?;
            }
        }
    }

    tracing::info!(
        pages,
        media_added,
        oldest_indexed,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "同期完了"
    );
    Ok(SyncOutcome {
        pages,
        media_added,
        oldest_indexed,
        cancelled: false,
    })
}

/// 1 ページ取得。401→refresh、429→バックオフ、ネットワーク→限定リトライ。
async fn fetch_page_with_retry(ctx: &PollContext, cursor: Option<&str>) -> Result<TimelinePage> {
    let mut rl_attempt = 0u32;
    let mut net_attempt = 0u32;
    let mut unauth_retried = false;

    loop {
        if ctx.runtime.is_cancelled() {
            return Err(AppError::Cancelled);
        }

        let token = ctx.session.valid_access_token().await?;
        match ctx.client.get_timeline(&token, cursor, PAGE_LIMIT).await {
            Ok(page) => return Ok(page),

            Err(AppError::Unauthorized) if !unauth_retried => {
                unauth_retried = true;
                ctx.session.refresh().await?; // 失敗すれば RefreshFailed が伝播
            }

            Err(AppError::RateLimited { retry_after_secs }) => {
                if rl_attempt >= MAX_RATELIMIT_RETRIES {
                    return Err(AppError::RateLimited { retry_after_secs });
                }
                let wait = retry_after_secs
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| backoff(rl_attempt));
                rl_attempt += 1;
                tracing::warn!(
                    attempt = rl_attempt,
                    wait_secs = wait.as_secs(),
                    from_header = retry_after_secs.is_some(),
                    "429 レート制限。バックオフして再試行"
                );
                emit_throttled(ctx, wait.as_secs());
                sleep_cancelable(ctx, wait).await?;
                ctx.runtime.update(|s| s.throttled_until = None);
            }

            Err(AppError::Network(_)) if net_attempt < MAX_NETWORK_RETRIES => {
                net_attempt += 1;
                sleep_cancelable(ctx, Duration::from_secs(2)).await?;
            }

            Err(e) => return Err(e),
        }
    }
}

/// 指数バックオフ + jitter。乱数クレート不使用のため時刻ナノ秒から jitter を得る。
fn backoff(attempt: u32) -> Duration {
    let exp = BACKOFF_BASE_SECS.saturating_mul(1u64 << attempt.min(6));
    let capped = exp.min(BACKOFF_MAX_SECS);
    let jitter_ms = (Utc::now().timestamp_subsec_nanos() as u64 / 1_000_000) % 1000;
    Duration::from_secs(capped) + Duration::from_millis(jitter_ms)
}

/// 中断可能なスリープ（250ms 刻みで cancel を確認）。
async fn sleep_cancelable(ctx: &PollContext, total: Duration) -> Result<()> {
    let step = Duration::from_millis(250);
    let mut elapsed = Duration::ZERO;
    while elapsed < total {
        if ctx.runtime.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        let remaining = total - elapsed;
        let this = remaining.min(step);
        tokio::time::sleep(this).await;
        elapsed += this;
    }
    Ok(())
}

fn emit_progress(ctx: &PollContext) {
    let _ = ctx.app.emit("sync:progress", ctx.runtime.snapshot());
}

fn emit_throttled(ctx: &PollContext, seconds: u64) {
    let until = Utc::now().timestamp() + seconds as i64;
    ctx.runtime.update(|s| s.throttled_until = Some(until));
    let _ = ctx.app.emit(
        "ratelimit:throttled",
        serde_json::json!({ "seconds": seconds, "until": until }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_exponential_and_capped() {
        // jitter は 0〜999ms。秒部分は指数（base * 2^attempt）を上限 300 秒で頭打ち。
        let base = BACKOFF_BASE_SECS;
        for attempt in 0..3 {
            let d = backoff(attempt);
            let expected = base * (1u64 << attempt);
            let lo = Duration::from_secs(expected);
            let hi = Duration::from_secs(expected) + Duration::from_millis(1000);
            assert!(
                d >= lo && d < hi,
                "attempt {attempt}: {d:?} は [{lo:?}, {hi:?}) の範囲外"
            );
        }
    }

    #[test]
    fn backoff_saturates_at_max() {
        // 大きな attempt でも上限 300 秒 + jitter に収まる。
        let d = backoff(20);
        let lo = Duration::from_secs(BACKOFF_MAX_SECS);
        let hi = Duration::from_secs(BACKOFF_MAX_SECS) + Duration::from_millis(1000);
        assert!(d >= lo && d < hi, "{d:?} は上限 300 秒に頭打ちされるべき");
    }
}
