# 花筏 (Hanaikada) — DESIGN.md

Bluesky のフォロー中アカウントが投稿／リポストした画像・動画を、一覧グリッドで閲覧するデスクトップ専用クライアント。

- 文書バージョン: 0.1（draft）
- 作成日: 2026-08-29
- アプリ名: 花筏（Hanaikada）
- バンドル ID: `io.github.osprey74.hanaikada`

---

## 1. 目的とスコープ

### 1.1 目的

Bluesky の通常タイムラインは時系列のテキスト主体であり、画像を「見る」用途には密度が低い。本アプリは、フォロー中アカウントのメディア投稿だけをローカルに蓄積し、高密度なグリッドで閲覧・検索できるようにする。

### 1.2 MVP スコープ

- App Password による単一アカウントのログイン
- `app.bsky.feed.getTimeline` のポーリングによるメディア投稿の収集
- SQLite への永続化（オフライン閲覧可）
- 仮想化グリッドでの一覧表示、拡大ビューア
- 投稿者・メディア種別・期間によるフィルタ
- 元投稿（bsky.app）へのジャンプ
- アダルト系ラベルの既定ブラー
- キャッシュ容量の上限管理と手動クリア

### 1.3 MVP 対象外

| 項目 | 理由 / 予定 |
|---|---|
| OAuth 認証 | v1.1 で対応。MVP は App Password |
| モバイル（iOS / Android） | デスクトップ単体で完結させる |
| 投稿・いいね・リポストなどの書き込み操作 | 閲覧専用。書き込みは既存クライアントに委ねる |
| 複数アカウント切替 | v1.2 以降 |
| フォロワー（被フォロー）側の収集 | リクエスト数が非現実的なため見送り |
| カスタムフィード対応 | v1.2 以降 |

### 1.4 非目標

- 通常の Bluesky クライアントの代替にはしない。テキスト投稿の閲覧・スレッド表示は行わない。

---

## 2. 技術スタック

| 層 | 採用技術 |
|---|---|
| シェル | Tauri v2 |
| フロントエンド | React + TypeScript + Vite |
| バックエンド | Rust |
| DB | SQLite（`rusqlite` または `sqlx`） |
| HTTP | `reqwest` |
| 秘匿情報 | OS キーチェーン（`keyring` crate） |
| 仮想化リスト | `react-virtuoso` |
| アイコン | Material Symbols Rounded |
| 対象 OS | Windows / macOS |

---

## 3. アーキテクチャ

```
┌──────────────────────────────────────────┐
│ React UI                                 │
│  Grid / Viewer / Filter / Settings       │
└───────────┬──────────────────────────────┘
            │ Tauri command / event
┌───────────▼──────────────────────────────┐
│ Rust core                                │
│  ┌────────┐  ┌──────────┐  ┌──────────┐  │
│  │ auth   │  │ syncer   │  │ media    │  │
│  │(session│  │(poller,  │  │(download,│  │
│  │ refresh│  │ backoff) │  │ LRU cache│  │
│  └────────┘  └────┬─────┘  └────┬─────┘  │
│                   ▼             ▼        │
│              ┌─────────────────────┐     │
│              │ SQLite + file cache │     │
│              └─────────────────────┘     │
└──────────────────────────────────────────┘
```

### 3.1 責務分離の方針

- **取得とキャッシュはすべて Rust 側**に置く。UI のマウント／アンマウントとポーリングのライフサイクルを切り離す。
- React 側は SQLite への問い合わせ結果を描画するのみ。API を直接叩かない。
- 新着は Tauri のイベント（`sync:progress` / `sync:completed`）で UI に push する。

---

## 4. 認証（MVP: App Password）

### 4.1 フロー

1. 設定画面で handle と App Password を入力
2. `com.atproto.server.createSession` を呼ぶ
3. レスポンスの `did` / `accessJwt` / `refreshJwt` を取得
4. **App Password 自体は保存しない**。`refreshJwt` のみを OS キーチェーンに保存する
5. `accessJwt` はメモリ上のみで保持。期限切れ（401）で `com.atproto.server.refreshSession` により更新
6. リフレッシュにも失敗した場合は再ログインを促す

### 4.2 注意事項

