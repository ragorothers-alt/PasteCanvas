/**
 * PdfPreview — renders the first page of a PDF item as a thumbnail.
 * Per AGENTS.md: "PDF viewer mounts only in focused detail/lightbox" —
 * this component is used in ItemDetail, not in list cards.
 * Per PRD: PDF 第一頁縮圖 (P1).
 */
import { useEffect, useRef, useState } from "react";
import { getItemAssetBase64 } from "./api";
import { base64ToUint8Array, renderFirstPage } from "../../adapters/pdf/PdfRenderer";

interface PdfPreviewProps {
  itemId: string;
  /** Max thumbnail width in CSS px. */
  maxWidth?: number;
}

type PreviewState = "loading" | "ready" | "error";

export function PdfPreview({ itemId, maxWidth = 320 }: PdfPreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [state, setState] = useState<PreviewState>("loading");
  const [pageCount, setPageCount] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    setState("loading");

    void (async () => {
      try {
        const asset = await getItemAssetBase64(itemId);
        if (cancelled) return;

        const data = base64ToUint8Array(asset.dataBase64);
        const canvas = canvasRef.current;
        if (!canvas) return;

        const result = await renderFirstPage(data, canvas, maxWidth);
        if (cancelled) return;

        setPageCount(result.pageCount);
        setState("ready");
      } catch {
        if (!cancelled) setState("error");
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [itemId, maxWidth]);

  return (
    <div className="pdf-preview">
      {state === "loading" && <div className="pdf-preview-loading">正在轉換 PDF 預覽…</div>}
      {state === "error" && (
        <div className="pdf-preview-error">無法預覽這份 PDF。</div>
      )}
      <canvas
        ref={canvasRef}
        className="pdf-preview-canvas"
        style={{ display: state === "ready" ? "block" : "none" }}
        aria-label="PDF 第一頁預覽"
      />
      {state === "ready" && pageCount != null && (
        <div className="pdf-preview-meta">{pageCount} 頁</div>
      )}
    </div>
  );
}
