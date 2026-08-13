/**
 * TrashView — lists soft-deleted items with restore + empty trash (DAT-003).
 * Per PRD DAT-003: "清空垃圾桶前顯示影響數量與資產大小" — the empty action
 * shows a confirmation with item count + asset size before permanent deletion.
 */
import { useState, useEffect, useCallback } from "react";
import { queryTrash, restoreItem, getTrashStats, emptyTrash } from "./api";
import type { ItemSummary, TrashStats } from "./api";
import { InboxItem } from "./InboxItem";
import { toggleFavorite, togglePin } from "./api";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function TrashView() {
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [stats, setStats] = useState<TrashStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [confirming, setConfirming] = useState(false);
  const [emptying, setEmptying] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [trashItems, trashStats] = await Promise.all([queryTrash(), getTrashStats()]);
      setItems(trashItems);
      setStats(trashStats);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleRestore = useCallback(
    async (itemId: string) => {
      try {
        await restoreItem(itemId);
        setItems((prev) => prev.filter((i) => i.id !== itemId));
        void getTrashStats().then(setStats).catch(() => undefined);
      } catch {
        // ignore
      }
    },
    [],
  );

  const handleEmptyTrash = useCallback(async () => {
    setEmptying(true);
    setResult(null);
    try {
      const res = await emptyTrash();
      setResult(
        `Permanently deleted ${res.deletedItems} item${res.deletedItems === 1 ? "" : "s"} and ${res.deletedAssets} asset file${res.deletedAssets === 1 ? "" : "s"}.`,
      );
      setConfirming(false);
      void load();
    } catch {
      setResult("Failed to empty trash.");
      setConfirming(false);
    } finally {
      setEmptying(false);
    }
  }, [load]);

  // For InboxItem callbacks in trash context (favorite/pin still work; trash = no-op here)
  const noop = useCallback(async () => undefined, []);

  return (
    <div className="inbox-view">
      <header className="inbox-header">
        <h2>Trash</h2>
        <span className="inbox-count">{items.length} items</span>
        {stats && stats.itemCount > 0 && (
          <span className="trash-size">{formatBytes(stats.totalBytes)}</span>
        )}
        {stats && stats.itemCount > 0 && (
          <button className="btn-empty-trash" onClick={() => setConfirming(true)}>
            Empty Trash
          </button>
        )}
      </header>

      {result && <div className="trash-result">{result}</div>}

      {/* DAT-003 confirmation dialog with impact stats */}
      {confirming && stats && (
        <div className="trash-confirm-overlay" role="dialog" aria-modal="true">
          <div className="trash-confirm">
            <h3>Empty Trash?</h3>
            <p>
              This will permanently delete <strong>{stats.itemCount}</strong> item
              {stats.itemCount === 1 ? "" : "s"} and free{" "}
              <strong>{formatBytes(stats.totalBytes)}</strong>. This cannot be undone.
            </p>
            <div className="trash-confirm-actions">
              <button
                className="btn-cancel"
                onClick={() => setConfirming(false)}
                disabled={emptying}
              >
                Cancel
              </button>
              <button
                className="btn-confirm-empty"
                onClick={() => void handleEmptyTrash()}
                disabled={emptying}
              >
                {emptying ? "Emptying…" : "Empty Trash"}
              </button>
            </div>
          </div>
        </div>
      )}

      {loading && <div className="inbox-loading">Loading…</div>}

      {!loading && items.length === 0 && (
        <div className="inbox-empty">
          <p>Trash is empty.</p>
        </div>
      )}

      {!loading && items.length > 0 && (
        <ul className="inbox-list">
          {items.map((item) => (
            <div key={item.id} className="trash-item-row">
              <InboxItem
                item={item}
                onToggleFavorite={() => void toggleFavorite(item.id).then(() => void load())}
                onTogglePin={() => void togglePin(item.id).then(() => void load())}
                onTrash={() => void noop()}
              />
              <button
                className="btn-restore"
                onClick={() => void handleRestore(item.id)}
                title="Restore to Inbox"
              >
                ↩ Restore
              </button>
            </div>
          ))}
        </ul>
      )}
    </div>
  );
}
