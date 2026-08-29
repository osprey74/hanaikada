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

**採用する実装パターン（実機確認で修正済み。下記「実機確認」参照）**
```ts
import Hls from "hls.js";

function attachHls(video: HTMLVideoElement, playlistUrl: string) {
  // hls.js を最優先で判定する。WebView2 は HLS をネイティブ再生できないのに
  // canPlayType が真値を返すことがあり、ネイティブ経路に入ると無音で停止するため。
  if (Hls.isSupported()) {
    const hls = new Hls({ enableWorker: false }); // ← Worker を切る（下記参照）
    hls.on(Hls.Events.MANIFEST_PARSED, () => video.play().catch(() => {}));
    hls.loadSource(playlistUrl);
    hls.attachMedia(video);
    return () => hls.destroy(); // クリーンアップ必須
  } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
    // macOS WKWebView 等（hls.js 非対応環境のみ）: ネイティブ HLS
    video.src = playlistUrl;
  } else {
    // 想定外環境のみ: サムネ + 外部ブラウザ誘導にフォールバック
  }
}
```

### 実機確認（2026-08-29・Windows WebView2）

Phase 4 で Windows 実機（WebView2）にて hls.js の動作を確認し、**2 点の落とし穴**を確定した。

1. **判定順は hls.js を先に。** WebView2 は `video.canPlayType("application/vnd.apple.mpegurl")` に真値（"maybe" 等）を返すことがあるが、実際には HLS をネイティブ再生できない。`canPlayType` を先に見るとネイティブ経路へ入り、**無音・無エラーで停止**する（`<video src=m3u8>` が何も再生しない）。→ **`Hls.isSupported()` を先に判定**する。
2. **`enableWorker: false` を推奨。** 既定（Worker 有効）だと TS→fMP4 変換が Worker 側で無音失敗する事象があった。メインスレッド変換に切り替えると安定再生。
3. 追加の堅牢化: `Hls.Events.ERROR` の fatal を数回リトライ（NETWORK/MEDIA）後に UI へ表示、`MANIFEST_PARSED` で `play()`（autoplay ブロック対策）。
4. bsky の動画 CDN（`https://video.bsky.app/...playlist.m3u8`）は `Access-Control-Allow-Origin: *`・`Range` 許可で、CORS 問題は無し。セグメントは MPEG-TS（`avc1` H.264）。

**確実性: 高**（Windows 実機で再生確認済み）。macOS(WKWebView) 側のネイティブ経路は Phase 5 のクロス OS ビルド時に実機確認する。

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

### 実測（2026-08-29・アカウント `osprey74.com`）

Phase 2 の同期エンジンで初回同期（既定 30 日設定）を実行し計測した。**空 DB からの1回実行**（過去の蓄積による汚染を排除するため DB を初期化して再計測）。

| 項目 | 実測値 |
|---|---|
| 取得ページ数 | **30 ページ（上限に到達して打ち切り。cutoff 未到達）** |
| 所要時間 | **約 10.6 秒**（`elapsed_ms=10627`） |
| 429 発生 | **0 回**（30 リクエストを約10秒で連射しても未発生。グローバル枠 3,000/5分 に対し余裕） |
| 追加メディア | **801 件**（画像 783 / 動画 18） |
| メディア投稿数 | 595 件（走査 ≈2,800 投稿中、密度 **約 21%**） |
| DB 収集メディアの日時 span | **約 4.02 時間**（07:36〜11:37 UTC） |
| 4枚組投稿 | 30 件（4レコード展開を実データで確認） |
| aspect 欠損 | 21/801（約 2.6%、UI で 1:1 フォールバック） |
| 推定メディア密度 | **約 4,783 件/日** → 30 日 ≈ **約 14 万件・約 5,400 ページ**相当 |

**判明した事実（重要）**

- フォロー先が活発な場合、**30 ページ（≈2,800 投稿）でも約 4.5 時間しか遡れない**。30 日に到達するには単純外挿で**数千ページ**が必要で、**上限 30 ページ（DESIGN §6.2）が常に先に効く**。したがって現状の初回同期は「30 日分」に**原理的に到達しない**。
- さらに、初回同期は毎回タイムライン先頭から再スキャンする実装で、`sync_state.cursor` / `oldest_seen`（`db/queries.rs` に定義済みだが未使用）を使った**遡りの継続（レジューム）が未実装**。再実行しても先頭 30 ページを見直すだけで過去へ進めない。

### 追加実測（2026-08-29・深掘り検証）: `getTimeline` の履歴上限

上限ページ数を 30 → 2,000 に引き上げ、cursor レジューム（`sync_state.cursor` を初回同期専用に保持）を実装したうえで、初回同期を最後まで走らせた結果：

| 項目 | 実測値 |
|---|---|
| 終了理由 | **タイムライン終端に到達**（170 ページ目で `cursor` が尽きた。cutoff 未到達） |
| 総ページ数 | 170 ページ（分割実行: 43 ページで中断 → 再開で残りを取得） |
| 収集メディア総数 | **4,995 件** |
| タイムライン全体の span | **約 36.5 時間**（08-27 23:06 〜 08-29 11:37 UTC） |

**決定的な知見**: **`getTimeline`（ライブなフォロー中フィード）が返す履歴は有限で、本アカウントでは約 1.5 日・約 5,000 件が上限**。それより過去は `cursor` が尽きて取得できない。したがって **「30 日遡り」は原理的に達成不能**（ページ上限ではなく `getTimeline` の仕様）。先の「30 日 ≈ 14 万件」の外挿は「30 日分の履歴が存在する」誤前提によるもので、実際にはタイムライン全体で約 5,000 件しか存在しない。

**確定した設計（2026-08-29 決定・実装済み）**

- 方針は **「cursor レジューム + 上限引き上げ」** を採用（総司様決定）。
- 初回同期は **1 回あたり上限 2,000 ページ**（安全弁）とし、`sync_state.cursor` を初回同期専用に保存して**複数回の実行・中断/再開で継続**できる。差分同期は cursor を壊さない（`touch_sync_state`）。
- 実運用では **cutoff（日数）より先にタイムライン終端が来る**ため、初回同期は事実上「利用可能なタイムライン全体をドレインする」動作になる（本アカウントで約 5,000 件・約 1 分）。
- cutoff（設定 7/30/90 日）は、履歴がそこまで残っている低頻度アカウントでのみ効く安全弁として維持する（UI 変更不要）。
- 既知の軽微な非効率: タイムライン完全ドレイン後（終端到達で cursor=NULL 保存）に再度「初回同期」を押すと先頭から再スキャンする（約 170 ページ・約 1 分、ON CONFLICT で実害なし）。通常の追随は差分同期（既知 URI で即打ち切り）に委ねるため運用上の問題は小さい。将来的に「ドレイン完了フラグ」で抑止する余地あり。

**確実性: 高**（実測データに基づく。方針決定・実装・実機確認まで完了）。

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