- App Password は Bluesky の設定画面（Settings → Privacy and Security → App Passwords）で発行したものを使う。アカウント本体のパスワードは受け付けない旨を UI に明記する。
- `createSession` には通常の API 呼び出しとは別枠の、より厳しいレート制限が設定されている。起動のたびに新規セッションを作らず、必ず `refreshSession` を優先すること。具体的な制限値は実装時に公式ドキュメントで確認する。
- v1.1 で OAuth に移行する前提のため、認証部分は trait で抽象化しておく。

---

## 5. データモデル

```sql
CREATE TABLE actors (
  did          TEXT PRIMARY KEY,
  handle       TEXT NOT NULL,
  display_name TEXT,
  avatar_url   TEXT,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE posts (
  uri          TEXT PRIMARY KEY,          -- at://...
  cid          TEXT NOT NULL,
  author_did   TEXT NOT NULL REFERENCES actors(did),
  reposter_did TEXT REFERENCES actors(did), -- リポスト経由の場合のみ
  created_at   INTEGER NOT NULL,          -- record.createdAt
  indexed_at   INTEGER NOT NULL,          -- 並び順の基準
  text         TEXT,
  labels_json  TEXT,                      -- ラベル配列をそのまま保持
  is_hidden    INTEGER NOT NULL DEFAULT 0 -- ブロック/ミュート後の論理削除
);

CREATE TABLE media (
  id           INTEGER PRIMARY KEY,
  post_uri     TEXT NOT NULL REFERENCES posts(uri) ON DELETE CASCADE,
  idx          INTEGER NOT NULL,          -- 投稿内の順序 0-3
  kind         TEXT NOT NULL,             -- 'image' | 'video'
  thumb_url    TEXT NOT NULL,
  fullsize_url TEXT,
  playlist_url TEXT,                      -- video の HLS
  alt          TEXT,
  aspect_w     INTEGER,
  aspect_h     INTEGER,
  local_path   TEXT,                      -- fullsize のディスクキャッシュ
  bytes        INTEGER,
  last_used_at INTEGER,                   -- LRU 用
  UNIQUE(post_uri, idx)
);

CREATE TABLE sync_state (
  key         TEXT PRIMARY KEY,           -- 'timeline'
  cursor      TEXT,
  last_run_at INTEGER,
  oldest_seen INTEGER                     -- 初回遡り用
);

CREATE INDEX idx_posts_indexed_at ON posts(indexed_at DESC);
CREATE INDEX idx_posts_author     ON posts(author_did, indexed_at DESC);
CREATE INDEX idx_media_kind       ON media(kind);
```

### 5.1 設計上の要点

- `aspect_w` / `aspect_h` は必ず埋める。`app.bsky.embed.images#view` の `aspectRatio` を使い、画像読み込み前にグリッドの箱を確定させてレイアウトシフトを防ぐ。
- 画像 URL は API のレスポンスに含まれる `thumb` / `fullsize` をそのまま保存する。CDN の URL を DID と CID から自前で組み立てない（形式変更に弱いため）。
- `labels_json` は解釈せずそのまま保持し、表示時に評価する。ラベル体系の変更に追従しやすくする。

---

## 6. 同期仕様

### 6.1 取得対象の判定

`app.bsky.feed.getTimeline`（`limit=100`）を呼び、各 `feedViewPost` について次を判定する。

| `post.embed.$type` | 扱い |
|---|---|
| `app.bsky.embed.images#view` | 取り込む（最大4枚） |
| `app.bsky.embed.video#view` | 取り込む |
| `app.bsky.embed.recordWithMedia#view` | `media` 部分を取り込む |
| `app.bsky.embed.external#view` | 既定で除外（設定で OGP サムネ取り込みを ON にできる） |
| `app.bsky.embed.record#view` | 除外 |
| embed なし | 除外 |

`reason` が `app.bsky.feed.defs#reasonRepost` の場合、`reason.by.did` を `reposter_did` に記録する。

### 6.2 差分同期

- 通常同期: 先頭ページから取得し、既知の `uri` に当たった時点で打ち切る。上限 5 ページ。
- 初回同期: `cursor` を辿って過去へ遡る。既定の到達点は 30 日前、または最大 30 ページのいずれか早い方。進捗を UI に表示し、中断可能にする。
- ポーリング間隔: 既定 5 分。設定で 1〜30 分。ウィンドウ非アクティブ時は 3 倍に延長する。

### 6.3 レート制限とエラー処理

