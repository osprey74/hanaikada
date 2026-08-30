import { useCallback, useEffect, useState } from "react";
import { cacheUsage, clearCache, reconcileHidden } from "../../lib/api";
import type { CacheUsage } from "../../lib/types";

function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(2)} GB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${n} B`;
}

/**
 * キャッシュ管理（design_handoff §2h キャッシュ節）とモデレーション突き合わせ（DESIGN §8）。
 * 使用量バー・上限表示・手動クリア。
 */
export function CachePanel() {
  const [usage, setUsage] = useState<CacheUsage | null>(null);
  const [clearing, setClearing] = useState(false);
  const [reconciling, setReconciling] = useState(false);
  const [reconMsg, setReconMsg] = useState<string | null>(null);

  const refresh = useCallback(() => {
    void cacheUsage().then(setUsage).catch(() => setUsage(null));
  }, []);

  useEffect(refresh, [refresh]);

  async function onClear() {
    setClearing(true);
    try {
      await clearCache();
      refresh();
    } finally {
      setClearing(false);
    }
  }

  async function onReconcile() {
    setReconciling(true);
    setReconMsg(null);
    try {
      const hidden = await reconcileHidden();
      setReconMsg(`${hidden} 件の投稿を非表示にしました`);
    } catch (e) {
      setReconMsg(typeof e === "string" ? e : "突き合わせに失敗しました");
    } finally {
      setReconciling(false);
    }
  }

  const pct =
    usage && usage.limitBytes > 0
      ? Math.min(100, (usage.totalBytes / usage.limitBytes) * 100)
      : 0;

  return (
    <div className="card sync-card">
      <div className="sync-head">
        <span className="sync-title">キャッシュ</span>
        <span className="status-line" style={{ color: "var(--text-2)" }}>
          {usage
            ? `${formatBytes(usage.totalBytes)} / 上限 ${formatBytes(usage.limitBytes)}`
            : "—"}
        </span>
      </div>

      <div className="cache-bar">
        <div className="cache-bar-fill" style={{ width: `${pct}%` }} />
      </div>

      <div className="sync-stats tabnum">
        <span>サムネ {usage ? formatBytes(usage.thumbsBytes) : "—"}</span>
        <span>原寸 {usage ? formatBytes(usage.fullsizeBytes) : "—"}</span>
      </div>

      <div className="help">
        上限を超えると、最後に使った時刻の古いものから自動で削除します。削除してもメタ情報は残り、
        次に開いたときに再取得します。
      </div>

      <div className="sync-actions">
        <button className="btn" onClick={onClear} disabled={clearing}>
          {clearing ? "削除中…" : "キャッシュを削除"}
        </button>
        <button className="btn" onClick={onReconcile} disabled={reconciling}>
          {reconciling ? "突き合わせ中…" : "ミュート/ブロックを反映"}
        </button>
        {reconMsg && (
          <span className="status-line" style={{ color: "var(--text-2)" }}>
            {reconMsg}
          </span>
        )}
      </div>
    </div>
  );
}
