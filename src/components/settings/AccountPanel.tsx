import { useState } from "react";
import { logout, validateSession } from "../../lib/api";
import type { SessionInfo } from "../../lib/types";
import { SyncPanel } from "./SyncPanel";

interface Props {
  session: SessionInfo;
  onSessionChange: (session: SessionInfo) => void;
  onLoggedOut: () => void;
}

function formatAuthTime(unixSec: number): string {
  const d = new Date(unixSec * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

type CheckState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "ok" }
  | { kind: "error"; message: string };

/**
 * Phase 1 の認証後シェル。アカウント情報の表示・セッション検証・ログアウト。
 * 「セッションを検証」は getSession を呼び、401 時の自動リフレッシュ経路を通す。
 */
export function AccountPanel({ session, onSessionChange, onLoggedOut }: Props) {
  const [check, setCheck] = useState<CheckState>({ kind: "idle" });
  const [busy, setBusy] = useState(false);

  async function onValidate() {
    setCheck({ kind: "checking" });
    try {
      const fresh = await validateSession();
      onSessionChange(fresh);
      setCheck({ kind: "ok" });
    } catch (err) {
      setCheck({
        kind: "error",
        message: typeof err === "string" ? err : "検証に失敗しました",
      });
    }
  }

  async function onLogout() {
    setBusy(true);
    try {
      await logout();
      onLoggedOut();
    } finally {
      setBusy(false);
    }
  }

  const initial = session.handle.charAt(0).toUpperCase();

  return (
    <div className="home">
      <div className="home-title">ログイン済みです</div>
      <div className="home-body">
        認証基盤と同期エンジンが動作しています。グリッド表示は後続フェーズで実装します。
      </div>

      <div className="card">
        <div className="avatar">{initial}</div>
        <div className="account-meta">
          <div className="account-handle">@{session.handle}</div>
          <div className="account-sub tabnum">
            {session.did} / 最終認証 {formatAuthTime(session.lastAuthAt)}
          </div>
        </div>
        <div className="spacer" />
        <button className="btn" onClick={onLogout} disabled={busy}>
          ログアウト
        </button>
      </div>

      <div className="submit-row">
        <button
          className="btn"
          onClick={onValidate}
          disabled={check.kind === "checking"}
        >
          {check.kind === "checking" ? "検証中…" : "セッションを検証"}
        </button>

        {check.kind === "ok" && (
          <span className="status-line ok">
            <span className="material-symbols-rounded">check_circle</span>
            有効です
          </span>
        )}
        {check.kind === "error" && (
          <span className="status-line warn">
            <span className="material-symbols-rounded">error</span>
            {check.message}
          </span>
        )}
      </div>

      <SyncPanel />
    </div>
  );
}
