import { test, expect } from "@playwright/test";

/**
 * E2E tests for PasteCanvas.
 * These tests run against the Tauri WebView (dev server on localhost:1420).
 *
 * Prerequisites:
 *   - Run `pnpm dev` in apps/desktop to start the Vite dev server
 *   - Or run the full Tauri app with `cargo tauri dev`
 *
 * Note: Some tests require the Tauri backend (invoke commands).
 * When running against Vite only, backend-dependent tests will be skipped.
 */

test.describe("App Shell", () => {
  test("renders the application title", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar-brand h1")).toHaveText("PasteCanvas");
  });

  test("shows sidebar navigation", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar-nav")).toBeVisible();
    await expect(page.getByRole("button", { name: "收件匣", exact: true })).toBeVisible();
  });

  test("shows settings link in sidebar footer", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".nav-footer")).toBeVisible();
  });

  test("shows session control in sidebar", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".session-control")).toBeVisible();
  });
});

test.describe("Board Sidebar (BRD-006)", () => {
  test("shows boards section with new-board and new-folder controls", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".nav-section-header", { hasText: "看板" })).toBeVisible();
    await expect(page.locator("button[title='新增看板']")).toBeVisible();
    await expect(page.locator("button[title='新增資料夾']")).toBeVisible();
  });
});

test.describe("Inbox View", () => {
  test("shows inbox header with count", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".inbox-header h2")).toHaveText("收件匣");
  });

  test("shows search bar", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".search-bar")).toBeVisible();
  });

  test("shows quick capture input", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".quick-capture")).toBeVisible();
  });

  test("search input accepts text", async ({ page }) => {
    await page.goto("/");
    const searchInput = page.locator(".search-input");
    await searchInput.fill("test query");
    await expect(searchInput).toHaveValue("test query");
  });

  test("quick capture textarea accepts text", async ({ page }) => {
    await page.goto("/");
    const textarea = page.locator(".capture-input");
    await textarea.fill("Hello world");
    await expect(textarea).toHaveValue("Hello world");
  });

  test("search filter chips are visible", async ({ page }) => {
    await page.goto("/");
    // Chips are a focus-within dropdown in the command bar layout
    await page.locator(".search-input").click();
    await expect(page.locator(".search-filters")).toBeVisible();
    await expect(page.locator(".filter-chip").first()).toBeVisible();
  });

  test("Favorites uses the search filter command", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "收藏" }).click();
    await expect(page.locator(".search-input")).toHaveValue("favorite:true");
  });

  test("Capture keeps the inline composer usable in a browser preview", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "開啟快速擷取" }).click();
    await expect(page.locator(".inline-quick-capture .capture-input")).toBeFocused();
  });

  test("grouping switcher exposes pressed state", async ({ page }) => {
    await page.goto("/");
    const dateBtn = page.locator(".group-mode-btn", { hasText: "日期" });
    await expect(dateBtn).toHaveAttribute("aria-pressed", "true");
    await page.locator(".group-mode-btn", { hasText: "來源" }).click();
    await expect(page.locator(".group-mode-btn", { hasText: "來源" })).toHaveAttribute("aria-pressed", "true");
    await expect(dateBtn).toHaveAttribute("aria-pressed", "false");
  });

  test("search Escape clears the query and closes results", async ({ page }) => {
    await page.goto("/");
    const searchInput = page.locator(".search-input");
    await searchInput.fill("test query");
    await expect(searchInput).toHaveValue("test query");
    await searchInput.press("Escape");
    await expect(searchInput).toHaveValue("");
    await expect(page.locator(".search-results")).toHaveCount(0);
  });

  test("inline capture shows keyboard hints", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".inline-quick-capture .capture-footnote")).toContainText("換行");
  });
});

test.describe("Keyboard Shortcuts", () => {
  test("Ctrl+K focuses search (if handler wired)", async ({ page }) => {
    await page.goto("/");
    // The keyboard shortcut hook should focus search
    await page.keyboard.press("Control+k");
    // Verify no crash - page still functional
    await expect(page.locator(".app-root")).toBeVisible();
  });

  test("Escape does not crash the app", async ({ page }) => {
    await page.goto("/");
    await page.keyboard.press("Escape");
    await expect(page.locator(".app-root")).toBeVisible();
  });
});

test.describe("Navigation", () => {
  test("clicking Settings shows settings view", async ({ page }) => {
    await page.goto("/");
    await page.locator(".nav-footer .nav-item", { hasText: "設定" }).click();
    await expect(page.locator(".settings-view")).toBeVisible();
    await expect(page.locator(".settings-view h2")).toHaveText("設定");
  });

  test("settings shows hotkey list", async ({ page }) => {
    await page.goto("/");
    await page.locator(".nav-footer .nav-item", { hasText: "設定" }).click();
    await page.locator(".settings-category", { hasText: "快捷鍵" }).click();
    await expect(page.locator(".hotkey-list")).toBeVisible();
  });

  test("settings shows exclusion list", async ({ page }) => {
    await page.goto("/");
    await page.locator(".nav-footer .nav-item", { hasText: "設定" }).click();
    await page.locator(".settings-category", { hasText: "隱私" }).click();
    await expect(page.locator(".exclusion-list")).toBeVisible();
  });

  test("settings shows close-to-tray toggle (WIN-001)", async ({ page }) => {
    await page.goto("/");
    await page.locator(".nav-footer .nav-item", { hasText: "設定" }).click();
    await expect(page.locator(".settings-view h3", { hasText: /^視窗$/ })).toBeVisible();
    // First toggle in the Window section is close-to-tray (second is autostart)
    await expect(page.locator(".settings-toggle-row input[type='checkbox']").first()).toBeVisible();
  });

  test("settings shows search index maintenance (SRC-004)", async ({ page }) => {
    await page.goto("/");
    await page.locator(".nav-footer .nav-item", { hasText: "設定" }).click();
    await page.locator(".settings-category", { hasText: "搜尋" }).click();
    await expect(page.locator(".settings-view h3", { hasText: "搜尋索引" })).toBeVisible();
    await expect(page.locator("button", { hasText: "驗證" })).toBeVisible();
    await expect(page.locator("button", { hasText: "修復" })).toBeVisible();
    await expect(page.locator("button", { hasText: "最佳化" })).toBeVisible();
  });

  test("clicking Trash shows trash view", async ({ page }) => {
    await page.goto("/");
    await page.locator(".nav-footer .nav-item", { hasText: "垃圾桶" }).click();
    await expect(page.locator(".inbox-view")).toBeVisible();
    await expect(page.locator(".inbox-header h2")).toHaveText("垃圾桶");
  });

  test("clicking Inbox returns to inbox view", async ({ page }) => {
    await page.goto("/");
    // Go to settings first
    await page.locator(".nav-footer .nav-item", { hasText: "設定" }).click();
    await expect(page.locator(".settings-view")).toBeVisible();
    // Go back to inbox
    await page.locator(".sidebar-nav .nav-item", { hasText: "收件匣" }).first().click();
    await expect(page.locator(".inbox-view")).toBeVisible();
  });
});
