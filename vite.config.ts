import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],

  // Unit tests cover the pure logic: formatting, parsing and the calculation
  // rules. They run in node, because none of that needs a DOM, and a jsdom
  // environment would only add a dependency and time to tests that render
  // nothing.
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },

  // Two windows, two entries: the dashboard and the menu-bar panel. Paths are
  // relative to the project root, which is where Vite resolves them from.
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        panel: "panel.html",
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
