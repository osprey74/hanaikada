//! `feedViewPost` からメディア付き投稿を抽出する（DESIGN §6.1）。
//!
//! | embed.$type | 扱い |
//! |---|---|
//! | images#view | 取り込む（最大4枚） |
//! | video#view | 取り込む |
//! | recordWithMedia#view | media 部分のみ取り込む |
//! | external#view | 既定で除外（include_external で OGP サムネ取込） |
//! | record#view / embed なし | 除外 |
//!
//! メディアが 1 枚も無い投稿は `None`（= DB に入れない）。

use crate::bsky::models::{Author, FeedViewPost};
use serde_json::Value;

pub const KIND_IMAGE: &str = "image";
pub const KIND_VIDEO: &str = "video";

const REASON_REPOST: &str = "app.bsky.feed.defs#reasonRepost";

/// actors 行に対応する抽出済みアクター。
#[derive(Debug, Clone)]
pub struct ActorRow {
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<&Author> for ActorRow {
    fn from(a: &Author) -> Self {
        ActorRow {
            did: a.did.clone(),
            handle: a.handle.clone(),
            display_name: a.display_name.clone(),
            avatar_url: a.avatar.clone(),
        }
    }
}

/// media 行に対応する抽出済みメディア。
#[derive(Debug, Clone)]
pub struct ExtractedMedia {
    pub idx: i64,
    pub kind: &'static str,
    pub thumb_url: String,
    pub fullsize_url: Option<String>,
    pub playlist_url: Option<String>,
    pub alt: Option<String>,
    pub aspect_w: Option<i64>,
    pub aspect_h: Option<i64>,
}

/// posts + media に対応する抽出済み投稿。
#[derive(Debug, Clone)]
pub struct ExtractedPost {
    pub uri: String,
    pub cid: String,
    pub author: ActorRow,
    pub reposter: Option<ActorRow>,
    pub created_at: i64,
    pub indexed_at: i64,
    pub text: Option<String>,
    pub labels_json: Option<String>,
    pub media: Vec<ExtractedMedia>,
}

/// feedViewPost を抽出する。メディアが無ければ `None`。
pub fn extract(item: &FeedViewPost, include_external: bool) -> Option<ExtractedPost> {
    let post = &item.post;

    let media = extract_media(post.embed.as_ref(), include_external);
    if media.is_empty() {
        return None;
    }

    // 並び順・遡りの基準はタイムラインの新しさ。リポストは reason.indexedAt（リポスト時刻）、
    // 通常投稿は post.indexedAt を使う（DESIGN §5「indexed_at は並び順の基準」）。
    let post_indexed = parse_ts(&post.indexed_at).unwrap_or(0);
    let (reposter, repost_ts) = parse_reason(item.reason.as_ref());
    let indexed_at = repost_ts.unwrap_or(post_indexed);

    let (text, created_at) = match &post.record {
        Some(rec) => {
            let text = rec
                .get("text")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let created = rec
                .get("createdAt")
                .and_then(Value::as_str)
                .and_then(parse_ts)
                .unwrap_or(indexed_at);
            (text, created)
        }
        None => (None, indexed_at),
    };

    // ラベルは解釈せず配列のまま JSON 文字列で保持（DESIGN §5.1）
    let labels_json = post.labels.as_ref().and_then(|v| match v {
        Value::Array(arr) if !arr.is_empty() => serde_json::to_string(v).ok(),
        _ => None,
    });

    Some(ExtractedPost {
        uri: post.uri.clone(),
        cid: post.cid.clone(),
        author: ActorRow::from(&post.author),
        reposter,
        created_at,
        indexed_at,
        text,
        labels_json,
        media,
    })
}

/// `reason` が reasonRepost なら (リポスト元アクター, リポスト時刻) を返す。
/// リポスト時刻はタイムライン上の並び順・遡り基準に用いる。
fn parse_reason(reason: Option<&Value>) -> (Option<ActorRow>, Option<i64>) {
    let Some(reason) = reason else {
        return (None, None);
    };
    if reason.get("$type").and_then(Value::as_str) != Some(REASON_REPOST) {
        return (None, None);
    }
    let actor = reason
        .get("by")
        .and_then(|by| serde_json::from_value::<Author>(by.clone()).ok())
        .map(|a| ActorRow::from(&a));
    let ts = reason
        .get("indexedAt")
        .and_then(Value::as_str)
        .and_then(parse_ts);
    (actor, ts)
}

/// embed からメディア列を取り出す。
fn extract_media(embed: Option<&Value>, include_external: bool) -> Vec<ExtractedMedia> {
    let Some(embed) = embed else {
        return Vec::new();
    };
    match embed.get("$type").and_then(Value::as_str) {
        Some("app.bsky.embed.images#view") => build_images(embed),
        Some("app.bsky.embed.video#view") => build_video(embed),
        Some("app.bsky.embed.recordWithMedia#view") => {
            // media 部分（images#view または video#view）のみ取り込む
            match embed.get("media") {
                Some(media) => extract_media(Some(media), include_external),
                None => Vec::new(),
            }
        }
        Some("app.bsky.embed.external#view") if include_external => build_external(embed),
        _ => Vec::new(),
    }
}

/// images#view → 最大 4 枚。
fn build_images(embed: &Value) -> Vec<ExtractedMedia> {
    let Some(images) = embed.get("images").and_then(Value::as_array) else {
        return Vec::new();
    };
    images
        .iter()
        .take(4)
        .enumerate()
        .filter_map(|(i, img)| {
            let thumb = img.get("thumb").and_then(Value::as_str)?.to_string();
            let (aw, ah) = aspect(img);
            Some(ExtractedMedia {
                idx: i as i64,
                kind: KIND_IMAGE,
                thumb_url: thumb,
                fullsize_url: img
                    .get("fullsize")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                playlist_url: None,
                alt: alt_of(img),
                aspect_w: aw,
                aspect_h: ah,
            })
        })
        .collect()
}

/// video#view → 1 件。thumb は thumbnail、再生は playlist（HLS）。
fn build_video(embed: &Value) -> Vec<ExtractedMedia> {
    let Some(playlist) = embed.get("playlist").and_then(Value::as_str) else {
        return Vec::new();
    };
    let (aw, ah) = aspect(embed);
    vec![ExtractedMedia {
        idx: 0,
        kind: KIND_VIDEO,
        thumb_url: embed
            .get("thumbnail")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        fullsize_url: None,
        playlist_url: Some(playlist.to_string()),
        alt: alt_of(embed),
        aspect_w: aw,
        aspect_h: ah,
    }]
}

/// external#view → OGP サムネがあれば 1 枚の画像として扱う（設定 ON 時のみ）。
fn build_external(embed: &Value) -> Vec<ExtractedMedia> {
    let ext = match embed.get("external") {
        Some(e) => e,
        None => return Vec::new(),
    };
    let Some(thumb) = ext.get("thumb").and_then(Value::as_str) else {
        return Vec::new();
    };
    vec![ExtractedMedia {
        idx: 0,
        kind: KIND_IMAGE,
        thumb_url: thumb.to_string(),
        fullsize_url: Some(thumb.to_string()),
        playlist_url: None,
        alt: ext
            .get("title")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        aspect_w: None,
        aspect_h: None,
    }]
}

fn aspect(v: &Value) -> (Option<i64>, Option<i64>) {
    let ar = v.get("aspectRatio");
    let w = ar.and_then(|a| a.get("width")).and_then(Value::as_i64);
    let h = ar.and_then(|a| a.get("height")).and_then(Value::as_i64);
    (w, h)
}

fn alt_of(v: &Value) -> Option<String> {
    v.get("alt")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// RFC3339 の日時文字列を Unix 秒へ。失敗時は None。
fn parse_ts(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bsky::models::FeedViewPost;

    fn parse(json: &str) -> FeedViewPost {
        serde_json::from_str(json).expect("valid feedViewPost json")
    }

    fn base_post(embed: &str, reason: &str) -> String {
        format!(
            r#"{{
              {reason}
              "post": {{
                "uri": "at://did:plc:alice/app.bsky.feed.post/1",
                "cid": "bafycid",
                "author": {{ "did": "did:plc:alice", "handle": "alice.bsky.social", "displayName": "Alice" }},
                "record": {{ "$type": "app.bsky.feed.post", "text": "hello", "createdAt": "2026-08-20T10:00:00Z" }},
                "indexedAt": "2026-08-20T10:00:05Z"
                {embed}
              }}
            }}"#
        )
    }

