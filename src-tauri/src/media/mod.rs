//! メディアのディスクキャッシュとカスタム URI スキーム配信（Phase 3–5）。
//!
//! - `thumb://…/<media_id>`  … サムネ（グリッド）。全タイルで使う。
//! - `full://…/<media_id>`   … fullsize（ビューア）。開いた時だけ取得・保存（DESIGN §9）。
//!
//! ディスクヒットは即配信、ミスは Bluesky CDN から DL → 保存 → 配信する。
//! API・CDN へのアクセスはすべて Rust に閉じる（DESIGN §3.1）。
//! 動画（HLS）はフロントの hls.js が playlist を直接ストリーミングする（キャッシュ対象外）。
//!
//! LRU（DESIGN §9）: 使用量が上限（既定 2GB）を超えたら、更新時刻の古いファイルから
//! 削除する。fullsize を消したときは media.local_path を NULL に戻す（メタは保持し再取得可能）。

use crate::db::{queries, Db};
use crate::error::Result;
use chrono::Utc;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager, UriSchemeContext, UriSchemeResponder};

pub const THUMB_SCHEME: &str = "thumb";
pub const FULL_SCHEME: &str = "full";

/// ディスクキャッシュ上限の既定値（2 GiB, DESIGN §9）。
pub const DEFAULT_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
/// エビクション後に落とす目標（上限の 90%）。ヒステリシスで頻発を防ぐ。
const EVICT_TARGET_RATIO: u64 = 90;

#[derive(Clone, Copy, PartialEq)]
enum CacheKind {
    Thumb,
    Full,
}

/// メディアのディスクキャッシュ（Tauri state として保持）。
pub struct MediaCache {
    thumbs_dir: PathBuf,
    fullsize_dir: PathBuf,
    client: reqwest::Client,
    /// 現使用量の概算（バイト）。起動時スキャンで初期化、書込/削除で増減。
    usage: AtomicU64,
    /// 上限（バイト）。
    limit: u64,
    /// エビクション実行中フラグ（多重起動防止）。
    evicting: AtomicBool,
}

impl MediaCache {
    /// キャッシュルート直下に `thumbs/` と `fullsize/` を用意する。
    pub fn new(cache_root: PathBuf) -> std::io::Result<Self> {
        let thumbs_dir = cache_root.join("thumbs");
        let fullsize_dir = cache_root.join("fullsize");
        std::fs::create_dir_all(&thumbs_dir)?;
        std::fs::create_dir_all(&fullsize_dir)?;
        let client = reqwest::Client::builder()
            .user_agent(concat!("Hanaikada/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client の生成に失敗");
        Ok(MediaCache {
            thumbs_dir,
            fullsize_dir,
            client,
            usage: AtomicU64::new(0),
            limit: DEFAULT_LIMIT,
            evicting: AtomicBool::new(false),
        })
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// thumbs/ と fullsize/ の実使用量を（バイト）で返す。
    pub fn scan_usage(&self) -> (u64, u64) {
        (dir_size(&self.thumbs_dir), dir_size(&self.fullsize_dir))
    }
}

/// URL パスから media_id を取り出す。
fn parse_media_id(uri: &tauri::http::Uri) -> Option<i64> {
    uri.path()
        .trim_matches('/')
        .rsplit('/')
        .next()?
        .parse::<i64>()
        .ok()
}

/// 先頭バイトから content-type を推定する。
fn content_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

fn respond(responder: UriSchemeResponder, status: u16, ctype: &str, body: Vec<u8>) {
    let resp = tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", ctype)
        .body(body)
        .expect("http::Response の生成に失敗");
    responder.respond(resp);
}

/// thumb / full 共通の解決・配信処理。
fn serve(
    ctx: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
    kind: CacheKind,
) {
    let app: AppHandle = ctx.app_handle().clone();
    let media_id = match parse_media_id(request.uri()) {
        Some(id) => id,
        None => {
            respond(responder, 400, "text/plain", b"bad media id".to_vec());
            return;
        }
    };

    // State から必要なものだけ取り出し、await 中は State を保持しない。
    let (client, path) = {
        let cache = app.state::<MediaCache>();
        let dir = match kind {
            CacheKind::Thumb => &cache.thumbs_dir,
            CacheKind::Full => &cache.fullsize_dir,
        };
        (cache.client.clone(), dir.join(media_id.to_string()))
    };

    tauri::async_runtime::spawn(async move {
        // 1) ディスクヒット
        if let Ok(bytes) = tokio::fs::read(&path).await {
            if kind == CacheKind::Full {
                // fullsize は最終アクセス時刻を更新（LRU の touch）。
                let db = app.state::<Arc<Db>>();
                let conn = db.0.lock().unwrap();
                let _ = queries::touch_media_used(&conn, media_id, Utc::now().timestamp());
            }
            let ct = content_type(&bytes);
            respond(responder, 200, ct, bytes);
            return;
        }

        // 2) 対象 URL を DB から引く（ロックは即解放）
        let url = {
            let db = app.state::<Arc<Db>>();
            let conn = db.0.lock().unwrap();
            let resolve = match kind {
                CacheKind::Thumb => queries::thumb_url_of,
                CacheKind::Full => queries::fullsize_url_of,
            };
            resolve(&conn, media_id).ok().flatten()
        };
        let Some(url) = url else {
            respond(responder, 404, "text/plain", b"not found".to_vec());
            return;
        };

        // 3) CDN から DL → 保存 → 配信
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => match r.bytes().await {
                Ok(b) => {
                    let bytes = b.to_vec();
                    let size = bytes.len() as u64;
                    // 保存失敗（容量など）は致命ではない。配信は続ける。
                    if tokio::fs::write(&path, &bytes).await.is_ok() {
                        on_cached(&app, kind, media_id, &path, size);
                    }
                    let ct = content_type(&bytes);
                    respond(responder, 200, ct, bytes);
                }
                Err(_) => respond(responder, 502, "text/plain", b"download error".to_vec()),
            },
            _ => respond(responder, 502, "text/plain", b"fetch failed".to_vec()),
        }
    });
}

/// 書込直後の会計処理。使用量を加算し、fullsize は DB に記録、上限超過ならエビクション。
fn on_cached(app: &AppHandle, kind: CacheKind, media_id: i64, path: &Path, size: u64) {
    let cache = app.state::<MediaCache>();
    let new_usage = cache.usage.fetch_add(size, Ordering::Relaxed) + size;

    if kind == CacheKind::Full {
        let db = app.state::<Arc<Db>>();
        let conn = db.0.lock().unwrap();
        let _ = queries::mark_fullsize_cached(
            &conn,
            media_id,
            &path.to_string_lossy(),
            size as i64,
            Utc::now().timestamp(),
        );
    }

    if new_usage > cache.limit && !cache.evicting.load(Ordering::Relaxed) {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            enforce_limit(&app2);
        });
    }
}

