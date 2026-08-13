/**
 * Typed API client for Tauri commands.
 * All backend communication goes through this layer.
 */
import { invoke } from "@tauri-apps/api/core";

// --- Types matching Rust DTOs ---

export interface ItemSummary {
  id: string;
  itemType: string;
  title: string | null;
  textPreview: string | null;
  displayUrl: string | null;
  inboxStatus: string;
  isFavorite: boolean;
  isPinned: boolean;
  captureMethod: string;
  sourceApp: string | null;
  /** Website domain for INB-002 smart grouping. */
  domain: string | null;
  /** Capture session ID for INB-002 smart grouping. */
  captureSessionId: string | null;
  createdAt: number;
  /** Optimistic-concurrency revision returned by the application layer. */
  revision: number;
}

export interface CaptureTextRequest {
  text: string;
  sourceApp?: string;
  windowTitle?: string;
  /** Optional capture method override (e.g., "canvas_paste" for BRD-002). */
  captureMethod?: string;
}

export interface CaptureResponse {
  itemId: string;
  committedAt: number;
  isDuplicate: boolean;
  /** ID of the existing item when duplicate detected (INB-004 dedup notice). */
  duplicateOf: string | null;
}

export interface QueryInboxRequest {
  limit?: number;
  offset?: number;
  status?: string;
}

// --- API functions ---

export async function captureText(request: CaptureTextRequest): Promise<CaptureResponse> {
  return invoke<CaptureResponse>("capture_text", { request });
}

export async function captureClipboard(clipboardText: string): Promise<CaptureResponse> {
  return invoke<CaptureResponse>("capture_clipboard", { clipboardText });
}

export async function queryInbox(request: QueryInboxRequest): Promise<ItemSummary[]> {
  return invoke<ItemSummary[]>("query_inbox", { request });
}

export async function toggleFavorite(itemId: string): Promise<boolean> {
  return invoke<boolean>("toggle_favorite", { itemId });
}

export async function togglePin(itemId: string): Promise<boolean> {
  return invoke<boolean>("toggle_pin", { itemId });
}

/** INB-003: reveals the item's managed asset in the OS file explorer. */
export async function revealItemAsset(itemId: string): Promise<string> {
  return invoke<string>("reveal_item_asset", { itemId });
}

export async function trashItem(itemId: string): Promise<void> {
  return invoke<void>("trash_item", { itemId });
}

/** INB-003: remove an item from Inbox without deleting it. */
export async function archiveItem(itemId: string): Promise<void> {
  return invoke<void>("archive_item", { itemId });
}

/** INB-003: archive a selection in one application-layer transaction. */
export async function batchArchive(itemIds: string[]): Promise<number> {
  return invoke<number>("batch_archive", { itemIds });
}

/** INB-003: move a selection to Trash in one application-layer transaction. */
export async function batchTrash(itemIds: string[]): Promise<number> {
  return invoke<number>("batch_trash", { itemIds });
}

export async function captureImage(dataBase64: string, sourceApp?: string): Promise<CaptureResponse> {
  return invoke<CaptureResponse>("capture_image", { request: { dataBase64, sourceApp } });
}

export async function editItem(
  itemId: string,
  expectedRevision: number,
  patch: { title?: string; inboxStatus?: string },
): Promise<number> {
  return invoke<number>("edit_item", {
    request: { itemId, expectedRevision, ...patch },
  });
}

export async function restoreItem(itemId: string): Promise<void> {
  return invoke<void>("restore_item", { itemId });
}

export interface ItemContent {
  itemId: string;
  itemType: string;
  plainText: string | null;
  displayUrl: string | null;
  title: string | null;
}

/** Returns full item content for copy/open-source actions (SRC-003). */
export async function getItemContent(itemId: string): Promise<ItemContent> {
  return invoke<ItemContent>("get_item_content", { itemId });
}

/** Opens a URL or file with the system default handler (SRC-003). */
export async function openSource(target: string): Promise<void> {
  return invoke<void>("open_source", { target });
}

export interface AssetData {
  mimeType: string;
  dataBase64: string;
  byteSize: number;
}

/** Returns the primary asset bytes as base64 (PDF/image preview). */
export async function getItemAssetBase64(itemId: string): Promise<AssetData> {
  return invoke<AssetData>("get_item_asset_base64", { itemId });
}

/** P1-CAP-007: bounded cached PNG thumbnail for image items. */
export async function getItemThumbnail(
  itemId: string,
  maxDimension?: number,
): Promise<AssetData> {
  return invoke<AssetData>("get_item_thumbnail", { itemId, maxDimension });
}

/** Captures one or more files into the Inbox (CAP-002 drag-in). */
export async function captureFiles(paths: string[], boardId?: string): Promise<string[]> {
  return invoke<string[]>("capture_files", { request: { paths, boardId } });
}

// --- Trash (DAT-003) ---

export interface TrashStats {
  itemCount: number;
  totalBytes: number;
}

export interface EmptyTrashResult {
  deletedItems: number;
  deletedAssets: number;
}

/** Returns trash impact stats shown before emptying (DAT-003). */
export async function getTrashStats(): Promise<TrashStats> {
  return invoke<TrashStats>("get_trash_stats");
}

/** Lists soft-deleted items (the trash). */
export async function queryTrash(): Promise<ItemSummary[]> {
  return invoke<ItemSummary[]>("query_trash");
}

/** Permanently deletes trashed items + orphaned assets (DAT-003). */
export async function emptyTrash(): Promise<EmptyTrashResult> {
  return invoke<EmptyTrashResult>("empty_trash");
}
