import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { InboxItem } from "../features/inbox/InboxItem";
import type { ItemSummary } from "../features/inbox/api";

// Mock the boards API
vi.mock("../features/boards/api", () => ({
  listBoards: vi.fn().mockResolvedValue([]),
  placeItemOnBoard: vi.fn().mockResolvedValue("node-1"),
}));

const mockItem: ItemSummary = {
  id: "item-1",
  itemType: "plain_text",
  title: "Test Note",
  textPreview: "Hello world content",
  displayUrl: null,
  inboxStatus: "new",
  isFavorite: false,
  isPinned: false,
  captureMethod: "manual_hotkey",
  sourceApp: "notepad.exe",
  domain: null,
  captureSessionId: null,
  createdAt: Date.now(),
  revision: 1,
  assetByteSize: null,
};

describe("InboxItem", () => {
  it("renders item title", () => {
    render(
      <InboxItem
        item={mockItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    expect(screen.getByText("Test Note")).toBeDefined();
  });

  it("renders item type and source", () => {
    render(
      <InboxItem
        item={mockItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    expect(screen.getByText("文字")).toBeDefined();
    expect(screen.getByText("notepad.exe")).toBeDefined();
  });

  it("shows favorite star when favorited", () => {
    const favItem = { ...mockItem, isFavorite: true };
    render(
      <InboxItem
        item={favItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    expect(screen.getByTitle("取消收藏")).toBeDefined();
  });

  it("calls onToggleFavorite when star clicked", () => {
    const onFav = vi.fn();
    render(
      <InboxItem
        item={mockItem}
        onToggleFavorite={onFav}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTitle("加入收藏"));
    expect(onFav).toHaveBeenCalledTimes(1);
  });

  it("calls onTrash when trash clicked", () => {
    const onTrash = vi.fn();
    render(
      <InboxItem
        item={mockItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={onTrash}
      />,
    );
    fireEvent.click(screen.getByTitle("移到垃圾桶"));
    expect(onTrash).toHaveBeenCalledTimes(1);
  });

  it("renders link icon for link type", () => {
    const linkItem = { ...mockItem, itemType: "link", title: "https://example.com" };
    const { container } = render(
      <InboxItem
        item={linkItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    expect(container.querySelector(".item-icon svg")).toBeDefined();
  });

  it("shows untitled for items without title or preview", () => {
    const noTitle = { ...mockItem, title: null, textPreview: null };
    render(
      <InboxItem
        item={noTitle}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    expect(screen.getByText("（未命名）")).toBeDefined();
  });

  it("opens a labelled board picker menu", async () => {
    const { listBoards } = await import("../features/boards/api");
    vi.mocked(listBoards).mockResolvedValueOnce([
      { id: "b1", folderId: null, name: "看板一", description: null, isFavorite: false, sortKey: "a", nodeCount: 0, viewportX: 0, viewportY: 0, viewportZoom: 1, createdAt: 0, updatedAt: 0 },
    ]);
    render(
      <InboxItem
        item={mockItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "加入看板" }));
    expect(await screen.findByRole("menu", { name: "選擇目標看板" })).toBeDefined();
    expect(screen.getByRole("menuitem", { name: "看板一" })).toBeDefined();
  });

  it("shows success state after adding to board", async () => {
    const { listBoards, placeItemOnBoard } = await import("../features/boards/api");
    vi.mocked(listBoards).mockResolvedValueOnce([
      { id: "b1", folderId: null, name: "看板一", description: null, isFavorite: false, sortKey: "a", nodeCount: 0, viewportX: 0, viewportY: 0, viewportZoom: 1, createdAt: 0, updatedAt: 0 },
    ]);
    vi.mocked(placeItemOnBoard).mockResolvedValueOnce("node-1");
    render(
      <InboxItem
        item={mockItem}
        onToggleFavorite={vi.fn()}
        onTogglePin={vi.fn()}
        onTrash={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "加入看板" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "看板一" }));
    expect(await screen.findByRole("button", { name: "已加入看板" })).toBeDefined();
  });
});
