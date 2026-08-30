//! ミュート/ブロック突き合わせ（DESIGN §8）。
//!
//! サーバ側で `getTimeline` はミュート/ブロックを反映済みだが、ローカル DB には
//! 過去に取り込んだ分が残る。`getMutes` / `getBlocks` と突き合わせ、該当する
//! 著者・リポスト元の投稿へ `is_hidden = 1` を立てる（解除された分は 0 に戻す）。
//!
//! 週次で実行する（起動時に前回から 7 日以上経過していれば走らせる）。

use crate::auth::SessionManager;
use crate::db::{queries, Db};
use crate::error::{AppError, Result};
use chrono::Utc;
use std::sync::Arc;

/// sync_state のキー（前回実行時刻の保存先）。
const KEY: &str = "moderation";
/// 週次間隔（秒）。
const WEEK_SECS: i64 = 7 * 24 * 60 * 60;

/// ミュート/ブロックの DID を取得し、該当投稿の is_hidden を更新する。
/// 401 は一度だけリフレッシュして再試行する。
pub async fn reconcile(db: &Arc<Db>, session: &Arc<SessionManager>) -> Result<usize> {
    if session.current().is_none() {
        return Err(AppError::NotLoggedIn);
    }
    let client = session.client();

    // getMutes / getBlocks を全ページ取得。401 は refresh 後に一度だけ再試行。
    let dids = match fetch_dids(&client, session).await {
        Ok(d) => d,
        Err(AppError::Unauthorized) => {
            session.refresh().await?;
            fetch_dids(&client, session).await?
        }
        Err(e) => return Err(e),
    };

    let now = Utc::now().timestamp();
    let hidden = {
        let conn = db.0.lock().unwrap();
        let n = queries::apply_hidden_dids(&conn, &dids)?;
        queries::touch_sync_state(&conn, KEY, now, None)?;
        n
    };
    tracing::info!(
        "モデレーション突き合わせ: ミュート/ブロック {} DID、{} 投稿を非表示に",
        dids.len(),
        hidden
    );
    Ok(hidden)
}

/// getMutes と getBlocks の DID を結合して返す（重複は残るが apply 側で問題なし）。
async fn fetch_dids(
    client: &crate::bsky::BskyClient,
    session: &Arc<SessionManager>,
) -> Result<Vec<String>> {
    let token = session.valid_access_token().await?;
    let mut dids = client
        .list_graph_dids(&token, "app.bsky.graph.getMutes", "mutes")
        .await?;
    let blocks = client
        .list_graph_dids(&token, "app.bsky.graph.getBlocks", "blocks")
        .await?;
    dids.extend(blocks);
    Ok(dids)
}

/// 起動時: 前回実行から 7 日以上経過していれば突き合わせを走らせる。
pub async fn maybe_reconcile_weekly(db: Arc<Db>, session: Arc<SessionManager>) {
    if session.current().is_none() {
        return;
    }
    let due = {
        let conn = db.0.lock().unwrap();
        match queries::get_last_run_at(&conn, KEY) {
            Ok(Some(last)) => Utc::now().timestamp() - last >= WEEK_SECS,
            _ => true, // 未実行なら実行
        }
    };
    if !due {
        return;
    }
    if let Err(e) = reconcile(&db, &session).await {
        tracing::warn!("週次モデレーション突き合わせに失敗: {e}");
    }
}
