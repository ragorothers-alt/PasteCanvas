import type { BoardSummary, Folder } from "./api";

/** A node in the sidebar board tree (BRD-006). */
export interface BoardTreeNode {
  /** null for the synthetic root group holding unfiled boards. */
  folder: Folder | null;
  boards: BoardSummary[];
  children: BoardTreeNode[];
}

/**
 * Builds a nested tree from flat folder/board lists.
 * - Folders nest according to parentFolderId (cycle-safe: folders whose parent
 *   chain cannot resolve are attached at the root instead of looping).
 * - Boards attach to their folderId; unfiled boards go to the root group.
 * - Ordering preserves the input order (backend already sorts by sort_key).
 */
export function buildBoardTree(
  folders: Folder[],
  boards: BoardSummary[],
): BoardTreeNode[] {
  const nodesById = new Map<string, BoardTreeNode>();
  for (const f of folders) {
    nodesById.set(f.id, { folder: f, boards: [], children: [] });
  }

  const roots: BoardTreeNode[] = [];
  const rootGroup: BoardTreeNode = { folder: null, boards: [], children: [] };

  // Attach folders to parents; detect broken/cyclic chains by walking up from
  // the proposed parent — if we reach the folder itself, attaching would loop.
  for (const f of folders) {
    const node = nodesById.get(f.id);
    if (!node) continue;
    const parentId = f.parentFolderId;
    const parent = parentId ? nodesById.get(parentId) : null;
    const wouldCycle = parentId !== null && isAncestor(nodesById, parentId, f.id);
    if (parent && parent !== node && !wouldCycle) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  }

  // Attach boards.
  for (const b of boards) {
    const target = b.folderId ? nodesById.get(b.folderId) : null;
    if (target) {
      target.boards.push(b);
    } else {
      rootGroup.boards.push(b);
    }
  }

  // Root group first (unfiled boards), then folder trees.
  return [rootGroup, ...roots];
}

/** True when walking up from `folderId` reaches `candidateId`. */
function isAncestor(
  nodesById: Map<string, BoardTreeNode>,
  folderId: string,
  candidateId: string,
): boolean {
  let cursor: string | null = folderId;
  let guard = 0;
  while (cursor && guard < 1000) {
    const node = nodesById.get(cursor);
    if (!node || !node.folder) return false;
    if (node.folder.id === candidateId) return true;
    cursor = node.folder.parentFolderId;
    guard += 1;
  }
  return false;
}
