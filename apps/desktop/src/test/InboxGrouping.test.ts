import { describe, it, expect } from "vitest";
import { groupItems } from "../features/inbox/useInbox";
import type { ItemSummary } from "../features/inbox/api";

function makeItem(overrides: Partial<ItemSummary>): ItemSummary {
  return {
    id: "item-1",
    itemType: "plain_text",
    title: "Item",
    textPreview: "text",
    displayUrl: null,
    inboxStatus: "new",
    isFavorite: false,
    isPinned: false,
    captureMethod: "manual_hotkey",
    sourceApp: null,
    domain: null,
    captureSessionId: null,
    createdAt: Date.now(),
    revision: 1,
    assetByteSize: null,
    ...overrides,
  };
}

describe("groupItems (INB-002 smart views)", () => {
  describe("date mode", () => {
    it("groups into Today / Yesterday / Last 7 days / Earlier", () => {
      // Anchor to local midnight so bucket membership is deterministic
      const now = new Date();
      const midnight = new Date(
        now.getFullYear(),
        now.getMonth(),
        now.getDate(),
      ).getTime();
      const day = 86400000;
      const items = [
        makeItem({ id: "a", createdAt: midnight + 1000 }), // today
        makeItem({ id: "b", createdAt: midnight - 0.5 * day }), // yesterday noon
        makeItem({ id: "c", createdAt: midnight - 3 * day }), // last 7 days
        makeItem({ id: "d", createdAt: midnight - 30 * day }), // earlier
      ];
      const groups = groupItems(items, "date");
      const labels = groups.map((g) => g.label);
      expect(labels).toContain("Today");
      expect(labels).toContain("Yesterday");
      expect(labels).toContain("Last 7 days");
      expect(labels).toContain("Earlier");
    });

    it("omits empty date buckets", () => {
      const items = [makeItem({ createdAt: Date.now() })];
      const groups = groupItems(items, "date");
      expect(groups).toHaveLength(1);
      expect(groups[0]!.label).toBe("Today");
    });
  });

  describe("source mode", () => {
    it("groups by source app", () => {
      const items = [
        makeItem({ id: "a", sourceApp: "vscode.exe" }),
        makeItem({ id: "b", sourceApp: "vscode.exe" }),
        makeItem({ id: "c", sourceApp: "chrome.exe" }),
      ];
      const groups = groupItems(items, "source");
      // Sorted by count desc: vscode first
      expect(groups[0]!.label).toBe("vscode.exe");
      expect(groups[0]!.items).toHaveLength(2);
      expect(groups[1]!.label).toBe("chrome.exe");
    });

    it("uses fallback for missing source", () => {
      const items = [makeItem({ sourceApp: null })];
      const groups = groupItems(items, "source");
      expect(groups[0]!.label).toBe("(no source)");
    });
  });

  describe("domain mode", () => {
    it("groups by website domain", () => {
      const items = [
        makeItem({ id: "a", domain: "github.com" }),
        makeItem({ id: "b", domain: "github.com" }),
        makeItem({ id: "c", domain: null }),
      ];
      const groups = groupItems(items, "domain");
      expect(groups[0]!.label).toBe("github.com");
      expect(groups[0]!.items).toHaveLength(2);
      expect(groups.some((g) => g.label === "(no domain)")).toBe(true);
    });
  });

  describe("type mode", () => {
    it("groups by item type", () => {
      const items = [
        makeItem({ id: "a", itemType: "link" }),
        makeItem({ id: "b", itemType: "link" }),
        makeItem({ id: "c", itemType: "image" }),
      ];
      const groups = groupItems(items, "type");
      expect(groups[0]!.label).toBe("link");
      expect(groups[0]!.items).toHaveLength(2);
      expect(groups[1]!.label).toBe("image");
    });
  });

  describe("session mode", () => {
    it("groups by capture session", () => {
      const items = [
        makeItem({ id: "a", captureSessionId: "sess-1" }),
        makeItem({ id: "b", captureSessionId: "sess-1" }),
        makeItem({ id: "c", captureSessionId: null }),
      ];
      const groups = groupItems(items, "session");
      expect(groups[0]!.label).toBe("sess-1");
      expect(groups[0]!.items).toHaveLength(2);
      expect(groups.some((g) => g.label === "(no session)")).toBe(true);
    });
  });

  it("returns empty array for empty input", () => {
    expect(groupItems([], "date")).toEqual([]);
    expect(groupItems([], "source")).toEqual([]);
  });
});
