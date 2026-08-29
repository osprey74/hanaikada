// Rust 側の SessionInfo（serde, camelCase）に対応する型。

export interface SessionInfo {
  did: string;
  handle: string;
  lastAuthAt: number; // Unix 秒
}
