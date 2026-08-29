import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso } from "react-virtuoso";
import { queryMedia, mediaCount } from "../../lib/api";
import type {
  MediaFilter,
  MediaTile as Tile,
  ModerationPrefs,
} from "../../lib/types";
import { MediaTile } from "./MediaTile";

const PAGE = 60;
/** 目標列幅 240px + gap 8px（DESIGN §7.2 / handoff 2a）。 */
const COL_TARGET = 248;
const GAP = 8;
/** 下端この距離手前で次ページを取得（handoff）。 */
const LOAD_AHEAD = 800;

interface Props {
  filter: MediaFilter;
  /** フィルタの安定キー。変化でグリッドをリセットする。 */
  filterKey: string;
  /** sync:completed のたびに増える。新着の差し込み判定に使う。 */
  syncTick: number;
  hasActiveFilters: boolean;
  selectedId: number | null;
  prefs: ModerationPrefs | null;
  revealedIds: Set<number>;
  onSelect: (tile: Tile) => void;
  onReveal: (mediaId: number) => void;
  onCountChange: (shown: number) => void;
  onClearFilters: () => void;
  onStartInitialSync: () => void;
}

export function MediaGrid({
  filter,
  filterKey,
  syncTick,
  hasActiveFilters,
  selectedId,
  prefs,
  revealedIds,
  onSelect,
  onReveal,
  onCountChange,
  onClearFilters,
  onStartInitialSync,
}: Props) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [parentEl, setParentEl] = useState<HTMLDivElement | null>(null);
  const setGridRef = useCallback((el: HTMLDivElement | null) => {
    scrollRef.current = el;
    setParentEl(el);
  }, []);
  const [cols, setCols] = useState(1);
  const [items, setItems] = useState<Tile[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);
  const [pill, setPill] = useState(0);

  // 状態を最新に保つための ref（scroll/イベントハンドラから参照）。
  const stateRef = useRef({ loading, hasMore, offset: 0, total: 0 });
  stateRef.current = { loading, hasMore, offset: items.length, total };

  // --- 列数をコンテナ幅から算出 ---
  useEffect(() => {
    if (!parentEl) return;
    const ro = new ResizeObserver(() => {
      const w = parentEl.clientWidth - 24; // padding 12px * 2
      setCols(Math.max(1, Math.floor((w + GAP) / COL_TARGET)));
    });
    ro.observe(parentEl);
    return () => ro.disconnect();
  }, [parentEl]);

  // --- 先頭から読み直す（フィルタ変更・新着反映） ---
  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const [page, cnt] = await Promise.all([
        queryMedia(filter, 0, PAGE),
        mediaCount(filter),
      ]);
      setItems(page);
      setTotal(cnt);
      setHasMore(page.length === PAGE);
      setPill(0);
      onCountChange(cnt);
    } finally {
      setLoading(false);
    }
  }, [filter, onCountChange]);

  const loadMore = useCallback(async () => {
    const s = stateRef.current;
    if (s.loading || !s.hasMore) return;
    setLoading(true);
    try {
      const page = await queryMedia(filter, s.offset, PAGE);
      setItems((prev) => [...prev, ...page]);
      setHasMore(page.length === PAGE);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  // フィルタ変更でリセット
  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filterKey]);

  // 新着（sync 完了）: 最上部なら即反映、そうでなければピル表示
  useEffect(() => {
    if (syncTick === 0) return;
    let alive = true;
    (async () => {
      const cnt = await mediaCount(filter);
      if (!alive) return;
      const nearTop = (scrollRef.current?.scrollTop ?? 0) < 300;
      if (nearTop) {
        void reload();
      } else if (cnt > stateRef.current.total) {
        setPill((p) => p + (cnt - stateRef.current.total));
        setTotal(cnt);
        onCountChange(cnt);
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [syncTick]);

  // 無限スクロール（共有スクロール親の下端検知）
  const onScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < LOAD_AHEAD) {
      void loadMore();
    }
  }, [loadMore]);

  // 列へラウンドロビン配分（i % N）。時系列の左→右読みを維持（handoff 実装方針）。
  const columns = useMemo(() => {
    const out: Tile[][] = Array.from({ length: cols }, () => []);
    items.forEach((t, i) => out[i % cols].push(t));
    return out;
  }, [items, cols]);

  const showPillClick = () => {
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
    void reload();
  };

  // --- 空状態 ---
  const empty = !loading && items.length === 0;

  return (
    <div className="grid-area" ref={setGridRef} onScroll={onScroll}>
      {pill > 0 && (
        <button className="new-pill" onClick={showPillClick}>
          <span className="material-symbols-rounded">arrow_upward</span>
          新着 {pill} 件
        </button>
      )}

      {empty && !hasActiveFilters && (
        <div className="grid-empty">
          <div className="empty-title">まだ何も溜まっていません</div>
          <div className="empty-body">
            フォロー中のアカウントが投稿・リポストした画像と動画を、これから取り込みます。
            以降は新着だけを確認します。
          </div>
          <div className="empty-notes">
            <div>
              <span className="material-symbols-rounded">download</span>
              取り込みは端末内で完結します。投稿・いいねは行いません。
            </div>
            <div>
              <span className="material-symbols-rounded">schedule</span>
              初回は少し時間がかかります。途中で中断できます。
            </div>
          </div>
          <button className="btn-accent" onClick={onStartInitialSync}>
            <span className="material-symbols-rounded">play_arrow</span>
            初回同期をはじめる
          </button>
        </div>
      )}

      {empty && hasActiveFilters && (
        <div className="grid-empty grid-empty-filtered">
          <div className="empty-title">条件に合うメディアがありません</div>
          <div className="empty-body">
            現在の絞り込み条件に一致するメディアがありません。条件を緩めてお試しください。
          </div>
          <button className="btn" onClick={onClearFilters}>
            すべての条件を解除
            <span className="key-hint">Esc</span>
          </button>
        </div>
      )}

      {!empty && parentEl && (
        <div className="grid-columns" style={{ gap: GAP }}>
          {columns.map((col, c) => (
            <div className="grid-col" key={c}>
              <Virtuoso
                useWindowScroll={false}
                customScrollParent={parentEl}
                data={col}
                computeItemKey={(_, t) => t.mediaId}
                itemContent={(_, t) => (
                  <div style={{ paddingBottom: GAP }}>
                    <MediaTile
                      tile={t}
                      selected={t.mediaId === selectedId}
                      prefs={prefs}
                      revealed={revealedIds.has(t.mediaId)}
                      onSelect={onSelect}
                      onReveal={onReveal}
                    />
                  </div>
                )}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
