/**
 * INB-003 drag-to-board payload (PRD: "拖到 Board").
 *
 * Inbox items use native HTML5 drag & drop; the canvas reads the payload on
 * drop and places the item at the pointer's flow coordinates. Kept as a tiny
 * framework-free module so both sides (inbox + canvas) share one contract.
 */

/** MIME-ish key identifying an inbox-item drag payload. */
export const INBOX_ITEM_DRAG_TYPE = "application/x-pastecanvas-item";

/** Marks a dragstart event as carrying an inbox item. */
export function setInboxItemDragPayload(
  dataTransfer: DataTransfer,
  itemId: string,
): void {
  dataTransfer.setData(INBOX_ITEM_DRAG_TYPE, itemId);
  dataTransfer.effectAllowed = "copy";
}

/**
 * True during dragover when the drag carries an inbox item.
 * (`getData` is protected until drop, so `types` must be used here.)
 */
export function hasInboxItemDragPayload(
  dataTransfer: DataTransfer | null | undefined,
): boolean {
  if (!dataTransfer) return false;
  return Array.from(dataTransfer.types).includes(INBOX_ITEM_DRAG_TYPE);
}

/**
 * Extracts the inbox item id from a drop event.
 * Returns null for unrelated drags (files, text, other apps).
 */
export function readInboxItemDragPayload(
  dataTransfer: DataTransfer | null | undefined,
): string | null {
  if (!dataTransfer) return null;
  const id = dataTransfer.getData(INBOX_ITEM_DRAG_TYPE);
  return typeof id === "string" && id.trim().length > 0 ? id : null;
}
