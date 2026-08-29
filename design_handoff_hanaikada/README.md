# Handoff: 花筏 (Hanaikada) — デスクトップ画像収集クライアント

## Overview

Bluesky のフォロー中アカウントが投稿／リポストした画像・動画を、ローカルにキャッシュして一覧グリッドで閲覧するデスクトップ専用クライアントの UI。本バンドルは UI-BRIEF.md §6.1〜§6.6 に対応する全画面のデザインリファレンスです。

グリッドのレイアウトは 3 案を検討し、**1a「静水」**（240px 固定サイドバー / 列幅 240px / 間隔 8px）が採用されました。以降の全画面はこの骨格に揃えてあります。

## About the Design Files

このバンドルの HTML ファイルは **HTML で作成したデザインリファレンス** です。意図した見た目と挙動を示すプロトタイプであり、そのまま製品コードとして流用する前提のものではありません。

実装タスクは、これらの HTML デザインを **対象コードベースの既存環境（Electron + React / Tauri など）の作法・ライブラリに沿って再現すること** です。まだ環境が存在しない場合は、デスクトップ専用・ローカル DB・OS キーチェーン利用という要件に適したフレームワークを選定してから実装してください。

ファイルは Design Component 形式（`.dc.html`）で、テンプレート部とロジッククラスに分かれています。マークアップは全てインラインスタイルです。CSS クラスを起こす／既存のスタイル体系に落とす作業は実装側の裁量です。

## Fidelity

**High-fidelity (hifi)** — 色・タイポグラフィ・余白・状態はすべて確定値です。ピクセル単位で再現してください。

ただし2点は例外です。

1. **画像は生成できないため、全タイル・全プレビューが無彩色のプレースホルダ**です。アスペクト比のみが意味を持ちます（実データは Bluesky の `aspectRatio` に従う）。実装では実際のサムネイルが入ります。
2. **ラベル付きメディア（2g）のブラー**は、無地面に対しては効果が見えないため、`#191A1D` のベタ塗り面で代替表現しています。実装では下記「モデレーション」の指定どおり `backdrop-filter: blur(24px)` を実画像に適用してください。

## Screens / Views

### 2a — メイングリッド（§6.1・採用案）

**Purpose**: アプリの主画面。取り込み済みメディアを一覧し、絞り込み、任意の1枚を開く。

**Layout**: 1440 × 900（最小想定）。縦 3 段の flex column。

- ヘッダー: 高さ 38px 固定、`background #17181B`、下境界 `1px solid #26282D`、左右 padding 12px
- 本体: `flex: 1`、横 flex
  - サイドバー: 幅 240px 固定、`background #17181B`、右境界 `1px solid #26282D`、上下 padding 14px、セクション間 gap 18px
  - グリッド領域: `flex: 1`、padding `10px 12px 0`
- フッター（ステータスバー）: 高さ 24px 固定、`background #17181B`、上境界 `1px solid #26282D`

**グリッドの実装方針（重要）**

デザインは masonry です。CSS `columns` を使うと縦方向に流れて視覚的な時系列順が崩れるため、**N 本の列 div に時系列を round-robin で振り分ける方式**を採ってください（`i % N`）。列は `flex: 1; min-width: 0`、列内は `display: flex; flex-direction: column; gap: 8px`、列間も gap 8px。

列数はコンテナ幅から算出します: `N = max(1, floor((w + 8) / 248))`（目標列幅 240px + gap 8px）。1440px 幅では 5 列。仮想スクロールを入れる場合も、列ごとに独立して積む構造は維持してください。

**ヘッダーの構成要素**

| 要素 | 指定 |
|---|---|
| ロゴ「花筏」 | Noto Sans JP 15px / 500 / letter-spacing 0.04em / `#E6E7EA` |
| 「HANAIKADA」 | 10px / letter-spacing 0.1em / `#6B6E76` |
| 検索フィールド | 幅 380px・高さ 24px、`background #0F1012`、`1px solid #26282D`、radius 4px、padding 0 8px、gap 6px。leading に `search` 16px `#6B6E76`、placeholder「ALT・本文を検索」12px `#6B6E76`、右端に `/` キーヒント（10px / `1px solid #26282D` / radius 3px / padding 1px 4px） |
| 同期ボタン | 高さ 24px、padding 0 9px、`1px solid #26282D`、radius 4px、12px `#9A9DA5`、`sync` アイコン + 「同期」 |
| 設定 | 24×24 のアイコンボタン、`settings` 17px `#9A9DA5` |

