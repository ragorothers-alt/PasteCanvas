import { describe, it, expect } from "vitest";
import {
  computeBoardReorder,
  computeFolderReorder,
  computeListReorder,
  BOARD_DRAG_TYPE,
  FOLDER_DRAG_TYPE,
} from "../features/boards/boardReorder";
import type { BoardSummary, Folder } from "../features/boards/api";

function boardsOf(ids: string[]): BoardSummary[] {
  return ids.map((id) => ({
    id,
    folderId: null,
    name: id,
    description: null,
    isFavorite: false,
    sortKey: id,
    nodeCount: 0,
    viewportX: 0,
    viewportY: 0,
    viewportZoom: 1,
    createdAt: 0,
    updatedAt: 0,
  }));
}

describe("computeBoardReorder (BRD-008 sidebar drag-reorder)", () => {
  it("dragging forward puts the dragged board into the target's slot", () => {
    // A onto C → A takes C's slot, C shifts down
    expect(computeBoardReorder(boardsOf(["A", "B", "C", "D"]), "A", "C")).toEqual([
      "B",
      "C",
      "A",
      "D",
    ]);
  });

  it("dragging backward puts the dragged board into the target's slot", () => {
    // D onto B → D takes B's slot, B/C shift down
    expect(computeBoardReorder(boardsOf(["A", "B", "C", "D"]), "D", "B")).toEqual([
      "A",
      "D",
      "B",
      "C",
    ]);
  });

  it("adjacent swap works both ways", () => {
    expect(computeBoardReorder(boardsOf(["A", "B", "C"]), "A", "B")).toEqual([
      "B",
      "A",
      "C",
    ]);
    expect(computeBoardReorder(boardsOf(["A", "B", "C"]), "B", "A")).toEqual([
      "B",
      "A",
      "C",
    ]);
  });

  it("dropping onto itself is a no-op", () => {
    expect(computeBoardReorder(boardsOf(["A", "B"]), "A", "A")).toBeNull();
  });

  it("unknown ids are rejected", () => {
    expect(computeBoardReorder(boardsOf(["A", "B"]), "X", "A")).toBeNull();
    expect(computeBoardReorder(boardsOf(["A", "B"]), "A", "X")).toBeNull();
  });

  it("single-element list drops onto itself → no-op", () => {
    expect(computeBoardReorder(boardsOf(["A"]), "A", "A")).toBeNull();
  });

  it("exposes stable drag payload keys", () => {
    expect(BOARD_DRAG_TYPE).toBe("application/x-pastecanvas-board");
    expect(FOLDER_DRAG_TYPE).toBe("application/x-pastecanvas-folder");
  });
});

function foldersOf(ids: string[]): Folder[] {
  return ids.map((id) => ({
    id,
    parentFolderId: null,
    name: id,
    sortKey: id,
    createdAt: 0,
    updatedAt: 0,
  }));
}

describe("computeFolderReorder (BRD-009 folder drag-reorder)", () => {
  it("reorders folders forward and backward", () => {
    expect(computeFolderReorder(foldersOf(["F1", "F2", "F3"]), "F1", "F3")).toEqual([
      "F2",
      "F3",
      "F1",
    ]);
    expect(computeFolderReorder(foldersOf(["F1", "F2", "F3"]), "F3", "F1")).toEqual([
      "F3",
      "F1",
      "F2",
    ]);
  });

  it("rejects no-op and unknown drops", () => {
    expect(computeFolderReorder(foldersOf(["F1", "F2"]), "F1", "F1")).toBeNull();
    expect(computeFolderReorder(foldersOf(["F1", "F2"]), "X", "F1")).toBeNull();
  });

  it("generic computeListReorder works over any {id} items", () => {
    const items = [{ id: "a" }, { id: "b" }, { id: "c" }];
    expect(computeListReorder(items, "a", "c")).toEqual(["b", "c", "a"]);
  });
});
