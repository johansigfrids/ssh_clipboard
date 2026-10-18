import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/browser",
  testMatch: "*.spec.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30000,
  outputDir: "artifacts/browser-results",
  reporter: [["line"], ["html", { outputFolder: "artifacts/browser-report", open: "never" }]],
  use: {
    browserName: "chromium",
    ...(process.env.PLAYWRIGHT_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHANNEL } : {}),
    baseURL: "http://127.0.0.1:1421",
    viewport: { width: 440, height: 670 },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm run dev:browser",
    url: "http://127.0.0.1:1421",
    reuseExistingServer: false,
    timeout: 30000,
  },
});
