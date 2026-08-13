import { describe, it, expect } from "vitest";
import { toFlowEdge, toFlowEdges, toFlowNode } from "../adapters/canvas/CanvasMapper";
import type { BoardEdge, BoardNode } from "../features/boards/api";

function makeNode(overrides: Partial<BoardNode>): BoardNode {
  return {
    id: "node-1",
    boardId: "board-1",
    nodeKind: "item",
    itemId: "item-1",
    targetBoardId: null,
    parentNodeId: null,
    x: 10,
    y: 20,
    width: 280,
    height: 180,
    zIndex: 1,
    locked: false,
    collapsed: false,
    titleOverride: null,
    itemTitle: "Item",
    itemType: "plain_text",
    itemTextPreview: "preview",
    ...overrides,
  };
}

describe("CanvasMapper node mapping (BRD-004 nested boards)", () => {
  it("maps item node to itemCard type", () => {
    const node = toFlowNode(makeNode({}));
    expect(node.type).toBe("itemCard");
    expect(node.data.targetBoardId).toBeNull();
  });

  it("maps group node to group type", () => {
    const node = toFlowNode(makeNode({ nodeKind: "group", itemId: null }));
    expect(node.type).toBe("group");
  });

  it("maps board_link node to boardLink type with targetBoardId", () => {
    const node = toFlowNode(
      makeNode({
        nodeKind: "board_link",
        itemId: null,
        targetBoardId: "sub-board-42",
        titleOverride: "My Sub-board",
      }),
    );
    expect(node.type).toBe("boardLink");
    expect(node.data.targetBoardId).toBe("sub-board-42");
    expect(node.data.titleOverride).toBe("My Sub-board");
  });
});

describe("CanvasMapper edge mapping (BRD-005)", () => {
  const sampleEdge: BoardEdge = {
    id: "edge-001",
    boardId: "board-1",
    sourceNodeId: "node-a",
    targetNodeId: "node-b",
    edgeType: "default",
    label: "relates to",
  };

  describe("toFlowEdge", () => {
    it("maps DTO to React Flow edge", () => {
      const edge = toFlowEdge(sampleEdge);
      expect(edge.id).toBe("edge-001");
      expect(edge.source).toBe("node-a");
      expect(edge.target).toBe("node-b");
      expect(edge.label).toBe("relates to");
      expect(edge.type).toBe("default");
    });

    it("maps null label to undefined", () => {
      const edge = toFlowEdge({ ...sampleEdge, label: null });
      expect(edge.label).toBeUndefined();
    });
  });

  describe("toFlowEdges", () => {
    it("maps array of DTOs", () => {
      const edges: BoardEdge[] = [
        sampleEdge,
        { ...sampleEdge, id: "edge-002", sourceNodeId: "node-c" },
      ];
      const result = toFlowEdges(edges);
      expect(result).toHaveLength(2);
      expect(result[0]!.id).toBe("edge-001");
      expect(result[1]!.id).toBe("edge-002");
      expect(result[1]!.source).toBe("node-c");
    });

    it("returns empty array for empty input", () => {
      expect(toFlowEdges([])).toEqual([]);
    });
  });
});
