import { describe, expect, it } from "vitest";
import {
  KEYBOARD_MOVE_STEP,
  KEYBOARD_MOVE_STEP_FINE,
  computeKeyboardMoves,
} from "../adapters/canvas/canvasKeyboard";

describe("computeKeyboardMoves (Workstream G keyboard movement)", () => {
  const nodes = [
    { id: "a", x: 100, y: 100, locked: false },
    { id: "b", x: 200, y: 50, locked: false },
  ];

  it("moves all nodes by 10px on arrow keys", () => {
    const moves = computeKeyboardMoves(nodes, "ArrowRight", false);
    expect(moves).toEqual([
      { nodeId: "a", x: 110, y: 100 },
      { nodeId: "b", x: 210, y: 50 },
    ]);
  });

  it("moves up/down on vertical arrows", () => {
    expect(computeKeyboardMoves(nodes, "ArrowUp", false)![0]!.y).toBe(
      100 - KEYBOARD_MOVE_STEP,
    );
    expect(computeKeyboardMoves(nodes, "ArrowDown", false)![1]!.y).toBe(
      50 + KEYBOARD_MOVE_STEP,
    );
  });

  it("shift enables fine 1px movement", () => {
    const moves = computeKeyboardMoves(nodes, "ArrowLeft", true);
    expect(moves![0]!.x).toBe(100 - KEYBOARD_MOVE_STEP_FINE);
  });

  it("never moves locked nodes", () => {
    const mixed = [
      { id: "a", x: 0, y: 0, locked: true },
      { id: "b", x: 0, y: 0, locked: false },
    ];
    const moves = computeKeyboardMoves(mixed, "ArrowDown", false);
    expect(moves).toHaveLength(1);
    expect(moves![0]!.nodeId).toBe("b");
  });

  it("returns null when all selected nodes are locked", () => {
    const locked = [{ id: "a", x: 0, y: 0, locked: true }];
    expect(computeKeyboardMoves(locked, "ArrowUp", false)).toBeNull();
  });

  it("returns null for non-arrow keys and empty input", () => {
    expect(computeKeyboardMoves(nodes, "Enter", false)).toBeNull();
    expect(computeKeyboardMoves([], "ArrowUp", false)).toBeNull();
  });
});
