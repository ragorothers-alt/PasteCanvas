import { describe, it, expect } from "vitest";
import {
  INBOX_ITEM_DRAG_TYPE,
  setInboxItemDragPayload,
  hasInboxItemDragPayload,
  readInboxItemDragPayload,
} from "../features/inbox/dragPayload";

/** Minimal DataTransfer double (jsdom does not implement it fully). */
function fakeDataTransfer(initialTypes: string[] = []) {
  const store = new Map<string, string>();
  const types: string[] = [...initialTypes];
  const dt = {
    types,
    effectAllowed: "none",
    dropEffect: "none",
    setData(type: string, value: string) {
      store.set(type, value);
      if (!types.includes(type)) types.push(type);
    },
    getData(type: string) {
      return store.get(type) ?? "";
    },
  };
  return dt as unknown as DataTransfer;
}

describe("dragPayload (INB-003 drag-to-board contract)", () => {
  it("set writes the item id under the contract key with copy effect", () => {
    const dt = fakeDataTransfer();
    setInboxItemDragPayload(dt, "item-42");
    expect(dt.getData(INBOX_ITEM_DRAG_TYPE)).toBe("item-42");
    expect(dt.effectAllowed).toBe("copy");
  });

  it("has detects the payload via types (dragover-safe)", () => {
    const dt = fakeDataTransfer();
    expect(hasInboxItemDragPayload(dt)).toBe(false);
    setInboxItemDragPayload(dt, "item-1");
    expect(hasInboxItemDragPayload(dt)).toBe(true);
  });

  it("has ignores unrelated drags (files, plain text)", () => {
    expect(hasInboxItemDragPayload(fakeDataTransfer(["Files"]))).toBe(false);
    expect(hasInboxItemDragPayload(fakeDataTransfer(["text/plain"]))).toBe(false);
    expect(hasInboxItemDragPayload(null)).toBe(false);
    expect(hasInboxItemDragPayload(undefined)).toBe(false);
  });

  it("read returns the id on drop", () => {
    const dt = fakeDataTransfer();
    setInboxItemDragPayload(dt, "item-99");
    expect(readInboxItemDragPayload(dt)).toBe("item-99");
  });

  it("read returns null for empty/blank/missing payloads", () => {
    expect(readInboxItemDragPayload(null)).toBeNull();
    expect(readInboxItemDragPayload(undefined)).toBeNull();
    expect(readInboxItemDragPayload(fakeDataTransfer(["Files"]))).toBeNull();
    const dt = fakeDataTransfer();
    dt.setData(INBOX_ITEM_DRAG_TYPE, "   ");
    expect(readInboxItemDragPayload(dt)).toBeNull();
  });
});
