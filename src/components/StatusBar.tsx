import type { SyncStatus } from "../lib/types";
import { relativeTime } from "../lib/format";

interface Props {
  syncStatus: SyncStatus | null;
  throttleSeconds: number | null;
  totalMedia: number;
  shownCount: number;
}

/** フッターのステータスバー（handoff 2a）。同期状態・件数・キャッシュ使用量。 */
export function StatusBar({
  syncStatus,
  throttleSeconds,
  totalMedia,
  shownCount,
}: Props) {
  const running = syncStatus?.running ?? false;

  return (
    <footer className="statusbar tabnum">
      <span className="status-seg">
        {throttleSeconds != null ? (
          <>
            <span className="material-symbols-rounded warn-ico">schedule</span>
            レート制限中 — 約 {throttleSeconds} 秒待機
          </>
        ) : running ? (
          <>
            <span className="material-symbols-rounded warn-ico">sync</span>
            {syncStatus?.phase === "initial" ? "初回同期中" : "同期中"} —{" "}
            {syncStatus?.page ?? 0} ページ
            <span className="mini-progress">
              <span className="mini-progress-bar" />
            </span>
          </>
        ) : syncStatus?.lastRunAt ? (
          <>
            <span className="material-symbols-rounded ok-ico">check_circle</span>
            最終同期 {relativeTime(syncStatus.lastRunAt)}
          </>
        ) : (
          <>
            <span className="material-symbols-rounded">schedule</span>
            未同期
          </>
        )}
      </span>

      <span className="status-seg">
        {totalMedia.toLocaleString()} 件中 {shownCount.toLocaleString()} 件を表示
      </span>

      <span className="status-seg status-right">キャッシュ 上限 2.00 GB</span>
    </footer>
  );
}
