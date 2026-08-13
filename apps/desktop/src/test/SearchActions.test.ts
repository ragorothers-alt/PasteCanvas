import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  openItemDetail,
  copyItemContent,
  openItemSource,
  favoriteItem,
} from "../features/search/searchActions";

// Mock clipboard plugin
const mockWriteText = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: (...args: unknown[]) => mockWriteText(...args),
}));

// Mock inbox api
const mockGetItemContent = vi.fn();
const mockOpenSource = vi.fn().mockResolvedValue(undefined);
const mockToggleFavorite = vi.fn().mockResolvedValue(true);
vi.mock("../features/inbox/api", () => ({
  getItemContent: (...args: unknown[]) => mockGetItemContent(...args),
  openSource: (...args: unknown[]) => mockOpenSource(...args),
  toggleFavorite: (...args: unknown[]) => mockToggleFavorite(...args),
}));

describe("searchActions (SRC-003)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("openItemDetail", () => {
    it("dispatches open-item event with itemId", () => {
      const dispatchSpy = vi.spyOn(window, "dispatchEvent");
      openItemDetail("item-abc");
      const event = dispatchSpy.mock.calls[0]![0] as CustomEvent;
      expect(event.type).toBe("paste-canvas://open-item");
      expect(event.detail.itemId).toBe("item-abc");
      dispatchSpy.mockRestore();
    });
  });

  describe("copyItemContent", () => {
    it("copies displayUrl for link items", async () => {
      mockGetItemContent.mockResolvedValue({
        itemId: "l1",
        itemType: "link",
        plainText: "https://example.com",
        displayUrl: "https://example.com/page",
        title: "Example",
      });

      const copied = await copyItemContent("l1");
      expect(copied).toBe("https://example.com/page");
      expect(mockWriteText).toHaveBeenCalledWith("https://example.com/page");
    });

    it("copies plainText for non-link items", async () => {
      mockGetItemContent.mockResolvedValue({
        itemId: "t1",
        itemType: "plain_text",
        plainText: "some note content",
        displayUrl: null,
        title: "Note",
      });

      const copied = await copyItemContent("t1");
      expect(copied).toBe("some note content");
      expect(mockWriteText).toHaveBeenCalledWith("some note content");
    });

    it("returns null when nothing to copy", async () => {
      mockGetItemContent.mockResolvedValue({
        itemId: "e1",
        itemType: "image",
        plainText: null,
        displayUrl: null,
        title: null,
      });

      const copied = await copyItemContent("e1");
      expect(copied).toBeNull();
      expect(mockWriteText).not.toHaveBeenCalled();
    });
  });

  describe("openItemSource", () => {
    it("opens displayUrl and returns true", async () => {
      mockGetItemContent.mockResolvedValue({
        itemId: "l1",
        itemType: "link",
        plainText: null,
        displayUrl: "https://example.com",
        title: null,
      });

      const opened = await openItemSource("l1");
      expect(opened).toBe(true);
      expect(mockOpenSource).toHaveBeenCalledWith("https://example.com");
    });

    it("returns false when no source available", async () => {
      mockGetItemContent.mockResolvedValue({
        itemId: "x1",
        itemType: "plain_text",
        plainText: null,
        displayUrl: null,
        title: null,
      });

      const opened = await openItemSource("x1");
      expect(opened).toBe(false);
      expect(mockOpenSource).not.toHaveBeenCalled();
    });
  });

  describe("favoriteItem", () => {
    it("delegates to toggleFavorite", async () => {
      const result = await favoriteItem("item-1");
      expect(result).toBe(true);
      expect(mockToggleFavorite).toHaveBeenCalledWith("item-1");
    });
  });
});
