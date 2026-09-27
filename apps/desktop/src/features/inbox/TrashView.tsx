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
import {
  listTrashedBoards,
  listTrashedFolders,
  restoreBoard,
  restoreFolder,
  BOARDS_CHANGED_EVENT,
} from "../boards/api";
import type { TrashedEntry } from "../boards/api";
import { ArrowCounterClockwise, Trash } from "@phosphor-icons/react";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function TrashView() {
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [stats, setStats] = useState<TrashStats | null>(null);
  const [trashedBoards, setTrashedBoards] = useState<TrashedEntry[]>([]);
  const [trashedFolders, setTrashedFolders] = useState<TrashedEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [confirming, setConfirming] = useState(false);
  const [emptying, setEmptying] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [trashItems, trashStats, boards, folders] = await Promise.all([
        queryTrash(),
        getTrashStats(),
        listTrashedBoards().catch(() => [] as TrashedEntry[]),
        listTrashedFolders().catch(() => [] as TrashedEntry[]),
      ]);
      setItems(trashItems);
      setStats(trashStats);
      setTrashedBoards(boards);
      setTrashedFolders(folders);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const notifyBoardsChanged = useCallback(() => {
    window.dispatchEvent(new Event(BOARDS_CHANGED_EVENT));
  }, []);

  const handleRestoreBoard = useCallback(
    async (boardId: string) => {
      try {
        await restoreBoard(boardId);
        setTrashedBoards((prev) => prev.filter((b) => b.id !== boardId));
        notifyBoardsChanged();
      } catch {
        // ignore
      }
    },
    [notifyBoardsChanged],
  );

  const handleRestoreFolder = useCallback(
    async (folderId: string) => {
      try {
        await restoreFolder(folderId);
        setTrashedFolders((prev) => prev.filter((f) => f.id !== folderId));
        notifyBoardsChanged();
      } catch {
        // ignore
      }
    },
    [notifyBoardsChanged],
  );

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

      {!loading && (trashedBoards.length > 0 || trashedFolders.length > 0) && (
        <section className="trash-boards-section">
          <h3 className="trash-section-title">看板與資料夾</h3>
          <ul className="trash-boards-list">
            {trashedFolders.map((f) => (
              <li key={`folder-${f.id}`} className="trash-board-row">
                <span className="trash-board-name">📁 {f.name}</span>
                <button
                  className="btn-restore"
                  onClick={() => void handleRestoreFolder(f.id)}
                  title="還原資料夾"
                  aria-label={`還原資料夾 ${f.name}`}
                >
                  <ArrowCounterClockwise aria-hidden="true" /> 還原
                </button>
              </li>
            ))}
            {trashedBoards.map((b) => (
              <li key={`board-${b.id}`} className="trash-board-row">
                <span className="trash-board-name">📋 {b.name}</span>
                <button
                  className="btn-restore"
                  onClick={() => void handleRestoreBoard(b.id)}
                  title="還原看板"
                  aria-label={`還原看板 ${b.name}`}
                >
                  <ArrowCounterClockwise aria-hidden="true" /> 還原
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {!loading && items.length === 0 && trashedBoards.length === 0 && trashedFolders.length === 0 && (
        <div className="inbox-empty">
          <p className="empty-title">垃圾桶是空的</p>
          <p className="hint">刪除的項目會在這裡保留 30 天，期間可隨時還原。</p>
        </div>
      )}

      {!loading && items.length > 0 && (
        <div className="trash-list">
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
                aria-label={`還原「${item.title ?? item.textPreview ?? "未命名項目"}」到收件匣`}
              >
                <ArrowCounterClockwise aria-hidden="true" /> 還原
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
