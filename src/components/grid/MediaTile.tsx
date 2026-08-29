import { memo, useState } from "react";
import type { MediaTile as Tile, ModerationPrefs } from "../../lib/types";
import { thumbSrc } from "../../lib/thumb";
import { relativeTime } from "../../lib/format";
import { decideBlur } from "../../lib/moderation";

interface Props {
  tile: Tile;
  selected: boolean;
  prefs: ModerationPrefs | null;
  revealed: boolean;
  onSelect: (tile: Tile) => void;
  onReveal: (mediaId: number) => void;
}

/**
 * グリッドの 1 タイル（まとめ表示）。
 * aspect-ratio で読み込み前に箱を確定しレイアウトシフトを防ぐ（DESIGN §5.1）。
 * サムネは Rust の thumb プロトコル経由（CDN 直読みしない）。
 * 警告ラベル付きは既定でブラーし、クリックで解除する（DESIGN §8）。
 */
function MediaTileImpl({
  tile,
  selected,
  prefs,
  revealed,
  onSelect,
  onReveal,
}: Props) {
  const [loaded, setLoaded] = useState(false);
  const [broken, setBroken] = useState(false);

  const ratio =
    tile.aspectW && tile.aspectH ? `${tile.aspectW} / ${tile.aspectH}` : "1 / 1";
  const { blurred, label } = decideBlur(tile.labelsJson, prefs);
  const covered = blurred && !revealed;

  const onClick = () => {
    if (covered) onReveal(tile.mediaId);
    else onSelect(tile);
  };

  return (
    <div
      className={"tile" + (selected ? " tile-selected" : "")}
      style={{ aspectRatio: ratio }}
      onClick={onClick}
      role="button"
      tabIndex={-1}
    >
      {broken ? (
        <div className="tile-broken">
          <span className="material-symbols-rounded">broken_image</span>
        </div>
      ) : (
        <img
          className={
            "tile-img" +
            (loaded ? " tile-img-loaded" : "") +
            (covered ? " tile-img-blurred" : "")
          }
          src={thumbSrc(tile.mediaId)}
          alt={covered ? "" : tile.alt ?? ""}
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setLoaded(true)}
          onError={() => setBroken(true)}
        />
      )}

      {covered && (
        <div className="tile-cover">
          <span className="material-symbols-rounded">visibility_off</span>
          {label && <span className="tile-cover-label">{label}</span>}
          <span className="tile-cover-hint">クリックで表示</span>
        </div>
      )}

      {blurred && !covered && (
        <button
          className="tile-rehide material-symbols-rounded"
          title="再度隠す"
          onClick={(e) => {
            e.stopPropagation();
            onReveal(tile.mediaId);
          }}
        >
          visibility_off
        </button>
      )}

      {!covered && tile.reposterHandle && (
        <span
          className="tile-repost material-symbols-rounded"
          title={`${tile.reposterHandle} がリポスト`}
        >
          repeat
        </span>
      )}

      {!covered && (
        <div className="tile-badges">
          {tile.mediaCount > 1 && (
            <span className="tile-badge tabnum">{tile.mediaCount}</span>
          )}
          {tile.hasVideo && (
            <span className="tile-badge tile-badge-icon material-symbols-rounded">
              play_arrow
            </span>
          )}
        </div>
      )}

      {!covered && (
        <div className="tile-hover">
          <span className="tile-handle">{tile.authorHandle}</span>
          <span className="tile-time tabnum">{relativeTime(tile.indexedAt)}</span>
        </div>
      )}
    </div>
  );
}

export const MediaTile = memo(MediaTileImpl);
