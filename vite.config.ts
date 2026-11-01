/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port in development and loads `dist/` in production.
// `--mode demo` replaces the Tauri backend with fixture data (src/demo).
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist" },
  resolve: {
    alias:
      mode === "demo"
        ? [
            {
              find: "@tauri-apps/api/core",
              replacement: "/src/demo/backend.ts",
            },
          ]
        : [],
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["src/test/setup.ts"],
  },
}));
