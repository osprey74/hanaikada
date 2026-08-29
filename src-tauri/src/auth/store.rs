//! 非機密のセッションメタデータ（did / handle / 最終認証時刻）を config.json に保存する。
//!
//! これにより再起動直後にネットワークを待たずログイン状態と handle を即表示できる。
//! 機密である refreshJwt はここには置かない（keychain 管轄）。

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub did: String,
    pub handle: String,
    /// 最終認証時刻（Unix 秒）。
    pub last_auth_at: i64,
}

fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// メタデータを読み出す。無ければ None。
pub fn load(dir: &Path) -> Result<Option<SessionMeta>> {
    let p = path(dir);
    if !p.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&raw).ok())
}

/// メタデータを保存する。
pub fn save(dir: &Path, meta: &SessionMeta) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let pretty = serde_json::to_string_pretty(meta)?;
    std::fs::write(path(dir), pretty)?;
    Ok(())
}

/// メタデータを削除する（ログアウト）。
pub fn clear(dir: &Path) -> Result<()> {
    let p = path(dir);
    if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}
