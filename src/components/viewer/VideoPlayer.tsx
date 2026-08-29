import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
import Hls from "hls.js";
import { openUrl } from "@tauri-apps/plugin-opener";

interface Props {
  playlistUrl: string;
  poster?: string;
}

/**
 * HLS 動画プレイヤー（learnings.md L2）。
 * macOS WKWebView はネイティブ HLS、Windows WebView2 は hls.js（MSE）で再生する。
 * playlist は CDN から直接ストリーミングする（DESIGN §9）。
 * autoplay はブラウザのポリシーでブロックされ得るため、マニフェスト解析後に明示的に
 * play() を呼ぶ。致命的エラー時はメッセージと外部ブラウザ誘導にフォールバックする。
 */
export const VideoPlayer = forwardRef<HTMLVideoElement, Props>(
  function VideoPlayer({ playlistUrl, poster }, ref) {
    const videoRef = useRef<HTMLVideoElement | null>(null);
    useImperativeHandle(ref, () => videoRef.current as HTMLVideoElement, []);
    const [err, setErr] = useState<string | null>(null);

    useEffect(() => {
      const video = videoRef.current;
      if (!video) return;
      setErr(null);

      // hls.js を最優先で判定する。WebView2 は HLS をネイティブ再生できないのに
      // canPlayType が真値を返すことがあり、ネイティブ経路に入ると無音で停止するため。
      if (Hls.isSupported()) {
        // enableWorker を切って TS→fMP4 変換をメインスレッドで行う。
        // Tauri/WebView2 で Worker 経由の変換が無音で失敗するのを避ける。
        const hls = new Hls({ enableWorker: false });
        let netRetry = 0;
        let mediaRetry = 0;
        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          video.play().catch(() => {}); // autoplay ブロック時は controls から再生
        });
        hls.on(Hls.Events.ERROR, (_evt, data) => {
          if (!data.fatal) return;
          console.error("[hls]", data.type, data.details);
          if (data.type === Hls.ErrorTypes.NETWORK_ERROR && netRetry < 2) {
            netRetry += 1;
            hls.startLoad();
          } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR && mediaRetry < 2) {
            mediaRetry += 1;
            hls.recoverMediaError();
          } else {
            setErr(data.details);
            hls.destroy();
          }
        });
        hls.loadSource(playlistUrl);
        hls.attachMedia(video);
        return () => hls.destroy();
      }

      // macOS WKWebView 等: ネイティブ HLS（hls.js 非対応環境のみ）
      if (video.canPlayType("application/vnd.apple.mpegurl")) {
        video.src = playlistUrl;
        video.play().catch(() => {});
        return;
      }

      // 最終フォールバック
      video.src = playlistUrl;
      video.play().catch(() => {});
    }, [playlistUrl]);

    if (err) {
      return (
        <div className="viewer-video-error">
          <span className="material-symbols-rounded">movie</span>
          <span>動画を再生できませんでした（{err}）</span>
          <button className="viewer-btn" onClick={() => void openUrl(playlistUrl)}>
            <span className="material-symbols-rounded">open_in_new</span>
            ブラウザで開く
          </button>
        </div>
      );
    }

    return (
      <video
        ref={videoRef}
        className="viewer-video"
        poster={poster}
        controls
        playsInline
      />
    );
  }
);
