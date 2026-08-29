# 花筏 (Hanaikada) — HANDOFF.md

DESIGN.md v0.1 に基づく実装引き継ぎ書。Claude Code での実装を想定し、フェーズ単位で受け入れ条件を定義する。

- 作成日: 2026-08-29
- 対応する設計書: `DESIGN.md` v0.1
- リポジトリ名: `hanaikada`
- バンドル ID: `io.github.osprey74.hanaikada`

---

## 0. 前提と厳守事項

- **閲覧専用アプリである。** 投稿・いいね・リポート・フォローなど、`com.atproto.repo.createRecord` 系の書き込み API は一切実装しない。認証も読み取りに必要な範囲に限る。
- **App Password の平文保存を禁止する。** キーチェーンに保存してよいのは `refreshJwt` のみ。`accessJwt` はプロセスメモリ内に限る。ログ・エラーメッセージ・パニック出力にトークンを含めないこと。
- **API 呼び出しは Rust 側に閉じる。** フロントエンドから `fetch` で bsky の API を直接叩く実装は不可。
- **CDN URL を自前で組み立てない。** API レスポンスに含まれる `thumb` / `fullsize` / `playlist` をそのまま使う。
- バンドル ID を `io.github.osprey74.*` にしているのは v1.1 の OAuth 移行を見据えたもの。AT Protocol のネイティブクライアントは、カスタム URI スキームを client_id のホスト名の逆ドメイン順に一致させる必要があり、client metadata を `osprey74.github.io` にホストする前提だとスキームは `io.github.osprey74:/callback` になる。**この識別子は変更しないこと。**

---

## 1. ディレクトリ構成

```
hanaikada/
├── src/                        # React + TS
│   ├── components/
│   │   ├── grid/               # MediaGrid, MediaTile
│   │   ├── viewer/             # LightboxViewer
│   │   ├── filter/             # FilterSidebar
│   │   └── settings/
│   ├── hooks/                  # useMediaQuery, useSyncStatus, useKeyboardNav
│   ├── lib/                    # Tauri command ラッパ、型定義
│   ├── stores/                 # UI 状態
│   └── App.tsx
├── src-tauri/
│   └── src/
│       ├── main.rs
│       ├── commands.rs         # #[tauri::command] 群
│       ├── auth/               # session, keychain
│       ├── bsky/               # client, models, ratelimit
│       ├── sync/               # poller, extractor
│       ├── media/              # downloader, cache(LRU)
│       └── db/                 # migrations, queries
└── docs/
    ├── DESIGN.md
    └── learnings.md
```

---

## 2. 実装フェーズ

各フェーズは独立して動作確認できる単位とし、受け入れ条件を満たしてから次へ進む。

### Phase 1 — 基盤と認証

**実装対象**

- Tauri v2 プロジェクトの初期化、React + TS + Vite の構成
- SQLite 接続とマイグレーション（DESIGN.md §5 のスキーマ）
- `com.atproto.server.createSession` / `refreshSession`
- `keyring` crate による `refreshJwt` の保存・読み出し
- 認証を `trait BskyAuth` として抽象化（v1.1 の OAuth 差し替え用）
- 設定画面のログインフォーム

**受け入れ条件**

- [ ] handle と App Password でログインでき、再起動後もセッションが復元される
- [ ] 401 発生時に自動で `refreshSession` が走り、透過的にリトライされる
- [ ] キーチェーンに保存されているのが `refreshJwt` のみであることを確認できる
- [ ] ログアウトでキーチェーンのエントリが削除される

### Phase 2 — 同期エンジン

**実装対象**

- `app.bsky.feed.getTimeline` クライアント（limit=100、cursor ページング）
- embed 抽出ロジック（DESIGN.md §6.1 の対応表どおり）
- `reasonRepost` の判定と `reposter_did` の記録
- 差分同期（既知 URI で打ち切り、上限 5 ページ）
- 初回同期（30 日 or 30 ページ、進捗イベント、中断可能）
- レート制限ハンドラ（429 / `Retry-After` / 指数バックオフ + jitter）
- 単一タスクでの直列実行保証

