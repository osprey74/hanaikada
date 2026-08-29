// Tauri コマンドの薄いラッパ。UI は API を直接叩かず、必ずここを経由する（DESIGN §3.1）。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ActorSummary,
  DbStats,
  MediaFilter,
  MediaTile,
  SessionInfo,
  SyncStatus,
  ThrottledEvent,
} from "./types";

/** handle と App Password でログインする。 */
export function login(handle: string, appPassword: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("login", { handle, appPassword });
}

/** ログアウトする（keychain / config.json をクリア）。 */
export function logout(): Promise<void> {
  return invoke<void>("logout");
}

/** 現在のセッション（メモリ内、ネットワークなし）。未ログインは null。 */
export function currentSession(): Promise<SessionInfo | null> {
  return invoke<SessionInfo | null>("current_session");
}

/** getSession でセッションを検証する（401 は自動でリフレッシュ再試行）。 */
export function validateSession(): Promise<SessionInfo> {
  return invoke<SessionInfo>("validate_session");
}

// --- 同期（Phase 2） ---

/** 差分同期を開始する。 */
export function syncNow(): Promise<void> {
  return invoke<void>("sync_now");
}

/** 初回同期を開始する（days 省略時は既定 30 日）。 */
export function startInitialSync(days?: number): Promise<void> {
  return invoke<void>("start_initial_sync", { days });
}

/** 実行中の同期を中断する。 */
export function cancelSync(): Promise<void> {
  return invoke<void>("cancel_sync");
}

/** 現在の同期状態を取得する。 */
export function syncStatus(): Promise<SyncStatus> {
  return invoke<SyncStatus>("sync_status");
}

/** DB の件数統計を取得する。 */
export function dbStats(): Promise<DbStats> {
  return invoke<DbStats>("db_stats");
}

// --- 参照（Phase 3） ---

/** 絞り込みに一致するタイルを新しい順にページング取得する。 */
export function queryMedia(
  filter: MediaFilter,
  offset: number,
  limit: number
): Promise<MediaTile[]> {
  return invoke<MediaTile[]>("query_media", { filter, offset, limit });
}

/** 絞り込みに一致するタイル総数を取得する。 */
export function mediaCount(filter: MediaFilter): Promise<number> {
  return invoke<number>("media_count", { filter });
}

/** メディア投稿を持つ投稿者の一覧（件数付き）を取得する。 */
export function listActors(): Promise<ActorSummary[]> {
  return invoke<ActorSummary[]>("list_actors");
}

/** 同期進捗・完了・エラー・レート制限イベントを購読する。戻り値で解除する。 */
export async function listenSyncEvents(handlers: {
  onProgress?: (s: SyncStatus) => void;
  onCompleted?: (s: SyncStatus) => void;
  onError?: (message: string) => void;
  onThrottled?: (e: ThrottledEvent) => void;
}): Promise<UnlistenFn> {
  const unlisteners: UnlistenFn[] = [];
  if (handlers.onProgress) {
    unlisteners.push(
      await listen<SyncStatus>("sync:progress", (e) => handlers.onProgress!(e.payload))
    );
  }
  if (handlers.onCompleted) {
    unlisteners.push(
      await listen<SyncStatus>("sync:completed", (e) => handlers.onCompleted!(e.payload))
    );
  }
  if (handlers.onError) {
    unlisteners.push(
      await listen<{ message: string }>("sync:error", (e) =>
        handlers.onError!(e.payload.message)
      )
    );
  }
  if (handlers.onThrottled) {
    unlisteners.push(
      await listen<ThrottledEvent>("ratelimit:throttled", (e) =>
        handlers.onThrottled!(e.payload)
      )
    );
  }
  return () => unlisteners.forEach((u) => u());
}
