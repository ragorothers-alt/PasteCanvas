import { useState, useCallback } from "react";
import type { ItemSummary } from "./api";
import { editItem, revealItemAsset } from "./api";
import { CodeEditor } from "../../adapters/code/CodeEditor";
import { detectLanguage } from "../../adapters/code/CodeMirrorAdapter";
import { TagSelector } from "../tags/TagSelector";
import { PdfPreview } from "./PdfPreview";
import { ImagePreview } from "./ImagePreview";

// Item types that have a managed asset file on disk (INB-003 reveal)
const ASSET_TYPES = new Set(["image", "screenshot", "pdf", "font", "svg", "file"]);

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
        <button className="btn-close-detail" onClick={onClose}>✕</button>
        <span className="detail-type">{item.itemType}</span>
        {ASSET_TYPES.has(item.itemType) && (
          <button
            className="btn-reveal-asset"
            onClick={() => void handleReveal()}
            title="Reveal managed asset in file explorer"
          >
            📁 Reveal
          </button>
        )}
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
                {saving ? "Saving…" : "Save"}
              </button>
              <button onClick={() => { setEditing(false); setTitle(item.title ?? ""); }}>
                Cancel
              </button>
            </div>
            {error && <div className="detail-error">{error}</div>}
          </div>
        ) : (
          <h3
            className="detail-title"
            onClick={() => setEditing(true)}
            title="Click to edit title"
          >
            {item.title ?? item.textPreview ?? "(untitled)"}
          </h3>
        )}
      </div>

      {item.textPreview && item.itemType === "code" && (
        <div className="detail-content">
          <CodeEditor
            code={item.textPreview}
            language={detectLanguage(item.textPreview)}
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

      {item.textPreview && item.itemType !== "code" && item.itemType !== "pdf" && (
        <div className="detail-content">
          <pre className="detail-text">{item.textPreview}</pre>
        </div>
      )}

      <div className="detail-meta">
        <div className="meta-row">
          <span className="meta-label">Tags</span>
        </div>
        <TagSelector itemId={item.id} />
        {item.sourceApp && (
          <div className="meta-row">
            <span className="meta-label">Source</span>
            <span className="meta-value">{item.sourceApp}</span>
          </div>
        )}
        <div className="meta-row">
          <span className="meta-label">Method</span>
          <span className="meta-value">{item.captureMethod.replace(/_/g, " ")}</span>
        </div>
        <div className="meta-row">
          <span className="meta-label">Status</span>
          <span className="meta-value">{item.inboxStatus}</span>
        </div>
        <div className="meta-row">
          <span className="meta-label">Captured</span>
          <span className="meta-value">{new Date(item.createdAt).toLocaleString()}</span>
        </div>
      </div>
    </div>
  );
}