ロゴ側・右側ともに幅 216px を確保し、検索フィールドを中央に固定します（`justify-content: center` の flex: 1 領域）。

**サイドバーの構成要素**

上から順に。セクション見出しは全て 11px `#6B6E76` Noto Sans JP、左右 padding 14px。

1. **メディア種別** — 3 分割のセグメント。外枠 `1px solid #26282D` radius 4px overflow hidden、各セグメント `flex: 1` / text-align center / padding 5px 0 / 12px。選択中は `background #26282D` + `#E6E7EA`、非選択は `#9A9DA5`、区切りは `border-left: 1px solid #26282D`。ラベル: すべて / 画像 / 動画
2. **期間** — チップの wrap 行、gap 5px。チップは padding 4px 9px / radius 4px / 12px。非選択 `1px solid #26282D` + `#9A9DA5`、選択中 `1px solid #3A3036` + `background #241C20` + `#D08C9C`。ラベル: 今日 / 7日 / 30日 / すべて / カスタム
3. **リポストを含める** — ラベル 12px `#9A9DA5` + トグル。トグル: 30×17 / radius 999px、ON は `background #3A3036` + ノブ 13×13 `#D08C9C` 右寄せ、OFF は `background #26282D` + ノブ `#6B6E76` 左寄せ、padding 2px
4. **投稿者** — 上に `border-top: 1px solid #26282D` + padding-top 14px。見出し行の右に「3 / 218」（11px `#6B6E76` / tabular-nums）。下にハンドル絞り込みフィールド（高さ 24px、検索フィールドと同スタイル、placeholder「ハンドルで絞り込む」）。以下スクロールするリスト。

**投稿者リストの行**: padding 6px 14px / gap 9px。アバター 22×22 radius 999px（実装では実アバター画像）。ハンドル 12px、1 行省略（`overflow: hidden; text-overflow: ellipsis; white-space: nowrap`）。右端に件数 11px `#6B6E76` tabular-nums。選択中の行は `background #202226` + ハンドル `#E6E7EA`、非選択は透明 + `#9A9DA5`。

**タイル（グリッドの1枚）**

- ラッパー: `position: relative; border-radius: 6px; overflow: hidden`
- 画像面: `width: 100%; aspect-ratio: <元画像の比>`
- 選択中: `box-shadow: inset 0 0 0 2px #D08C9C`（外側 outline ではなく inset。レイアウトを動かさないため）
- **複数枚バッジ** — 右上 6px。`background rgba(15,16,18,0.72)` / `#E6E7EA` / 10px / padding 2px 5px / radius 3px / tabular-nums。枚数を数字で表示
- **動画バッジ** — 右上 6px。18×18 / `background rgba(15,16,18,0.72)` / radius 3px / `play_arrow` 14px `#E6E7EA`。複数枚バッジと同時に出る場合は gap 4px で横並び
- **リポスト印** — 左上 6px、`repeat` 15px `#9A9DA5`
- **ホバー帯** — 下端全幅。`background rgba(15,16,18,0.78)` / padding 5px 8px / gap 8px。左にハンドル 11px `#E6E7EA`（1 行省略）、右に相対時刻 11px `#9A9DA5` tabular-nums。`opacity 0 → 1`、`transition: opacity 140ms ease-out`

**ステータスバー**: 11px `#6B6E76`、gap 18px、padding 0 12px。

- 同期状態 — 完了時 `check_circle` 13px `#6FA97A` + 「最終同期 3分前」／同期中 `sync` 13px `#E08A4A` + 「同期中 — 3 / 5 ページ」+ 幅 120px・高さ 3px のプログレスバー（トラック `#26282D` / バー `#6B6E76` / radius 999px）
- 件数 — 「12,480 件中 1,204 件を表示」tabular-nums
- 右端 — 「キャッシュ 1.24 / 2.00 GB」tabular-nums

---

### 2b — ライトボックスビューア（§6.2）

