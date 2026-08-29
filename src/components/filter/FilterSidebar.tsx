import { useMemo, useState } from "react";
import type { ActorSummary } from "../../lib/types";
import type { Period, UiFilters } from "../../lib/filters";

interface Props {
  ui: UiFilters;
  onChange: (next: UiFilters) => void;
  actors: ActorSummary[];
}

const PERIODS: { key: Period; label: string }[] = [
  { key: "today", label: "今日" },
  { key: "7d", label: "7日" },
  { key: "30d", label: "30日" },
  { key: "all", label: "すべて" },
];

/** フィルタサイドバー（handoff 2a）。検索語はヘッダー側で扱う。 */
export function FilterSidebar({ ui, onChange, actors }: Props) {
  const [actorQuery, setActorQuery] = useState("");

  const filteredActors = useMemo(() => {
    const q = actorQuery.trim().toLowerCase();
    if (!q) return actors;
    return actors.filter(
      (a) =>
        a.handle.toLowerCase().includes(q) ||
        (a.displayName ?? "").toLowerCase().includes(q)
    );
  }, [actors, actorQuery]);

  const toggleActor = (did: string) => {
    const set = new Set(ui.actorDids);
    if (set.has(did)) set.delete(did);
    else set.add(did);
    onChange({ ...ui, actorDids: [...set] });
  };

  return (
    <aside className="sidebar">
      {/* メディア種別 */}
      <section className="side-section">
        <div className="side-heading">メディア種別</div>
        <div className="segment">
          {(["all", "image", "video"] as const).map((t) => (
            <button
              key={t}
              className={"segment-item" + (ui.mediaType === t ? " on" : "")}
              onClick={() => onChange({ ...ui, mediaType: t })}
            >
              {t === "all" ? "すべて" : t === "image" ? "画像" : "動画"}
            </button>
          ))}
        </div>
      </section>

      {/* 期間 */}
      <section className="side-section">
        <div className="side-heading">期間</div>
        <div className="chips">
          {PERIODS.map((p) => (
            <button
              key={p.key}
              className={"chip" + (ui.period === p.key ? " on" : "")}
              onClick={() => onChange({ ...ui, period: p.key })}
            >
              {p.label}
            </button>
          ))}
        </div>
      </section>

      {/* リポストを含める */}
      <section className="side-section">
        <label className="toggle-row">
          <span>リポストを含める</span>
          <button
            className={"toggle" + (ui.includeReposts ? " on" : "")}
            role="switch"
            aria-checked={ui.includeReposts}
            onClick={() =>
              onChange({ ...ui, includeReposts: !ui.includeReposts })
            }
          >
            <span className="toggle-knob" />
          </button>
        </label>
      </section>

      {/* 投稿者 */}
      <section className="side-section side-actors">
        <div className="side-heading side-heading-row">
          <span>投稿者</span>
          <span className="tabnum side-count">
            {ui.actorDids.length} / {actors.length}
          </span>
        </div>
        <input
          className="side-actor-filter"
          placeholder="ハンドルで絞り込む"
          value={actorQuery}
          onChange={(e) => setActorQuery(e.target.value)}
        />
        <div className="actor-list">
          {filteredActors.map((a) => {
            const on = ui.actorDids.includes(a.did);
            return (
              <button
                key={a.did}
                className={"actor-row" + (on ? " on" : "")}
                onClick={() => toggleActor(a.did)}
                title={a.handle}
              >
                {a.avatarUrl ? (
                  <img className="actor-avatar" src={a.avatarUrl} alt="" />
                ) : (
                  <span className="actor-avatar actor-avatar-empty" />
                )}
                <span className="actor-handle">{a.handle}</span>
                <span className="actor-count tabnum">{a.count}</span>
              </button>
            );
          })}
          {filteredActors.length === 0 && (
            <div className="actor-empty">該当する投稿者がいません</div>
          )}
        </div>
      </section>
    </aside>
  );
}
