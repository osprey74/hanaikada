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

// --- app.bsky.feed.getTimeline（Phase 2） ---
//
// embed / record / reason / labels は $type によるバリアントが多く、体系変更にも追従したいので
// `serde_json::Value` のまま保持し、抽出は extractor で行う（DESIGN §5.1「labels は解釈せず保持」に倣う）。

use serde_json::Value;

/// `app.bsky.feed.getTimeline` のレスポンス。
#[derive(Debug, Deserialize)]
pub struct GetTimelineResponse {
    pub feed: Vec<FeedViewPost>,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// タイムライン 1 件（投稿＋リポスト等の理由）。
#[derive(Debug, Deserialize)]
pub struct FeedViewPost {
    pub post: PostView,
    /// `app.bsky.feed.defs#reasonRepost` など。存在すれば extractor が解釈する。
    #[serde(default)]
    pub reason: Option<Value>,
}

/// 投稿ビュー。
#[derive(Debug, Deserialize)]
pub struct PostView {
    pub uri: String,
    pub cid: String,
    pub author: Author,
    /// `app.bsky.feed.post` レコード（text / createdAt を取り出す）。
    #[serde(default)]
    pub record: Option<Value>,
    /// 添付。`app.bsky.embed.*#view`。
    #[serde(default)]
    pub embed: Option<Value>,
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
    /// 自己ラベル・ラベラー由来ラベルの配列。生のまま保持する。
    #[serde(default)]
    pub labels: Option<Value>,
}

/// 投稿者／リポスト元。
#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub did: String,
    pub handle: String,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}
