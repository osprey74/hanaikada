//! フロントエンドから呼ぶ `#[tauri::command]` 群。
//!
//! Phase 1: 認証 / Phase 2: 同期。参照・キャッシュ系は後続フェーズで追加する。

use crate::auth::{SessionInfo, SessionManager};
use crate::db::{queries, Db};
use crate::error::Result;
use crate::sync::{SyncStatus, Syncer, DEFAULT_INITIAL_DAYS};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

// --- 認証（Phase 1） ---

/// handle と App Password でログインする。成功時にセッション情報を返す。
#[tauri::command]
pub async fn login(
    handle: String,
    app_password: String,
    manager: State<'_, Arc<SessionManager>>,
) -> Result<SessionInfo> {
    manager.login(&handle, &app_password).await
}

/// ログアウトする（keychain / config.json をクリア）。
#[tauri::command]
pub async fn logout(manager: State<'_, Arc<SessionManager>>) -> Result<()> {
    manager.logout()
}

/// 現在のセッション情報を返す（メモリ内、ネットワークなし）。未ログインは None。
#[tauri::command]
pub async fn current_session(
    manager: State<'_, Arc<SessionManager>>,
) -> Result<Option<SessionInfo>> {
    Ok(manager.current())
}

/// getSession でセッションを検証する（401 は透過的にリフレッシュ再試行）。
#[tauri::command]
pub async fn validate_session(manager: State<'_, Arc<SessionManager>>) -> Result<SessionInfo> {
    manager.validate().await
}

// --- 同期（Phase 2） ---

/// 差分同期を開始する（先頭ページから、既知 URI or 上限 5 ページで打ち切り）。
#[tauri::command]
pub async fn sync_now(app: AppHandle, syncer: State<'_, Syncer>) -> Result<()> {
    syncer.sync_now(app)
}

/// 初回同期を開始する。days 省略時は既定 30 日遡る。
#[tauri::command]
pub async fn start_initial_sync(
    app: AppHandle,
    days: Option<u32>,
    syncer: State<'_, Syncer>,
) -> Result<()> {
    syncer.start_initial_sync(app, days.unwrap_or(DEFAULT_INITIAL_DAYS))
}

/// 実行中の同期を中断する。
#[tauri::command]
pub async fn cancel_sync(syncer: State<'_, Syncer>) -> Result<()> {
    syncer.cancel();
    Ok(())
}

/// 現在の同期状態を返す。
#[tauri::command]
pub async fn sync_status(syncer: State<'_, Syncer>) -> Result<SyncStatus> {
    Ok(syncer.status())
}

/// DB の件数統計（Phase 2 の検証・ステータスバー用）。
#[derive(Serialize)]
pub struct DbStats {
    pub media: i64,
    pub posts: i64,
    pub actors: i64,
}

#[tauri::command]
pub async fn db_stats(db: State<'_, Arc<Db>>) -> Result<DbStats> {
    let conn = db.0.lock().unwrap();
    Ok(DbStats {
        media: queries::media_total(&conn)?,
        posts: queries::post_total(&conn)?,
        actors: queries::actor_total(&conn)?,
    })
}

// --- 参照（Phase 3） ---

/// 絞り込みに一致するタイル（投稿）を新しい順に返す（まとめ表示・ページング）。
#[tauri::command]
pub async fn query_media(
    filter: queries::MediaFilter,
    offset: i64,
    limit: i64,
    db: State<'_, Arc<Db>>,
) -> Result<Vec<queries::MediaTile>> {
    let conn = db.0.lock().unwrap();
    queries::query_media(&conn, &filter, offset, limit)
}

/// 絞り込みに一致するタイル総数（ステータスバー用）。
#[tauri::command]
pub async fn media_count(
    filter: queries::MediaFilter,
    db: State<'_, Arc<Db>>,
) -> Result<i64> {
    let conn = db.0.lock().unwrap();
    queries::media_count(&conn, &filter)
}

/// メディア投稿を持つ投稿者の一覧（件数付き・降順）。サイドバー用。
#[tauri::command]
pub async fn list_actors(db: State<'_, Arc<Db>>) -> Result<Vec<queries::ActorSummary>> {
    let conn = db.0.lock().unwrap();
    queries::list_actors(&conn)
}
