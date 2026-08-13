/**
 * ImagePreview — bounded thumbnail preview for image/screenshot items.
 * Per PRD P1-CAP-007 / Workstream G: preview uses a bounded thumbnail
 * variant (server-side, cached), never the full-size asset.
 * Mounted only in the focused detail view (same rule as PdfPreview).
 */
import { useEffect, useState } from "react";
import { getItemThumbnail } from "./api";

interface ImagePreviewProps {
  itemId: string;
  /** Max thumbnail dimension requested from the backend. */
  maxDimension?: number;
}

type PreviewState = "loading" | "ready" | "error";

export function ImagePreview({ itemId, maxDimension = 480 }: ImagePreviewProps) {
  const [state, setState] = useState<PreviewState>("loading");
  const [dataUrl, setDataUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setState("loading");
    setDataUrl(null);

    void (async () => {
      try {
        const thumb = await getItemThumbnail(itemId, maxDimension);
        if (cancelled) return;
        setDataUrl(`data:${thumb.mimeType};base64,${thumb.dataBase64}`);
        setState("ready");
      } catch {
        if (!cancelled) setState("error");
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [itemId, maxDimension]);

  return (
    <div className="image-preview">
      {state === "loading" && <div className="image-preview-loading">Loading preview…</div>}
      {state === "error" && (
        <div className="image-preview-error">Unable to preview this image.</div>
      )}
      {state === "ready" && dataUrl && (
        <img
          className="image-preview-img"
          src={dataUrl}
          alt="Captured image preview"
          style={{ maxWidth: maxDimension, maxHeight: maxDimension }}
        />
      )}
    </div>
  );
}
