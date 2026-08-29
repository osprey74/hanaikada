import { memo, useState } from "react";
import type { MediaTile as Tile } from "../../lib/types";
import { thumbSrc } from "../../lib/thumb";
import { relativeTime } from "../../lib/format";

interface Props {
  tile: Tile;
  selected: boolean;
  onSelect: (tile: Tile) => void;
}

/**
 * グリッドの 1 タイル（まとめ表示）。
 * aspect-ratio で読み込み前に箱を確定しレイアウトシフトを防ぐ（DESIGN §5.1）。
 * サムネは Rust の thumb プロトコル経由（CDN 直読みしない）。
 */
function MediaTileImpl({ tile, selected, onSelect }: Props) {
  const [loaded, setLoaded] = useState(false);
  const [broken, setBroken] = useState(false);

  const ratio =
    tile.aspectW && tile.aspectH ? `${tile.aspectW} / ${tile.aspectH}` : "1 / 1";

  return (
    <div
      className={"tile" + (selected ? " tile-selected" : "")}
      style={{ aspectRatio: ratio }}
      onClick={() => onSelect(tile)}
      role="button"
      tabIndex={-1}
    >
      {broken ? (
        <div className="tile-broken">
          <span className="material-symbols-rounded">broken_image</span>
        </div>
      ) : (
        <img
          className={"tile-img" + (loaded ? " tile-img-loaded" : "")}
          src={thumbSrc(tile.mediaId)}
          alt={tile.alt ?? ""}
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setLoaded(true)}
          onError={() => setBroken(true)}
        />
      )}

      {tile.reposterHandle && (
        <span
          className="tile-repost material-symbols-rounded"
          title={`${tile.reposterHandle} がリポスト`}
        >
          repeat
        </span>
      )}

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

      <div className="tile-hover">
        <span className="tile-handle">{tile.authorHandle}</span>
        <span className="tile-time tabnum">{relativeTime(tile.indexedAt)}</span>
      </div>
    </div>
  );
}

export const MediaTile = memo(MediaTileImpl);
