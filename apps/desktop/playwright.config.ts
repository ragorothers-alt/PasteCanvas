import { defineConfig } from "@playwright/test";

/**
 * Playwright E2E configuration for PasteCanvas Tauri app.
 *
 * Usage:
 *   1. Build the app: cargo tauri build --debug
 *   2. Run tests: pnpm test:e2e
 *
 * For Tauri E2E, we connect to the WebView via WebDriver.
 * The app must be built and running before tests execute.
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 30000,
  retries: 1,
  use: {
    // Tauri WebView connects on this port when using --remote-debugging-port
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  reporter: [["html", { open: "never" }], ["list"]],
  projects: [
    {
      name: "tauri-webview",
      use: {
        browserName: "chromium",
      },
    },
  ],
});
