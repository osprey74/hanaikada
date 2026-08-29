import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 開発サーバは固定ポート 1420 を前提とする（tauri.conf.json の devUrl と一致させる）。
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri は独自の CLI 出力を使うため Vite の画面クリアを無効化する
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // src-tauri の変更は Vite 側で監視しない（Cargo が担当）
      ignored: ["**/src-tauri/**"],
    },
  },
});
