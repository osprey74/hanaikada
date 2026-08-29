// 日時整形。相対時刻はホバー帯・ステータスバー、絶対時刻はビューア（Phase 4）で使う。

import { formatDistanceToNowStrict } from "date-fns";
import { ja } from "date-fns/locale";

/** Unix 秒 → 「3分前」などの相対表記。 */
export function relativeTime(unixSec: number): string {
  return formatDistanceToNowStrict(new Date(unixSec * 1000), {
    addSuffix: true,
    locale: ja,
  });
}

/** Unix 秒 → 「YYYY/MM/DD HH:mm」。 */
export function absoluteTime(unixSec: number): string {
  const d = new Date(unixSec * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())} ${p(
    d.getHours()
  )}:${p(d.getMinutes())}`;
}

/** バイト数 → 「1.24 GB」。ステータスバー・設定のキャッシュ表示用。 */
export function formatGB(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}
