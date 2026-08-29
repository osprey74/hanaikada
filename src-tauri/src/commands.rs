//! フロントエンドから呼ぶ `#[tauri::command]` 群（Phase 1: 認証）。
//!
//! 同期・参照・キャッシュ系コマンドは後続フェーズで追加する。

use crate::auth::{SessionInfo, SessionManager};
use crate::error::Result;
use tauri::State;

/// handle と App Password でログインする。成功時にセッション情報を返す。
#[tauri::command]
pub async fn login(
    handle: String,
    app_password: String,
    manager: State<'_, SessionManager>,
) -> Result<SessionInfo> {
    manager.login(&handle, &app_password).await
}

/// ログアウトする（keychain / config.json をクリア）。
#[tauri::command]
pub async fn logout(manager: State<'_, SessionManager>) -> Result<()> {
    manager.logout()
}

/// 現在のセッション情報を返す（メモリ内、ネットワークなし）。未ログインは None。
#[tauri::command]
pub async fn current_session(
    manager: State<'_, SessionManager>,
) -> Result<Option<SessionInfo>> {
    Ok(manager.current())
}

/// getSession でセッションを検証する（401 は透過的にリフレッシュ再試行）。
#[tauri::command]
pub async fn validate_session(
    manager: State<'_, SessionManager>,
) -> Result<SessionInfo> {
    manager.validate().await
}
