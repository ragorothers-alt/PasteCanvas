import { useState, useCallback, useEffect } from "react";
import type { ItemSummary } from "./api";
import { editItem, getItemContent, revealItemAsset } from "./api";
import { CodeEditor } from "../../adapters/code/CodeEditor";
import { detectLanguage } from "../../adapters/code/CodeMirrorAdapter";
import { TagSelector } from "../tags/TagSelector";
import { PdfPreview } from "./PdfPreview";
import { ImagePreview } from "./ImagePreview";
import { ArrowSquareOut, Copy, FloppyDisk, X } from "@phosphor-icons/react";

// Item types that have a managed asset file on disk (INB-003 reveal)
const ASSET_TYPES = new Set(["image", "screenshot", "pdf", "font", "svg", "file"]);
const ITEM_TYPE_LABELS: Record<string, string> = { plain_text: "文字", link: "連結", image: "圖片", screenshot: "螢幕截圖", pdf: "PDF", code: "程式碼", color: "色彩", font: "字型", file: "檔案", svg: "SVG" };

interface ItemDetailProps {
  item: ItemSummary;
  onClose: () => void;
  onUpdated: () => void;
}

export function ItemDetail({ item, onClose, onUpdated }: ItemDetailProps) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(item.title ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [fullText, setFullText] = useState<string | null>(item.textPreview);
  const [copyMsg, setCopyMsg] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setFullText(item.textPreview);
    void getItemContent(item.id)
      .then((content) => {
        if (cancelled) return;
        if (content.plainText != null) setFullText(content.plainText);
        if (content.title && !item.title) setTitle(content.title);
      })
      .catch(() => {
        /* keep preview */
      });
    return () => {
      cancelled = true;
    };
  }, [item.id, item.textPreview, item.title]);

  const handleCopy = useCallback(async () => {
    const text = fullText ?? item.displayUrl ?? item.title ?? "";
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopyMsg("已複製");
      setTimeout(() => setCopyMsg(null), 1500);
    } catch {
      setCopyMsg("複製失敗");
    }
  }, [fullText, item.displayUrl, item.title]);

  const handleReveal = useCallback(async () => {
    try {
      await revealItemAsset(item.id);
    } catch {
      // Item has no asset or file missing — ignore
    }
  }, [item.id]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setError(null);
    try {
      await editItem(item.id, item.revision, { title: title.trim() || undefined });
      setEditing(false);
      onUpdated();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }, [item.id, item.revision, title, onUpdated]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void handleSave();
      }
      if (e.key === "Escape") {
        setEditing(false);
        setTitle(item.title ?? "");
      }
    },
    [handleSave, item.title],
  );

  return (
    <div className="item-detail">
      <div className="detail-header">
        <button className="btn-close-detail" onClick={onClose} aria-label="關閉詳細資料"><X aria-hidden="true" /></button>
        <span className="detail-type">{ITEM_TYPE_LABELS[item.itemType] ?? item.itemType}</span>
        {ASSET_TYPES.has(item.itemType) && (
          <button
            className="btn-reveal-asset"
            onClick={() => void handleReveal()}
            title="在檔案總管顯示資產"
          >
            <ArrowSquareOut aria-hidden="true" /> 顯示檔案
          </button>
        )}
        <button className="btn-reveal-asset" onClick={() => void handleCopy()} title="複製內容">
          <Copy aria-hidden="true" /> {copyMsg ?? "複製"}
        </button>
      </div>

      <div className="detail-title-section">
        {editing ? (
          <div className="detail-edit">
            <input
              className="detail-title-input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={handleKeyDown}
              autoFocus
              disabled={saving}
            />
            <div className="detail-edit-actions">
              <button onClick={() => void handleSave()} disabled={saving}>
                {saving ? "儲存中…" : <><FloppyDisk aria-hidden="true" /> 儲存</>}
              </button>
              <button onClick={() => { setEditing(false); setTitle(item.title ?? ""); }}>
                取消
              </button>
            </div>
            {error && <div className="detail-error">{error}</div>}
          </div>
        ) : (
          <h3
            className="detail-title"
            onClick={() => setEditing(true)}
            title="點選即可編輯標題"
          >
            {item.title ?? item.textPreview ?? "(未命名)"}
          </h3>
        )}
      </div>

      {(fullText || item.textPreview) && item.itemType === "code" && (
        <div className="detail-content">
          <CodeEditor
            code={fullText ?? item.textPreview ?? ""}
            language={detectLanguage(fullText ?? item.textPreview ?? "")}
            editable={false}
          />
        </div>
      )}

      {/* PDF first-page preview (PRD P1) — mounted only in focused detail view */}
      {item.itemType === "pdf" && (
        <div className="detail-content">
          <PdfPreview itemId={item.id} />
        </div>
      )}

      {/* Image/screenshot bounded thumbnail preview (P1-CAP-007) */}
      {(item.itemType === "image" || item.itemType === "screenshot") && (
        <div className="detail-content">
          <ImagePreview itemId={item.id} />
        </div>
      )}

      {(fullText || item.textPreview) && item.itemType !== "code" && item.itemType !== "pdf" && (
        <div className="detail-content">
          <pre className="detail-text">{fullText ?? item.textPreview}</pre>
        </div>
      )}

      <div className="detail-meta">
        <div className="meta-row">
          <span className="meta-label">標籤</span>
        </div>
        <TagSelector itemId={item.id} />
        {item.sourceApp && (
          <div className="meta-row">
            <span className="meta-label">來源</span>
            <span className="meta-value">{item.sourceApp}</span>
          </div>
        )}
        <div className="meta-row">
          <span className="meta-label">方式</span>
            <span className="meta-value">{item.captureMethod.replace(/_/g, " ")}</span>
        </div>
        <div className="meta-row">
          <span className="meta-label">狀態</span>
          <span className="meta-value">{item.inboxStatus}</span>
        </div>
        <div className="meta-row">
          <span className="meta-label">擷取時間</span>
          <span className="meta-value">{new Date(item.createdAt).toLocaleString()}</span>
        </div>
      </div>
    </div>
  );
}
