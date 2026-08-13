/**
 * canvasKeyboard — pure keyboard-movement computation for canvas nodes
 * (Phase-2 Workstream G: "keyboard movement/group/lock/fit/edit").
 * Kept free of React Flow types so it can be unit-tested in isolation.
 */

export interface KeyboardMovableNode {
  id: string;
  x: number;
  y: number;
  locked: boolean;
}

export interface NodeMove {
  nodeId: string;
  x: number;
  y: number;
}

export const KEYBOARD_MOVE_STEP = 10;
export const KEYBOARD_MOVE_STEP_FINE = 1;

const ARROW_STEPS: Record<string, readonly [number, number]> = {
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
};

/** Computes the new positions for arrow-key movement. Locked nodes never move.
 * Returns null when the key is not an arrow key or nothing can move. */
export function computeKeyboardMoves(
  nodes: readonly KeyboardMovableNode[],
  key: string,
  shiftKey: boolean,
): NodeMove[] | null {
  const step = ARROW_STEPS[key];
  if (!step) return null;
  const dist = shiftKey ? KEYBOARD_MOVE_STEP_FINE : KEYBOARD_MOVE_STEP;
  const moves = nodes
    .filter((n) => !n.locked)
    .map((n) => ({
      nodeId: n.id,
      x: n.x + step[0] * dist,
      y: n.y + step[1] * dist,
    }));
  return moves.length > 0 ? moves : null;
}
