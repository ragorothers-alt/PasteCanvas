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
import { ArrowCounterClockwise, Trash } from "@phosphor-icons/react";

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
        `已永久刪除 ${res.deletedItems} 個項目與 ${res.deletedAssets} 個資產檔案。`,
      );
      setConfirming(false);
      void load();
    } catch {
      setResult("清空垃圾桶失敗。");
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
        <h2>垃圾桶</h2>
        <span className="inbox-count">{items.length} 個項目</span>
        {stats && stats.itemCount > 0 && (
          <span className="trash-size">{formatBytes(stats.totalBytes)}</span>
        )}
        {stats && stats.itemCount > 0 && (
          <button className="btn-empty-trash" onClick={() => setConfirming(true)}>
            <Trash aria-hidden="true" /> 清空垃圾桶
          </button>
        )}
      </header>

      {result && <div className="trash-result">{result}</div>}

      {/* DAT-003 confirmation dialog with impact stats */}
      {confirming && stats && (
        <div className="trash-confirm-overlay" role="dialog" aria-modal="true">
          <div className="trash-confirm">
            <h3>要清空垃圾桶嗎？</h3>
            <p>
              這會永久刪除 <strong>{stats.itemCount}</strong> 個項目並釋放 <strong>{formatBytes(stats.totalBytes)}</strong>。此動作無法復原。
            </p>
            <div className="trash-confirm-actions">
              <button
                className="btn-cancel"
                onClick={() => setConfirming(false)}
                disabled={emptying}
              >
                取消
              </button>
              <button
                className="btn-confirm-empty"
                onClick={() => void handleEmptyTrash()}
                disabled={emptying}
              >
                {emptying ? "清空中…" : "清空垃圾桶"}
              </button>
            </div>
          </div>
        </div>
      )}

      {loading && <div className="inbox-loading">載入中…</div>}

      {!loading && items.length === 0 && (
        <div className="inbox-empty">
          <p>垃圾桶是空的。</p>
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
                title="還原到收件匣"
              >
                <ArrowCounterClockwise aria-hidden="true" /> 還原
              </button>
            </div>
          ))}
        </ul>
      )}
    </div>
  );
}