**受け入れ条件**

- [ ] 初回同期で 30 日分のメディア投稿が DB に入る
- [ ] 2 回目以降の同期が既知 URI で早期打ち切りされる（ログで確認）
- [ ] 429 を人為的に発生させたとき、バックオフして復帰する
- [ ] 4 枚組の画像投稿が `media` に 4 レコードとして正しく展開される
- [ ] `recordWithMedia`（引用＋画像）が取り込まれ、`record` 単体は除外される
- [ ] `aspect_w` / `aspect_h` が欠損なく入る（欠損時は 1:1 でフォールバック）
- [ ] 同期中に UI 操作をしても固まらない

**注意点**

- `getTimeline` にはメディア絞り込みパラメータが存在しない。100 件取得してもメディア付きは十数件になるため、グリッドを埋めるには複数ページの先読みが必要。
- 動画は `app.bsky.embed.video#view` の `playlist`（HLS）と `thumbnail` を保存する。Phase 2 では保存のみで、再生は Phase 4。

### Phase 3 — グリッド UI

**実装対象**

- `react-virtuoso` による仮想化 Masonry グリッド
- `aspectRatio` を用いたプレースホルダ（レイアウトシフト防止）
- サムネイルのメモリ＋ディスク二段キャッシュ
- ホバー時の投稿者・日時・リポスト元表示
- フィルタサイドバー（投稿者 / 種別 / 期間 / リポスト有無）
- ALT テキストの全文検索（SQLite FTS5）
- 同期ステータスバー

**受け入れ条件**

- [ ] 10,000 件のメディアでスクロールが 60fps を維持する
- [ ] 画像読み込み前後でタイル位置がずれない
- [ ] フィルタ変更が 200ms 以内に反映される
- [ ] 新着同期時、スクロール位置を保ったまま上部に追加される
- [ ] 通常利用時のメモリが 500MB を超えない

### Phase 4 — ビューアとモデレーション

**実装対象**

- 全画面ライトボックス、`fullsize` の遅延取得
- 同一投稿内の複数枚の送り
- 動画再生（HLS）
- `app.bsky.actor.getPreferences` によるラベラー設定の反映
- 警告系ラベルの既定ブラー、タイル単位での解除
- キーボード操作（DESIGN.md §7.4）
- 元投稿を既定ブラウザで開く

**受け入れ条件**

- [ ] `Esc` ですべてのオーバーレイが確実に閉じる（多重に開いた場合も含む）
- [ ] ラベル付きメディアが既定でブラーされ、クリックで解除できる
- [ ] Windows / macOS 双方で動画がアプリ内再生できる（Windows は hls.js 経由、macOS はネイティブ HLS）。想定外環境のみサムネイル＋外部ブラウザ誘導にフォールバックする
- [ ] ビューアを閉じた際、グリッドのスクロール位置とフォーカスが保持される

### Phase 5 — キャッシュ管理と仕上げ

**実装対象**

- ディスクキャッシュの LRU（既定上限 2GB）
- 設定画面での使用量表示と手動クリア
- ミュート／ブロック突き合わせによる `is_hidden` 更新（週次）
- エラー表示の整理、オフライン時の挙動
- Windows / macOS のビルド、署名、配布物の作成
- README、操作マニュアル（**ショートカットは Windows 表記と macOS 表記を併記すること**）

**受け入れ条件**

- [ ] 上限超過時に古いファイルから削除され、DB の `local_path` が NULL に戻る
- [ ] キャッシュ削除後もメタデータは残り、再取得できる
- [ ] オフライン時にキャッシュ済みメディアが閲覧できる
- [ ] 両 OS でビルドが通り、インストーラから起動できる

---

## 3. 主要な依存関係

**Rust**

