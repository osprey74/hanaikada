// Rust 側の serde（camelCase）に対応する型。

export interface SessionInfo {
  did: string;
  handle: string;
  lastAuthAt: number; // Unix 秒
}

export type SyncPhase = "idle" | "diff" | "initial";

export interface SyncStatus {
  running: boolean;
  phase: SyncPhase;
  page: number;
  mediaAdded: number;
  lastRunAt: number | null;
  lastError: string | null;
  oldestIndexedAt: number | null;
  throttledUntil: number | null;
  cancelled: boolean;
}

export interface DbStats {
  media: number;
  posts: number;
  actors: number;
}

/** `ratelimit:throttled` イベントのペイロード。 */
export interface ThrottledEvent {
  seconds: number;
  until: number;
}
