import { formatFileSize, titleFromFilename } from "../../adapters/pdf/PdfAdapter";

interface PdfCardProps {
  fileName: string;
  fileSize: number;
  pageCount?: number | null;
}

/**
 * PdfCard — displays PDF item metadata in the Inbox.
 * Per AGENTS.md: "PDF viewer mounts only in focused detail/lightbox"
 * This card shows metadata only; actual PDF rendering is deferred.
 */
export function PdfCard({ fileName, fileSize, pageCount }: PdfCardProps) {
  const title = titleFromFilename(fileName);

  return (
    <div className="pdf-card">
      <div className="pdf-icon" aria-hidden="true">📄</div>
      <div className="pdf-info">
        <div className="pdf-title">{title}</div>
        <div className="pdf-meta">
          <span>{formatFileSize(fileSize)}</span>
          {pageCount != null && <span>{pageCount} pages</span>}
          <span className="pdf-ext">PDF</span>
        </div>
      </div>
    </div>
  );
}
