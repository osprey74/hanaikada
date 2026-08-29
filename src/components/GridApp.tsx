import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ActorSummary,
  MediaTile,
  ModerationPrefs,
  SessionInfo,
  SyncStatus,
} from "../lib/types";
import {
  getModerationPrefs,
  listActors,
  listenSyncEvents,
  mediaCount,
  startInitialSync,
  syncNow,
  syncStatus as fetchSyncStatus,
} from "../lib/api";
import {
  DEFAULT_FILTERS,
  hasActiveFilters,
  loadFilters,
  saveFilters,
  toQueryFilter,
  type UiFilters,
} from "../lib/filters";
import { FilterSidebar } from "./filter/FilterSidebar";
import { MediaGrid } from "./grid/MediaGrid";
import { StatusBar } from "./StatusBar";
import { AccountPanel } from "./settings/AccountPanel";
import { LightboxViewer } from "./viewer/LightboxViewer";

interface Props {
  session: SessionInfo;
  onSessionChange: (s: SessionInfo) => void;
  onLoggedOut: () => void;
}

/** メイングリッド画面（handoff 2a）。ヘッダー + サイドバー + グリッド + ステータスバー。 */
export function GridApp({ session, onSessionChange, onLoggedOut }: Props) {
  const [ui, setUi] = useState<UiFilters>(() => loadFilters());
  const [searchText, setSearchText] = useState(ui.query);
  const [actors, setActors] = useState<ActorSummary[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [throttle, setThrottle] = useState<number | null>(null);
  const [syncTick, setSyncTick] = useState(0);
  const [totalMedia, setTotalMedia] = useState(0);
  const [shownCount, setShownCount] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [prefs, setPrefs] = useState<ModerationPrefs | null>(null);
  const [revealedIds, setRevealedIds] = useState<Set<number>>(new Set());
  const [viewer, setViewer] = useState<MediaTile | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

  const updateUi = useCallback((next: UiFilters) => {
    setUi(next);
    saveFilters(next);
  }, []);

  // 検索語は 200ms デバウンスして UI フィルタへ反映（DESIGN §7.3）
  useEffect(() => {
    const id = window.setTimeout(() => {
      updateUi({ ...ui, query: searchText });
    }, 200);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchText]);

  const queryFilter = useMemo(() => toQueryFilter(ui), [ui]);
  const filterKey = useMemo(() => JSON.stringify(queryFilter), [queryFilter]);
  const active = hasActiveFilters(ui);

  // 投稿者一覧・総件数を初期化 & 同期完了ごとに更新
  const refreshMeta = useCallback(() => {
    void listActors().then(setActors);
    void mediaCount({}).then(setTotalMedia);
  }, []);

  useEffect(() => {
    refreshMeta();
    void fetchSyncStatus().then(setSyncStatus);
    // モデレーション設定（getPreferences）。取得失敗時は既定ラベルのみで判定。
    getModerationPrefs()
      .then(setPrefs)
      .catch(() => setPrefs(null));
  }, [refreshMeta]);

  // ラベルタイルの表示/再ブラーをトグルする（セッション内のみ保持）。
  const revealTile = useCallback((mediaId: number) => {
    setRevealedIds((prev) => {
      const next = new Set(prev);
      if (next.has(mediaId)) next.delete(mediaId);
      else next.add(mediaId);
      return next;
    });
  }, []);

  // 同期イベント購読
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenSyncEvents({
      onProgress: (s) => {
        setSyncStatus(s);
        setThrottle(null);
      },
      onCompleted: (s) => {
        setSyncStatus(s);
        setThrottle(null);
        setSyncTick((t) => t + 1);
        refreshMeta();
      },
      onThrottled: (e) => setThrottle(e.seconds),
      onError: () => setThrottle(null),
    }).then((u) => (unlisten = u));
    return () => unlisten?.();
  }, [refreshMeta]);

  const clearFilters = useCallback(() => {
    setSearchText("");
    updateUi({ ...DEFAULT_FILTERS });
  }, [updateUi]);

  // キーボード: / 検索フォーカス、Esc 設定閉じ/検索解除/条件解除、r 手動同期
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // ビューアが開いている間はビューア側のキー操作に委ねる
      if (viewer) return;
      const el = e.target as HTMLElement | null;
      const typing =
        el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA");

      if (e.key === "/" && !typing) {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.key === "Escape") {
        if (settingsOpen) setSettingsOpen(false);
        else if (typing) (el as HTMLElement).blur();
        else if (active) clearFilters();
      } else if ((e.key === "r" || e.key === "R") && !typing) {
        void syncNow();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settingsOpen, active, clearFilters, viewer]);

  return (
    <div className="grid-app">
      <header className="g-header">
        <div className="g-brand">
          <span className="brand">花筏</span>
          <span className="brand-latin">HANAIKADA</span>
        </div>

        <div className="g-search-wrap">
          <div className="g-search">
            <span className="material-symbols-rounded">search</span>
            <input
              ref={searchRef}
              className="g-search-input"
              placeholder="ALT・本文を検索"
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
            />
            {searchText ? (
              <button
                className="g-search-clear material-symbols-rounded"
                onClick={() => setSearchText("")}
                title="クリア"
              >
                close
              </button>
            ) : (
              <span className="key-hint">/</span>
            )}
          </div>
        </div>

        <div className="g-actions">
          <button
            className="g-btn"
            onClick={() => void syncNow()}
            disabled={syncStatus?.running}
          >
            <span className="material-symbols-rounded">sync</span>
            同期
          </button>
          <button
            className="g-icon-btn material-symbols-rounded"
            onClick={() => setSettingsOpen(true)}
            title="設定"
          >
            settings
          </button>
        </div>
      </header>

      <div className="g-body">
        <FilterSidebar ui={ui} onChange={updateUi} actors={actors} />
        <MediaGrid
          filter={queryFilter}
          filterKey={filterKey}
          syncTick={syncTick}
          hasActiveFilters={active}
          selectedId={selectedId}
          prefs={prefs}
          revealedIds={revealedIds}
          onSelect={(t: MediaTile) => {
            setSelectedId(t.mediaId);
            setViewer(t);
          }}
          onReveal={revealTile}
          onCountChange={setShownCount}
          onClearFilters={clearFilters}
          onStartInitialSync={() => void startInitialSync()}
        />
      </div>

      <StatusBar
        syncStatus={syncStatus}
        throttleSeconds={throttle}
        totalMedia={totalMedia}
        shownCount={shownCount}
      />

      {viewer && (
        <LightboxViewer tile={viewer} onClose={() => setViewer(null)} />
      )}

      {settingsOpen && (
        <div className="overlay" onClick={() => setSettingsOpen(false)}>
          <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
            <div className="settings-head">
              <span className="settings-title">設定</span>
              <button
                className="g-icon-btn material-symbols-rounded"
                onClick={() => setSettingsOpen(false)}
                title="閉じる"
              >
                close
              </button>
            </div>
            <AccountPanel
              session={session}
              onSessionChange={onSessionChange}
              onLoggedOut={() => {
                setSettingsOpen(false);
                onLoggedOut();
              }}
            />
          </div>
        </div>
      )}
    </div>
  );
}
