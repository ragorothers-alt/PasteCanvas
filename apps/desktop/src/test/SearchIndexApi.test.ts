import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  verifySearchIndex,
  rebuildSearchIndex,
  optimizeSearchIndex,
} from "../features/search/api";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe("search index maintenance api (SRC-004)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("verifySearchIndex invokes verify_search_index", async () => {
    const health = {
      liveItems: 10,
      indexedDocs: 10,
      missingDocs: 0,
      orphanDocs: 0,
      ftsUnicodeOk: true,
      ftsTrigramOk: true,
      healthy: true,
    };
    mockInvoke.mockResolvedValue(health);

    const result = await verifySearchIndex();
    expect(mockInvoke).toHaveBeenCalledWith("verify_search_index");
    expect(result).toEqual(health);
    expect(result.healthy).toBe(true);
  });

  it("rebuildSearchIndex invokes rebuild_search_index", async () => {
    mockInvoke.mockResolvedValue({ rebuilt: 3, removedOrphans: 1 });

    const result = await rebuildSearchIndex();
    expect(mockInvoke).toHaveBeenCalledWith("rebuild_search_index");
    expect(result.rebuilt).toBe(3);
    expect(result.removedOrphans).toBe(1);
  });

  it("optimizeSearchIndex invokes optimize_search_index", async () => {
    mockInvoke.mockResolvedValue(undefined);

    await optimizeSearchIndex();
    expect(mockInvoke).toHaveBeenCalledWith("optimize_search_index");
  });

  it("propagates backend errors", async () => {
    mockInvoke.mockRejectedValue(new Error("database locked"));
    await expect(verifySearchIndex()).rejects.toThrow("database locked");
  });
});
