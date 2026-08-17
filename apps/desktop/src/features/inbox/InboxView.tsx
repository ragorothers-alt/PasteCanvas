import { useState, useEffect, useCallback } from "react";
import { useInbox } from "./useInbox";
import type { GroupMode } from "./useInbox";
import { InboxItem } from "./InboxItem";
import { ItemDetail } from "./ItemDetail";
import { batchArchive, batchTrash, getItemContent } from "./api";
import type { ItemSummary } from "./api";
import { listBoards, placeItemOnBoard } from "../boards/api";
import type { BoardSummary } from "../boards/api";
import { showCaptureError, showCaptureSuccess } from "../../stores/toastStore";
import { ArrowClockwise } from "@phosphor-icons/react";

/** INB-002 smart-view mode labels. */
const GROUP_MODES: Array<{ value: GroupMode; label: string }> = [
  { value: "date", label: "日期" },
  { value: "source", label: "來源" },
  { value: "domain", label: "網域" },
  { value: "type", label: "類型" },
  { value: "session", label: "工作階段" },
];

interface InboxViewProps {
  openItemId?: string | null;
  onOpenItemHandled?: () => void;
}

function summaryFromContent(
  content: Awaited<ReturnType<typeof getItemContent>>,
): ItemSummary {
  return {
    id: content.itemId,
    itemType: content.itemType,
    title: content.title,
    textPreview: content.plainText,
    displayUrl: content.displayUrl,
    inboxStatus: content.inboxStatus,
    isFavorite: false,
    isPinned: false,
    captureMethod: content.captureMethod,
    sourceApp: null,
    domain: null,
    captureSessionId: null,
    createdAt: content.createdAt,
    revision: content.revision,
    assetByteSize: null,
  };
}

