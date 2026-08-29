//! 花筏 — Bluesky メディアグリッド閲覧クライアント（閲覧専用）のエントリポイント。

mod auth;
mod bsky;
mod commands;
mod db;
mod error;
mod media;
mod sync;

use auth::SessionManager;
use db::Db;
use std::sync::Arc;
use sync::Syncer;
use tauri::Manager;

/// アプリデータの保存先ディレクトリ名（DESIGN §9）。
///   Windows: %APPDATA%\Hanaikada
///   macOS:   ~/Library/Application Support/Hanaikada
const APP_DIR: &str = "Hanaikada";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // トークンをログに出さないため、既定は info。詳細は RUST_LOG で制御。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hanaikada_lib=info,warn".into()),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol(media::THUMB_SCHEME, media::handle_thumb_request)
        .setup(|app| {
            let data_dir = app.path().data_dir()?.join(APP_DIR);

            // SQLite（DESIGN §5）。同期エンジンと UI クエリで共有するため Arc 化。
            let db = Arc::new(Db::open(&data_dir.join("hanaikada.db"))?);

            // サムネのディスクキャッシュ（thumb スキームが参照）。
            let thumb_cache = media::ThumbCache::new(data_dir.clone())?;

            // セッション管理。起動時に前回セッションを復元する。
            let manager = Arc::new(SessionManager::new(data_dir)?);
            match manager.restore() {
                Ok(Some(info)) => {
                    tracing::info!(handle = %info.handle, "セッションを復元しました");
                }
                Ok(None) => tracing::info!("保存済みセッションはありません"),
                Err(e) => tracing::warn!("セッション復元に失敗: {e}"),
            }

            // 同期エンジン。
            let syncer = Syncer::new(db.clone(), manager.clone());

            app.manage(db);
            app.manage(manager);
            app.manage(syncer);
            app.manage(thumb_cache);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::logout,
            commands::current_session,
            commands::validate_session,
            commands::sync_now,
            commands::start_initial_sync,
            commands::cancel_sync,
            commands::sync_status,
            commands::db_stats,
            commands::query_media,
            commands::media_count,
            commands::list_actors,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
