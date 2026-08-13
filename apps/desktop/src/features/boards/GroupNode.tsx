import { memo } from "react";
import { useReactFlow } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import type { CanvasNode, ItemCardData } from "../../adapters/canvas/CanvasMapper";
import { setNodeCollapsed } from "./api";

/** BRD-010 (Workstream D): group frame with title and collapse toggle.
 * Collapsed groups hide their children — BoardView filters them out of the
 * rendered node set while the collapsed flag persists server-side. */
export const GroupNode = memo(function GroupNode({ id, data }: NodeProps) {
  const d = data as ItemCardData;
  const { setNodes } = useReactFlow<CanvasNode>();
  const collapsed = d.collapsed === true;

  const toggle = () => {
    if (!d.boardId) return;
    const next = !collapsed;
    void setNodeCollapsed(d.boardId, id, next);
    setNodes((ns) =>
      ns.map((n) =>
        n.id === id ? { ...n, data: { ...n.data, collapsed: next } } : n,
      ),
    );
  };

  return (
    <div className="canvas-group">
      <div className="group-header">
        <button
          className="group-collapse-btn"
          title={collapsed ? "Expand group" : "Collapse group"}
          onClick={toggle}
        >
          {collapsed ? "▸" : "▾"}
        </button>
        <span className="group-title">{d.titleOverride ?? "Group"}</span>
      </div>
    </div>
  );
});
