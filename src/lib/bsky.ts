// AT-URI と bsky.app の相互変換など Bluesky 固有のユーティリティ。

/** at://did/app.bsky.feed.post/rkey → https://bsky.app/profile/did/post/rkey */
export function postUrl(atUri: string): string | null {
  const m = atUri.match(/^at:\/\/([^/]+)\/app\.bsky\.feed\.post\/(.+)$/);
  if (!m) return null;
  return `https://bsky.app/profile/${m[1]}/post/${m[2]}`;
}
