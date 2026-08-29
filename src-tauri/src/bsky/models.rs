//! Bluesky XRPC のリクエスト/レスポンス型（手書き）。
//!
//! `@atproto/api` は導入せず、必要な範囲のみを定義する（HANDOFF §3）。
//! Phase 1 では認証系（createSession / refreshSession / getSession）のみ。

use serde::{Deserialize, Serialize};

/// `com.atproto.server.createSession` のリクエストボディ。
#[derive(Debug, Serialize)]
pub struct CreateSessionRequest<'a> {
    pub identifier: &'a str,
    pub password: &'a str,
}

/// createSession / refreshSession の共通レスポンス（必要なフィールドのみ）。
/// `active` はアカウント状態表示（Phase 4 モデレーション）で参照予定。
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SessionResponse {
    pub did: String,
    pub handle: String,
    #[serde(rename = "accessJwt")]
    pub access_jwt: String,
    #[serde(rename = "refreshJwt")]
    pub refresh_jwt: String,
    /// アカウントが無効化されている場合など。存在すれば注意表示に使う。
    #[serde(default)]
    pub active: Option<bool>,
}

/// `com.atproto.server.getSession` のレスポンス（セッション検証用）。
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GetSessionResponse {
    pub did: String,
    pub handle: String,
    #[serde(default)]
    pub active: Option<bool>,
}

/// XRPC のエラーレスポンス。`error` は `ExpiredToken` / `InvalidToken` などの機械可読名。
/// `message` は将来のエラー詳細表示用に保持する。
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct XrpcErrorBody {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub message: Option<String>,
}
