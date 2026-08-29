// Tauri コマンドの薄いラッパ。UI は API を直接叩かず、必ずここを経由する（DESIGN §3.1）。

import { invoke } from "@tauri-apps/api/core";
import type { SessionInfo } from "./types";

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
