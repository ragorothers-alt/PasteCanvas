import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TrashView } from "../features/inbox/TrashView";

// Mock inbox API (TrashView + InboxItem imports)
const queryTrash = vi.fn();
const getTrashStats = vi.fn();
const restoreItem = vi.fn();
vi.mock("../features/inbox/api", () => ({
  queryTrash: (...args: unknown[]) => queryTrash(...args),
  getTrashStats: (...args: unknown[]) => getTrashStats(...args),
  restoreItem: (...args: unknown[]) => restoreItem(...args),
  emptyTrash: vi.fn().mockResolvedValue({ deletedItems: 0, deletedAssets: 0 }),
  toggleFavorite: vi.fn().mockResolvedValue(false),
  togglePin: vi.fn().mockResolvedValue(false),
}));

// Mock boards API (restore section)
const listTrashedBoards = vi.fn();
const listTrashedFolders = vi.fn();
const restoreBoard = vi.fn();
const restoreFolder = vi.fn();
vi.mock("../features/boards/api", () => ({
  listBoards: vi.fn().mockResolvedValue([]),
  placeItemOnBoard: vi.fn().mockResolvedValue("node-1"),
  listTrashedBoards: (...args: unknown[]) => listTrashedBoards(...args),
  listTrashedFolders: (...args: unknown[]) => listTrashedFolders(...args),
  restoreBoard: (...args: unknown[]) => restoreBoard(...args),
  restoreFolder: (...args: unknown[]) => restoreFolder(...args),
  BOARDS_CHANGED_EVENT: "paste-canvas://boards-changed",
}));

const emptyStats = { itemCount: 0, totalBytes: 0 };

describe("TrashView restore section", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryTrash.mockResolvedValue([]);
    getTrashStats.mockResolvedValue(emptyStats);
  });

  it("lists trashed boards and folders with restore buttons", async () => {
    listTrashedBoards.mockResolvedValue([
      { id: "board-1", name: "舊看板", deletedAt: 1 },
    ]);
    listTrashedFolders.mockResolvedValue([
      { id: "folder-1", name: "舊資料夾", deletedAt: 1 },
    ]);

    render(<TrashView />);

    await waitFor(() => {
      expect(screen.getByText("看板與資料夾")).toBeDefined();
    });
    expect(screen.getByText(/舊看板/)).toBeDefined();
    expect(screen.getByText(/舊資料夾/)).toBeDefined();
    // One restore button per trashed entry
    expect(screen.getAllByText("還原").length).toBe(2);
  });

  it("restoring a board calls the command, hides the row and notifies the sidebar", async () => {
    listTrashedBoards.mockResolvedValue([
      { id: "board-1", name: "舊看板", deletedAt: 1 },
    ]);
    listTrashedFolders.mockResolvedValue([]);
    restoreBoard.mockResolvedValue(undefined);

    const events: string[] = [];
    const listener = (e: Event) => events.push(e.type);
    window.addEventListener("paste-canvas://boards-changed", listener);

    render(<TrashView />);
    await waitFor(() => {
      expect(screen.getByText(/舊看板/)).toBeDefined();
    });

    fireEvent.click(screen.getByText("還原"));

    await waitFor(() => {
      expect(restoreBoard).toHaveBeenCalledWith("board-1");
    });
    await waitFor(() => {
      expect(screen.queryByText(/舊看板/)).toBeNull();
    });
    expect(events).toContain("paste-canvas://boards-changed");

    window.removeEventListener("paste-canvas://boards-changed", listener);
  });

  it("restoring a folder calls the folder command", async () => {
    listTrashedBoards.mockResolvedValue([]);
    listTrashedFolders.mockResolvedValue([
      { id: "folder-1", name: "舊資料夾", deletedAt: 1 },
    ]);
    restoreFolder.mockResolvedValue(undefined);

    render(<TrashView />);
    await waitFor(() => {
      expect(screen.getByText(/舊資料夾/)).toBeDefined();
    });

    fireEvent.click(screen.getByText("還原"));

    await waitFor(() => {
      expect(restoreFolder).toHaveBeenCalledWith("folder-1");
    });
  });

  it("shows the empty state when items, boards and folders are all empty", async () => {
    listTrashedBoards.mockResolvedValue([]);
    listTrashedFolders.mockResolvedValue([]);

    render(<TrashView />);

    await waitFor(() => {
      expect(screen.getByText("垃圾桶是空的")).toBeDefined();
    });
    expect(screen.queryByText("看板與資料夾")).toBeNull();
  });

  it("survives backend failures when listing trashed boards", async () => {
    listTrashedBoards.mockRejectedValue(new Error("no backend"));
    listTrashedFolders.mockRejectedValue(new Error("no backend"));

    render(<TrashView />);

    await waitFor(() => {
      expect(screen.getByText("垃圾桶是空的")).toBeDefined();
    });
  });
});
