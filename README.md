# Hanaikada (花筏)

**A Windows / macOS desktop app that collects only the images and videos posted or reposted by the accounts you follow on Bluesky, and lets you browse them in a dense grid.**

Bluesky's regular timeline is text-first and chronological, which is a low-density way to *look* at images. Hanaikada stores just the media posts from the accounts you follow, locally on your device, so you can browse and search them in a high-density grid.

[日本語](./README.ja.md) ・ [User manual](./docs/MANUAL.ja.md)

> **Read-only app.** It never posts, likes, reposts, or follows — no write operations at all. All fetched media is stored only on your device.

---

## Features

- **Media-only grid** — Only image/video posts, newest first, in a dense virtualized masonry. Multi-image posts collapse to one tile with a count badge; videos get a play icon; reposts get a subtle marker.
- **Local storage & offline viewing** — Persisted to SQLite; thumbnails are disk-cached. Cached media stays viewable offline.
- **Filtering** — By author (multi-select with handle search), media type (image/video), time range (today / 7d / 30d / all / custom), repost inclusion, and full-text search over ALT text and post body.
- **Lightbox** — Full-screen view, paging through multiple images in a post, in-app HLS video playback, and "open original" in your default browser.
- **Moderation** — Reflects your Bluesky labeler settings (`getPreferences`) and blurs media with warning labels by default (click to reveal). Reconciles mutes/blocks to hide previously-collected posts too.
- **Cache management** — Disk cache defaults to a 2 GB limit; oldest files are evicted automatically (LRU). The settings screen shows usage and offers a manual clear.

## Privacy & safety

- **No write APIs are implemented** (read-only).
- **Your App Password is never stored.** Only the refresh token (refreshJwt) is kept in the OS keychain (Windows Credential Manager / macOS Keychain).
- All API/CDN access is confined to the Rust backend; the frontend never talks to external hosts directly.
- Log in with a Bluesky **App Password** (create one under Settings → Privacy and Security → App Passwords). Your account password will not work.

## Tech stack

| Layer | Technology |
|---|---|
| Shell | Tauri v2 |
| Frontend | React + TypeScript + Vite |
| Backend | Rust |
| DB | SQLite (rusqlite, FTS5) |
| Secrets | OS keychain (keyring crate) |
| Virtualized list | react-virtuoso |
| Video playback | hls.js |
| Target OS | Windows / macOS |

## Install

Use the installers from the releases page.

- **Windows**: `Hanaikada_x.y.z_x64-setup.exe` (NSIS) or `Hanaikada_x.y.z_x64_en-US.msi` (MSI)
- **macOS**: `Hanaikada_x.y.z_universal.dmg`

> Current builds are unsigned, so the OS may warn on first launch. On Windows choose "More info → Run anyway"; on macOS right-click → Open.

## Getting started

1. Log in with your Bluesky **handle** and an **App Password**.
2. Click "Start initial sync" to collect media posts from the accounts you follow.
3. After that, use the "Sync" button (or the `r` key) to pull only what's new.
4. Click a tile to enlarge, filter from the left sidebar, and search ALT/body text from the top bar.

See the [user manual](./docs/MANUAL.ja.md) for details.

## Keyboard shortcuts

Shortcuts are **the same on Windows and macOS** (all single keys, no modifiers).

**Grid**

| Key | Action |
|---|---|
| `/` | Focus the search field |
| `Esc` | Clear search / clear filters / close settings |
| `r` | Sync now |

**Lightbox (while enlarged)**

| Key | Action |
|---|---|
| `←` / `→` | Previous / next media within the post |
| `o` | Open the original post in your default browser |
| `Space` | Play / pause video |
| `Esc` | Close the viewer |

## Development

```bash
npm install          # install dependencies
npm run tauri dev    # run a dev build
npm run tauri build  # release build + installers
```

- Prerequisites: Node.js, the Rust toolchain, and per-OS build tools (Windows: MSVC / Visual Studio Build Tools; macOS: Xcode Command Line Tools).
- See `DESIGN.md` for the design, `HANDOFF.md` for implementation phases and acceptance criteria, and `learnings.md` for verification notes.

## Sister app

By the same developer: the Bluesky client **Kazahana (風花)**. Where Kazahana is a single-column, text-first client, Hanaikada is its grid-first sibling built for *looking* at media.

## License

[MIT](./LICENSE)
