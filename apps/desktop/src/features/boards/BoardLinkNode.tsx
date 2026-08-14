/**
 * BoardLinkNode — renders a nested board entry on the canvas (BRD-004).
 * Double-click navigates into the target board (handled by BoardView).
 */
import { memo } from "react";
import { Kanban } from "@phosphor-icons/react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import type { ItemCardData } from "../../adapters/canvas/CanvasMapper";

export const BoardLinkNode = memo(function BoardLinkNode({ data }: NodeProps) {
  const d = data as ItemCardData;
  const title = d.titleOverride ?? "子看板";

  return (
    <div className="canvas-board-link" title="雙擊開啟">
      <Handle type="target" position={Position.Top} className="card-handle" />
      <div className="board-link-icon" aria-hidden="true"><Kanban /></div>
      <div className="board-link-title">{title}</div>
      <div className="board-link-hint">雙擊開啟</div>
      <Handle type="source" position={Position.Bottom} className="card-handle" />
    </div>
  );
});
