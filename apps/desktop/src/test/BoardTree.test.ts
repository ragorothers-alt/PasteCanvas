import { describe, it, expect } from "vitest";
import { buildBoardTree } from "../features/boards/boardTree";
import type { BoardSummary, Folder } from "../features/boards/api";

function folder(id: string, parentFolderId: string | null, name = id): Folder {
  return { id, parentFolderId, name, sortKey: id, createdAt: 0, updatedAt: 0 };
}

function board(id: string, folderId: string | null, name = id): BoardSummary {
  return {
    id,
    folderId,
    name,
    description: null,
    isFavorite: false,
    sortKey: id,
    nodeCount: 0,
    viewportX: 0,
    viewportY: 0,
    viewportZoom: 1,
    createdAt: 0,
    updatedAt: 0,
  };
}

describe("buildBoardTree (BRD-006)", () => {
  it("groups unfiled boards into the root group", () => {
    const tree = buildBoardTree([], [board("b1", null), board("b2", null)]);
    expect(tree).toHaveLength(1);
    expect(tree[0]!.folder).toBeNull();
    expect(tree[0]!.boards.map((b) => b.id)).toEqual(["b1", "b2"]);
  });

  it("nests boards under folders and folders under parents", () => {
    const folders = [folder("f1", null), folder("f2", "f1")];
    const boards = [board("b1", "f2"), board("b2", "f1"), board("b3", null)];
    const tree = buildBoardTree(folders, boards);

    // root group + f1
    expect(tree).toHaveLength(2);
    expect(tree[0]!.boards.map((b) => b.id)).toEqual(["b3"]);

    const f1 = tree[1]!;
    expect(f1.folder!.id).toBe("f1");
    expect(f1.boards.map((b) => b.id)).toEqual(["b2"]);
    expect(f1.children).toHaveLength(1);
    expect(f1.children[0]!.folder!.id).toBe("f2");
    expect(f1.children[0]!.boards.map((b) => b.id)).toEqual(["b1"]);
  });

  it("keeps ordering from backend sort_key (input order)", () => {
    const folders = [folder("z", null), folder("a", null)];
    const tree = buildBoardTree(folders, []);
    expect(tree.slice(1).map((n) => n.folder!.id)).toEqual(["z", "a"]);
  });

  it("breaks cyclic folder chains at the root instead of looping", () => {
    // Corrupted data: x → y → x (should never happen, must not hang).
    const folders = [folder("x", "y"), folder("y", "x")];
    const tree = buildBoardTree(folders, []);
    // Both attached at root (one is the cycle-breaker); no infinite recursion.
    expect(tree.length).toBeGreaterThanOrEqual(2);
    const ids = tree.slice(1).map((n) => n.folder!.id).sort();
    expect(ids).toEqual(["x", "y"]);
  });

  it("attaches boards whose folder is missing to the root group", () => {
    const tree = buildBoardTree([], [board("b1", "ghost-folder")]);
    expect(tree[0]!.boards.map((b) => b.id)).toEqual(["b1"]);
  });

  it("handles deep nesting", () => {
    const folders = [
      folder("l1", null),
      folder("l2", "l1"),
      folder("l3", "l2"),
    ];
    const tree = buildBoardTree(folders, [board("deep", "l3")]);
    const l1 = tree[1]!;
    const l2 = l1.children[0]!;
    const l3 = l2.children[0]!;
    expect(l3.boards.map((b) => b.id)).toEqual(["deep"]);
  });
});
