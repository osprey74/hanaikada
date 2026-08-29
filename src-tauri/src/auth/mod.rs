//! セッション管理。認証の高レベル API を提供し、Tauri state として保持される。
//!
//! - accessJwt はメモリ内のみ（`Current`）
//! - refreshJwt は keychain
//! - did / handle / 最終認証時刻は config.json（非機密メタ）
//! - accessJwt 失効（401）時は refreshSession で透過的にリカバリする

pub mod keychain;
pub mod session;
pub mod store;

use crate::bsky::BskyClient;
use crate::error::{AppError, Result};
use chrono::Utc;
use session::{AppPasswordAuth, BskyAuth, Session};
use std::path::PathBuf;
use std::sync::Mutex;
use store::SessionMeta;

/// フロントエンドへ返すセッション情報（機密を含まない）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionInfo {
    pub did: String,
    pub handle: String,
    #[serde(rename = "lastAuthAt")]
    pub last_auth_at: i64,
}

/// メモリ内の現在セッション。
struct Current {
    did: String,
    handle: String,
    refresh_jwt: String,
    /// accessJwt。未取得（復元直後など）は None。
    access_jwt: Option<String>,
    last_auth_at: i64,
}

pub struct SessionManager {
    auth: Box<dyn BskyAuth>,
    client: BskyClient,
    data_dir: PathBuf,
    current: Mutex<Option<Current>>,
}

impl SessionManager {
    /// App Password 認証で初期化する。
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        let client = BskyClient::new()?;
        let auth = Box::new(AppPasswordAuth::new(client.clone()));
        Ok(Self {
            auth,
            client,
            data_dir,
            current: Mutex::new(None),
        })
    }

    /// 起動時の復元。config.json と keychain が揃っていればログイン状態を復帰する。
    /// ネットワークアクセスはせず、accessJwt は初回利用時に遅延取得する。
    /// 片方だけ残る不整合状態は掃除してログアウト扱いにする。
    pub fn restore(&self) -> Result<Option<SessionInfo>> {
        let meta = store::load(&self.data_dir)?;
        let refresh = keychain::load()?;

        match (meta, refresh) {
            (Some(meta), Some(refresh_jwt)) => {
                let info = SessionInfo {
                    did: meta.did.clone(),
                    handle: meta.handle.clone(),
                    last_auth_at: meta.last_auth_at,
                };
                *self.current.lock().unwrap() = Some(Current {
                    did: meta.did,
                    handle: meta.handle,
                    refresh_jwt,
                    access_jwt: None,
                    last_auth_at: meta.last_auth_at,
                });
                Ok(Some(info))
            }
            (meta, refresh) => {
                // 不整合: 残骸を掃除する
                if meta.is_some() {
                    store::clear(&self.data_dir)?;
                }
                if refresh.is_some() {
                    keychain::delete()?;
                }
                Ok(None)
            }
        }
    }

    /// handle と App Password でログインする。
    pub async fn login(&self, identifier: &str, app_password: &str) -> Result<SessionInfo> {
        let identifier = identifier.trim();
        let app_password = app_password.trim();
        if identifier.is_empty() || app_password.is_empty() {
            return Err(AppError::InvalidCredentials);
        }

        let session = self.auth.create_session(identifier, app_password).await?;
        let info = self.persist(session)?;
        Ok(info)
    }

    /// ログアウト。メモリ・keychain・config.json をすべてクリアする。
    pub fn logout(&self) -> Result<()> {
        *self.current.lock().unwrap() = None;
        keychain::delete()?;
        store::clear(&self.data_dir)?;
        Ok(())
    }

    /// 現在のセッション情報（メモリ内）。未ログインなら None。
    pub fn current(&self) -> Option<SessionInfo> {
        self.current.lock().unwrap().as_ref().map(|c| SessionInfo {
            did: c.did.clone(),
            handle: c.handle.clone(),
            last_auth_at: c.last_auth_at,
        })
    }

    /// 有効な accessJwt を返す。無ければ refreshSession で取得する。
    /// 認証系の他コマンド（Phase 2 以降）はこれを起点に使う。
    pub async fn valid_access_token(&self) -> Result<String> {
        if let Some(token) = self.snapshot_access() {
            return Ok(token);
        }
        self.refresh().await
    }

    /// refreshJwt から accessJwt を再取得し、状態と保存を更新する。新しい accessJwt を返す。
    pub async fn refresh(&self) -> Result<String> {
        let refresh_jwt = self.snapshot_refresh().ok_or(AppError::NotLoggedIn)?;
        let session = self.auth.refresh_session(&refresh_jwt).await?;
        let access = session.access_jwt.clone();
        self.persist(session)?;
        Ok(access)
    }

    /// getSession でセッションを検証する。401 なら一度だけリフレッシュして再試行する。
    /// Phase 1 の「401 発生時に透過的にリトライ」受け入れ条件を満たすための実装。
    pub async fn validate(&self) -> Result<SessionInfo> {
        let token = self.valid_access_token().await?;
        let result = self.client.get_session(&token).await;
        let resp = match result {
            Ok(r) => r,
            Err(AppError::Unauthorized) => {
                let fresh = self.refresh().await?;
                self.client.get_session(&fresh).await?
            }
            Err(e) => return Err(e),
        };
        // getSession の did/handle をメモリへ反映（handle 変更に追従）
        let mut guard = self.current.lock().unwrap();
        if let Some(c) = guard.as_mut() {
            c.did = resp.did.clone();
            c.handle = resp.handle.clone();
        }
        Ok(SessionInfo {
            did: resp.did,
            handle: resp.handle,
            last_auth_at: guard.as_ref().map(|c| c.last_auth_at).unwrap_or_default(),
        })
    }

    // --- 内部ヘルパ ---

    /// セッションをメモリ・keychain・config.json に反映する。
    fn persist(&self, session: Session) -> Result<SessionInfo> {
        let now = Utc::now().timestamp();
        keychain::save(&session.refresh_jwt)?;
        store::save(
            &self.data_dir,
            &SessionMeta {
                did: session.did.clone(),
                handle: session.handle.clone(),
                last_auth_at: now,
            },
        )?;

        let info = SessionInfo {
            did: session.did.clone(),
            handle: session.handle.clone(),
            last_auth_at: now,
        };
        *self.current.lock().unwrap() = Some(Current {
            did: session.did,
            handle: session.handle,
            refresh_jwt: session.refresh_jwt,
            access_jwt: Some(session.access_jwt),
            last_auth_at: now,
        });
        Ok(info)
    }

    fn snapshot_access(&self) -> Option<String> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|c| c.access_jwt.clone())
    }

    fn snapshot_refresh(&self) -> Option<String> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .map(|c| c.refresh_jwt.clone())
    }
}
