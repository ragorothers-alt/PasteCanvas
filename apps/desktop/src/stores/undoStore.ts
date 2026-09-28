/**
 * Canvas Undo/Redo store — command/snapshot pattern.
 * Per AGENTS.md: "Undo/redo 是 command/snapshot 層，不得依賴 React Flow Pro 範例程式碼"
 *
 * Each command has:
 * - A description for debugging
 * - An undo function that reverses the operation via backend API
 * - A redo function that re-applies it
 *
 * History is bounded (max 50 entries) to prevent memory growth.
 */
import { create } from "zustand";
import { moveNodes, removeNode, placeItemOnBoard, resizeNodes } from "../features/boards/api";
import { groupNodes, ungroupNodes } from "../features/boards/api";

interface UndoCommand {
  description: string;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
}

interface UndoState {
  history: UndoCommand[];
  pointer: number; // Points to next available slot (0 = empty)
  boardId: string | null;

  setBoard: (boardId: string) => void;
  push: (cmd: UndoCommand) => void;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  canUndo: () => boolean;
  canRedo: () => boolean;
  clear: () => void;
}

const MAX_HISTORY = 50;

export const useUndoStore = create<UndoState>((set, get) => ({
  history: [],
  pointer: 0,
  boardId: null,

  setBoard: (boardId) => set({ boardId, history: [], pointer: 0 }),

  push: (cmd) => {
    const { history, pointer } = get();
    // Truncate any redo history beyond current pointer
    const truncated = history.slice(0, pointer);
    const newHistory = [...truncated, cmd].slice(-MAX_HISTORY);
    set({ history: newHistory, pointer: newHistory.length });
  },

  undo: async () => {
    const { history, pointer } = get();
    if (pointer <= 0) return;
    const cmd = history[pointer - 1];
    if (!cmd) return;
    await cmd.undo();
    set({ pointer: pointer - 1 });
  },

  redo: async () => {
    const { history, pointer } = get();
    if (pointer >= history.length) return;
    const cmd = history[pointer];
    if (!cmd) return;
    await cmd.redo();
    set({ pointer: pointer + 1 });
  },

  canUndo: () => get().pointer > 0,
  canRedo: () => get().pointer < get().history.length,

  clear: () => set({ history: [], pointer: 0 }),
}));

// --- Command factories ---

export function createResizeCommand(
  boardId: string,
  resizes: Array<{ nodeId: string; width: number; height: number }>,
  previousSizes: Array<{ nodeId: string; width: number; height: number }>,
): UndoCommand {
  return {
    description: `Resize ${resizes.length} node(s)`,
    undo: async () => {
      await resizeNodes(boardId, previousSizes);
    },
    redo: async () => {
      await resizeNodes(boardId, resizes);
    },
  };
}

export function createMoveCommand(
  boardId: string,
  moves: Array<{ nodeId: string; x: number; y: number }>,
  previousPositions: Array<{ nodeId: string; x: number; y: number }>,
): UndoCommand {
  return {
    description: `Move ${moves.length} node(s)`,
    undo: async () => {
      await moveNodes(boardId, previousPositions);
    },
    redo: async () => {
      await moveNodes(boardId, moves);
    },
  };
}

export function createRemoveNodeCommand(
  boardId: string,
  nodeId: string,
  itemId: string,
  x: number,
  y: number,
): UndoCommand {
  return createRemoveNodesCommand(boardId, [{ nodeId, itemId, x, y }]);
}

export interface RemovedNodeSnapshot {
  nodeId: string;
  /** Null for group / boardLink nodes — re-created via snapshot reload. */
  itemId: string | null;
  x: number;
  y: number;
}

/**
 * Batch remove: one history entry per gesture. Undo re-places every
 * item-backed node and tracks the NEW ids it comes back with, so redo
 * removes the live nodes instead of the stale pre-undo ids. Nodes without
 * an item backing (groups, board links) are restored by the caller's
 * snapshot refresh — they are still removed atomically with the batch.
 */
export function createRemoveNodesCommand(
  boardId: string,
  removed: RemovedNodeSnapshot[],
): UndoCommand {
  let liveIds = removed.map((r) => r.nodeId);
  return {
    description: `Remove ${removed.length} node(s)`,
    undo: async () => {
      const restored: string[] = [];
      for (const r of removed) {
        if (r.itemId) {
          // Re-place the item at its previous position; track the new id.
          const newId = await placeItemOnBoard(boardId, r.itemId, r.x, r.y);
          restored.push(typeof newId === "string" ? newId : r.nodeId);
        } else {
          restored.push(r.nodeId);
        }
      }
      liveIds = restored;
    },
    redo: async () => {
      for (const id of liveIds) {
        await removeNode(boardId, id);
      }
      liveIds = removed.map((r) => r.nodeId);
    },
  };
}

export function createGroupCommand(
  boardId: string,
  nodeIds: string[],
  groupId: string,
): UndoCommand {
  return {
    description: `Group ${nodeIds.length} nodes`,
    undo: async () => {
      await ungroupNodes(boardId, groupId);
    },
    redo: async () => {
      await groupNodes(boardId, nodeIds);
    },
  };
}
