/**
 * BRD-008/009 sidebar drag-reorder (PRD BRD-001 "側邊欄可拖曳排序").
 *
 * Pure computation for reordering boards/folders by drag & drop in the
 * sidebar. The backend persists the new order via reorder_boards /
 * reorder_folders (sort_key rewrite).
 */
import type { BoardSummary, Folder } from "./api";

/** DataTransfer key for a sidebar board drag. */
export const BOARD_DRAG_TYPE = "application/x-pastecanvas-board";

/** DataTransfer key for a sidebar folder drag (BRD-009). */
export const FOLDER_DRAG_TYPE = "application/x-pastecanvas-folder";

/**
 * Generic list reorder: computes the new id order when `draggedId` is dropped
 * onto `targetId`. Drop semantics: the dragged entry takes the target's slot;
 * the target (and everything after it) shifts down one position. Returns null
 * when the drop is a no-op (same entry or unknown id).
 */
export function computeListReorder<T extends { id: string }>(
  items: T[],
  draggedId: string,
  targetId: string,
): string[] | null {
  if (draggedId === targetId) return null;
  const from = items.findIndex((i) => i.id === draggedId);
  const to = items.findIndex((i) => i.id === targetId);
  if (from < 0 || to < 0) return null;

  const ids = items.map((i) => i.id);
  ids.splice(from, 1);
  // After removing the dragged id, the target's slot index equals its original
  // index in both drag directions.
  ids.splice(to, 0, draggedId);
  return ids;
}

/**
 * Computes the new board id order when `draggedId` is dropped onto
 * `targetId`. Returns null when the drop is a no-op.
 */
export function computeBoardReorder(
  boards: BoardSummary[],
  draggedId: string,
  targetId: string,
): string[] | null {
  return computeListReorder(boards, draggedId, targetId);
}

/**
 * BRD-009: computes the new folder id order (flat sort_key order) when
 * `draggedId` is dropped onto `targetId`. Reparenting is intentionally a
 * separate action (move_folder); this only reorders.
 */
export function computeFolderReorder(
  folders: Folder[],
  draggedId: string,
  targetId: string,
): string[] | null {
  return computeListReorder(folders, draggedId, targetId);
}
