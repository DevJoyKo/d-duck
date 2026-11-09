import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  // `npm run dev:mock` serves the UI in a plain browser with fake data for design review; never used for builds.
  resolve:
    mode === "mock"
      ? {
          alias: {
            "@tauri-apps/api/core": "/src/mock/core.ts",
            "@tauri-apps/api/event": "/src/mock/event.ts",
          },
        }
      : undefined,
  plugins: [react(), tailwindcss()],

  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    // Hosted Intel runners are several times slower than a local Mac; give UI tests room there only.
    testTimeout: process.env.CI ? 30_000 : 5_000,
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
