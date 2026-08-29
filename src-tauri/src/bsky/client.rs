//! Bluesky XRPC クライアント（低レベル HTTP）。
//!
//! MVP では PDS エントリポイントを `https://bsky.social` に固定する。
//! ハンドル解決による PDS ディスカバリは v1.1（OAuth 移行）で扱う。

use super::models::{
    CreateSessionRequest, GetSessionResponse, GetTimelineResponse, SessionResponse, XrpcErrorBody,
};
use crate::error::{AppError, Result};
use reqwest::{header::HeaderMap, StatusCode};

/// getTimeline レスポンスに付随するレート制限情報（自発的な間引き判断に使う）。
/// reset / limit は Phase 3 のステータス表示で参照予定。
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct RateInfo {
    pub remaining: Option<i64>,
    /// リセット時刻（Unix 秒）。
    pub reset: Option<i64>,
    pub limit: Option<i64>,
}

/// 1 ページ分のタイムライン取得結果。
pub struct TimelinePage {
    pub feed: super::models::GetTimelineResponse,
    pub rate: RateInfo,
}

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

    /// `app.bsky.feed.getTimeline`。accessJwt が必要。失効時は `Unauthorized`、
    /// 429 は `RateLimited`（待機秒数つき）を返す。
    pub async fn get_timeline(
        &self,
        access_jwt: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<TimelinePage> {
        let limit_s = limit.to_string();
        let mut query: Vec<(&str, &str)> = vec![("limit", &limit_s)];
        if let Some(c) = cursor {
            query.push(("cursor", c));
        }

        let resp = self
            .http
            .get(self.url("app.bsky.feed.getTimeline"))
            .bearer_auth(access_jwt)
            .query(&query)
            .send()
            .await?;

        let status = resp.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::RateLimited {
                retry_after_secs: retry_after(resp.headers()),
            });
        }
        let rate = parse_rate_info(resp.headers());
        if status.is_success() {
            let feed = resp.json::<GetTimelineResponse>().await?;
            return Ok(TimelinePage { feed, rate });
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

/// ヘッダから 1 個の整数値を読む。
fn header_i64(headers: &HeaderMap, name: &str) -> Option<i64> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<i64>().ok())
}

/// `RateLimit-*` ヘッダを解釈する（Bluesky は小文字ヘッダで返す）。
fn parse_rate_info(headers: &HeaderMap) -> RateInfo {
    RateInfo {
        remaining: header_i64(headers, "ratelimit-remaining"),
        reset: header_i64(headers, "ratelimit-reset"),
        limit: header_i64(headers, "ratelimit-limit"),
    }
}

/// 429 応答から待機秒数を推定する。`Retry-After`（秒）を優先し、
/// 無ければ `RateLimit-Reset`（Unix 秒）から現在との差を求める。
fn retry_after(headers: &HeaderMap) -> Option<u64> {
    if let Some(secs) = header_i64(headers, "retry-after") {
        if secs >= 0 {
            return Some(secs as u64);
        }
    }
    if let Some(reset) = header_i64(headers, "ratelimit-reset") {
        let now = chrono::Utc::now().timestamp();
        if reset > now {
            return Some((reset - now) as u64);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            let name = HeaderName::from_bytes(k.as_bytes()).unwrap();
            h.insert(name, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn retry_after_prefers_retry_after_header() {
        let h = headers(&[("retry-after", "42")]);
        assert_eq!(retry_after(&h), Some(42));
    }

    #[test]
    fn retry_after_falls_back_to_ratelimit_reset() {
        // reset は Unix 秒。十分未来に置けば now との差が正の待機秒数になる。
        let future = chrono::Utc::now().timestamp() + 120;
        let h = headers(&[("ratelimit-reset", &future.to_string())]);
        let secs = retry_after(&h).expect("reset から待機秒数を算出");
        assert!(secs > 0 && secs <= 120, "待機秒数は 0〜120 の範囲: {secs}");
    }

    #[test]
    fn retry_after_none_when_no_headers() {
        assert_eq!(retry_after(&HeaderMap::new()), None);
    }

    #[test]
    fn retry_after_ignores_past_reset() {
        let past = chrono::Utc::now().timestamp() - 60;
        let h = headers(&[("ratelimit-reset", &past.to_string())]);
        assert_eq!(retry_after(&h), None, "過去の reset は待機に使わない");
    }

    #[test]
    fn parse_rate_info_reads_all_fields() {
        let h = headers(&[
            ("ratelimit-remaining", "15"),
            ("ratelimit-reset", "1788000000"),
            ("ratelimit-limit", "3000"),
        ]);
        let r = parse_rate_info(&h);
        assert_eq!(r.remaining, Some(15));
        assert_eq!(r.reset, Some(1788000000));
        assert_eq!(r.limit, Some(3000));
    }
}
