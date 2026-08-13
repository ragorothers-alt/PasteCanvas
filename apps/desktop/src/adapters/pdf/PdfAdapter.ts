/**
 * PDF Adapter — metadata extraction and display utilities.
 * Per GOAL.md: "PDF rendering：PDF.js"
 * Per AGENTS.md: "adapters/pdf/" isolates PDF-specific logic.
 * Per AGENTS.md: "PDF viewer mounts only in focused detail/lightbox"
 *
 * This adapter provides:
 * - PDF metadata parsing (page count, title from filename)
 * - File size formatting
 * - PDF type detection from magic bytes
 */

export interface PdfMetadata {
  pageCount: number | null;
  title: string | null;
  fileName: string;
  fileSize: number;
  fileSizeFormatted: string;
}

/** PDF magic bytes: %PDF- */
const PDF_MAGIC = [0x25, 0x50, 0x44, 0x46, 0x2d]; // %PDF-

/** Checks if a byte array starts with PDF magic bytes. */
export function isPdfBytes(bytes: Uint8Array): boolean {
  if (bytes.length < 5) return false;
  return PDF_MAGIC.every((b, i) => bytes[i] === b);
}

/** Checks if a filename has a PDF extension. */
export function isPdfFilename(filename: string): boolean {
  return filename.toLowerCase().endsWith(".pdf");
}

/** Formats a byte size into a human-readable string. */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/** Extracts a display title from a PDF filename. */
export function titleFromFilename(filename: string): string {
  // Remove extension
  const withoutExt = filename.replace(/\.pdf$/i, "");
  // Replace common separators with spaces
  return withoutExt
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** Creates PdfMetadata from available information. */
export function createPdfMetadata(
  fileName: string,
  fileSize: number,
  pageCount: number | null = null,
): PdfMetadata {
  return {
    pageCount,
    title: titleFromFilename(fileName),
    fileName,
    fileSize,
    fileSizeFormatted: formatFileSize(fileSize),
  };
}

/**
 * Estimates page count from PDF file size (rough heuristic).
 * Average PDF page is ~50-100KB for text-heavy documents.
 * This is only used as a fallback when actual page count is unavailable.
 */
export function estimatePageCount(fileSize: number): number {
  const avgPageBytes = 75 * 1024; // 75KB average
  return Math.max(1, Math.round(fileSize / avgPageBytes));
}