    #[test]
    fn extracts_four_images_with_aspect() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.images#view",
          "images": [
            {"thumb":"t0","fullsize":"f0","alt":"a0","aspectRatio":{"width":16,"height":9}},
            {"thumb":"t1","fullsize":"f1","alt":"","aspectRatio":{"width":1,"height":1}},
            {"thumb":"t2","fullsize":"f2"},
            {"thumb":"t3","fullsize":"f3","aspectRatio":{"width":4,"height":3}}
          ]
        }"#;
        let item = parse(&base_post(embed, ""));
        let ex = extract(&item, false).expect("has media");
        assert_eq!(ex.media.len(), 4);
        assert_eq!(ex.media[0].kind, KIND_IMAGE);
        assert_eq!(ex.media[0].aspect_w, Some(16));
        assert_eq!(ex.media[0].aspect_h, Some(9));
        assert_eq!(ex.media[0].alt.as_deref(), Some("a0"));
        assert_eq!(ex.media[1].alt, None); // 空 alt は None
        assert_eq!(ex.media[2].aspect_w, None); // aspectRatio 欠損 → None（UI で 1:1 フォールバック）
        assert_eq!(ex.media[3].idx, 3);
        assert_eq!(ex.text.as_deref(), Some("hello"));
    }

    #[test]
    fn caps_images_at_four() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.images#view",
          "images": [
            {"thumb":"t0","fullsize":"f0"},{"thumb":"t1","fullsize":"f1"},
            {"thumb":"t2","fullsize":"f2"},{"thumb":"t3","fullsize":"f3"},
            {"thumb":"t4","fullsize":"f4"}
          ]
        }"#;
        let item = parse(&base_post(embed, ""));
        let ex = extract(&item, false).unwrap();
        assert_eq!(ex.media.len(), 4);
    }

    #[test]
    fn extracts_video_playlist() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.video#view",
          "playlist": "https://v/playlist.m3u8",
          "thumbnail": "https://v/thumb.jpg",
          "aspectRatio": {"width":9,"height":16}
        }"#;
        let item = parse(&base_post(embed, ""));
        let ex = extract(&item, false).unwrap();
        assert_eq!(ex.media.len(), 1);
        assert_eq!(ex.media[0].kind, KIND_VIDEO);
        assert_eq!(ex.media[0].playlist_url.as_deref(), Some("https://v/playlist.m3u8"));
        assert_eq!(ex.media[0].thumb_url, "https://v/thumb.jpg");
        assert_eq!(ex.media[0].aspect_h, Some(16));
    }

    #[test]
    fn extracts_record_with_media() {
        // 引用（record）＋画像（media）→ media 部分のみ取り込む
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.recordWithMedia#view",
          "record": { "record": { "uri": "at://did:plc:bob/app.bsky.feed.post/9" } },
          "media": {
            "$type": "app.bsky.embed.images#view",
            "images": [ {"thumb":"t","fullsize":"f","aspectRatio":{"width":2,"height":1}} ]
          }
        }"#;
        let item = parse(&base_post(embed, ""));
        let ex = extract(&item, false).unwrap();
        assert_eq!(ex.media.len(), 1);
        assert_eq!(ex.media[0].aspect_w, Some(2));
    }

    #[test]
    fn excludes_record_only_quote() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.record#view",
          "record": { "uri": "at://did:plc:bob/app.bsky.feed.post/9" }
        }"#;
        let item = parse(&base_post(embed, ""));
        assert!(extract(&item, false).is_none());
    }

    #[test]
    fn excludes_external_by_default_but_includes_when_enabled() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.external#view",
          "external": { "uri":"https://ex", "title":"T", "description":"D", "thumb":"https://ex/thumb.jpg" }
        }"#;
        let item = parse(&base_post(embed, ""));
        assert!(extract(&item, false).is_none()); // 既定は除外
        let ex = extract(&item, true).expect("included when enabled");
        assert_eq!(ex.media.len(), 1);
        assert_eq!(ex.media[0].thumb_url, "https://ex/thumb.jpg");
    }

    #[test]
    fn excludes_post_without_embed() {
        let item = parse(&base_post("", ""));
        assert!(extract(&item, false).is_none());
    }

    #[test]
    fn records_reposter_and_uses_repost_time_for_ordering() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.images#view",
          "images": [ {"thumb":"t","fullsize":"f"} ]
        }"#;
        // 原投稿は 2026-08-20 作成だが、2026-08-29 にリポストされた。
        // 並び順の基準(indexed_at)はリポスト時刻、created_at は原投稿の作成日。
        let reason = r#""reason": {
          "$type": "app.bsky.feed.defs#reasonRepost",
          "by": { "did": "did:plc:carol", "handle": "carol.bsky.social" },
          "indexedAt": "2026-08-29T12:00:00Z"
        },"#;
        let item = parse(&base_post(embed, reason));
        let ex = extract(&item, false).unwrap();

        let reposter = ex.reposter.expect("reposter recorded");
        assert_eq!(reposter.did, "did:plc:carol");
        assert_eq!(reposter.handle, "carol.bsky.social");

        let repost_ts = parse_ts("2026-08-29T12:00:00Z").unwrap();
        let created_ts = parse_ts("2026-08-20T10:00:00Z").unwrap();
        assert_eq!(ex.indexed_at, repost_ts, "並び順はリポスト時刻");
        assert_eq!(ex.created_at, created_ts, "created_at は原投稿の作成日");
    }

    #[test]
    fn no_reposter_for_non_repost_reason() {
        let embed = r#", "embed": {
          "$type": "app.bsky.embed.images#view",
          "images": [ {"thumb":"t","fullsize":"f"} ]
        }"#;
        let item = parse(&base_post(embed, ""));
        let ex = extract(&item, false).unwrap();
        assert!(ex.reposter.is_none());
    }
}
