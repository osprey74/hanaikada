import { useEffect, useRef, useState } from "react";
import {
  cancelSync,
  dbStats,
  listenSyncEvents,
  startInitialSync,
  syncNow,
  syncStatus,
} from "../../lib/api";
import type { DbStats, SyncStatus } from "../../lib/types";

/**
 * Phase 2 検証用の同期パネル。
 * 初回同期・差分同期・中断を起動し、進捗イベントと DB 件数を表示する。
 * 本格的なステータスバー・グリッドは Phase 3 で実装する。
 */
export function SyncPanel() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [stats, setStats] = useState<DbStats | null>(null);
  const [throttle, setThrottle] = useState<{ seconds: number; until: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const statsTimer = useRef<number | null>(null);

  async function refreshStats() {
    try {
      setStats(await dbStats());
    } catch {
      /* 取得失敗は無視（次周期で再取得） */
    }
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void syncStatus().then(setStatus);
    void refreshStats();

    listenSyncEvents({
      onProgress: (s) => {
        setStatus(s);
        void refreshStats();
      },
      onCompleted: (s) => {
        setStatus(s);
        setThrottle(null);
        void refreshStats();
      },
      onError: (message) => setError(message),
      onThrottled: (e) => setThrottle(e),
    }).then((u) => (unlisten = u));

    // 実行中は件数を定期更新
    statsTimer.current = window.setInterval(refreshStats, 2000);

    return () => {
      unlisten?.();
      if (statsTimer.current) window.clearInterval(statsTimer.current);
    };
  }, []);

  const running = status?.running ?? false;

  async function onInitial() {
    setError(null);
    try {
      await startInitialSync();
    } catch (e) {
      setError(typeof e === "string" ? e : "初回同期の開始に失敗しました");
    }
  }
  async function onDiff() {
    setError(null);
    try {
      await syncNow();
    } catch (e) {
      setError(typeof e === "string" ? e : "同期の開始に失敗しました");
    }
  }

  return (
    <div className="card sync-card">
      <div className="sync-head">
        <span className="sync-title">同期（Phase 2 検証）</span>
        {running ? (
          <span className="status-line warn">
            <span className="material-symbols-rounded">sync</span>
            {status?.phase === "initial" ? "初回同期中" : "同期中"} — {status?.page ?? 0} ページ
          </span>
        ) : (
          <span className="status-line ok">
            <span className="material-symbols-rounded">check_circle</span>
            {status?.cancelled ? "中断しました" : "待機中"}
          </span>
        )}
      </div>

      <div className="sync-stats tabnum">
        <span>メディア {stats?.media ?? "—"} 件</span>
        <span>投稿 {stats?.posts ?? "—"} 件</span>
        <span>投稿者 {stats?.actors ?? "—"} 件</span>
        {running && <span>今回追加 {status?.mediaAdded ?? 0} 件</span>}
      </div>

      {throttle && (
        <div className="status-line warn">
          <span className="material-symbols-rounded">schedule</span>
          レート制限中 — 約 {throttle.seconds} 秒待機
        </div>
      )}
      {error && (
        <div className="status-line warn">
          <span className="material-symbols-rounded">error</span>
          {error}
        </div>
      )}

      <div className="sync-actions">
        <button className="btn" onClick={onInitial} disabled={running}>
          初回同期（30日）
        </button>
        <button className="btn" onClick={onDiff} disabled={running}>
          差分同期
        </button>
        <button className="btn" onClick={() => void cancelSync()} disabled={!running}>
          中断
        </button>
      </div>
    </div>
  );
}
