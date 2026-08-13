import { describe, it, expect } from "vitest";
import { parseSearchQuery, hasActiveFilters } from "../features/search/queryParser";

describe("parseSearchQuery (SRC-002)", () => {
  it("extracts type filter and keeps free text", () => {
    const result = parseSearchQuery("rust type:code");
    expect(result.freeText).toBe("rust");
    expect(result.filters.itemType).toBe("code");
  });

  it("extracts multiple filters", () => {
    const result = parseSearchQuery("notes source:vscode.exe domain:github.com favorite:true");
    expect(result.freeText).toBe("notes");
    expect(result.filters.sourceApp).toBe("vscode.exe");
    expect(result.filters.domain).toBe("github.com");
    expect(result.filters.favoriteOnly).toBe(true);
  });

  it("parses after date as epoch ms", () => {
    const result = parseSearchQuery("after:2024-01-01");
    expect(result.freeText).toBe("");
    expect(result.filters.after).toBeDefined();
    expect(result.filters.after).toBe(new Date("2024-01-01").getTime());
  });

  it("parses before date", () => {
    const result = parseSearchQuery("before:2024-06-15");
    expect(result.filters.before).toBe(new Date("2024-06-15").getTime());
  });

  it("parses epoch milliseconds directly", () => {
    const result = parseSearchQuery("after:1704067200000");
    expect(result.filters.after).toBe(1704067200000);
  });

  it("parses board and session filters", () => {
    const result = parseSearchQuery("board:abc-123 session:xyz-789");
    expect(result.filters.boardId).toBe("abc-123");
    expect(result.filters.sessionId).toBe("xyz-789");
  });

  it("parses inbox status", () => {
    const result = parseSearchQuery("inbox:archived");
    expect(result.filters.inboxStatus).toBe("archived");
  });

  it("handles quoted filter values with spaces", () => {
    const result = parseSearchQuery('source:"Visual Studio Code.exe"');
    expect(result.filters.sourceApp).toBe("Visual Studio Code.exe");
  });

  it("ignores unknown filter keys (kept as free text)", () => {
    const result = parseSearchQuery("unknown:value rust");
    expect(result.filters.itemType).toBeUndefined();
    expect(result.freeText).toContain("unknown:value");
    expect(result.freeText).toContain("rust");
  });

  it("returns empty filters for plain text", () => {
    const result = parseSearchQuery("just some text");
    expect(result.freeText).toBe("just some text");
    expect(hasActiveFilters(result.filters)).toBe(false);
  });

  it("handles filter-only query (no free text)", () => {
    const result = parseSearchQuery("type:link favorite:true");
    expect(result.freeText).toBe("");
    expect(result.filters.itemType).toBe("link");
    expect(result.filters.favoriteOnly).toBe(true);
    expect(hasActiveFilters(result.filters)).toBe(true);
  });

  it("favorite:false does not set favoriteOnly true", () => {
    const result = parseSearchQuery("favorite:false");
    expect(result.filters.favoriteOnly).toBe(false);
  });
});