**Purpose**: 1 枚を大きく見る。同一投稿内の複数枚を送る。元投稿へ飛ぶ。保存する。

**Layout**: 全画面オーバーレイ `background rgba(11,12,13,0.94)`。背後のグリッドは opacity 0.5 で見えたまま。縦 3 段。

- 上バー: 高さ 46px。左に「2 / 4」12px `#9A9DA5` tabular-nums。右にアクション 3 つ（高さ 26px / padding 0 10px / `1px solid #26282D` / radius 4px / 12px `#9A9DA5` / gap 5px）
  - `open_in_new` 「元投稿を開く」+ キーヒント `O`
  - `download` 「保存」
  - `close` 「閉じる」+ キーヒント `Esc`
- 画像領域: `flex: 1`、左右 padding 20px、gap 20px。画像は高さ 92% を上限に元比率を維持（拡大はしない）。左右に 40×40 の送りボタン（radius 4px / `background rgba(38,40,45,0.6)` / `chevron_left|right` 22px `#9A9DA5`）
- メタ領域: 最大幅 900px 中央寄せ、padding `14px 80px 20px`、gap 8px
  - 投稿者行: アバター 24×24 radius 999px / 表示名 13px `#E6E7EA` / ハンドル 12px `#6B6E76` / 右端に投稿日時 12px `#6B6E76` tabular-nums（`YYYY/MM/DD HH:mm`）
  - 本文: 13px / line-height 1.7 / `#E6E7EA`
  - ALT: 上に `border-top: 1px solid #26282D`、ラベル「ALT」11px `#6B6E76`、本文 12px / line-height 1.7 / `#9A9DA5`。ALT がない場合はこのブロックごと出しません

**UI の自動フェード**: マウス静止 2 秒で上バー・送りボタン・メタ領域を opacity 0.35 程度まで落とし、マウス移動・キー操作で即座に復帰。画像自体はフェードしません。

**キーボード**: `←`/`→` 同一投稿内の送り、`Esc` 閉じる、`O` 元投稿を開く、`Space` 動画の再生/停止。

---

### 2c — 初回起動の空グリッド（§6.3）

**Purpose**: 何も溜まっていない状態から初回同期を始めさせる。

**Layout**: 中央寄せ、最大幅 460px、gap 16px。ヘッダーの同期ボタンは `#6B6E76`（無効相当の見た目）。

- 見出し「まだ何も溜まっていません」15px / 500 / `#E6E7EA`
- 本文 13px / line-height 1.9 / `#9A9DA5` — 「フォロー中のアカウントが投稿・リポストした画像と動画を、これから 30 日分さかのぼって取り込みます。以降は 5 分ごとに新着だけを確認します。」（遡る期間・間隔は設定値を差し込む）
- 注記 2 行 12px `#6B6E76` / line-height 1.7、gap 7px、リーディングアイコン 15px
  - `download` 「取り込みは端末内で完結します。投稿・いいねは行いません。」
  - `schedule` 「初回は数分かかります。途中で中断できます。」
- 主アクション: 高さ 30px / padding 0 14px / `1px solid #3A3036` / `background #241C20` / radius 4px / 13px `#D08C9C` / `play_arrow` 16px + 「初回同期をはじめる」。右に 12px `#6B6E76` で「遡る期間は設定で変更できます」
- ステータスバー: 「未同期」「0 件」「キャッシュ 0.00 / 2.00 GB」

---

### 2d — 初回同期の進捗（§6.3）

**Purpose**: 長時間かかる初回取り込みの進行を伝える。中断・バックグラウンド継続を選ばせる。

**Layout**: 取り込み済みのタイルが背後に opacity 0.45 で積まれ、`rgba(15,16,18,0.86)` のベールの上に幅 420px のパネル。パネル: `background #17181B` / `1px solid #26282D` / radius 6px / padding 20px / gap 14px。

