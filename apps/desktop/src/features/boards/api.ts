import { invoke } from "@tauri-apps/api/core";

export interface BoardSummary {
  id: string;
  folderId: string | null;
  name: string;
  description: string | null;
  isFavorite: boolean;
  sortKey: string;
  nodeCount: number;
  viewportX: number;
  viewportY: number;
  viewportZoom: number;
  createdAt: number;
  updatedAt: number;
}

export interface BoardNode {
  id: string;
  boardId: string;
  nodeKind: string;
  itemId: string | null;
  targetBoardId: string | null;
  parentNodeId: string | null;
  x: number;
  y: number;
  width: number;
  height: number;
  zIndex: number;
  locked: boolean;
  collapsed: boolean;
  titleOverride: string | null;
  itemTitle: string | null;
  itemType: string | null;
  itemTextPreview: string | null;
}

export interface BoardEdge {
  id: string;
  boardId: string;
  sourceNodeId: string;
  targetNodeId: string;
  edgeType: string;
  label: string | null;
}

export interface BoardSnapshot {
  board: BoardSummary;
  nodes: BoardNode[];
  edges: BoardEdge[];
}

export async function createBoard(name: string, folderId?: string): Promise<BoardSummary> {
  return invoke<BoardSummary>("create_board", { request: { name, folderId } });
}

export async function listBoards(): Promise<BoardSummary[]> {
  return invoke<BoardSummary[]>("list_boards");
}

/** BRD-007: most recently used boards (sidebar Recent section). */
export async function getRecentBoards(limit = 5): Promise<BoardSummary[]> {
  return invoke<BoardSummary[]>("get_recent_boards", { limit });
}

export async function getBoardSnapshot(boardId: string): Promise<BoardSnapshot> {
  return invoke<BoardSnapshot>("get_board_snapshot", { boardId });
}

export async function placeItemOnBoard(boardId: string, itemId: string, x: number, y: number): Promise<string> {
  return invoke<string>("place_item_on_board", { request: { boardId, itemId, x, y } });
}

export interface ItemBoardNode {
  boardId: string;
  boardName: string;
  nodeId: string;
}

/** SRC-003: returns boards (and node ids) containing the item. */
export async function getItemBoardNodes(itemId: string): Promise<ItemBoardNode[]> {
  return invoke<ItemBoardNode[]>("get_item_board_nodes", { itemId });
}

export async function moveNodes(boardId: string, moves: Array<{ nodeId: string; x: number; y: number }>): Promise<void> {
  return invoke<void>("move_nodes", { request: { boardId, moves } });
}

/** BRD-010: persist node resize (batch, on resize end). */
export async function resizeNodes(
  boardId: string,
  resizes: Array<{ nodeId: string; width: number; height: number }>,
): Promise<void> {
  return invoke<void>("resize_nodes", { request: { boardId, resizes } });
}

/** BRD-010: lock/unlock a node. */
export async function setNodeLocked(boardId: string, nodeId: string, locked: boolean): Promise<void> {
  return invoke<void>("set_node_locked", { boardId, nodeId, locked });
}

/** BRD-010: collapse/expand a group node. */
export async function setNodeCollapsed(boardId: string, nodeId: string, collapsed: boolean): Promise<void> {
  return invoke<void>("set_node_collapsed", { boardId, nodeId, collapsed });
}

/** BRD-010: raise a node above all others (z-order). */
export async function bringNodeToFront(boardId: string, nodeId: string): Promise<void> {
  return invoke<void>("bring_node_to_front", { boardId, nodeId });
}

export async function saveViewport(boardId: string, x: number, y: number, zoom: number): Promise<void> {
  return invoke<void>("save_viewport", { request: { boardId, x, y, zoom } });
}

export async function renameBoard(boardId: string, name: string): Promise<void> {
  return invoke<void>("rename_board", { boardId, name });
}

export async function trashBoard(boardId: string): Promise<void> {
  return invoke<void>("trash_board", { boardId });
}

export async function moveBoard(boardId: string, folderId?: string | null): Promise<void> {
  return invoke<void>("move_board", { boardId, folderId: folderId ?? null });
}

export async function removeNode(boardId: string, nodeId: string): Promise<void> {
  return invoke<void>("remove_node", { boardId, nodeId });
}

export async function groupNodes(boardId: string, nodeIds: string[], title?: string): Promise<string> {
  return invoke<string>("group_nodes", { request: { boardId, nodeIds, title } });
}

export async function ungroupNodes(boardId: string, groupId: string): Promise<void> {
  return invoke<void>("ungroup_nodes", { boardId, groupId });
}

export async function placeBoardLink(boardId: string, targetBoardId: string, x: number, y: number): Promise<string> {
  return invoke<string>("place_board_link", { request: { boardId, targetBoardId, x, y } });
}

export async function createEdge(
  boardId: string,
  sourceNodeId: string,
  targetNodeId: string,
  label?: string,
): Promise<string> {
  return invoke<string>("create_edge", { request: { boardId, sourceNodeId, targetNodeId, label } });
}

export async function deleteEdge(boardId: string, edgeId: string): Promise<void> {
  return invoke<void>("delete_edge", { boardId, edgeId });
}

// --- BRD-006: folder hierarchy & board organization ---

export interface Folder {
  id: string;
  parentFolderId: string | null;
  name: string;
  sortKey: string;
  createdAt: number;
  updatedAt: number;
}

/** Minimal row for a trashed board/folder (Trash restore section). */
export interface TrashedEntry {
  id: string;
  name: string;
  deletedAt: number;
}

/** Fired when board/folder lists change so the sidebar can refresh. */
export const BOARDS_CHANGED_EVENT = "paste-canvas://boards-changed";

export async function listFolders(): Promise<Folder[]> {
  return invoke<Folder[]>("list_folders");
}

export async function createFolder(name: string, parentFolderId?: string): Promise<Folder> {
  return invoke<Folder>("create_folder", { request: { name, parentFolderId } });
}

export async function renameFolder(folderId: string, name: string): Promise<void> {
  return invoke<void>("rename_folder", { folderId, name });
}

export async function trashFolder(folderId: string): Promise<void> {
  return invoke<void>("trash_folder", { folderId });
}

/** Trashed boards (restore section of the Trash view). */
export async function listTrashedBoards(): Promise<TrashedEntry[]> {
  return invoke<TrashedEntry[]>("list_trashed_boards");
}

/** Trashed folders (restore section of the Trash view). */
export async function listTrashedFolders(): Promise<TrashedEntry[]> {
  return invoke<TrashedEntry[]>("list_trashed_folders");
}

export async function restoreBoard(boardId: string): Promise<void> {
  return invoke<void>("restore_board", { boardId });
}

export async function restoreFolder(folderId: string): Promise<void> {
  return invoke<void>("restore_folder", { folderId });
}

export async function reorderFolders(orderedIds: string[]): Promise<void> {
  return invoke<void>("reorder_folders", { request: { orderedIds } });
}

export async function reorderBoards(orderedIds: string[]): Promise<void> {
  return invoke<void>("reorder_boards", { request: { orderedIds } });
}

export async function toggleBoardFavorite(boardId: string): Promise<boolean> {
  return invoke<boolean>("toggle_board_favorite", { boardId });
}

