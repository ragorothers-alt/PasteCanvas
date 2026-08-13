/**
 * BoardLinkNode — renders a nested board entry on the canvas (BRD-004).
 * Double-click navigates into the target board (handled by BoardView).
 */
import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import type { ItemCardData } from "../../adapters/canvas/CanvasMapper";

export const BoardLinkNode = memo(function BoardLinkNode({ data }: NodeProps) {
  const d = data as ItemCardData;
  const title = d.titleOverride ?? "Sub-board";

  return (
    <div className="canvas-board-link" title="Double-click to open">
      <Handle type="target" position={Position.Top} className="card-handle" />
      <div className="board-link-icon" aria-hidden="true">🗂</div>
      <div className="board-link-title">{title}</div>
      <div className="board-link-hint">double-click to open</div>
      <Handle type="source" position={Position.Bottom} className="card-handle" />
    </div>
  );
});
