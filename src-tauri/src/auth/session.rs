//! 認証の抽象化。v1.1 の OAuth 差し替えを見据え `BskyAuth` trait を挟む（HANDOFF §0）。

use crate::bsky::{models::SessionResponse, BskyClient};
use crate::error::Result;
use async_trait::async_trait;

/// ログイン成功時に得られるセッション。`access_jwt` はメモリ内のみ、
/// `refresh_jwt` だけを OS キーチェーンへ保存する（DESIGN §4）。
#[derive(Debug, Clone)]
pub struct Session {
    pub did: String,
    pub handle: String,
    pub access_jwt: String,
    pub refresh_jwt: String,
}

impl From<SessionResponse> for Session {
    fn from(r: SessionResponse) -> Self {
        Session {
            did: r.did,
            handle: r.handle,
            access_jwt: r.access_jwt,
            refresh_jwt: r.refresh_jwt,
        }
    }
}

/// 認証方式の抽象。MVP は App Password 実装のみ。
#[async_trait]
pub trait BskyAuth: Send + Sync {
    /// handle と App Password で新規セッションを作成する。
    async fn create_session(&self, identifier: &str, app_password: &str) -> Result<Session>;

    /// refreshJwt からセッションを更新する。
    async fn refresh_session(&self, refresh_jwt: &str) -> Result<Session>;
}

/// App Password による認証（MVP）。
pub struct AppPasswordAuth {
    client: BskyClient,
}

impl AppPasswordAuth {
    pub fn new(client: BskyClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl BskyAuth for AppPasswordAuth {
    async fn create_session(&self, identifier: &str, app_password: &str) -> Result<Session> {
        let resp = self.client.create_session(identifier, app_password).await?;
        Ok(resp.into())
    }

    async fn refresh_session(&self, refresh_jwt: &str) -> Result<Session> {
        let resp = self.client.refresh_session(refresh_jwt).await?;
        Ok(resp.into())
    }
}
