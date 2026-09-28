import { describe, it, expect, beforeEach, vi } from "vitest";
import { useUndoStore, createMoveCommand, createResizeCommand, createRemoveNodesCommand } from "../stores/undoStore";

// Mock the boards API
vi.mock("../features/boards/api", () => ({
  moveNodes: vi.fn().mockResolvedValue(undefined),
  removeNode: vi.fn().mockResolvedValue(undefined),
  placeItemOnBoard: vi.fn().mockResolvedValue("node-1"),
  groupNodes: vi.fn().mockResolvedValue("group-1"),
  ungroupNodes: vi.fn().mockResolvedValue(undefined),
  resizeNodes: vi.fn().mockResolvedValue(undefined),
}));

describe("useUndoStore", () => {
  beforeEach(() => {
    // Reset store state
    useUndoStore.setState({ history: [], pointer: 0, boardId: null });
  });

  it("starts with empty history", () => {
    const state = useUndoStore.getState();
    expect(state.history).toHaveLength(0);
    expect(state.pointer).toBe(0);
    expect(state.canUndo()).toBe(false);
    expect(state.canRedo()).toBe(false);
  });

  it("push adds command and enables undo", () => {
    const cmd = createMoveCommand(
      "board-1",
      [{ nodeId: "n1", x: 100, y: 100 }],
      [{ nodeId: "n1", x: 0, y: 0 }],
    );

    useUndoStore.getState().push(cmd);

    const state = useUndoStore.getState();
    expect(state.history).toHaveLength(1);
    expect(state.pointer).toBe(1);
    expect(state.canUndo()).toBe(true);
    expect(state.canRedo()).toBe(false);
  });

  it("undo decrements pointer", async () => {
    const cmd = createMoveCommand(
      "board-1",
      [{ nodeId: "n1", x: 100, y: 100 }],
      [{ nodeId: "n1", x: 0, y: 0 }],
    );

    useUndoStore.getState().push(cmd);
    await useUndoStore.getState().undo();

    const state = useUndoStore.getState();
    expect(state.pointer).toBe(0);
    expect(state.canUndo()).toBe(false);
    expect(state.canRedo()).toBe(true);
  });

  it("redo increments pointer", async () => {
    const cmd = createMoveCommand(
      "board-1",
      [{ nodeId: "n1", x: 100, y: 100 }],
      [{ nodeId: "n1", x: 0, y: 0 }],
    );

    useUndoStore.getState().push(cmd);
    await useUndoStore.getState().undo();
    await useUndoStore.getState().redo();

    const state = useUndoStore.getState();
    expect(state.pointer).toBe(1);
    expect(state.canUndo()).toBe(true);
    expect(state.canRedo()).toBe(false);
  });

  it("push after undo truncates redo history", async () => {
    const cmd1 = createMoveCommand("b", [{ nodeId: "n1", x: 1, y: 1 }], [{ nodeId: "n1", x: 0, y: 0 }]);
    const cmd2 = createMoveCommand("b", [{ nodeId: "n2", x: 2, y: 2 }], [{ nodeId: "n2", x: 0, y: 0 }]);
    const cmd3 = createMoveCommand("b", [{ nodeId: "n3", x: 3, y: 3 }], [{ nodeId: "n3", x: 0, y: 0 }]);

    const store = useUndoStore.getState();
    store.push(cmd1);
    store.push(cmd2);

    // Undo one (await since it's async)
    await useUndoStore.getState().undo();

    // Push new command (should truncate cmd2)
    useUndoStore.getState().push(cmd3);

    const state = useUndoStore.getState();
    expect(state.history).toHaveLength(2); // cmd1 + cmd3
    expect(state.pointer).toBe(2);
    expect(state.canRedo()).toBe(false);
  });

  it("respects max history bound of 50", () => {
    const store = useUndoStore.getState();
    for (let i = 0; i < 60; i++) {
      store.push(createMoveCommand("b", [{ nodeId: `n${i}`, x: i, y: i }], [{ nodeId: `n${i}`, x: 0, y: 0 }]));
    }

    const state = useUndoStore.getState();
    expect(state.history.length).toBeLessThanOrEqual(50);
  });

  it("setBoard clears history", () => {
    const store = useUndoStore.getState();
    store.push(createMoveCommand("b", [{ nodeId: "n1", x: 1, y: 1 }], [{ nodeId: "n1", x: 0, y: 0 }]));

    useUndoStore.getState().setBoard("new-board");

    const state = useUndoStore.getState();
    expect(state.history).toHaveLength(0);
    expect(state.pointer).toBe(0);
    expect(state.boardId).toBe("new-board");
  });

  it("undo with empty history is no-op", async () => {
    await useUndoStore.getState().undo();
    expect(useUndoStore.getState().pointer).toBe(0);
  });

  it("redo with no redo history is no-op", async () => {
    await useUndoStore.getState().redo();
    expect(useUndoStore.getState().pointer).toBe(0);
  });
});

