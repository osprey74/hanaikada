// サムネイルのカスタムプロトコル URL を組み立てる。
// Rust 側で `thumb` スキームを登録済み。WebView2(Windows) は http://thumb.localhost、
// WKWebView(macOS)/Linux は thumb://localhost で配信される。

const isWindows =
  typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");

const THUMB_BASE = isWindows ? "http://thumb.localhost" : "thumb://localhost";
const FULL_BASE = isWindows ? "http://full.localhost" : "full://localhost";

/** 代表メディアの id からサムネ配信 URL を返す。 */
export function thumbSrc(mediaId: number): string {
  return `${THUMB_BASE}/${mediaId}`;
}

/** メディア id から fullsize 配信 URL を返す（ビューアで開いた時のみ）。 */
export function fullSrc(mediaId: number): string {
  return `${FULL_BASE}/${mediaId}`;
}
