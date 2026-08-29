import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { login } from "../../lib/api";
import type { SessionInfo } from "../../lib/types";

const APP_PASSWORD_HELP =
  "https://bsky.app/settings/app-passwords";

interface Props {
  onLoggedIn: (session: SessionInfo) => void;
}

/** ログイン画面（design_handoff §2f）。handle と App Password で認証する。 */
export function LoginForm({ onLoggedIn }: Props) {
  const [handle, setHandle] = useState("");
  const [appPassword, setAppPassword] = useState("");
  const [reveal, setReveal] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // handle は空でない・空白を含まない（.bsky.social の自動付与はしない: §2f）
  const handleValid = handle.trim().length > 0 && !/\s/.test(handle.trim());
  const canSubmit = handleValid && appPassword.trim().length > 0 && !submitting;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      const session = await login(handle, appPassword);
      onLoggedIn(session);
    } catch (err) {
      setError(typeof err === "string" ? err : "ログインに失敗しました");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form className="login" onSubmit={submit}>
      <div>
        <div className="login-brand">花筏</div>
        <div className="login-lead">
          Bluesky のアカウントでログインします。取得したメディアは端末内にのみ保存されます。
        </div>
      </div>

      {error && (
        <div className="error-banner">
          <span className="material-symbols-rounded">error</span>
          <span>{error}</span>
        </div>
      )}

      <div className="field">
        <label className="field-label" htmlFor="handle">
          ハンドル
        </label>
        <div className="input-wrap">
          <input
            id="handle"
            className="input"
            type="text"
            autoComplete="username"
            spellCheck={false}
            placeholder="yourname.bsky.social"
            value={handle}
            onChange={(e) => setHandle(e.target.value)}
          />
        </div>
      </div>

      <div className="field">
        <label className="field-label" htmlFor="app-password">
          App Password
        </label>
        <div className="input-wrap">
          <input
            id="app-password"
            className={`input${reveal ? "" : " masked"}`}
            type={reveal ? "text" : "password"}
            autoComplete="current-password"
            placeholder="xxxx-xxxx-xxxx-xxxx"
            value={appPassword}
            onChange={(e) => setAppPassword(e.target.value)}
          />
          <button
            type="button"
            className="input-icon-btn"
            aria-label={reveal ? "隠す" : "表示する"}
            onClick={() => setReveal((v) => !v)}
          >
            <span className="material-symbols-rounded">
              {reveal ? "visibility_off" : "visibility"}
            </span>
          </button>
        </div>
      </div>

      <div className="help">
        アカウント本体のパスワードは使えません。Bluesky の Settings → Privacy and
        Security → App Passwords で発行した App Password をご利用ください。{" "}
        <a
          className="link"
          onClick={(e) => {
            e.preventDefault();
            void openUrl(APP_PASSWORD_HELP);
          }}
        >
          発行手順を開く
        </a>
      </div>

      <div className="submit-row">
        <button className="btn-primary" type="submit" disabled={!canSubmit}>
          {submitting ? "ログイン中…" : "ログイン"}
        </button>
        <span className="submit-note">
          App Password は保存せず、更新用トークンのみを OS キーチェーンに預けます。
        </span>
      </div>
    </form>
  );
}
