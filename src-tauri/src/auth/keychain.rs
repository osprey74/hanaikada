//! refreshJwt の OS キーチェーン保存（DESIGN §4 / HANDOFF §0）。
//!
//! ここに保存してよいのは refreshJwt のみ。accessJwt や App Password は保存しない。
//!   macOS:   Keychain
//!   Windows: Credential Manager (DPAPI)
//!   Linux:   Secret Service (libsecret)

use crate::error::Result;
use keyring::Entry;

const SERVICE: &str = "io.github.osprey74.hanaikada";
const ACCOUNT: &str = "refresh_jwt";

fn entry() -> Result<Entry> {
    Ok(Entry::new(SERVICE, ACCOUNT)?)
}

/// refreshJwt を保存（上書き）する。
pub fn save(refresh_jwt: &str) -> Result<()> {
    entry()?.set_password(refresh_jwt)?;
    Ok(())
}

/// 保存済みの refreshJwt を取得。未保存なら None。
pub fn load() -> Result<Option<String>> {
    match entry()?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// refreshJwt を削除（ログアウト）。未保存でもエラーにしない。
pub fn delete() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
