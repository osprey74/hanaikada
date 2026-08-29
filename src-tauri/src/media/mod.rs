//! サムネイルのディスクキャッシュとカスタム URI スキーム配信（Phase 3）。
//!
//! フロントは `<img src="…/thumb/<media_id>">` で参照し、Rust が解決する。
//! ディスクヒットは即配信、ミスは Bluesky CDN から DL → 保存 → 配信する。
//! API・CDN へのアクセスはすべて Rust に閉じる（DESIGN §3.1）。
//!
//! LRU による上限管理（既定 2GB）は Phase 5 で追加する。現状は無制限にキャッシュする。

use crate::db::{queries, Db};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, UriSchemeContext, UriSchemeResponder};

/// カスタム URI スキーム名。フロントは Windows で `http://thumb.localhost/<id>`、
/// macOS/Linux で `thumb://localhost/<id>` を使う。
pub const THUMB_SCHEME: &str = "thumb";

/// サムネのディスクキャッシュ（Tauri state として保持）。
pub struct ThumbCache {
    dir: PathBuf,
    client: reqwest::Client,
}

impl ThumbCache {
    /// キャッシュルート直下に `thumbs/` を用意する。
    pub fn new(cache_root: PathBuf) -> std::io::Result<Self> {
        let dir = cache_root.join("thumbs");
        std::fs::create_dir_all(&dir)?;
        let client = reqwest::Client::builder()
            .user_agent(concat!("Hanaikada/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client の生成に失敗");
        Ok(ThumbCache { dir, client })
    }

    fn path_for(&self, media_id: i64) -> PathBuf {
        self.dir.join(media_id.to_string())
    }
}

/// URL パスから media_id を取り出す。`/<id>`（macOS）も `thumb.localhost/<id>`（Windows）も
/// path は `/<id>` になるため末尾セグメントを数値化する。
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

/// `thumb` スキームのハンドラ。`setup` で登録する。
pub fn handle_thumb_request(
    ctx: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app: AppHandle = ctx.app_handle().clone();
    let media_id = match parse_media_id(request.uri()) {
        Some(id) => id,
        None => {
            respond(responder, 400, "text/plain", b"bad media id".to_vec());
            return;
        }
    };

    // State から必要なものだけ取り出し、await 中は State を保持しない
    // （reqwest::Client の clone は内部 Arc の複製で安価）。
    let (client, path) = {
        let cache = app.state::<ThumbCache>();
        (cache.client.clone(), cache.path_for(media_id))
    };

    tauri::async_runtime::spawn(async move {
        // 1) ディスクヒット
        if let Ok(bytes) = tokio::fs::read(&path).await {
            let ct = content_type(&bytes);
            respond(responder, 200, ct, bytes);
            return;
        }

        // 2) サムネ URL を DB から引く（ロックは即解放）
        let url = {
            let db = app.state::<Arc<Db>>();
            let conn = db.0.lock().unwrap();
            queries::thumb_url_of(&conn, media_id).ok().flatten()
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
                    // 保存失敗（容量など）は致命ではない。配信は続ける。
                    let _ = tokio::fs::write(&path, &bytes).await;
                    let ct = content_type(&bytes);
                    respond(responder, 200, ct, bytes);
                }
                Err(_) => respond(responder, 502, "text/plain", b"download error".to_vec()),
            },
            _ => respond(responder, 502, "text/plain", b"fetch failed".to_vec()),
        }
    });
}