- 見出し「初回同期中」15px / 500
- プログレス: 高さ 3px / トラック `#26282D` / バー `#9A9DA5` / radius 999px。下に「14 / 30 ページ」（12px `#9A9DA5` tabular-nums）と「2026/08/16 まで遡りました」（`#6B6E76`）を両端寄せ
- 実績 3 行（両端寄せ / 12px / ラベル `#9A9DA5` / 値 `#E6E7EA` / tabular-nums）: 取得したメディア・投稿者・経過
- 注記: 上に `border-top: 1px solid #26282D` + padding-top 12px。`schedule` 15px `#E08A4A` + 11px `#6B6E76` 「レート制限を受けたため 12 秒待機しています。中断しても取得済みの分は残ります。」— 待機秒数はカウントダウンさせてください
- アクション 2 つ: 「中断する」「バックグラウンドで続ける」（高さ 28px / padding 0 12px / `1px solid #26282D` / radius 4px / 12px `#9A9DA5`）
- ヘッダー右に `sync` + 「同期中」12px `#E08A4A`

---

### 2e — フィルタ結果 0 件（§6.3）

**Purpose**: 絞り込みすぎた状態から復帰させる。

**Layout**: 中央寄せ、最大幅 440px、左揃え、gap 14px。ヘッダーの検索フィールドは入力あり状態（`1px solid #3A3036` / 入力語 12px `#E6E7EA` / 右端に `close` 14px `#6B6E76`）。

- 見出し「条件に合うメディアがありません」15px / 500
- 本文 13px / line-height 1.8 / `#9A9DA5` — 現在の条件を列挙し、**「条件をひとつ外すと N 件が該当します」** と続けます。この N は実際に計算して出してください（条件を 1 つずつ落として最大ヒット数を取る）。文言テンプレート: 「『動画のみ』『直近 7 日』『投稿者 3 件』『ALT に "硝子"』で絞り込んでいます。条件をひとつ外すと 42 件が該当します。」
- 解除チップ行 gap 6px / 12px。アクティブ条件は `1px solid #3A3036` + `background #241C20` + `#D08C9C` + `close` 13px。末尾に「すべての条件を解除」（`1px solid #26282D` / `#9A9DA5`）+ `Esc` キーヒント
- ステータスバー: 「12,480 件中 0 件を表示」

---

### 2f — ログイン（§6.4）

**Purpose**: handle と App Password で認証する。App Password が必要な理由を理解させる。

**Layout**: 720 × 560。上バー 32px に「HANAIKADA」11px `#6B6E76` letter-spacing 0.1em のみ。本体は左右 padding 60px、中央寄せ、gap 18px。

- ブランド: 「花筏」20px / 500 / letter-spacing 0.04em。下に 12px `#9A9DA5` / line-height 1.7 「Bluesky のアカウントでログインします。取得したメディアは端末内にのみ保存されます。」
- **エラー表示**（失敗時のみ）: padding 9px 11px / `1px solid #4A3A2A` / `background #221A12` / radius 4px / gap 8px。`error` 16px `#E08A4A` + 12px `#E6E7EA` 「認証に失敗しました。handle と App Password をご確認ください。」
- 入力フィールド: ラベル 11px `#9A9DA5`、フィールド 高さ 32px / `background #17181B` / `1px solid #26282D` / radius 4px / padding 0 10px / 13px `#E6E7EA`
- フォーカス時: `border-color #D08C9C` + `box-shadow: 0 0 0 2px rgba(208,140,156,0.22)`
- App Password フィールド: マスク表示（letter-spacing 0.2em / `#9A9DA5`）+ 右端に `visibility` 16px `#6B6E76` の表示切替
- 補助文 11px `#6B6E76` / line-height 1.7: 「アカウント本体のパスワードは使えません。Bluesky の Settings → Privacy and Security → App Passwords で発行した App Password をご利用ください。」+ リンク「発行手順を開く」（`#D08C9C`、hover で下線 / `text-underline-offset: 3px`）
- 送信: 高さ 32px / padding 0 16px / `1px solid #3A3036` / `background #241C20` / radius 4px / 13px `#D08C9C` 「ログイン」。右に 11px `#6B6E76` 「App Password は保存せず、更新用トークンのみを OS キーチェーンに預けます。」

**バリデーション**: handle は空でない・空白を含まない（`.bsky.social` の付与はしない）。App Password は `xxxx-xxxx-xxxx-xxxx` 形式を推奨として扱い、外れても送信は許可（サーバー判断に委ねる）。送信中はボタンを 40% opacity + `cursor: not-allowed`。

---

### 2g — ラベル付きメディアの混在表示（§6.6）

