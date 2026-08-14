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
});
