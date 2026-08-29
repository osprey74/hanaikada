# 花筏 (Hanaikada) — learnings.md

実装上の検証結果・調査記録。DESIGN.md / HANDOFF.md の未決事項に対する一次情報の裏取りを残す。

- 最終更新: 2026-08-29

---

## L1. `createSession` / `refreshSession` のレート制限（HANDOFF §5-1・DESIGN §12-3）

**結論**: `createSession` は **アカウント単位で 30 回 / 5 分・300 回 / 日**。`refreshSession` には専用の制限がなく、グローバル制限（**IP 単位 3,000 回 / 5 分**）に含まれる。

| 対象 | 制限値 | キー |
|---|---|---|
| グローバル（全エンドポイント） | 3,000 回 / 5 分 | IP アドレス |
| `com.atproto.server.createSession` | 30 回 / 5 分・300 回 / 日 | アカウント（DID） |
| `com.atproto.server.refreshSession` | 専用制限なし（グローバルに含む） | — |
| `updateHandle`（参考） | 10 回 / 5 分・50 回 / 日 | アカウント |
| 書き込み点数制（参考・本アプリは非該当） | 5,000 点 / 時・35,000 点 / 日（CREATE 3 / UPDATE 2 / DELETE 1 点） | アカウント |

**設計への含意**
- 起動のたびに `createSession` を呼ぶ実装は不可。300 回 / 日の枠を、通常利用（頻繁な再起動・複数ウィンドウ）で使い切りかねない。**キーチェーンの `refreshJwt` から `refreshSession` で復帰する経路を最優先**とする（DESIGN §4.2 の方針は正しいことを確認）。
- `createSession` を呼ぶのは「初回ログイン」「`refreshJwt` 失効による再ログイン」のみに限定する。
- リフレッシュ自体はグローバル枠なので、ポーリング（既定 5 分間隔）と合わせても 3,000 / 5 分にはまず届かない。ただしバックオフ実装（§6.3）は 429 に対して必須。

**出典**
- Bluesky Rate Limits（公式）: https://bsky.network/docs/rate-limits/ （旧 https://docs.bsky.app/docs/advanced-guides/rate-limits からリダイレクト）
- Rate Limits, PDS Distribution v3 発表: https://docs.bsky.app/blog/rate-limits-pds-v3

**確実性: 高**（公式ドキュメントの明示値）

---

## L2. WebView2 / WKWebView の HLS 再生可否（HANDOFF §5-2・DESIGN §12-4）

**結論**: **Windows の WebView2 は `<video>` で `.m3u8`（HLS）をネイティブ再生できない。macOS の WKWebView はネイティブ再生できる。** よって **hls.js を同梱し、`Hls.isSupported()` で分岐する統一実装**を採る。外部ブラウザ誘導へのフォールバックは不要になる。

**根拠**
- WebView2 は Chromium ベース。Chromium 系（Chrome / Edge / WebView2）は `<video>` の HLS ネイティブ再生に非対応で、MSE 上で動く JS ライブラリ（hls.js 等）が必要。WebView2 への HLS ネイティブ対応要望は Issue #5092 として 2025-02 に起票され、2025 年時点で未対応（open）。
- macOS の Tauri は WKWebView（WebKit）を使い、WebKit は Safari 同様 `<video>` で HLS をネイティブ再生できる。
- hls.js は MSE（Media Source Extensions）対応ブラウザで動作する。WebView2 は MSE 対応のため hls.js が使える。

**採用する実装パターン**
```ts
import Hls from "hls.js";

function attachHls(video: HTMLVideoElement, playlistUrl: string) {
  if (video.canPlayType("application/vnd.apple.mpegurl")) {
    // macOS WKWebView: ネイティブ HLS
    video.src = playlistUrl;
  } else if (Hls.isSupported()) {
    // Windows WebView2: MSE 経由で hls.js
    const hls = new Hls();
    hls.loadSource(playlistUrl);
    hls.attachMedia(video);
    return () => hls.destroy(); // クリーンアップ必須
  } else {
    // 想定外環境のみ: サムネ + 外部ブラウザ誘導にフォールバック
  }
}
```