**Purpose**: モデレーションラベル付きのメディアを、開く意思のあるときだけ見せる。

**Layout**: グリッドは 2a と同一。上バーに「ラベル付き 18 件を含む」12px `#9A9DA5` と、右に「この画面ではブラーを解除」（`visibility` 15px / 高さ 24px / `1px solid #26282D` / radius 4px / 12px `#9A9DA5`）。

**ラベル付きタイルのカバー**: タイル全面。実装では `backdrop-filter: blur(24px)` + `background rgba(15,16,18,0.55)`。中央に縦積み gap 7px / text-align center / padding 10px。

- `visibility_off` 18px `#6B6E76`
- ラベル名 11px `#9A9DA5` / line-height 1.6 — 表示は日本語 + 原語の併記: 「成人向け（porn）」「性的表現（sexual）」「露出（nudity）」「衝撃的な映像（graphic-media）」
- 「クリックで表示」11px `#6B6E76` / `1px solid #26282D` / radius 4px / padding 2px 8px

クリックでそのタイルのみブラー解除。上バーのボタンで画面内一括解除。解除状態はセッション内のみ保持し、再起動で戻します（設定でユーザーが恒久解除を選んだ場合を除く）。

---

### 2h — 設定（§6.5）

**Layout**: 980 × 700。左に幅 200px のセクションナビ（`background #17181B` / 右境界 `1px solid #26282D` / 上下 padding 14px）、右に本文（padding 20px 24px / gap 22px）。

ナビ項目: padding 7px 14px / gap 9px / 12px、アイコン 16px。選択中は `background #202226` + `#E6E7EA`、非選択は `#9A9DA5`。項目: アカウント(`person`) / 同期(`sync`) / 表示(`grid_view`) / モデレーション(`visibility_off`) / キャッシュ(`database`)。

セクション見出しは 13px / 500 / `#E6E7EA`。設定行はラベル幅 150px（12px `#9A9DA5`）+ コントロール。

| セクション | 項目 | コントロール | 既定 |
|---|---|---|---|
| アカウント | 認証情報カード | アバター 30px / ハンドル 13px / `did:plc:… / 最終認証 YYYY/MM/DD HH:mm` 11px `#6B6E76` / 右に「ログアウト」 | — |
| 同期 | ポーリング間隔 | スライダー（トラック 3px `#26282D` / バー `#6B6E76` / ノブ 11px `#D08C9C`）+ 値 12px tabular-nums | 5 分 |
| 同期 | 初回に遡る期間 | チップ 7 日 / 30 日 / 90 日 | 30 日 |
| 同期 | 外部リンクのサムネ | トグル + 注記「OGP 画像も取り込みます（既定は取り込みません）」 | OFF |
| 表示 | テーマ | チップ ダーク / ライト / OS に合わせる | ダーク |
| 表示 | タイル密度 | チップ「ゆったり 8px」/「詰める 4px」 | ゆったり |
| 表示 | 複数枚投稿 | チップ「1 タイルにまとめる」/「ばらして並べる」 | まとめる |
| キャッシュ | 使用量 | カード内。「1.24 GB / 上限 2.00 GB」+ 高さ 4px のバー + 注記 + 「キャッシュを削除」 | 上限 2.00 GB |

カード: `background #17181B` / `1px solid #26282D` / radius 6px / padding 12px。

---

## Interactions & Behavior

### グリッド

- タイル hover → 下部帯を `opacity 0 → 1`、`140ms ease-out`。画像は動かしません（scale なし、translate なし）
- タイル click → ライトボックス（2b）。複数枚投稿は 1 枚目から開き、`←`/`→` で同一投稿内を送る
- 選択中タイルは `inset 0 0 0 2px #D08C9C`。キーボードの矢印キーでも選択を移せます
- 無限スクロール。下端 800px 手前で次ページを取得。**仮想スクロール必須**（万件規模を想定）
- 新着の差し込みはスクロール位置が最上部のときのみ即時反映。それ以外は上部に「新着 N 件」のピルを出し、クリックで反映（読んでいる位置を動かさない）

### フィルタ

