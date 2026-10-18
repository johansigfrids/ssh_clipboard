import { defineConfig, mergeConfig } from "vite";
import { fileURLToPath } from "node:url";
import config from "./vite.config.ts";

// Test-only aliases: normal dev/build/installed apps never load fixture dispatch.
export default mergeConfig(config, defineConfig({
  server: { host: "127.0.0.1", port: 1421 },
  resolve: { alias: {
    "@tauri-apps/api/core": fileURLToPath(new URL("./tests/browser/tauri-core.ts", import.meta.url)),
    "@tauri-apps/api/event": fileURLToPath(new URL("./tests/browser/tauri-event.ts", import.meta.url)),
  } },
}));
