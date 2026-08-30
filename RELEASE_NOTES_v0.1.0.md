# 花筏 (Hanaikada) v0.1.0

**初回リリース — First release**

Bluesky のフォロー中アカウントが投稿・リポストした画像と動画だけを、ローカルに溜めて高密度なグリッドで眺める、Windows / macOS 向けデスクトップアプリです。閲覧専用で、投稿・いいね・リポスト・フォローなどの書き込み操作は一切行いません。

---

## 日本語

### 主な機能

- **メディアだけのグリッド** — 画像・動画の投稿だけを新しい順に高密度表示（仮想化 Masonry）。複数枚はまとめて 1 タイル（枚数バッジ）、動画は再生アイコン、リポストは控えめな印。
- **ローカル蓄積とオフライン閲覧** — SQLite に永続化、サムネはディスクにキャッシュ。オフラインでもキャッシュ済みメディアを閲覧できます。
- **絞り込み** — 投稿者（複数選択・ハンドル検索）／種別（画像・動画）／期間（今日・7日・30日・すべて・カスタム）／リポストの有無／ALT・本文の全文検索。
- **ライトボックス** — 全画面表示、同一投稿内の複数枚送り、動画（HLS）のアプリ内再生、元投稿を既定ブラウザで開く。
- **モデレーション** — ラベラー設定を反映し警告系ラベル付きメディアを既定でブラー（クリックで解除）。ミュート／ブロックを突き合わせてローカルの過去分も非表示化。
- **キャッシュ管理** — ディスクキャッシュ既定上限 2GB、超過時は古いものから自動削除（LRU）。設定に使用量表示と手動クリア。

### プライバシーと安全性

- 書き込み API を一切実装していません（閲覧専用）。
- App Password は保存せず、更新用トークン（refreshJwt）のみを OS のキーチェーンに保管します。
- API・CDN へのアクセスはすべて Rust 側に閉じています。
- ログインには Bluesky の **App Password**（Settings → Privacy and Security → App Passwords で発行）を使います。アカウント本体のパスワードは使えません。

### インストール

- **Windows**: `Hanaikada_0.1.0_x64-setup.exe`（NSIS）または `Hanaikada_0.1.0_x64_en-US.msi`（MSI）
- **macOS**: `Hanaikada_0.1.0_universal.dmg`

> 本リリースはコード署名を行っていません。初回起動時に OS の警告が出た場合、Windows は「詳細情報 → 実行」、macOS は「右クリック → 開く」で起動できます。

### 既知の制限

- Bluesky のタイムライン（フォロー中フィード）が返す履歴には上限があり、取り込めるのは概ね直近の数千件です。
- 定期ポーリングは未実装で、取り込みは手動（同期ボタン / `r` キー）です。
- 配布物は未署名です。

---

## English

### Highlights

- **Media-only grid** — Only image/video posts, newest first, in a dense virtualized masonry. Multi-image posts collapse to one tile with a count badge; videos get a play icon; reposts get a subtle marker.
- **Local storage & offline viewing** — Persisted to SQLite; thumbnails are disk-cached. Cached media stays viewable offline.
- **Filtering** — By author (multi-select with handle search), media type, time range (today / 7d / 30d / all / custom), repost inclusion, and full-text search over ALT text and post body.
- **Lightbox** — Full-screen view, paging through multiple images in a post, in-app HLS video playback, and "open original" in your browser.
- **Moderation** — Reflects your labeler settings and blurs labeled media by default (click to reveal). Reconciles mutes/blocks to hide previously-collected posts.
- **Cache management** — Disk cache defaults to a 2 GB limit with automatic LRU eviction; the settings screen shows usage and offers a manual clear.

### Privacy & safety

- No write APIs are implemented (read-only).
- Your App Password is never stored; only the refresh token is kept in the OS keychain.
- All API/CDN access is confined to the Rust backend.
- Log in with a Bluesky **App Password** (Settings → Privacy and Security → App Passwords). Your account password will not work.

### Install

- **Windows**: `Hanaikada_0.1.0_x64-setup.exe` (NSIS) or `Hanaikada_0.1.0_x64_en-US.msi` (MSI)
- **macOS**: `Hanaikada_0.1.0_universal.dmg`

> These builds are unsigned. On first launch, on Windows choose "More info → Run anyway"; on macOS right-click → Open.

### Known limitations

- The Bluesky following feed returns a bounded history (roughly the most recent few thousand items).
- Periodic polling is not implemented; syncing is manual (the Sync button / `r` key).
- Builds are unsigned.

---

License: [MIT](./LICENSE)
