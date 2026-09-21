import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The frontend source lives in ui/; build output goes to ui/dist, which
// Tauri serves (see src-tauri/tauri.conf.json -> build.frontendDist).
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