export function InboxView({ openItemId = null, onOpenItemHandled }: InboxViewProps) {
  const {
    groups,
    groupMode,
    setGroupMode,
    items,
    state,
    error,
    hasMore,
    reload,
    loadMore,
    toggleFavorite,
    togglePin,
    trash,
    archive,
    removeLocal,
  } = useInbox();
  const [selectedItem, setSelectedItem] = useState<ItemSummary | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchBoards, setBatchBoards] = useState<BoardSummary[]>([]);
  const [batchBoardId, setBatchBoardId] = useState("");
  const [batchBusy, setBatchBusy] = useState(false);

  const toggleSelected = useCallback((itemId: string, selected: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (selected) next.add(itemId);
      else next.delete(itemId);
      return next;
    });
  }, []);

  const clearSelection = useCallback(() => setSelectedIds(new Set()), []);

  const handleBatchArchive = useCallback(async () => {
    if (selectedIds.size === 0) return;
    setBatchBusy(true);
    try {
      const ids = [...selectedIds];
      await batchArchive(ids);
      removeLocal(ids);
      clearSelection();
    } catch (error) {
      showCaptureError(`無法封存所選項目：${String(error)}`);
    } finally {
      setBatchBusy(false);
    }
  }, [selectedIds, removeLocal, clearSelection]);

  const handleBatchTrash = useCallback(async () => {
    if (selectedIds.size === 0) return;
    setBatchBusy(true);
    try {
      await batchTrash([...selectedIds]);
      clearSelection();
      await reload();
    } catch (error) {
      showCaptureError(`無法將所選項目移到垃圾桶：${String(error)}`);
    } finally {
      setBatchBusy(false);
    }
  }, [selectedIds, reload, clearSelection]);

  const handleLoadBatchBoards = useCallback(async () => {
    try {
      setBatchBoards(await listBoards());
    } catch (error) {
      setBatchBoards([]);
      showCaptureError(`無法載入看板：${String(error)}`);
    }
  }, []);

  const handleBatchAddToBoard = useCallback(async () => {
    if (!batchBoardId || selectedIds.size === 0) return;
    setBatchBusy(true);
    try {
      let index = 0;
      for (const itemId of selectedIds) {
        await placeItemOnBoard(batchBoardId, itemId, 100 + (index % 4) * 220, 100 + Math.floor(index / 4) * 170);
        index += 1;
      }
      clearSelection();
      showCaptureSuccess(`已將 ${selectedIds.size} 個項目加入看板`);
    } catch (error) {
      showCaptureError(`無法將所選項目加入看板：${String(error)}`);
    } finally {
      setBatchBusy(false);
    }
  }, [batchBoardId, selectedIds, clearSelection]);

  const openById = useCallback(
    async (itemId: string) => {
      const found = items.find((i) => i.id === itemId);
      if (found) {
        setSelectedItem(found);
        return;
      }
      try {
        const content = await getItemContent(itemId);
        setSelectedItem(summaryFromContent(content));
      } catch (error) {
        showCaptureError(`無法開啟項目：${String(error)}`);
      }
    },
    [items],
  );

  // Listen for open-item events (from dedup toast, search results) — SRC-003
  useEffect(() => {
    const handleOpenItem = (e: Event) => {
      const itemId = (e as CustomEvent<{ itemId: string }>).detail?.itemId;
      if (itemId) void openById(itemId);
    };
    window.addEventListener("paste-canvas://open-item", handleOpenItem);
    return () => window.removeEventListener("paste-canvas://open-item", handleOpenItem);
  }, [openById]);

  useEffect(() => {
    if (!openItemId) return;
    void openById(openItemId).finally(() => onOpenItemHandled?.());
  }, [openItemId, openById, onOpenItemHandled]);

  return (
    <div className="inbox-view">
      <header className="inbox-header">
        <h2>收件匣</h2>
        <span className="inbox-count">{items.length} 個項目</span>
        {/* INB-002 smart-view grouping selector */}
        <div className="group-mode-selector" role="tablist" aria-label="分組方式">
          {GROUP_MODES.map((m) => (
            <button
              key={m.value}
              role="tab"
              aria-selected={groupMode === m.value}
              className={`group-mode-btn ${groupMode === m.value ? "active" : ""}`}
              onClick={() => setGroupMode(m.value)}
            >
              {m.label}
            </button>
          ))}
        </div>
        <button className="btn-refresh" onClick={() => void reload()} title="重新整理收件匣" aria-label="重新整理收件匣">
          <ArrowClockwise aria-hidden="true" /> 重新整理
        </button>
      </header>

      {selectedIds.size > 0 && (
        <div className="batch-action-bar" role="toolbar" aria-label="所選項目動作">
          <strong>已選 {selectedIds.size} 個</strong>
          <button type="button" onClick={() => void handleBatchArchive()} disabled={batchBusy}>封存</button>
          <button type="button" onClick={() => void handleBatchTrash()} disabled={batchBusy}>移到垃圾桶</button>
          {batchBoards.length === 0 ? (
            <button type="button" onClick={() => void handleLoadBatchBoards()} disabled={batchBusy}>加入看板…</button>
          ) : (
            <>
              <select aria-label="目標看板" value={batchBoardId} onChange={(e) => setBatchBoardId(e.target.value)}>
                <option value="">選擇看板…</option>
                {batchBoards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}
              </select>
              <button type="button" onClick={() => void handleBatchAddToBoard()} disabled={batchBusy || !batchBoardId}>加入</button>
            </>
          )}
          <button type="button" className="batch-clear" onClick={clearSelection} disabled={batchBusy}>清除</button>
        </div>
      )}

      {state === "loading" && <div className="inbox-loading">載入中…</div>}

      {state === "error" && (
        <div className="inbox-error" role="alert">
          <p>{error}</p>
          <button onClick={() => void reload()}>重試</button>
        </div>
      )}

      {state === "success" && items.length === 0 && (
        <div className="inbox-empty">
          <p>還沒有項目。</p>
          <p className="hint">
            按下 <kbd>Ctrl+Shift+V</kbd> 擷取剪貼簿內容。
          </p>
        </div>
      )}

      {state === "success" && items.length > 0 && (
        <div className="inbox-groups">
          {groups.map((group) => (
            <section key={group.label} className="inbox-group">
              <h3 className="group-label">
                {group.label}
                <span className="group-count">{group.items.length}</span>
              </h3>
              <ul className="inbox-list">
                {group.items.map((item) => (
                  <InboxItem
                    key={item.id}
                    item={item}
                    onToggleFavorite={() => void toggleFavorite(item.id)}
                    onTogglePin={() => void togglePin(item.id)}
                    onTrash={() => void trash(item.id)}
                    onArchive={() => {
                      void archive(item.id);
                      if (selectedItem?.id === item.id) setSelectedItem(null);
                    }}
                    onSelect={(selected) => toggleSelected(item.id, selected)}
                    selected={selectedIds.has(item.id)}
                    onOpen={() => setSelectedItem(item)}
                  />
                ))}
              </ul>
            </section>
          ))}
          {hasMore && (
            <button className="btn-add-exclusion" onClick={() => void loadMore()}>
              載入更多
            </button>
          )}
        </div>
      )}

      {selectedItem && (
        <ItemDetail
          item={selectedItem}
          onClose={() => setSelectedItem(null)}
          onUpdated={() => { void reload(); setSelectedItem(null); }}
        />
      )}
    </div>
  );
}
