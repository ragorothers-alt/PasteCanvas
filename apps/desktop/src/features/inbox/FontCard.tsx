interface FontCardProps {
  fileName: string;
  fileSize: number;
  format?: string;
  familyName?: string;
}

/**
 * FontCard — displays font file metadata in the Inbox.
 * Per GOAL.md P1: "字型檔預覽" — shows metadata without installing.
 */
export function FontCard({ fileName, fileSize, format, familyName }: FontCardProps) {
  const displayName = familyName ?? cleanFontName(fileName);
  const sizeFormatted = formatSize(fileSize);

  return (
    <div className="font-card">
      <div className="font-preview" aria-hidden="true">
        <span className="font-preview-text">Aa</span>
      </div>
      <div className="font-info">
        <div className="font-name">{displayName}</div>
        <div className="font-meta">
          {format && <span className="font-format">{format}</span>}
          <span>{sizeFormatted}</span>
          <span className="font-ext">{getExtension(fileName)}</span>
        </div>
      </div>
    </div>
  );
}

function cleanFontName(filename: string): string {
  const withoutExt = filename.replace(/\.(ttf|otf|woff2?|ttc)$/i, "");
  return withoutExt.replace(/[-_]+/g, " ").trim();
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getExtension(filename: string): string {
  const ext = filename.split(".").pop()?.toUpperCase() ?? "";
  return ext;
}
