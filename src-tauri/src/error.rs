//! アプリ全体で使う共通エラー型。
//!
//! 重要: トークンやパスワードを Display に含めないこと（HANDOFF §0）。
//! reqwest 由来のエラーは URL を含み得るが、認証情報はクエリに載せないため許容する。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("認証情報が正しくありません")]
    InvalidCredentials,

    /// アクセストークン失効。呼び出し側でリフレッシュを試みる契機に使う。
    #[error("認証が必要です")]
    Unauthorized,

    /// リフレッシュ不能（refreshJwt 失効など）。再ログインが必要。
    #[error("セッションの更新に失敗しました。再ログインしてください")]
    RefreshFailed,

    #[error("未ログインです")]
    NotLoggedIn,

    /// Bluesky から想定外の XRPC エラーが返った。`error` はレスポンスの error 名。
    #[error("Bluesky API エラー: {error}")]
    Xrpc { status: u16, error: String },

    #[error("ネットワークエラー: {0}")]
    Network(String),

    #[error("データベースエラー: {0}")]
    Db(String),

    #[error("キーチェーンエラー: {0}")]
    Keychain(String),

    #[error("入出力エラー: {0}")]
    Io(String),

    #[error("内部エラー: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        AppError::Keychain(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("JSON: {e}"))
    }
}

/// Tauri コマンド境界ではエラーを文字列化して返す（フロントで表示するメッセージ）。
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
