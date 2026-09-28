import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TagSelector } from "../features/tags/TagSelector";

// Mock the tags API
const listTags = vi.fn();
const getItemTags = vi.fn();
const renameTag = vi.fn();
const deleteTag = vi.fn();
vi.mock("../features/tags/api", () => ({
  listTags: (...args: unknown[]) => listTags(...args),
  getItemTags: (...args: unknown[]) => getItemTags(...args),
  addTagToItem: vi.fn().mockResolvedValue(undefined),
  removeTagFromItem: vi.fn().mockResolvedValue(undefined),
  createTag: vi.fn().mockResolvedValue({ id: "new", name: "new", color: null, itemCount: 0 }),
  renameTag: (...args: unknown[]) => renameTag(...args),
  deleteTag: (...args: unknown[]) => deleteTag(...args),
}));

const existingTag = { id: "tag-1", name: "工作", color: null, itemCount: 3 };

describe("TagSelector management actions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getItemTags.mockResolvedValue([]);
    listTags.mockResolvedValue([existingTag]);
  });

  it("shows rename and delete actions in the dropdown", async () => {
    render(<TagSelector itemId="item-1" />);

    // Open the dropdown by focusing the input
    fireEvent.focus(screen.getByPlaceholderText("新增標籤…"));

    await waitFor(() => {
      expect(screen.getByText("工作")).toBeDefined();
    });
    expect(screen.getByTitle("重新命名")).toBeDefined();
    expect(screen.getByTitle("刪除標籤")).toBeDefined();
  });

  it("renaming saves the new name via renameTag", async () => {
    renameTag.mockResolvedValue(undefined);
    render(<TagSelector itemId="item-1" />);

    fireEvent.focus(screen.getByPlaceholderText("新增標籤…"));
    await waitFor(() => {
      expect(screen.getByTitle("重新命名")).toBeDefined();
    });

    fireEvent.click(screen.getByTitle("重新命名"));

    const input = screen.getByDisplayValue("工作");
    fireEvent.change(input, { target: { value: "生活" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(renameTag).toHaveBeenCalledWith("tag-1", "生活");
    });
  });

  it("Escape cancels rename without saving", async () => {
    render(<TagSelector itemId="item-1" />);

    fireEvent.focus(screen.getByPlaceholderText("新增標籤…"));
    await waitFor(() => {
      expect(screen.getByTitle("重新命名")).toBeDefined();
    });

    fireEvent.click(screen.getByTitle("重新命名"));
    const input = screen.getByDisplayValue("工作");
    fireEvent.keyDown(input, { key: "Escape" });

    expect(renameTag).not.toHaveBeenCalled();
  });

  it("deleting a confirmed tag calls deleteTag", async () => {
    deleteTag.mockResolvedValue(undefined);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<TagSelector itemId="item-1" />);

    fireEvent.focus(screen.getByPlaceholderText("新增標籤…"));
    await waitFor(() => {
      expect(screen.getByTitle("刪除標籤")).toBeDefined();
    });

    fireEvent.click(screen.getByTitle("刪除標籤"));

    await waitFor(() => {
      expect(deleteTag).toHaveBeenCalledWith("tag-1");
    });
    // The confirmation message reports the affected item count
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining("3 個項目"));
    confirmSpy.mockRestore();
  });

  it("cancelling the delete confirmation does not delete", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<TagSelector itemId="item-1" />);

    fireEvent.focus(screen.getByPlaceholderText("新增標籤…"));
    await waitFor(() => {
      expect(screen.getByTitle("刪除標籤")).toBeDefined();
    });

    fireEvent.click(screen.getByTitle("刪除標籤"));

    expect(deleteTag).not.toHaveBeenCalled();
    vi.restoreAllMocks();
  });
});