- HTTP 429 を受けたら `Retry-After` ヘッダに従う。ヘッダが無い場合は指数バックオフ（初回 5 秒、上限 5 分、jitter あり）。
- レスポンスのレート制限ヘッダを読み、残量が閾値を下回ったら自発的にポーリングを間引く。
- ネットワークエラーは 3 回までリトライ。それ以降はステータスバーに表示して次サイクルへ。
- 同期は常に単一タスクで直列実行し、多重起動を防ぐ。

---

## 7. UI 仕様

### 7.1 画面構成

- **メイン**: 左サイドバー（フィルタ）＋ 右メイングリッド。1カラムレイアウトの Kazahana とは異なり、横幅を使い切る。
- **ビューア**: グリッドのタイルをクリックで全画面オーバーレイ。同一投稿内の複数枚は左右キーで送る。
- **設定**: アカウント、同期間隔、キャッシュ、ラベル表示、外部リンクサムネの ON/OFF。

### 7.2 グリッド

- 仮想化 Masonry。列数はウィンドウ幅から算出（既定 200〜320px/列、設定でサムネサイズ変更可）。
- タイルのホバーで投稿者ハンドル、投稿日時、リポスト元を表示。
- 同一投稿の複数枚は「まとめて 1 タイル（枚数バッジ付き）」と「ばらして表示」を設定で切替。

### 7.3 フィルタ

- 投稿者（複数選択、ハンドル検索付き）
- メディア種別（画像 / 動画 / すべて）
- 期間（今日 / 7日 / 30日 / すべて / カスタム）
- リポストを含む / 含まない
- ALT テキストの全文検索（SQLite FTS5）

### 7.4 キーボード操作

| キー | 動作 |
|---|---|
| `j` / `k`、方向キー | タイル移動 |
| `Enter` | 拡大ビューアを開く |
| `Esc` | ビューアを閉じる / フィルタを解除 |
| `←` / `→` | ビューア内で前後の画像へ |
| `o` | 元投稿をブラウザで開く |
| `r` | 手動同期 |
| `/` | 検索フォーカス |

`Esc` は必ずすべてのオーバーレイを閉じられるようにする（Kazahana で同種の取りこぼしがあったため、初期実装から徹底する）。

---

## 8. モデレーション

- 起動時に `app.bsky.actor.getPreferences` を取得し、ユーザーのラベラー設定とアダルトコンテンツ設定を反映する。
- 警告系ラベル（`porn` / `sexual` / `nudity` / `graphic-media` など）が付いたメディアは既定でブラー。タイル単位でクリック解除、設定で一括解除も可能とする。
- 一覧表示は通常のタイムラインより露出面積が大きい。**ブラー処理は MVP に含める**（後付けするとグリッド実装全体に影響するため）。
- ミュート／ブロックはサーバー側で `getTimeline` に反映済みだが、ローカル DB には過去分が残る。週次で `app.bsky.graph.getMutes` / ブロック一覧と突き合わせ、該当分に `is_hidden = 1` を立てる。

---

## 9. キャッシュ管理

- サムネイルはメモリ＋ディスクの二段キャッシュ。
- `fullsize` はビューアで開いたものだけをディスクに保存する。全件先読みはしない。
- 上限は既定 2GB。超過時は `last_used_at` の古い順に削除（LRU）。
- 設定画面に現在の使用量と「キャッシュを削除」ボタンを置く。DB レコードは保持し、`local_path` のみ NULL に戻す。
- 保存先: `AppData/Roaming/Hanaikada`（Windows）、`~/Library/Application Support/Hanaikada`（macOS）。

---

## 10. 非機能要件

| 項目 | 目標 |
|---|---|
| 初回起動から一覧表示まで | 3 秒以内（DB に既存データがある場合） |
| グリッドのスクロール | 10,000 件で 60fps を維持 |
| メモリ使用量 | 通常利用時 500MB 以下 |
| ライセンス | MIT（Kazahana に合わせる） |

---

## 11. 将来拡張

- v1.1: OAuth 認証への移行
- v1.2: 複数アカウント、カスタムフィード対応
- v1.3: お気に入り／ローカルコレクション機能
- 検討: ALT テキストの自動生成（Kazahana と同じく Claude API、opt-in）

---

## 12. 未決事項

1. 初回同期の既定遡り期間（30 日で妥当か、実データで要検証）
2. 同一投稿の複数枚をまとめるか、ばらすか（どちらを既定にするか）
3. `createSession` の具体的なレート制限値の確認
4. 動画再生の実装方式（WebView の HLS 対応状況を要検証。macOS は Safari 系エンジンで再生可、Windows の WebView2 は要確認）
