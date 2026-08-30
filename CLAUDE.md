# 花筏 (Hanaikada)

> Bluesky のフォロー中アカウントが投稿・リポストした画像・動画だけを、ローカルに蓄積して高密度グリッドで閲覧する **Windows / macOS 専用・閲覧専用**デスクトップクライアント。姉妹アプリは 1 カラムのテキスト主体 Bluesky クライアント **Kazahana（風花）**。

- リポジトリ名: `hanaikada`
- バンドル ID: `io.github.osprey74.hanaikada`（**変更禁止** — v1.1 の OAuth 移行で client metadata のホストと逆ドメイン一致させるため）
- 現在の版: v0.1.0（初回リリース済み）

## 技術スタック

| 層 | 採用技術 |
|---|---|
| シェル | Tauri v2 |
| フロントエンド | React 19 + TypeScript + Vite |
| バックエンド | Rust |
| DB | SQLite（rusqlite bundled、FTS5 trigram） |
| HTTP | reqwest（rustls） |
| 秘匿情報 | OS キーチェーン（keyring crate） |
| 仮想化リスト | react-virtuoso |
| 動画再生 | hls.js |
| アイコン | Material Symbols Rounded |
| 対象 OS | Windows / macOS（モバイルは非対象） |

## ディレクトリ

- `src/` — React フロントエンド（`components/` grid・viewer・filter・settings、`lib/` Tauri コマンドラッパ・型）
- `src-tauri/src/` — Rust（`auth/` セッション・keychain、`bsky/` XRPC クライアント・型、`sync/` poller・extractor、`db/` migrations・queries、`media/` キャッシュ、`moderation.rs`、`commands.rs`）
- `docs/` — `MANUAL.ja.md`、`images/`
- `design_handoff_hanaikada/` — 確定版 UI リファレンス（`.dc.html`）

## 開発コマンド

```bash
npm install            # 依存インストール
npm run tauri dev      # 開発ビルドで起動
npm run tauri build    # リリースビルド＋インストーラ生成（Win: MSI/NSIS）
cargo test --manifest-path src-tauri/Cargo.toml            # Rust テスト（28件）
# 10k 規模のクエリ性能ベンチ（通常は #[ignore]）
cargo test --manifest-path src-tauri/Cargo.toml --release bench_query_10k -- --ignored --nocapture
```

- 前提: Node.js、Rust ツールチェーン、**Windows は MSVC / Visual Studio Build Tools**、macOS は Xcode Command Line Tools。
- ビルド環境の注意は `learnings.md` L5 参照（過去に AV 誤検知で cargo/link.exe が隔離された事例あり。現在 AV はアンインストール済み）。

## 厳守事項（HANDOFF.md §0）

- **閲覧専用。** `com.atproto.repo.createRecord` 系の書き込み API（投稿・いいね・リポスト・フォロー）は一切実装しない。
- **App Password を平文保存しない。** キーチェーンに置くのは `refreshJwt` のみ。`accessJwt` はプロセスメモリ内に限る。ログ・エラー・パニック出力にトークンを含めない。
- **API 呼び出しは Rust 側に閉じる。** フロントから bsky の API を直接叩かない（サムネ等は `thumb://` / `full://` カスタムスキーム経由）。
- **CDN URL を自前で組み立てない。** レスポンスの `thumb` / `fullsize` / `playlist` をそのまま使う。

## ドキュメント

- **タスク管理**: `HANDOFF.md`（フェーズ別チェックリスト、完了は `[x]`。受け入れ条件に実機確認の根拠を併記する）
- **検証記録**: `learnings.md`（L1〜: レート制限・HLS 再生・getTimeline 履歴上限・複数枚既定・コード署名）
- **設計仕様**: `DESIGN.md`（機能追加時に更新）
- **公開ドキュメント**: `README.md`(EN) ↔ `README.ja.md`(JA) は**対で更新**、操作マニュアルは `docs/MANUAL.ja.md`

## バージョニング

- **version_files**: `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`
- **cargo_lockfile**: あり（版更新後 `cargo generate-lockfile`）
- リリースノート: `RELEASE_NOTES_v{version}.md`（1 ファイルに日本語＋英語）

## CI/CD

- GitHub Actions（`.github/workflows/release.yml`）。**タグ push（`v*.*.*`）で自動ビルド＆ドラフト Release 作成**。
- ジョブ: `test`（Windows で `cargo test`）→ `build`（Windows x86_64 + macOS universal、tauri-action）。
- 配布物は現状**未署名**（初回起動時に OS 警告。Windows「詳細情報→実行」/ macOS「右クリック→開く」）。公開配布時は信頼された CA の OV/EV コード署名を別途取得（`learnings.md` L5）。

## SNS 告知

- 想定先: **Bluesky**（本アプリ自体が Bluesky クライアントのため）。
- 告知ドラフトはワークスペース直下 `bluesky-posts-hanaikada-*.md` に保存する（g:\dev のリポジトリ外ファイル）。
- **ハンドル・メンションを類推して入れない**（誤 DID facet の原因になる）。投稿は総司様が実施。

## コーディング規約

- コミット: Conventional Commits（`feat`/`fix`/`docs`/`test`/`ci`/`assets`/`chore`）。メッセージ末尾に `Co-Authored-By` トレーラを付ける。
- 既存コードに倣う（コメントは日本語、モジュール構成・命名を踏襲）。デスクトップ専用のため `tauri icon` 生成物の android/ios は含めない。
- 言語: 総司様とのやり取りは日本語。ドキュメントは EN/JA 併記（対を同時更新）。
