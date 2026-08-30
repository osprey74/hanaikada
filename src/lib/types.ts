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

/** キャッシュ使用量（バイト）。 */
export interface CacheUsage {
  thumbsBytes: number;
  fullsizeBytes: number;
  totalBytes: number;
  limitBytes: number;
}

// --- グリッド / フィルタ（Phase 3） ---

export type MediaType = "all" | "image" | "video";

/** Rust の `MediaFilter`（serde camelCase）に対応。全条件 AND。 */
export interface MediaFilter {
  mediaType?: MediaType;
  sinceTs?: number | null;
  untilTs?: number | null;
  includeReposts?: boolean;
  actorDids?: string[];
  query?: string;
}

/** グリッドの 1 タイル（まとめ表示: 投稿単位）。Rust の `MediaTile` に対応。 */
export interface MediaTile {
  postUri: string;
  mediaId: number;
  kind: string;
  thumbUrl: string;
  aspectW: number | null;
  aspectH: number | null;
  alt: string | null;
  mediaCount: number;
  hasVideo: boolean;
  authorDid: string;
  authorHandle: string;
  authorDisplayName: string | null;
  authorAvatar: string | null;
  reposterHandle: string | null;
  indexedAt: number;
  createdAt: number;
  text: string | null;
  labelsJson: string | null;
}

/** サイドバーの投稿者リスト行。Rust の `ActorSummary` に対応。 */
export interface ActorSummary {
  did: string;
  handle: string;
  displayName: string | null;
  avatarUrl: string | null;
  count: number;
}

// --- ビューア / モデレーション（Phase 4） ---

/** 投稿内の 1 メディア。Rust の `PostMediaItem` に対応。 */
export interface PostMediaItem {
  mediaId: number;
  idx: number;
  kind: string;
  thumbUrl: string;
  fullsizeUrl: string | null;
  playlistUrl: string | null;
  alt: string | null;
  aspectW: number | null;
  aspectH: number | null;
}

export interface LabelPref {
  label: string;
  visibility: string; // "ignore" | "show" | "warn" | "hide"
}

export interface ModerationPrefs {
  adultContentEnabled: boolean;
  labelPrefs: LabelPref[];
}