/// `thumb` スキームのハンドラ。
pub fn handle_thumb_request(
    ctx: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    serve(ctx, request, responder, CacheKind::Thumb);
}

/// `full` スキームのハンドラ。fullsize（ビューアで開いた分のみ）。
pub fn handle_full_request(
    ctx: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    serve(ctx, request, responder, CacheKind::Full);
}

/// ディレクトリ配下の総バイト数（1 階層）。
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// (パス, サイズ, mtime(秒), fullsize か, media_id) を集める。
fn scan_entries(dir: &Path, is_full: bool) -> Vec<(PathBuf, u64, u64, bool, i64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let id = path.file_name()?.to_str()?.parse::<i64>().ok()?;
            Some((path, meta.len(), mtime, is_full, id))
        })
        .collect()
}

/// 上限を超えていれば古い順（mtime 昇順）にファイルを削除する（DESIGN §9）。
/// fullsize を消したら media.local_path/bytes を NULL に戻す。
pub fn enforce_limit(app: &AppHandle) {
    let cache = app.state::<MediaCache>();
    // 多重実行を防ぐ
    if cache
        .evicting
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let mut entries = scan_entries(&cache.thumbs_dir, false);
    entries.extend(scan_entries(&cache.fullsize_dir, true));
    let mut total: u64 = entries.iter().map(|e| e.1).sum();
    cache.usage.store(total, Ordering::Relaxed);

    let limit = cache.limit;
    if total <= limit {
        cache.evicting.store(false, Ordering::SeqCst);
        return;
    }

    let target = limit / 100 * EVICT_TARGET_RATIO;
    entries.sort_by_key(|e| e.2); // mtime 昇順（古い順）

    let mut freed = 0u64;
    let mut cleared_full: Vec<i64> = Vec::new();
    for (path, size, _mtime, is_full, id) in entries {
        if total <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
            freed += size;
            if is_full {
                cleared_full.push(id);
            }
        }
    }

    if !cleared_full.is_empty() {
        let db = app.state::<Arc<Db>>();
        let conn = db.0.lock().unwrap();
        for id in &cleared_full {
            let _ = queries::clear_fullsize_cache(&conn, *id);
        }
    }

    cache.usage.store(total, Ordering::Relaxed);
    cache.evicting.store(false, Ordering::SeqCst);
    tracing::info!(
        "キャッシュエビクション: {} MB 解放（fullsize {} 件クリア）",
        freed / 1024 / 1024,
        cleared_full.len()
    );
}

/// 起動時: 実使用量を集計して usage に反映し、必要ならエビクションする。
pub fn init_and_enforce(app: &AppHandle) {
    let (thumbs, full) = {
        let cache = app.state::<MediaCache>();
        cache.scan_usage()
    };
    {
        let cache = app.state::<MediaCache>();
        cache.usage.store(thumbs + full, Ordering::Relaxed);
    }
    enforce_limit(app);
}

/// すべてのキャッシュファイルを削除し、DB の fullsize 記録もクリアする（手動クリア）。
pub fn clear_all(app: &AppHandle) -> Result<()> {
    let (thumbs_dir, fullsize_dir) = {
        let cache = app.state::<MediaCache>();
        (cache.thumbs_dir.clone(), cache.fullsize_dir.clone())
    };
    remove_dir_files(&thumbs_dir);
    remove_dir_files(&fullsize_dir);
    {
        let db = app.state::<Arc<Db>>();
        let conn: std::sync::MutexGuard<Connection> = db.0.lock().unwrap();
        queries::clear_all_fullsize_cache(&conn)?;
    }
    let cache = app.state::<MediaCache>();
    cache.usage.store(0, Ordering::Relaxed);
    Ok(())
}

fn remove_dir_files(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_file() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}
