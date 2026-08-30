//! 同期エンジンが使う書き込み系クエリ（Phase 2）。
//! 参照系（グリッド用）は Phase 3 で追加する。
//!
//! 一部の集計・遡り再開クエリは Phase 3 で参照するため、現時点では未使用。
#![allow(dead_code)]

use crate::error::Result;
use crate::sync::extractor::{ActorRow, ExtractedPost};
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

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
        if n == 1 {
            // 全文検索の索引を同時に張る（rowid = 挿入した media.id）。
            let media_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO media_fts (rowid, alt, text) VALUES (?1, ?2, ?3)",
                params![
                    media_id,
                    m.alt.as_deref().unwrap_or(""),
                    post.text.as_deref().unwrap_or(""),
                ],
            )?;
        }
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

// --- 参照系（グリッド用・Phase 3） ---

/// フロントから渡すグリッドの絞り込み条件（DESIGN §7.3）。全条件 AND。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFilter {
    /// "all" | "image" | "video"。None/"all" は種別無指定。
    pub media_type: Option<String>,
    /// indexed_at >= since_ts（期間の下限。今日/7日/30日はフロントで算出）。
    pub since_ts: Option<i64>,
    /// indexed_at < until_ts（カスタム期間の上限）。
    pub until_ts: Option<i64>,
    /// false ならリポスト経由（reposter_did あり）を除外。None は含める。
    pub include_reposts: Option<bool>,
    /// 投稿者 DID の OR 絞り込み（author_did）。
    pub actor_dids: Option<Vec<String>>,
    /// ALT・本文の検索語（3 文字以上は FTS5 trigram、以下は LIKE）。
    pub query: Option<String>,
}

/// グリッドの 1 タイル（既定はまとめ表示: 投稿単位、代表メディア + 枚数）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTile {
    pub post_uri: String,
    /// 代表メディア（最小 idx。種別絞り込み時はその種別の最小 idx）の id。サムネ配信のキー。
    pub media_id: i64,
    pub kind: String,
    pub thumb_url: String,
    pub aspect_w: Option<i64>,
    pub aspect_h: Option<i64>,
    pub alt: Option<String>,
    /// 投稿内のメディア総数（>1 で枚数バッジ）。
    pub media_count: i64,
    /// 投稿内に動画を含むか（動画バッジ）。
    pub has_video: bool,
    pub author_did: String,
    pub author_handle: String,
    pub author_display_name: Option<String>,
    pub author_avatar: Option<String>,
    /// リポスト経由の場合のリポスト元ハンドル。
    pub reposter_handle: Option<String>,
    pub indexed_at: i64,
    pub created_at: i64,
    pub text: Option<String>,
    pub labels_json: Option<String>,
}

/// サイドバーの投稿者リスト行（メディア投稿を持つ author のみ・件数付き）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorSummary {
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub count: i64,
}

/// WHERE 条件と対応するパラメータを組み立てる（query_media / media_count で共用）。
/// `m`（メディア）・`p`（投稿）・`a`/`ra`（アクター）のエイリアス前提。
fn build_conditions(filter: &MediaFilter) -> (Vec<String>, Vec<Value>) {
    let mut conds: Vec<String> = vec!["p.is_hidden = 0".to_string()];
    let mut params: Vec<Value> = Vec::new();

    match filter.media_type.as_deref() {
        Some("image") | Some("video") => {
            conds.push("m.kind = ?".to_string());
            params.push(Value::Text(filter.media_type.clone().unwrap()));
        }
        _ => {}
    }
    if let Some(since) = filter.since_ts {
        conds.push("p.indexed_at >= ?".to_string());
        params.push(Value::Integer(since));
    }
    if let Some(until) = filter.until_ts {
        conds.push("p.indexed_at < ?".to_string());
        params.push(Value::Integer(until));
    }
    if filter.include_reposts == Some(false) {
        conds.push("p.reposter_did IS NULL".to_string());
    }
    if let Some(dids) = filter.actor_dids.as_ref().filter(|v| !v.is_empty()) {
        let placeholders = vec!["?"; dids.len()].join(", ");
        conds.push(format!("p.author_did IN ({placeholders})"));
        for d in dids {
            params.push(Value::Text(d.clone()));
        }
    }
    if let Some(q) = filter.query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if q.chars().count() >= 3 {
            // FTS5 trigram。フレーズとして扱い、内部の " は "" にエスケープする。
            conds.push(
                "p.uri IN (SELECT m2.post_uri FROM media m2 \
                 JOIN media_fts ON media_fts.rowid = m2.id WHERE media_fts MATCH ?)"
                    .to_string(),
            );
            params.push(Value::Text(format!("\"{}\"", q.replace('"', "\"\""))));
        } else {
            // 2 文字以下は trigram で拾えないため LIKE にフォールバック。
            conds.push(
                "(p.text LIKE ? OR EXISTS(SELECT 1 FROM media ml \
                 WHERE ml.post_uri = p.uri AND ml.alt LIKE ?))"
                    .to_string(),
            );
            let like = format!("%{q}%");
            params.push(Value::Text(like.clone()));
            params.push(Value::Text(like));
        }
    }
    (conds, params)
}

