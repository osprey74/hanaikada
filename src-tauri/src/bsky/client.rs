//! Bluesky XRPC クライアント（低レベル HTTP）。
//!
//! MVP では PDS エントリポイントを `https://bsky.social` に固定する。
//! ハンドル解決による PDS ディスカバリは v1.1（OAuth 移行）で扱う。

use super::models::{
    CreateSessionRequest, GetSessionResponse, SessionResponse, XrpcErrorBody,
};
use crate::error::{AppError, Result};
use reqwest::StatusCode;

/// MVP の固定エントリポイント。
const DEFAULT_SERVICE: &str = "https://bsky.social";

#[derive(Clone)]
pub struct BskyClient {
    http: reqwest::Client,
    service: String,
}

impl BskyClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Hanaikada/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            service: DEFAULT_SERVICE.to_string(),
        })
    }

    fn url(&self, method: &str) -> String {
        format!("{}/xrpc/{}", self.service, method)
    }

    /// `com.atproto.server.createSession`。厳しいレート制限（30/5分・300/日）に注意。
    pub async fn create_session(
        &self,
        identifier: &str,
        app_password: &str,
    ) -> Result<SessionResponse> {
        let resp = self
            .http
            .post(self.url("com.atproto.server.createSession"))
            .json(&CreateSessionRequest {
                identifier,
                password: app_password,
            })
            .send()
            .await?;

        self.parse_session(resp, /* is_auth_attempt */ true).await
    }

    /// `com.atproto.server.refreshSession`。Authorization に refreshJwt を用いる。
    pub async fn refresh_session(&self, refresh_jwt: &str) -> Result<SessionResponse> {
        let resp = self
            .http
            .post(self.url("com.atproto.server.refreshSession"))
            .bearer_auth(refresh_jwt)
            .send()
            .await?;

        // refresh 失敗（期限切れ等）は再ログインが必要
        match self.parse_session(resp, false).await {
            Ok(s) => Ok(s),
            Err(AppError::Unauthorized) | Err(AppError::InvalidCredentials) => {
                Err(AppError::RefreshFailed)
            }
            Err(e) => Err(e),
        }
    }

    /// `com.atproto.server.getSession`。accessJwt の有効性検証に使う。
    /// 失効時は `AppError::Unauthorized` を返す（呼び出し側でリフレッシュ）。
    pub async fn get_session(&self, access_jwt: &str) -> Result<GetSessionResponse> {
        let resp = self
            .http
            .get(self.url("com.atproto.server.getSession"))
            .bearer_auth(access_jwt)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            return Ok(resp.json::<GetSessionResponse>().await?);
        }
        Err(self.map_error(status, resp.text().await.unwrap_or_default()))
    }

    /// session 系レスポンスの共通パース。
    async fn parse_session(
        &self,
        resp: reqwest::Response,
        is_auth_attempt: bool,
    ) -> Result<SessionResponse> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp.json::<SessionResponse>().await?);
        }
        let body = resp.text().await.unwrap_or_default();
        let err = self.map_error(status, body);
        // ログイン試行時の 401 は資格情報エラーとして扱う
        if is_auth_attempt && matches!(err, AppError::Unauthorized) {
            return Err(AppError::InvalidCredentials);
        }
        Err(err)
    }

    /// HTTP ステータス＋ボディを AppError にマップする。
    fn map_error(&self, status: StatusCode, body: String) -> AppError {
        let parsed: Option<XrpcErrorBody> = serde_json::from_str(&body).ok();
        let error_name = parsed.as_ref().map(|e| e.error.as_str()).unwrap_or("");

        match status {
            StatusCode::UNAUTHORIZED => AppError::Unauthorized,
            StatusCode::BAD_REQUEST
                if matches!(error_name, "ExpiredToken" | "InvalidToken") =>
            {
                AppError::Unauthorized
            }
            StatusCode::TOO_MANY_REQUESTS => AppError::Xrpc {
                status: status.as_u16(),
                error: if error_name.is_empty() {
                    "RateLimitExceeded".to_string()
                } else {
                    error_name.to_string()
                },
            },
            _ => AppError::Xrpc {
                status: status.as_u16(),
                error: if error_name.is_empty() {
                    status
                        .canonical_reason()
                        .unwrap_or("UnknownError")
                        .to_string()
                } else {
                    error_name.to_string()
                },
            },
        }
    }
}
