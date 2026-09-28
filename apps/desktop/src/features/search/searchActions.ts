/**
 * Search result actions (SRC-003).
 * Per PRD SRC-003: results can open Item detail, locate Board Node, add to current
 * Board, Favorite/Pin, open source, and copy.
 *
 * These pure functions wrap Tauri commands + clipboard so they can be unit tested.
 */
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { getItemContent, openSource, toggleFavorite } from "../inbox/api";
import { getItemBoardNodes } from "../boards/api";
import type { ItemBoardNode } from "../boards/api";

/** Dispatches the open-item event so the Inbox shows the item detail panel. */
export function openItemDetail(itemId: string): void {
  window.dispatchEvent(
    new CustomEvent("paste-canvas://open-item", { detail: { itemId } }),
  );
}

/**
 * Copies an item's content to the clipboard.
 * For links, copies the display URL; otherwise copies plain_text.
 * Returns the text that was copied, or null if nothing to copy.
 */
export async function copyItemContent(itemId: string): Promise<string | null> {
  const content = await getItemContent(itemId);
  const textToCopy =
    content.itemType === "link" && content.displayUrl
      ? content.displayUrl
      : content.plainText ?? content.title ?? "";

  if (!textToCopy) return null;
  await writeText(textToCopy);
  return textToCopy;
}

/**
 * Opens an item's source (URL for links, otherwise nothing).
 * Returns true if a source was opened.
 */
export async function openItemSource(itemId: string): Promise<boolean> {
  const content = await getItemContent(itemId);
  const target = content.displayUrl ?? content.plainText;
  if (!target) return false;
  await openSource(target);
  return true;
}

/** Toggles the favorite flag on an item. Returns the new state. */
export async function favoriteItem(itemId: string): Promise<boolean> {
  return toggleFavorite(itemId);
}

/** SRC-003: returns the boards that contain the item (for locate-on-board). */
export async function locateItemBoards(itemId: string): Promise<ItemBoardNode[]> {
  return getItemBoardNodes(itemId);
}

/** Dispatches a navigate-to-board event so the app opens that board (locate). */
export function navigateToBoard(boardId: string): void {
  window.dispatchEvent(
    new CustomEvent("paste-canvas://open-board", { detail: { boardId } }),
  );
}