- 全条件は AND。種別 × 期間 × リポスト有無 × 投稿者（複数選択は OR）× 検索語
- 検索語は投稿本文と ALT の両方を対象。ローカル DB の全文検索（SQLite FTS5 想定）
- 条件変更は即時反映。デバウンス 200ms（検索語のみ）
- 条件はアプリ再起動後も復元します

### キーボード

| キー | 動作 |
|---|---|
| `/` | 検索フィールドにフォーカス |
| `Esc` | ビューアを閉じる／検索を抜ける／全条件を解除（フォーカス位置に応じて） |
| `←` `→` `↑` `↓` | グリッド／ビューア内の移動 |
| `O` | 元投稿をブラウザで開く |
| `Space` | 動画の再生/停止 |

### アニメーション

- ホバー帯: `opacity 140ms ease-out`
- ビューアの開閉: `opacity 180ms ease-out`。画像の拡大アニメーションはしません
- パネル・オーバーレイの出現: `opacity 180ms ease-out`
- 新規タイルの差し込み: フェードインのみ、`200ms ease-out`。移動アニメーションはしません（読んでいる最中に動くため）
- バウンス・スプリング・ループアニメーションは使いません

### エラー / 例外

| 状況 | 表示 |
|---|---|
| 認証失敗 | 2f のエラーバナー |
| トークン失効 | ステータスバーを `#E08A4A` にし「再ログインが必要です」+ ログイン画面へのリンク |
| レート制限 | 2d の注記と同様。待機秒数をカウントダウン。自動再開 |
| ネットワーク断 | ステータスバー「オフライン — キャッシュのみ表示中」。グリッドはキャッシュから表示を続けます |
| 画像取得失敗 | タイルを `#191A1D` 無地にし、中央に `broken_image` 18px `#6B6E76`。再試行はビューアを開いたときに 1 回 |
| キャッシュ上限到達 | 最後に開いた順に古いものから削除。ユーザーへの通知はステータスバーのみ |

## State Management

セッション状態（永続化しない）

- `selectedTileId` — グリッドの選択
- `viewer: { postId, index } | null` — ライトボックス
- `revealedLabeledIds: Set` — ブラー解除したメディア
- `syncProgress: { page, totalPages, mediaCount, actorCount, elapsedMs, waitingUntil } | null`
- `newItemsPending: number` — 「新着 N 件」ピルの数

永続化する状態（ローカル設定）

- `filters: { mediaType, period, includeReposts, actorIds[], query }`
- `settings` — 2h の全項目
- `session` — refresh token は **OS キーチェーン**（macOS Keychain / Windows Credential Manager）。App Password は保存しません
- `lastSyncCursor` — 差分取得用

データ取得

- ローカル DB（SQLite 想定）が唯一の読み取り元。UI は API を直接叩きません
- 同期ワーカーが `app.bsky.feed.getTimeline` 相当をページングし、メディア添付のある投稿のみ抽出して DB に upsert
- ポーリング間隔は設定値（既定 5 分）。差分取得はカーソル以降のみ
- 画像本体は遅延取得 + ディスクキャッシュ。上限超過分は LRU で削除

## Design Tokens

### 色

| 用途 | 値 |
|---|---|
| 最深面（グリッド背景・ビューア台） | `#0F1012` |
| クローム面（ヘッダー・サイドバー・フッター・カード） | `#17181B` |
| 選択行の面 | `#202226` |
| 欠損タイル / カバー面 | `#191A1D` |
| 境界・区切り | `#26282D` |
| 主要テキスト | `#E6E7EA` |
| 副次テキスト | `#9A9DA5` |
| 三次テキスト・アイコン | `#6B6E76` |
| アクセント（選択・主アクション・リンク） | `#D08C9C` |
| アクセント面 | `#241C20` |
| アクセント境界 | `#3A3036` |
| 成功（同期完了） | `#6FA97A` |
| 注意（同期中・レート制限・トークン失効） | `#E08A4A` |
| 警告面 / 警告境界 | `#221A12` / `#4A3A2A` |
| オーバーレイ（バッジ・ホバー帯） | `rgba(15,16,18,0.72)` / `rgba(15,16,18,0.78)` |
| ビューア台 | `rgba(11,12,13,0.94)` |
| モーダルのベール | `rgba(15,16,18,0.86)` |
| フォーカスリング | `0 0 0 2px rgba(208,140,156,0.22)` |

