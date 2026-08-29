//! 同期エンジンが使う書き込み系クエリ（Phase 2）。
//! 参照系（グリッド用）は Phase 3 で追加する。
//!
//! 一部の集計・遡り再開クエリは Phase 3 で参照するため、現時点では未使用。
#![allow(dead_code)]

use crate::error::Result;
use crate::sync::extractor::{ActorRow, ExtractedPost};
use rusqlite::{params, Connection, OptionalExtension};

/// アクターを upsert する（handle / 表示名 / アバターを最新化）。
pub fn upsert_actor(conn: &Connection, actor: &ActorRow, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO actors (did, handle, display_name, avatar_url, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(did) DO UPDATE SET
           handle = excluded.handle,
           display_name = excluded.display_name,
           avatar_url = excluded.avatar_url,
           updated_at = excluded.updated_at",
        params![
            actor.did,
            actor.handle,
            actor.display_name,
            actor.avatar_url,
            now
        ],
    )?;
    Ok(())
}

/// 指定 URI の投稿が既に DB にあるか（差分同期の打ち切り判定）。
pub fn is_post_known(conn: &Connection, uri: &str) -> Result<bool> {
    let found: Option<i64> = conn
        .query_row("SELECT 1 FROM posts WHERE uri = ?1", params![uri], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(found.is_some())
}

/// 抽出済み投稿を actors / posts / media に格納する。
/// 戻り値は新規に挿入された media 行数（0 なら既知で全てスキップ）。
pub fn insert_post_with_media(conn: &Connection, post: &ExtractedPost, now: i64) -> Result<usize> {
    upsert_actor(conn, &post.author, now)?;
    if let Some(reposter) = &post.reposter {
        upsert_actor(conn, reposter, now)?;
    }

    conn.execute(
        "INSERT INTO posts
           (uri, cid, author_did, reposter_did, created_at, indexed_at, text, labels_json, is_hidden)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
         ON CONFLICT(uri) DO NOTHING",
        params![
            post.uri,
            post.cid,
            post.author.did,
            post.reposter.as_ref().map(|r| &r.did),
            post.created_at,
            post.indexed_at,
            post.text,
            post.labels_json,
        ],
    )?;

    let mut inserted = 0usize;
    for m in &post.media {
        let n = conn.execute(
            "INSERT INTO media
               (post_uri, idx, kind, thumb_url, fullsize_url, playlist_url, alt, aspect_w, aspect_h)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(post_uri, idx) DO NOTHING",
            params![
                post.uri,
                m.idx,
                m.kind,
                m.thumb_url,
                m.fullsize_url,
                m.playlist_url,
                m.alt,
                m.aspect_w,
                m.aspect_h,
            ],
        )?;
        inserted += n;
    }
    Ok(inserted)
}

/// media の総件数。
pub fn media_total(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT count(*) FROM media", [], |r| r.get(0))?)
}

/// posts の総件数。
pub fn post_total(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT count(*) FROM posts", [], |r| r.get(0))?)
}

/// actors の総件数。
pub fn actor_total(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT count(*) FROM actors", [], |r| r.get(0))?)
}

// --- sync_state（バックフィル cursor・遡り基準） ---
//
// `cursor` は初回（バックフィル）同期の遡り位置を保持する。差分同期は先頭から
// 取得し直すため cursor を必要とせず、上書きもしない（`touch_sync_state` を使う）。

/// 初回同期用: cursor を含めて sync_state を upsert する（バックフィルの遡り位置を保存）。
pub fn set_sync_state(
    conn: &Connection,
    key: &str,
    cursor: Option<&str>,
    last_run_at: i64,
    oldest_seen: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_state (key, cursor, last_run_at, oldest_seen)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET
           cursor = excluded.cursor,
           last_run_at = excluded.last_run_at,
           oldest_seen = COALESCE(excluded.oldest_seen, sync_state.oldest_seen)",
        params![key, cursor, last_run_at, oldest_seen],
    )?;
    Ok(())
}

/// 差分同期用: cursor には触れず last_run_at / oldest_seen のみ更新する。
/// バックフィル cursor（初回同期の遡り位置）を差分同期が壊さないためのもの。
pub fn touch_sync_state(
    conn: &Connection,
    key: &str,
    last_run_at: i64,
    oldest_seen: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_state (key, cursor, last_run_at, oldest_seen)
         VALUES (?1, NULL, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           last_run_at = excluded.last_run_at,
           oldest_seen = COALESCE(excluded.oldest_seen, sync_state.oldest_seen)",
        params![key, last_run_at, oldest_seen],
    )?;
    Ok(())
}

/// 初回同期のレジューム用 cursor（遡り位置）。未保存なら None。
pub fn get_sync_cursor(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT cursor FROM sync_state WHERE key = ?1",
            params![key],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

/// sync_state の oldest_seen（初回遡りの到達点）。
pub fn get_oldest_seen(conn: &Connection, key: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT oldest_seen FROM sync_state WHERE key = ?1",
            params![key],
            |r| r.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use rusqlite::Connection;

    const KEY: &str = "timeline";

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        migrations::migrate(&c).unwrap();
        c
    }

    #[test]
    fn touch_sync_state_preserves_backfill_cursor() {
        let c = conn();
        // 初回同期がバックフィル cursor を保存する
        set_sync_state(&c, KEY, Some("CUR_DEEP"), 100, Some(5000)).unwrap();
        assert_eq!(get_sync_cursor(&c, KEY).unwrap().as_deref(), Some("CUR_DEEP"));
        assert_eq!(get_oldest_seen(&c, KEY).unwrap(), Some(5000));

        // 差分同期は cursor を壊さず last_run_at / oldest_seen のみ更新する
        touch_sync_state(&c, KEY, 200, Some(4000)).unwrap();
        assert_eq!(
            get_sync_cursor(&c, KEY).unwrap().as_deref(),
            Some("CUR_DEEP"),
            "差分同期はバックフィル cursor を保持する"
        );
        assert_eq!(get_oldest_seen(&c, KEY).unwrap(), Some(4000));

        // oldest_seen は COALESCE で None を無視し既存値を維持する
        touch_sync_state(&c, KEY, 300, None).unwrap();
        assert_eq!(
            get_oldest_seen(&c, KEY).unwrap(),
            Some(4000),
            "None の oldest_seen は既存値を維持する"
        );
        assert_eq!(get_sync_cursor(&c, KEY).unwrap().as_deref(), Some("CUR_DEEP"));
    }

    #[test]
    fn touch_sync_state_inserts_when_absent() {
        let c = conn();
        touch_sync_state(&c, KEY, 100, Some(9000)).unwrap();
        assert_eq!(get_sync_cursor(&c, KEY).unwrap(), None);
        assert_eq!(get_oldest_seen(&c, KEY).unwrap(), Some(9000));
    }

    #[test]
    fn set_sync_state_updates_cursor_on_resume() {
        let c = conn();
        set_sync_state(&c, KEY, Some("CUR_1"), 100, Some(5000)).unwrap();
        // レジューム継続で cursor が前進し、oldest_seen はより古い値へ更新される
        set_sync_state(&c, KEY, Some("CUR_2"), 200, Some(3000)).unwrap();
        assert_eq!(get_sync_cursor(&c, KEY).unwrap().as_deref(), Some("CUR_2"));
        assert_eq!(get_oldest_seen(&c, KEY).unwrap(), Some(3000));
    }
}