/// 絞り込みに一致する投稿（タイル）を新しい順に返す（まとめ表示・ページング）。
pub fn query_media(
    conn: &Connection,
    filter: &MediaFilter,
    offset: i64,
    limit: i64,
) -> Result<Vec<MediaTile>> {
    let (conds, mut params) = build_conditions(filter);
    let where_sql = conds.join(" AND ");

    // min(m.idx) を単一集約として使い、bare column（m.*）を代表行（最小 idx）から取る
    // SQLite の仕様を利用する。media_count / has_video は投稿全体の相関サブクエリ。
    let sql = format!(
        "SELECT p.uri, m.id, m.kind, m.thumb_url, m.aspect_w, m.aspect_h, m.alt,
                (SELECT count(*) FROM media mc WHERE mc.post_uri = p.uri) AS media_count,
                EXISTS(SELECT 1 FROM media mv WHERE mv.post_uri = p.uri AND mv.kind = 'video') AS has_video,
                p.author_did, a.handle, a.display_name, a.avatar_url,
                ra.handle AS reposter_handle,
                p.indexed_at, p.created_at, p.text, p.labels_json,
                min(m.idx) AS rep_idx
         FROM posts p
         JOIN actors a ON a.did = p.author_did
         LEFT JOIN actors ra ON ra.did = p.reposter_did
         JOIN media m ON m.post_uri = p.uri
         WHERE {where_sql}
         GROUP BY p.uri
         ORDER BY p.indexed_at DESC, p.uri DESC
         LIMIT ? OFFSET ?"
    );
    params.push(Value::Integer(limit));
    params.push(Value::Integer(offset));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params), |r| {
        Ok(MediaTile {
            post_uri: r.get(0)?,
            media_id: r.get(1)?,
            kind: r.get(2)?,
            thumb_url: r.get(3)?,
            aspect_w: r.get(4)?,
            aspect_h: r.get(5)?,
            alt: r.get(6)?,
            media_count: r.get(7)?,
            has_video: r.get::<_, i64>(8)? != 0,
            author_did: r.get(9)?,
            author_handle: r.get(10)?,
            author_display_name: r.get(11)?,
            author_avatar: r.get(12)?,
            reposter_handle: r.get(13)?,
            indexed_at: r.get(14)?,
            created_at: r.get(15)?,
            text: r.get(16)?,
            labels_json: r.get(17)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 絞り込みに一致するタイル（投稿）総数（ステータスバー「N 件中 M 件」用）。
pub fn media_count(conn: &Connection, filter: &MediaFilter) -> Result<i64> {
    let (conds, params) = build_conditions(filter);
    let where_sql = conds.join(" AND ");
    let sql = format!(
        "SELECT COUNT(DISTINCT p.uri)
         FROM posts p JOIN media m ON m.post_uri = p.uri
         WHERE {where_sql}"
    );
    Ok(conn.query_row(&sql, params_from_iter(params), |r| r.get(0))?)
}

/// メディア投稿を持つ投稿者の一覧（件数の多い順）。サイドバー用。
pub fn list_actors(conn: &Connection) -> Result<Vec<ActorSummary>> {
    let mut stmt = conn.prepare(
        "SELECT p.author_did, a.handle, a.display_name, a.avatar_url,
                COUNT(DISTINCT p.uri) AS cnt
         FROM posts p
         JOIN actors a ON a.did = p.author_did
         JOIN media m ON m.post_uri = p.uri
         WHERE p.is_hidden = 0
         GROUP BY p.author_did
         ORDER BY cnt DESC, a.handle ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ActorSummary {
            did: r.get(0)?,
            handle: r.get(1)?,
            display_name: r.get(2)?,
            avatar_url: r.get(3)?,
            count: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 代表メディアのサムネ URL を引く（サムネ配信プロトコルの解決用）。
pub fn thumb_url_of(conn: &Connection, media_id: i64) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT thumb_url FROM media WHERE id = ?1",
            params![media_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

/// メディアの fullsize URL を引く（fullsize 配信プロトコルの解決用）。動画等は None。
pub fn fullsize_url_of(conn: &Connection, media_id: i64) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT fullsize_url FROM media WHERE id = ?1",
            params![media_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

/// ビューア用: 投稿内の全メディア（idx 順）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostMediaItem {
    pub media_id: i64,
    pub idx: i64,
    pub kind: String,
    pub thumb_url: String,
    pub fullsize_url: Option<String>,
    pub playlist_url: Option<String>,
    pub alt: Option<String>,
    pub aspect_w: Option<i64>,
    pub aspect_h: Option<i64>,
}

/// 指定投稿の全メディアを idx 順で返す（ライトボックスの送り用）。
pub fn get_post_media(conn: &Connection, post_uri: &str) -> Result<Vec<PostMediaItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, idx, kind, thumb_url, fullsize_url, playlist_url, alt, aspect_w, aspect_h
         FROM media WHERE post_uri = ?1 ORDER BY idx",
    )?;
    let rows = stmt.query_map(params![post_uri], |r| {
        Ok(PostMediaItem {
            media_id: r.get(0)?,
            idx: r.get(1)?,
            kind: r.get(2)?,
            thumb_url: r.get(3)?,
            fullsize_url: r.get(4)?,
            playlist_url: r.get(5)?,
            alt: r.get(6)?,
            aspect_w: r.get(7)?,
            aspect_h: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
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

    // --- 参照系（query_media / list_actors / media_count）---

    use crate::sync::extractor::{ActorRow, ExtractedMedia, ExtractedPost};

    fn actor(did: &str, handle: &str) -> ActorRow {
        ActorRow {
            did: did.into(),
            handle: handle.into(),
            display_name: None,
            avatar_url: None,
        }
    }

    fn img(idx: i64, alt: Option<&str>) -> ExtractedMedia {
        ExtractedMedia {
            idx,
            kind: "image",
            thumb_url: format!("https://cdn/thumb{idx}"),
            fullsize_url: Some(format!("https://cdn/full{idx}")),
            playlist_url: None,
            alt: alt.map(str::to_string),
            aspect_w: Some(4),
            aspect_h: Some(3),
        }
    }

    fn video(idx: i64) -> ExtractedMedia {
        ExtractedMedia {
            idx,
            kind: "video",
            thumb_url: format!("https://cdn/vthumb{idx}"),
            fullsize_url: None,
            playlist_url: Some("https://cdn/playlist.m3u8".into()),
            alt: None,
            aspect_w: Some(9),
            aspect_h: Some(16),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn post(
        uri: &str,
        author: ActorRow,
        reposter: Option<ActorRow>,
        indexed_at: i64,
        text: Option<&str>,
        media: Vec<ExtractedMedia>,
    ) -> ExtractedPost {
        ExtractedPost {
            uri: uri.into(),
            cid: "cid".into(),
            author,
            reposter,
            created_at: indexed_at,
            indexed_at,
            text: text.map(str::to_string),
            labels_json: None,
            media,
        }
    }

    /// テスト用データ: 4 枚組（alice, alt "a cat photo"）/ 動画（bob）/ リポスト（carol→alice, 単画像）。
    fn seed(c: &Connection) {
        let alice = actor("did:alice", "alice.test");
        let bob = actor("did:bob", "bob.test");
        let carol = actor("did:carol", "carol.test");

        insert_post_with_media(
            c,
            &post(
                "at://alice/1",
                alice.clone(),
                None,
                1000,
                Some("四枚組の投稿"),
                vec![
                    img(0, Some("a cat photo")),
                    img(1, None),
                    img(2, None),
                    img(3, None),
                ],
            ),
            9999,
        )
        .unwrap();

        insert_post_with_media(
            c,
            &post("at://bob/1", bob, None, 2000, Some("動画です"), vec![video(0)]),
            9999,
        )
        .unwrap();

        insert_post_with_media(
            c,
            &post(
                "at://alice/2",
                alice,
                Some(carol),
                3000,
                Some("硝子細工の写真"),
                vec![img(0, Some("硝子のグラス"))],
            ),
            9999,
        )
        .unwrap();
    }

    #[test]
    fn query_media_groups_by_post_newest_first() {
        let c = conn();
        seed(&c);
        let tiles = query_media(&c, &MediaFilter::default(), 0, 100).unwrap();
        assert_eq!(tiles.len(), 3, "投稿単位で 3 タイル");
        // 新しい順: alice/2(3000) → bob/1(2000) → alice/1(1000)
        assert_eq!(tiles[0].post_uri, "at://alice/2");
        assert_eq!(tiles[1].post_uri, "at://bob/1");
        assert_eq!(tiles[2].post_uri, "at://alice/1");

        let four = &tiles[2];
        assert_eq!(four.media_count, 4, "4 枚組は media_count=4");
        assert_eq!(four.kind, "image");
        assert!(!four.has_video);
        // 代表は最小 idx（idx=0, alt="a cat photo"）
        assert_eq!(four.alt.as_deref(), Some("a cat photo"));

        // リポストはリポスト元ハンドルを持つ
        assert_eq!(tiles[0].reposter_handle.as_deref(), Some("carol.test"));
        // 動画投稿は has_video
        assert!(tiles[1].has_video);
        assert_eq!(tiles[1].kind, "video");
    }

    #[test]
    fn query_media_filters_by_kind() {
        let c = conn();
        seed(&c);
        let f = MediaFilter {
            media_type: Some("video".into()),
            ..Default::default()
        };
        let tiles = query_media(&c, &f, 0, 100).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].post_uri, "at://bob/1");
    }

    #[test]
    fn query_media_excludes_reposts_when_disabled() {
        let c = conn();
        seed(&c);
        let f = MediaFilter {
            include_reposts: Some(false),
            ..Default::default()
        };
        let tiles = query_media(&c, &f, 0, 100).unwrap();
        assert!(tiles.iter().all(|t| t.reposter_handle.is_none()));
        assert_eq!(tiles.len(), 2, "リポスト 1 件を除外");
    }

    #[test]
    fn query_media_filters_by_actor() {
        let c = conn();
        seed(&c);
        let f = MediaFilter {
            actor_dids: Some(vec!["did:alice".into()]),
            ..Default::default()
        };
        let tiles = query_media(&c, &f, 0, 100).unwrap();
        assert_eq!(tiles.len(), 2);
        assert!(tiles.iter().all(|t| t.author_did == "did:alice"));
    }

    #[test]
    fn query_media_search_fts_and_like_fallback() {
        let c = conn();
        seed(&c);

        // 3 文字以上（英字）→ FTS5 trigram。alt "a cat photo" にヒット
        let f = MediaFilter {
            query: Some("cat".into()),
            ..Default::default()
        };
        let tiles = query_media(&c, &f, 0, 100).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].post_uri, "at://alice/1");

        // 3 文字以上（日本語）→ 本文 "硝子細工の写真" にヒット
        let f = MediaFilter {
            query: Some("硝子細".into()),
            ..Default::default()
        };
        let tiles = query_media(&c, &f, 0, 100).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].post_uri, "at://alice/2");

        // 2 文字 → LIKE フォールバック。本文/alt の "硝子" にヒット
        let f = MediaFilter {
            query: Some("硝子".into()),
            ..Default::default()
        };
        let tiles = query_media(&c, &f, 0, 100).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].post_uri, "at://alice/2");
    }

    /// 10,000 件規模のメディアを持つ一時ファイル DB を組み、参照系クエリの応答時間を測る。
    /// 通常テストからは除外（`--ignored` で実行）。実行例:
    ///   cargo test --release -p hanaikada bench_query_10k -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_query_10k() {
        use std::time::Instant;

        // WAL の一時ファイル DB（本番と同条件に近づける）
        let dir = std::env::temp_dir().join("hanaikada_bench");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bench.db");
        let _ = std::fs::remove_file(&path);
        let c = Connection::open(&path).unwrap();
        c.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )
        .unwrap();
        migrations::migrate(&c).unwrap();

        // --- 約 10,000 メディアを投入（1〜4枚・動画・リポスト・ALT/本文つきを混在） ---
        let n_actors = 300;
        for i in 0..n_actors {
            upsert_actor(&c, &actor(&format!("did:a{i}"), &format!("user{i}.test")), 1).unwrap();
        }
        let target_media = 10_000i64;
        let mut media = 0i64;
        let mut p = 0i64;
        c.execute_batch("BEGIN").unwrap();
        while media < target_media {
            let author = actor(&format!("did:a{}", p % n_actors), &format!("user{}.test", p % n_actors));
            let reposter = if p % 5 == 0 {
                Some(actor(&format!("did:a{}", (p + 7) % n_actors), &format!("user{}.test", (p + 7) % n_actors)))
            } else {
                None
            };
            // メディア構成: 60% 1枚 / 20% 4枚 / 12% 2枚 / 8% 動画
            let items: Vec<ExtractedMedia> = match p % 25 {
                r if r < 15 => vec![img(0, if p % 11 == 0 { Some("a cat photo by the window") } else { None })],
                r if r < 20 => (0..4).map(|k| img(k, if k == 0 && p % 13 == 0 { Some("硝子細工のグラス") } else { None })).collect(),
                r if r < 23 => (0..2).map(|k| img(k, None)).collect(),
                _ => vec![video(0)],
            };
            let text = if p % 7 == 0 { Some("窓辺の硝子と猫の写真です") } else { None };
            let indexed = 2_000_000_000 - p; // 新しい順に並ぶよう単調減少
            let ep = post(&format!("at://user{}/{}", p % n_actors, p), author, reposter, indexed, text, items);
            media += insert_post_with_media(&c, &ep, 1).unwrap() as i64;
            p += 1;
        }
        c.execute_batch("COMMIT").unwrap();
        c.execute_batch("ANALYZE").unwrap();

        let total: i64 = c.query_row("SELECT count(*) FROM media", [], |r| r.get(0)).unwrap();
        let posts: i64 = c.query_row("SELECT count(*) FROM posts", [], |r| r.get(0)).unwrap();
        eprintln!("\n=== bench_query_10k: media={total} posts={posts} ===");

        // 計測ヘルパ: 数回まわして中央値相当（最小値）を採る
        let bench = |label: &str, f: &dyn Fn()| {
            // ウォームアップ
            f();
            let mut best = f64::MAX;
            for _ in 0..5 {
                let t = Instant::now();
                f();
                best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            }
            eprintln!("  {label:<40} {best:>7.2} ms");
            best
        };

        let all = MediaFilter::default();
        let kind = MediaFilter { media_type: Some("video".into()), ..Default::default() };
        let no_rt = MediaFilter { include_reposts: Some(false), ..Default::default() };
        let actors_f = MediaFilter { actor_dids: Some(vec!["did:a1".into(), "did:a2".into(), "did:a3".into()]), ..Default::default() };
        let fts = MediaFilter { query: Some("cat".into()), ..Default::default() };
        let fts_jp = MediaFilter { query: Some("硝子".into()), ..Default::default() };

        let worst_first_page = [
            bench("query_media all (page0,60)", &|| { query_media(&c, &all, 0, 60).unwrap(); }),
            bench("query_media kind=video", &|| { query_media(&c, &kind, 0, 60).unwrap(); }),
            bench("query_media no-reposts", &|| { query_media(&c, &no_rt, 0, 60).unwrap(); }),
            bench("query_media actors(3)", &|| { query_media(&c, &actors_f, 0, 60).unwrap(); }),
            bench("query_media FTS 'cat'", &|| { query_media(&c, &fts, 0, 60).unwrap(); }),
            bench("query_media LIKE '硝子'(2字)", &|| { query_media(&c, &fts_jp, 0, 60).unwrap(); }),
            bench("query_media deep offset(9000)", &|| { query_media(&c, &all, 9000, 60).unwrap(); }),
        ].iter().cloned().fold(0.0_f64, f64::max);

        bench("media_count all", &|| { media_count(&c, &all).unwrap(); });
        bench("media_count no-reposts", &|| { media_count(&c, &no_rt).unwrap(); });
        bench("list_actors", &|| { list_actors(&c).unwrap(); });

        eprintln!("=== 最も遅いフィルタ適用（1ページ目取得）: {worst_first_page:.2} ms（目標 <200ms）===\n");
        assert!(
            worst_first_page < 200.0,
            "フィルタ適用の 1 ページ目取得が 200ms を超過: {worst_first_page:.2} ms"
        );

        drop(c);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn media_count_and_list_actors() {
        let c = conn();
        seed(&c);
        assert_eq!(media_count(&c, &MediaFilter::default()).unwrap(), 3);

        let f = MediaFilter {
            media_type: Some("image".into()),
            ..Default::default()
        };
        assert_eq!(media_count(&c, &f).unwrap(), 2, "画像投稿は 2 件");

        let actors = list_actors(&c).unwrap();
        // alice=2, bob=1（件数降順）
        assert_eq!(actors[0].did, "did:alice");
        assert_eq!(actors[0].count, 2);
        assert!(actors.iter().any(|a| a.did == "did:bob" && a.count == 1));
    }
}