| クレート | 用途 |
|---|---|
| `reqwest` | HTTP（`rustls` 推奨） |
| `serde` / `serde_json` | Lexicon レスポンスのデシリアライズ |
| `rusqlite`（bundled） | SQLite |
| `keyring` | OS キーチェーン |
| `tokio` | 非同期ランタイム |
| `thiserror` | エラー型 |
| `tracing` | ログ |

**フロントエンド**

| パッケージ | 用途 |
|---|---|
| `react-virtuoso` | 仮想化グリッド |
| `@tauri-apps/api` | コマンド／イベント |
| `date-fns` | 日時整形 |
| `hls.js` | 動画（HLS）再生。WebView2 が HLS ネイティブ非対応のため MSE 経由で再生（`learnings.md` L2）。オフライン動作のためローカル同梱 |
| Material Symbols Rounded | アイコン |

`@atproto/api` は導入しない。Rust 側で叩くため、必要な型のみ手書きで定義する。

---

## 4. Tauri コマンド一覧（案）

```rust
// 認証
login(handle: String, app_password: String) -> Result<Session>
logout() -> Result<()>
current_session() -> Result<Option<Session>>

// 同期
sync_now() -> Result<()>
start_initial_sync(days: u32) -> Result<()>
cancel_sync() -> Result<()>
sync_status() -> Result<SyncStatus>

// 参照
query_media(filter: MediaFilter, offset: u32, limit: u32) -> Result<Vec<MediaItem>>
list_actors() -> Result<Vec<Actor>>
ensure_fullsize(media_id: i64) -> Result<String>   // ローカルパスを返す

// キャッシュ / 設定
cache_usage() -> Result<CacheUsage>
clear_cache() -> Result<()>
get_settings() / set_settings(...)
```

イベント: `sync:progress`（件数・ページ数）、`sync:completed`、`sync:error`、`ratelimit:throttled`

---

## 5. 実装時に確認が必要な事項

DESIGN.md §12 の未決事項に対応する。**4 点とも一次情報で解決済み**（2026-08-29 調査、詳細は `learnings.md` L1〜L4）。以下は解決結果と、実装フェーズで残る確認作業。

1. **`createSession` のレート制限値** — **[解決]** アカウント単位 30 回 / 5 分・300 回 / 日。`refreshSession` は専用制限なし（IP 3,000 回 / 5 分のグローバル枠）。Phase 1 では **起動時に必ず `refreshSession` を優先**し、`createSession` は初回ログインと `refreshJwt` 失効時のみに限定すること。→ `learnings.md` L1
2. **WebView2 の HLS 再生可否** — **[解決]** WebView2（Windows）はネイティブ非対応、WKWebView（macOS）はネイティブ対応。**hls.js を同梱し `Hls.isSupported()` で分岐**する統一実装を採る（両 OS でアプリ内再生を主経路に）。Phase 4 着手時に Windows 実機で hls.js の動作サンプルを 1 度だけ確認する。→ `learnings.md` L2
3. **初回遡り期間** — **[解決（暫定）]** 既定 30 日を据え置き（設定で 7 / 30 / 90 日）。**Phase 2 の計測タスク**として、実アカウントで 30 日分の総ページ数・件数・所要時間・429 発生を計測し、`learnings.md` L3 に追記して確定する。→ `learnings.md` L3
4. **複数枚のまとめ表示** — **[解決]** 既定は「1 タイルにまとめる（枚数バッジ付き）」、設定でばらし表示に切替可能（design_handoff 確定仕様）。Phase 3 で両方式を実装する。→ `learnings.md` L4

---

## 6. 参考

- AT Protocol Lexicon: https://github.com/bluesky-social/atproto/tree/main/lexicons/app/bsky
- Bluesky API リファレンス: https://docs.bsky.app/docs/api/app-bsky-feed-get-timeline
- Rate Limits: https://docs.bsky.app/docs/advanced-guides/rate-limits
- OAuth 仕様（v1.1 で使用）: https://atproto.com/specs/oauth
