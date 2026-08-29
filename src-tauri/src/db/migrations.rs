//! SQLite スキーマのマイグレーション（DESIGN.md §5 準拠）。
//!
//! `user_version` PRAGMA で版管理する。マイグレーションは前方追記のみ。
//! 既存の版から順に適用し、最終的に `SCHEMA_VERSION` へ引き上げる。

use crate::error::Result;
use rusqlite::Connection;

/// 現在のスキーマ版。マイグレーションを追加したらインクリメントする。
pub const SCHEMA_VERSION: i64 = 1;

/// v1: DESIGN §5 の初期スキーマ（actors / posts / media / sync_state）。
const V1: &str = r#"
CREATE TABLE actors (
  did          TEXT PRIMARY KEY,
  handle       TEXT NOT NULL,
  display_name TEXT,
  avatar_url   TEXT,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE posts (
  uri          TEXT PRIMARY KEY,
  cid          TEXT NOT NULL,
  author_did   TEXT NOT NULL REFERENCES actors(did),
  reposter_did TEXT REFERENCES actors(did),
  created_at   INTEGER NOT NULL,
  indexed_at   INTEGER NOT NULL,
  text         TEXT,
  labels_json  TEXT,
  is_hidden    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE media (
  id           INTEGER PRIMARY KEY,
  post_uri     TEXT NOT NULL REFERENCES posts(uri) ON DELETE CASCADE,
  idx          INTEGER NOT NULL,
  kind         TEXT NOT NULL,
  thumb_url    TEXT NOT NULL,
  fullsize_url TEXT,
  playlist_url TEXT,
  alt          TEXT,
  aspect_w     INTEGER,
  aspect_h     INTEGER,
  local_path   TEXT,
  bytes        INTEGER,
  last_used_at INTEGER,
  UNIQUE(post_uri, idx)
);

CREATE TABLE sync_state (
  key         TEXT PRIMARY KEY,
  cursor      TEXT,
  last_run_at INTEGER,
  oldest_seen INTEGER
);

CREATE INDEX idx_posts_indexed_at ON posts(indexed_at DESC);
CREATE INDEX idx_posts_author     ON posts(author_did, indexed_at DESC);
CREATE INDEX idx_media_kind       ON media(kind);
"#;

/// 現在の `user_version` を読み、必要なマイグレーションを順に適用する。
pub fn migrate(conn: &Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current < 1 {
        conn.execute_batch(V1)?;
    }
    // 将来: if current < 2 { conn.execute_batch(V2)?; } ...

    if current != SCHEMA_VERSION {
        // PRAGMA はパラメータバインドできないため直書き（値は内部定数で安全）
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
    }
    Ok(())
}