describe("createMoveCommand", () => {
  it("creates command with correct description", () => {
    const cmd = createMoveCommand(
      "board-1",
      [{ nodeId: "n1", x: 100, y: 200 }, { nodeId: "n2", x: 50, y: 50 }],
      [{ nodeId: "n1", x: 0, y: 0 }, { nodeId: "n2", x: 0, y: 0 }],
    );
    expect(cmd.description).toBe("Move 2 node(s)");
  });
});

describe("createRemoveNodesCommand (batch remove)", () => {
  it("undo re-places item nodes and redo removes the live ids", async () => {
    const { placeItemOnBoard, removeNode } = await import("../features/boards/api");
    vi.mocked(placeItemOnBoard)
      .mockResolvedValueOnce("new-n1")
      .mockResolvedValueOnce("new-n2");
    vi.mocked(removeNode).mockClear();

    const cmd = createRemoveNodesCommand("board-1", [
      { nodeId: "n1", itemId: "item-1", x: 10, y: 20 },
      { nodeId: "n2", itemId: "item-2", x: 30, y: 40 },
      { nodeId: "g1", itemId: null, x: 0, y: 0 },
    ]);
    expect(cmd.description).toBe("Remove 3 node(s)");

    await cmd.undo();
    expect(placeItemOnBoard).toHaveBeenCalledWith("board-1", "item-1", 10, 20);
    expect(placeItemOnBoard).toHaveBeenCalledWith("board-1", "item-2", 30, 40);
    // Group node (no item) is not re-placed via API
    expect(vi.mocked(placeItemOnBoard)).toHaveBeenCalledTimes(2);

    // Redo removes the NEW ids from undo, not the stale pre-undo ids
    await cmd.redo();
    expect(removeNode).toHaveBeenCalledWith("board-1", "new-n1");
    expect(removeNode).toHaveBeenCalledWith("board-1", "new-n2");
    expect(removeNode).toHaveBeenCalledWith("board-1", "g1");
  });
});

describe("createResizeCommand (Workstream F)", () => {
  it("undo restores previous sizes, redo re-applies new sizes", async () => {
    const { resizeNodes } = await import("../features/boards/api");
    const cmd = createResizeCommand(
      "board-1",
      [{ nodeId: "n1", width: 500, height: 320 }],
      [{ nodeId: "n1", width: 280, height: 180 }],
    );
    expect(cmd.description).toBe("Resize 1 node(s)");

    await cmd.undo();
    expect(resizeNodes).toHaveBeenCalledWith("board-1", [
      { nodeId: "n1", width: 280, height: 180 },
    ]);

    await cmd.redo();
    expect(resizeNodes).toHaveBeenCalledWith("board-1", [
      { nodeId: "n1", width: 500, height: 320 },
    ]);
  });

  it("integrates with store push/undo/redo", async () => {
    const { resizeNodes } = await import("../features/boards/api");
    vi.mocked(resizeNodes).mockClear();
    useUndoStore.getState().setBoard("board-r");
    useUndoStore.getState().push(
      createResizeCommand(
        "board-r",
        [{ nodeId: "n1", width: 400, height: 400 }],
        [{ nodeId: "n1", width: 280, height: 180 }],
      ),
    );

    await useUndoStore.getState().undo();
    expect(useUndoStore.getState().pointer).toBe(0);
    expect(resizeNodes).toHaveBeenLastCalledWith("board-r", [
      { nodeId: "n1", width: 280, height: 180 },
    ]);

    await useUndoStore.getState().redo();
    expect(useUndoStore.getState().pointer).toBe(1);
    expect(resizeNodes).toHaveBeenLastCalledWith("board-r", [
      { nodeId: "n1", width: 400, height: 400 },
    ]);
  });
});
