/**
 * PDF first-page renderer using pdf.js (PRD: PDF 第一頁縮圖).
 * Per AGENTS.md: "PDF viewer mounts only in focused detail/lightbox" —
 * this renderer is invoked lazily by PdfPreview, not eagerly in list cards.
 * pdf.js itself (~1MB) is loaded on demand via dynamic import so it never
 * lands in the main bundle.
 */
// Vite ?url import gives a resolvable worker asset path (asset reference only)
import pdfWorkerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

let pdfjsPromise: Promise<typeof import("pdfjs-dist")> | null = null;

/** Loads pdf.js once, configuring the worker on first use. */
function loadPdfJs(): Promise<typeof import("pdfjs-dist")> {
  if (!pdfjsPromise) {
    pdfjsPromise = import("pdfjs-dist").then((pdfjsLib) => {
      pdfjsLib.GlobalWorkerOptions.workerSrc = pdfWorkerUrl;
      return pdfjsLib;
    });
  }
  return pdfjsPromise;
}

/** Decodes a base64 string to a Uint8Array (for pdf.js input). */
export function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export interface RenderResult {
  pageCount: number;
  width: number;
  height: number;
}

/**
 * Renders the first page of a PDF (as Uint8Array) onto a canvas.
 * Scales to fit maxWidth while preserving aspect ratio.
 * Returns page count and rendered dimensions.
 */
export async function renderFirstPage(
  data: Uint8Array,
  canvas: HTMLCanvasElement,
  maxWidth: number,
): Promise<RenderResult> {
  const pdfjsLib = await loadPdfJs();
  const loadingTask = pdfjsLib.getDocument({ data });
  const pdf = await loadingTask.promise;
  const pageCount = pdf.numPages;

  const page = await pdf.getPage(1);
  const unscaled = page.getViewport({ scale: 1 });
  const scale = maxWidth / unscaled.width;
  const viewport = page.getViewport({ scale });

  canvas.width = Math.floor(viewport.width);
  canvas.height = Math.floor(viewport.height);

  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("failed to get 2d context");
  }

  await page.render({ canvasContext: ctx, viewport }).promise;
  await pdf.destroy();

  return {
    pageCount,
    width: canvas.width,
    height: canvas.height,
  };
}
