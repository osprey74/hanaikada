//! SQLite スキーマのマイグレーション（DESIGN.md §5 準拠）。
//!
//! `user_version` PRAGMA で版管理する。マイグレーションは前方追記のみ。
//! 既存の版から順に適用し、最終的に `SCHEMA_VERSION` へ引き上げる。

use crate::error::Result;
use rusqlite::Connection;

/// 現在のスキーマ版。マイグレーションを追加したらインクリメントする。
pub const SCHEMA_VERSION: i64 = 2;

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

/// v2: ALT・本文の全文検索（DESIGN §7.3）。
/// 日本語の部分一致に対応するため `trigram` トークナイザを使う（3 文字以上で有効。
/// 2 文字以下のクエリは呼び出し側で LIKE にフォールバックする）。
/// rowid = media.id とし、alt（当該メディア）と text（親投稿）を索引する。
const V2: &str = r#"
CREATE VIRTUAL TABLE media_fts USING fts5(
  alt, text,
  tokenize = 'trigram'
);

-- 既存メディアの索引を作り直す（Phase 2 までに取り込んだ分の backfill）
INSERT INTO media_fts(rowid, alt, text)
  SELECT m.id, COALESCE(m.alt, ''), COALESCE(p.text, '')
  FROM media m JOIN posts p ON p.uri = m.post_uri;
"#;

/// 現在の `user_version` を読み、必要なマイグレーションを順に適用する。
pub fn migrate(conn: &Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current < 1 {
        conn.execute_batch(V1)?;
    }
    if current < 2 {
        conn.execute_batch(V2)?;
    }

    if current != SCHEMA_VERSION {
        // PRAGMA はパラメータバインドできないため直書き（値は内部定数で安全）
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
    }
    Ok(())
}
