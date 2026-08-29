import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { MediaTile, PostMediaItem } from "../../lib/types";
import { getPostMedia } from "../../lib/api";
import { fullSrc, thumbSrc } from "../../lib/thumb";
import { absoluteTime } from "../../lib/format";
import { postUrl } from "../../lib/bsky";
import { VideoPlayer } from "./VideoPlayer";

interface Props {
  /** 開いた投稿のメタ情報と起点メディア。 */
  tile: MediaTile;
  onClose: () => void;
}

/** ライトボックスビューア（handoff 2b）。同一投稿内の複数枚を左右で送る。 */
export function LightboxViewer({ tile, onClose }: Props) {
  const [media, setMedia] = useState<PostMediaItem[]>([]);
  const [index, setIndex] = useState(0);
  const [faded, setFaded] = useState(false);
  const [imgLoaded, setImgLoaded] = useState(false);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const fadeTimer = useRef<number | null>(null);

  // 投稿内の全メディアを取得し、起点メディアの位置から開始
  useEffect(() => {
    let alive = true;
    getPostMedia(tile.postUri).then((list) => {
      if (!alive) return;
      setMedia(list);
      const i = list.findIndex((m) => m.mediaId === tile.mediaId);
      setIndex(i >= 0 ? i : 0);
    });
    return () => {
      alive = false;
    };
  }, [tile.postUri, tile.mediaId]);

  const current = media[index];
  const count = media.length;

  const go = useCallback(
    (delta: number) => {
      setIndex((i) => {
        const next = Math.min(Math.max(i + delta, 0), count - 1);
        return next;
      });
      setImgLoaded(false);
    },
    [count]
  );

  const openOriginal = useCallback(() => {
    const url = postUrl(tile.postUri);
    if (url) void openUrl(url);
  }, [tile.postUri]);

  const save = useCallback(async () => {
    if (!current) return;
    try {
      const res = await fetch(fullSrc(current.mediaId));
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `hanaikada-${current.mediaId}.jpg`;
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      /* 保存失敗は無視 */
    }
  }, [current]);

  // UI 自動フェード（マウス静止 2 秒）。画像自体はフェードしない。
  const wake = useCallback(() => {
    setFaded(false);
    if (fadeTimer.current) window.clearTimeout(fadeTimer.current);
    fadeTimer.current = window.setTimeout(() => setFaded(true), 2000);
  }, []);
  useEffect(() => {
    wake();
    return () => {
      if (fadeTimer.current) window.clearTimeout(fadeTimer.current);
    };
  }, [wake]);

  // キーボード（ビューアが開いている間は GridApp 側は無効化される）
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      wake();
      switch (e.key) {
        case "Escape":
          onClose();
          break;
        case "ArrowLeft":
          go(-1);
          break;
        case "ArrowRight":
          go(1);
          break;
        case "o":
        case "O":
          openOriginal();
          break;
        case " ": {
          const v = videoRef.current;
          if (v) {
            e.preventDefault();
            if (v.paused) void v.play();
            else v.pause();
          }
          break;
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, onClose, openOriginal, wake]);

  const chrome = faded ? " viewer-faded" : "";

  return (
    <div className="viewer" onMouseMove={wake} onClick={onClose}>
      <div className={"viewer-top" + chrome} onClick={(e) => e.stopPropagation()}>
        <span className="viewer-count tabnum">
          {count ? index + 1 : 0} / {count}
        </span>
        <div className="viewer-actions">
          <button className="viewer-btn" onClick={openOriginal}>
            <span className="material-symbols-rounded">open_in_new</span>
            元投稿を開く<span className="key-hint">O</span>
          </button>
          <button className="viewer-btn" onClick={() => void save()}>
            <span className="material-symbols-rounded">download</span>
            保存
          </button>
          <button className="viewer-btn" onClick={onClose}>
            <span className="material-symbols-rounded">close</span>
            閉じる<span className="key-hint">Esc</span>
          </button>
        </div>
      </div>

      <div className="viewer-stage" onClick={(e) => e.stopPropagation()}>
        {count > 1 && (
          <button
            className={"viewer-nav viewer-nav-left" + chrome}
            onClick={() => go(-1)}
            disabled={index === 0}
          >
            <span className="material-symbols-rounded">chevron_left</span>
          </button>
        )}

        {current && current.kind === "video" ? (
          <VideoPlayer
            ref={videoRef}
            playlistUrl={current.playlistUrl ?? ""}
            poster={thumbSrc(current.mediaId)}
          />
        ) : current ? (
          <img
            className={"viewer-img" + (imgLoaded ? " viewer-img-loaded" : "")}
            src={fullSrc(current.mediaId)}
            alt={current.alt ?? ""}
            draggable={false}
            onLoad={() => setImgLoaded(true)}
          />
        ) : null}

        {count > 1 && (
          <button
            className={"viewer-nav viewer-nav-right" + chrome}
            onClick={() => go(1)}
            disabled={index === count - 1}
          >
            <span className="material-symbols-rounded">chevron_right</span>
          </button>
        )}
      </div>

      <div className={"viewer-meta" + chrome} onClick={(e) => e.stopPropagation()}>
        <div className="viewer-author">
          {tile.authorAvatar ? (
            <img className="viewer-avatar" src={tile.authorAvatar} alt="" />
          ) : (
            <span className="viewer-avatar actor-avatar-empty" />
          )}
          <span className="viewer-name">
            {tile.authorDisplayName ?? tile.authorHandle}
          </span>
          <span className="viewer-handle">@{tile.authorHandle}</span>
          <span className="viewer-date tabnum">{absoluteTime(tile.createdAt)}</span>
        </div>
        {tile.text && <div className="viewer-text">{tile.text}</div>}
        {current?.alt && (
          <div className="viewer-alt">
            <span className="viewer-alt-label">ALT</span>
            <span className="viewer-alt-body">{current.alt}</span>
          </div>
        )}
      </div>
    </div>
  );
}
