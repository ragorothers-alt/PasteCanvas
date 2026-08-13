/**
 * CanvasMapper — converts Rust BoardNode DTOs to React Flow nodes and back.
 * React Flow-specific types stay here; persistence types stay in features/boards/api.
 */
import type { Node, Edge, XYPosition } from "@xyflow/react";
import type { BoardNode, BoardEdge } from "../../features/boards/api";

export interface ItemCardData {
  itemId: string | null;
  itemTitle: string | null;
  itemType: string | null;
  itemTextPreview: string | null;
  titleOverride: string | null;
  locked: boolean;
  /** BRD-010: group collapse state (group nodes). */
  collapsed: boolean;
  /** Owning board (needed by resize/z-order persistence, BRD-010). */
  boardId: string | null;
  /** Target board ID for board_link nodes (BRD-004 nested boards). */
  targetBoardId: string | null;
  [key: string]: unknown;
}

export type CanvasNode = Node<ItemCardData>;

/** Maps a Rust BoardNode DTO to a React Flow node. */
export function toFlowNode(dto: BoardNode): CanvasNode {
  // BRD-004: board_link nodes render as navigable sub-board entries
  const type =
    dto.nodeKind === "group"
      ? "group"
      : dto.nodeKind === "board_link"
        ? "boardLink"
        : "itemCard";

  return {
    id: dto.id,
    type,
    position: { x: dto.x, y: dto.y },
    parentId: dto.parentNodeId ?? undefined,
    data: {
      itemId: dto.itemId,
      itemTitle: dto.itemTitle,
      itemType: dto.itemType,
      itemTextPreview: dto.itemTextPreview,
      titleOverride: dto.titleOverride,
      locked: dto.locked,
      collapsed: dto.collapsed,
      boardId: dto.boardId,
      targetBoardId: dto.targetBoardId,
    },
    style: {
      width: dto.width,
      height: dto.height,
      zIndex: dto.zIndex,
    },
    draggable: !dto.locked,
  };
}

/** Maps an array of BoardNode DTOs to React Flow nodes. */
export function toFlowNodes(dtos: BoardNode[]): CanvasNode[] {
  return dtos.map(toFlowNode);
}

/** Maps a Rust BoardEdge DTO to a React Flow edge (BRD-005). */
export function toFlowEdge(dto: BoardEdge): Edge {
  return {
    id: dto.id,
    source: dto.sourceNodeId,
    target: dto.targetNodeId,
    label: dto.label ?? undefined,
    type: "default",
  };
}

/** Maps an array of BoardEdge DTOs to React Flow edges. */
export function toFlowEdges(dtos: BoardEdge[]): Edge[] {
  return dtos.map(toFlowEdge);
}

/** Extracts position changes from React Flow nodes for persistence. */
export function extractMoves(
  nodes: CanvasNode[],
  originalPositions: Map<string, XYPosition>,
): Array<{ nodeId: string; x: number; y: number }> {
  const moves: Array<{ nodeId: string; x: number; y: number }> = [];
  for (const node of nodes) {
    const orig = originalPositions.get(node.id);
    if (orig && (orig.x !== node.position.x || orig.y !== node.position.y)) {
      moves.push({ nodeId: node.id, x: node.position.x, y: node.position.y });
    }
  }
  return moves;
}
