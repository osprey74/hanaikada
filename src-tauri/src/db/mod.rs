//! SQLite 接続の生成とマイグレーション適用。
//!
//! 接続は `Db`（`Mutex<Connection>`）で Tauri state として保持する。
//! 同期エンジン（Phase 2）とクエリ（Phase 3）が同じ接続を共有する。

pub mod migrations;

use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Tauri state として保持する DB ハンドル。
/// 内部接続は Phase 2（同期）・Phase 3（クエリ）で参照する。
#[allow(dead_code)]
pub struct Db(pub Mutex<Connection>);

impl Db {
    /// 指定パスに接続し、PRAGMA 設定とマイグレーションを適用する。
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        configure(&conn)?;
        migrations::migrate(&conn)?;
        Ok(Db(Mutex::new(conn)))
    }

    /// テスト用のインメモリ DB。
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        configure(&conn)?;
        migrations::migrate(&conn)?;
        Ok(Db(Mutex::new(conn)))
    }
}

/// 全接続に共通の PRAGMA。
fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_to_latest_version() {
        let db = Db::open_in_memory().expect("open");
        let conn = db.0.lock().unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, migrations::SCHEMA_VERSION);

        // 主要テーブルが存在する
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('actors','posts','media','sync_state')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 4);
    }
}
