import { useEffect, useState } from "react";
import { currentSession } from "./lib/api";
import type { SessionInfo } from "./lib/types";
import { LoginForm } from "./components/settings/LoginForm";
import { AccountPanel } from "./components/settings/AccountPanel";

type View =
  | { kind: "loading" }
  | { kind: "login" }
  | { kind: "home"; session: SessionInfo };

export default function App() {
  const [view, setView] = useState<View>({ kind: "loading" });

  // 起動時に復元済みセッションを問い合わせる（ネットワークなし）。
  useEffect(() => {
    let alive = true;
    currentSession()
      .then((session) => {
        if (!alive) return;
        setView(session ? { kind: "home", session } : { kind: "login" });
      })
      .catch(() => alive && setView({ kind: "login" }));
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="app">
      <header className="header">
        <span className="brand">花筏</span>
        <span className="brand-latin">HANAIKADA</span>
      </header>

      <div className="center">
        {view.kind === "loading" && (
          <span className="status-line">読み込み中…</span>
        )}

        {view.kind === "login" && (
          <LoginForm
            onLoggedIn={(session) => setView({ kind: "home", session })}
          />
        )}

        {view.kind === "home" && (
          <AccountPanel
            session={view.session}
            onSessionChange={(session) => setView({ kind: "home", session })}
            onLoggedOut={() => setView({ kind: "login" })}
          />
        )}
      </div>
    </div>
  );
}