**設計への含意**
- Phase 4 の受け入れ条件「Windows / macOS 双方で動画が再生できる」は、hls.js 同梱により **両 OS でアプリ内再生を主経路にできる**。「不可な環境では外部ブラウザ誘導にフォールバック」は最終手段として残すが、通常は発火しない。
- hls.js を HANDOFF §3 のフロントエンド依存に追加する。オフライン動作のためローカル同梱（CDN 参照しない）。
- 動画本体（HLS セグメント）のディスクキャッシュは MVP では見送り、再生時ストリーミングとする（DESIGN §9 はサムネと fullsize 画像のみをキャッシュ対象と規定しており、これに沿う）。

**注意点**
- hls.js は `destroy()` を呼ばないとバッファとイベントリスナが残る。ビューアのアンマウント・動画切替時に必ず破棄する。
- Windows で実機確認する際は WebView2 ランタイムのバージョンに依存しないよう、Tauri のバンドル設定で WebView2 の配布方式（evergreen / fixed）を後日決める。

**出典**
- WebView2Feedback Issue #5092「Natively support play HLS video」: https://github.com/MicrosoftEdge/WebView2Feedback/issues/5092
- hls.js（MSE ベース、Chromium で HLS 再生）: https://github.com/video-dev/hls.js/
- HTTP Live Streaming（ブラウザ対応状況）: https://en.wikipedia.org/wiki/HTTP_Live_Streaming

**確実性: 高**（WebView2 非対応・WKWeb’View ネイティブ対応・hls.js の MSE 動作はいずれも確立した事実）。ただし**実機（Windows WebView2）での hls.js 動作サンプルによる最終確認は Phase 4 着手時に一度行うこと**。

---

## L3. 初回同期の既定遡り期間（HANDOFF §5-3・DESIGN §12-1）

**結論（暫定決定）**: 既定 **30 日**を維持する。設定で 7 / 30 / 90 日を選べる（design_handoff の設定仕様に準拠）。ただし**確定にはリリース前に実アカウントでの計測が必要**。これは実装（Phase 2）が動かないと取れないため、本項は「計測タスク付きの暫定決定」とする。

**根拠・考え方**
- `getTimeline` はメディア絞り込み不可。100 件取得してもメディア付きは十数件のため、30 日を埋めるにはページングが必要（HANDOFF §2 Phase 2 注意点）。
- 数百アカウントをフォローする想定利用者（UI-BRIEF §2）では、30 日で数千〜万件規模のメディアになりうる。初回取得の所要時間と `createSession` 枠は無関係（同期は `getTimeline` = グローバル枠）だが、初回の総リクエスト数と体感時間が UX を左右する。

**Phase 2 で計測すべき項目**（learnings に追記する）
1. 実アカウントで 30 日分を取得したときの: 総ページ数 / 総メディア件数 / 所要時間 / 429 発生有無
2. 上記から、既定 30 日が「数分で終わる」目標（HANDOFF §2 の空グリッド文言「初回は数分かかります」）に収まるか
3. 収まらない場合、既定を短縮（例 14 日）するか、初回上限ページ数（現状 30 ページ）を調整するか

**確実性: 低**（実データ未取得。現時点は設計上の妥当な既定値の据え置き）

---

## L4. 同一投稿の複数枚: まとめる / ばらす（HANDOFF §5-4・DESIGN §12-2）

**結論（決定）**: 既定は **「1 タイルにまとめる（枚数バッジ付き）」**。設定で「ばらして並べる」に切替可能。

**根拠**
- design_handoff の確定 UI 仕様（`design_handoff_hanaikada/DESIGN.md` §2h 設定表）で、表示設定「複数枚投稿」の既定が **まとめる** と確定済み。グリッドのタイル仕様（複数枚バッジ）もまとめ表示前提で描かれている。
- 「ぼんやり眺める」が利用の 8 割（UI-BRIEF §2）。まとめ表示のほうが 1 投稿 = 1 タイルで時系列の密度が上がり、眺める用途に合う。ばらし表示は「特定投稿者を遡る」ユースケース向けにオプションとして残す。

**設計への含意**
- Phase 3 は**両方式を実装**し（HANDOFF §5-4 の指示どおり）、既定を「まとめる」にする。
- まとめタイルのクリックはライトボックスを 1 枚目から開き、`←`/`→` で同一投稿内を送る（design_handoff §2b と整合）。

**確実性: 中**（UI 確定仕様に基づく決定。実利用での最終評価は Phase 3 の実機確認で行う）