### タイポグラフィ

- 日本語: **Noto Sans JP** 400 / 500 / 700
- ラテン・数値: **Inter** 400 / 500 / 600
- アイコン: **Material Symbols Rounded**（opsz 20 / wght 400 / FILL 0 / GRAD 0）

| 役割 | サイズ / 太さ / 行間 |
|---|---|
| 画面見出し・ブランド | 15px / 500（ログインのみ 20px / 500） |
| セクション見出し | 13px / 500 |
| 本文 | 13px / 400 / 1.7〜1.9 |
| UI ラベル・コントロール | 12px / 400 |
| メタ・補助・キャプション | 11px / 400 / 1.6〜1.7 |
| バッジ | 10px / 400 |

- 数値は必ず `font-variant-numeric: tabular-nums`（件数・容量・日時・ページ数）
- ロゴのトラッキング 0.04em、`HANAIKADA` 等のラテン小見出しは 0.1em
- 長文には `text-wrap: pretty`

### 余白 / 寸法

4px グリッド。ヘッダー 38px / ステータスバー 24px / サイドバー 240px / 設定ナビ 200px。グリッド列 240px・間隔 8px（密モード 4px）。コントロール高さ 24px（クローム内）/ 26〜32px（本文内）。

### 角丸

3px（小バッジ）/ 4px（コントロール・入力・チップ）/ 6px（タイル・カード・パネル）/ 999px（トグル・ピル）。

### 影

**影は使いません。** 面の分離は背景色の差（`#0F1012` / `#17181B` / `#202226`）と 1px の境界線で行います。

## Assets

このバンドルに画像アセットは含まれていません。

- **すべての画像・動画サムネイルはプレースホルダ**です（無彩色 `#2C3438`〜`#4B4340` 系の面 + アスペクト比のみ）。実装では Bluesky から取得した実データが入ります
- アイコンは **Material Symbols Rounded**（Google Fonts CDN）。ローカルにバンドルする場合は使用グリフのみのサブセットを推奨します。使用グリフ: `search` `sync` `settings` `play_arrow` `repeat` `close` `chevron_left` `chevron_right` `open_in_new` `download` `visibility` `visibility_off` `check_circle` `error` `schedule` `person` `grid_view` `database` `calendar_today` `date_range` `all_inclusive` `edit_calendar` `density_medium` `group` `movie` `broken_image`
- フォントは Google Fonts CDN 参照。デスクトップアプリではオフライン動作のためローカル同梱を推奨します
- アバターは Bluesky の `avatar` URL。本デザインではイニシャル入りの円で代替しています

## Files

| ファイル | 内容 |
|---|---|
| `hanaikada-screens.dc.html` | **主要リファレンス。** 2a〜2h の全画面（プロジェクト内の名称: 花筏 画面一覧.dc.html） |
| `hanaikada-grid-options.dc.html` | グリッドレイアウト 3 案の比較（1a 採用 / 1b 密 / 1c 日付区切り）。1b・1c は不採用ですが、密度モードと日付見出しの参考として残しています（プロジェクト内の名称: 花筏 メイングリッド.dc.html） |
| `UI-BRIEF.md` | 元のデザインブリーフ |
| `DESIGN.md` | 元の設計ドキュメント |

`.dc.html` はブラウザで直接開けます。1 ファイル内でテンプレートとロジッククラスが分かれており、マークアップはすべてインラインスタイルです。

### 実装上の注意（デザインからは読み取れない点）

1. **グリッドは CSS `columns` で組まないでください。** 上記「グリッドの実装方針」の列振り分け方式を使ってください。CSS columns は縦に流れるため時系列が読めなくなります
2. **仮想スクロール前提**で設計してください。1 万件規模のタイルを素朴に DOM に置くと破綻します
3. **タイル高さは事前に確定できます**（`aspectRatio` が API から来る）。画像ロード前に正しい高さで場所を確保し、レイアウトシフトを起こさないでください
4. 密モード（間隔 4px / 列 176px / 角丸なし）は 1b 案を参照してください。設定の「タイル密度」に対応します
5. リポスト印・複数枚バッジ・動画バッジは同時に出ます。重ならない配置（左上 / 右上の横並び）を維持してください
