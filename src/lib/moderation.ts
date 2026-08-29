// ラベル評価。labels_json（生のラベル配列）とユーザーの getPreferences 設定から
// タイルをブラーすべきか判定する（DESIGN §8）。

import type { ModerationPrefs } from "./types";

/** 既定でブラーする警告系ラベル（DESIGN §8）。ユーザー設定が優先される。 */
const DEFAULT_WARN = new Set(["porn", "sexual", "nudity", "graphic-media"]);

/** 表示名（日本語 + 原語併記、handoff 2g）。 */
const LABEL_JA: Record<string, string> = {
  porn: "成人向け",
  sexual: "性的表現",
  nudity: "露出",
  "graphic-media": "衝撃的な映像",
};

export interface LabelDecision {
  blurred: boolean;
  /** ブラー時の表示ラベル（「成人向け（porn）」形式）。 */
  label: string | null;
}

const NO_BLUR: LabelDecision = { blurred: false, label: null };

function displayLabel(val: string): string {
  return LABEL_JA[val] ? `${LABEL_JA[val]}（${val}）` : val;
}

/** labels_json とモデレーション設定から、ブラー要否と表示ラベルを決める。 */
export function decideBlur(
  labelsJson: string | null,
  prefs: ModerationPrefs | null
): LabelDecision {
  if (!labelsJson) return NO_BLUR;

  let labels: { val?: string }[];
  try {
    const parsed = JSON.parse(labelsJson);
    if (!Array.isArray(parsed)) return NO_BLUR;
    labels = parsed;
  } catch {
    return NO_BLUR;
  }

  const prefMap = new Map(
    (prefs?.labelPrefs ?? []).map((p) => [p.label, p.visibility])
  );

  for (const l of labels) {
    const val = l.val;
    if (!val) continue;

    const vis = prefMap.get(val);
    let hide: boolean;
    if (vis === "hide" || vis === "warn") hide = true;
    else if (vis === "ignore" || vis === "show") hide = false;
    else hide = DEFAULT_WARN.has(val); // 設定がなければ既定警告ラベルをブラー

    if (hide) return { blurred: true, label: displayLabel(val) };
  }
  return NO_BLUR;
}
