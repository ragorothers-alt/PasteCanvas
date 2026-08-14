import { memo, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Camera, FileCode, FilePdf, Image, LinkSimple, LockSimple, LockSimpleOpen, Note } from "@phosphor-icons/react";
import { Handle, NodeResizer, Position, useReactFlow } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import type { CanvasNode, ItemCardData } from "../../adapters/canvas/CanvasMapper";
import { getItemThumbnail } from "../inbox/api";
import { resizeNodes, setNodeLocked } from "./api";
import { useUndoStore, createResizeCommand } from "../../stores/undoStore";

function typeIcon(itemType: string | null): ReactNode {
  switch (itemType) {
    case "link": return <LinkSimple aria-hidden="true" />;
    case "image": return <Image aria-hidden="true" />;
    case "screenshot": return <Camera aria-hidden="true" />;
    case "pdf": return <FilePdf aria-hidden="true" />;
    case "code": return <FileCode aria-hidden="true" />;
    default: return <Note aria-hidden="true" />;
  }
}

function typeLabel(itemType: string | null): string {
  const labels: Record<string, string> = {
    plain_text: "文字", link: "連結", image: "圖片", screenshot: "螢幕截圖",
    pdf: "PDF", code: "程式碼", color: "色彩", font: "字型", file: "檔案", svg: "SVG",
  };
  return labels[itemType ?? ""] ?? "筆記";
}

const THUMB_TYPES = new Set(["image", "screenshot"]);

/** Bounded thumbnail for image/screenshot cards (P1-CAP-007). Loads lazily;
 * failures fall back to the type icon silently. */
function CardThumbnail({ itemId }: { itemId: string }) {
  const [dataUrl, setDataUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getItemThumbnail(itemId, 240)
      .then((thumb) => {
        if (!cancelled) {
          setDataUrl(`data:${thumb.mimeType};base64,${thumb.dataBase64}`);
        }
      })
      .catch(() => {
        /* thumbnail unavailable — icon fallback stays */
      });
    return () => {
      cancelled = true;
    };
  }, [itemId]);

  if (!dataUrl) return null;
  return <img className="card-thumb-img" src={dataUrl} alt="" loading="lazy" />;
}

export const ItemCardNode = memo(function ItemCardNode({ id, data, selected }: NodeProps) {
  const d = data as ItemCardData;
  const { setNodes, getNode } = useReactFlow<CanvasNode>();
  const { push } = useUndoStore();
  const originalSize = useRef<{ width: number; height: number } | null>(null);
  const title = d.titleOverride ?? d.itemTitle ?? d.itemTextPreview ?? "（空白）";
  const showThumb = THUMB_TYPES.has(d.itemType ?? "") && d.itemId != null;
  const locked = d.locked === true;

  // Workstream F: capture the pre-resize size so the gesture is undoable.
  const handleResizeStart = () => {
    const node = getNode(id);
    originalSize.current = {
      width: Number(node?.style?.width ?? 0),
      height: Number(node?.style?.height ?? 0),
    };
  };

  // BRD-010: persist the final size once the resize gesture ends
  // (one undo history entry per completed interaction).
  const handleResizeEnd = (
    _event: unknown,
    params: { width: number; height: number },
  ) => {
    if (!d.boardId) return;
    const resizes = [{ nodeId: id, width: params.width, height: params.height }];
    void resizeNodes(d.boardId, resizes);
    const prev = originalSize.current;
    if (prev && prev.width > 0 && prev.height > 0) {
      push(
        createResizeCommand(d.boardId, resizes, [
          { nodeId: id, width: prev.width, height: prev.height },
        ]),
      );
    }
    originalSize.current = null;
  };

  // BRD-010: lock toggle — locked nodes are not draggable/resizable.
  const toggleLock = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!d.boardId) return;
    const next = !locked;
    void setNodeLocked(d.boardId, id, next);
    setNodes((ns) =>
      ns.map((n) =>
        n.id === id
          ? { ...n, draggable: !next, data: { ...n.data, locked: next } }
          : n,
      ),
    );
  };

  return (
    <div className={`canvas-item-card${locked ? " locked" : ""}`}>
      <NodeResizer
        isVisible={!locked && Boolean(selected)}
        minWidth={120}
        minHeight={80}
        onResizeStart={handleResizeStart}
        onResizeEnd={handleResizeEnd}
      />
      <Handle type="target" position={Position.Top} className="card-handle" />
      <div className="card-header">
        <span className="card-icon">{typeIcon(d.itemType)}</span>
        <span className="card-type">{typeLabel(d.itemType)}</span>
        <button
          className="card-lock-btn"
          title={locked ? "解除鎖定節點" : "鎖定節點"}
          aria-label={locked ? "解除鎖定節點" : "鎖定節點"}
          onClick={toggleLock}
        >
          {locked ? <LockSimple aria-hidden="true" /> : <LockSimpleOpen aria-hidden="true" />}
        </button>
      </div>
      {showThumb && <CardThumbnail itemId={d.itemId as string} />}
      <div className="card-title">{title}</div>
      {d.itemTextPreview && d.itemType !== "plain_text" && !showThumb && (
        <div className="card-preview">{d.itemTextPreview}</div>
      )}
      <Handle type="source" position={Position.Bottom} className="card-handle" />
    </div>
  );
});
