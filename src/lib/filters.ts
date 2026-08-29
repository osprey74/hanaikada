// グリッドの絞り込み条件。UI 用の状態（期間はチップ選択で保持）と、
// バックエンドへ渡す MediaFilter（sinceTs に変換）を分ける。再起動後も復元する。

import type { MediaFilter, MediaType } from "./types";

export type Period = "today" | "7d" | "30d" | "all";

/** 永続化する UI フィルタ状態。 */
export interface UiFilters {
  mediaType: MediaType;
  period: Period;
  includeReposts: boolean;
  actorDids: string[];
  query: string;
}

export const DEFAULT_FILTERS: UiFilters = {
  mediaType: "all",
  period: "all",
  includeReposts: true,
  actorDids: [],
  query: "",
};

const STORAGE_KEY = "hanaikada.filters.v1";

export function loadFilters(): UiFilters {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_FILTERS };
    const parsed = JSON.parse(raw) as Partial<UiFilters>;
    return { ...DEFAULT_FILTERS, ...parsed };
  } catch {
    return { ...DEFAULT_FILTERS };
  }
}

export function saveFilters(f: UiFilters): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(f));
  } catch {
    /* 保存失敗は無視 */
  }
}

/** 期間チップ → sinceTs（Unix 秒）。all は下限なし。 */
function sinceForPeriod(period: Period): number | undefined {
  const now = Date.now();
  switch (period) {
    case "today": {
      const d = new Date();
      d.setHours(0, 0, 0, 0);
      return Math.floor(d.getTime() / 1000);
    }
    case "7d":
      return Math.floor((now - 7 * 86_400_000) / 1000);
    case "30d":
      return Math.floor((now - 30 * 86_400_000) / 1000);
    case "all":
      return undefined;
  }
}

/** UI フィルタ → バックエンドの MediaFilter。 */
export function toQueryFilter(ui: UiFilters): MediaFilter {
  const query = ui.query.trim();
  return {
    mediaType: ui.mediaType,
    sinceTs: sinceForPeriod(ui.period) ?? null,
    includeReposts: ui.includeReposts,
    actorDids: ui.actorDids.length ? ui.actorDids : undefined,
    query: query.length ? query : undefined,
  };
}

/** 絞り込みが 1 つでも掛かっているか（空状態文言・解除ボタンの判定）。 */
export function hasActiveFilters(ui: UiFilters): boolean {
  return (
    ui.mediaType !== "all" ||
    ui.period !== "all" ||
    !ui.includeReposts ||
    ui.actorDids.length > 0 ||
    ui.query.trim().length > 0
  );
}
