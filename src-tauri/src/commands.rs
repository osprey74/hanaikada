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

// --- ビューア / モデレーション（Phase 4） ---

/// 指定投稿の全メディア（idx 順）。ライトボックスの送り用。
#[tauri::command]
pub async fn get_post_media(
    post_uri: String,
    db: State<'_, Arc<Db>>,
) -> Result<Vec<queries::PostMediaItem>> {
    let conn = db.0.lock().unwrap();
    queries::get_post_media(&conn, &post_uri)
}

/// フロントへ返すモデレーション設定。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelPref {
    pub label: String,
    pub visibility: String, // "ignore" | "show" | "warn" | "hide"
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModerationPrefs {
    pub adult_content_enabled: bool,
    pub label_prefs: Vec<LabelPref>,
}

fn parse_prefs(value: &serde_json::Value) -> ModerationPrefs {
    let mut adult_content_enabled = false;
    let mut label_prefs = Vec::new();
    if let Some(arr) = value.get("preferences").and_then(|v| v.as_array()) {
        for item in arr {
            match item.get("$type").and_then(|v| v.as_str()) {
                Some("app.bsky.actor.defs#adultContentPref") => {
                    adult_content_enabled =
                        item.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                }
                Some("app.bsky.actor.defs#contentLabelPref") => {
                    if let (Some(label), Some(vis)) = (
                        item.get("label").and_then(|v| v.as_str()),
                        item.get("visibility").and_then(|v| v.as_str()),
                    ) {
                        label_prefs.push(LabelPref {
                            label: label.to_string(),
                            visibility: vis.to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    ModerationPrefs {
        adult_content_enabled,
        label_prefs,
    }
}

// --- キャッシュ管理（Phase 5） ---

/// キャッシュ使用量（バイト）。設定画面の使用量バー用。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheUsage {
    pub thumbs_bytes: u64,
    pub fullsize_bytes: u64,
    pub total_bytes: u64,
    pub limit_bytes: u64,
}

#[tauri::command]
pub async fn cache_usage(cache: State<'_, crate::media::MediaCache>) -> Result<CacheUsage> {
    let (thumbs, full) = cache.scan_usage();
    Ok(CacheUsage {
        thumbs_bytes: thumbs,
        fullsize_bytes: full,
        total_bytes: thumbs + full,
        limit_bytes: cache.limit(),
    })
}

/// ディスクキャッシュを全削除する（メタは保持し、再取得可能）。
#[tauri::command]
pub async fn clear_cache(app: AppHandle) -> Result<()> {
    crate::media::clear_all(&app)
}

/// ミュート/ブロックを取得し is_hidden を更新する（手動トリガ）。隠した投稿数を返す。
#[tauri::command]
pub async fn reconcile_hidden(
    db: State<'_, Arc<Db>>,
    manager: State<'_, Arc<SessionManager>>,
) -> Result<usize> {
    crate::moderation::reconcile(&db, &manager).await
}

/// 起動時に呼ぶ。getPreferences からアダルト設定・ラベル可視性を取得する。
/// 401 は一度だけリフレッシュして再試行する。
#[tauri::command]
pub async fn get_moderation_prefs(
    manager: State<'_, Arc<SessionManager>>,
) -> Result<ModerationPrefs> {
    let client = manager.client();
    let token = manager.valid_access_token().await?;
    let value = match client.get_preferences(&token).await {
        Ok(v) => v,
        Err(crate::error::AppError::Unauthorized) => {
            let fresh = manager.refresh().await?;
            client.get_preferences(&fresh).await?
        }
        Err(e) => return Err(e),
    };
    Ok(parse_prefs(&value))
}
