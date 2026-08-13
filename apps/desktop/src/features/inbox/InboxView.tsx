import { useState, useEffect, useCallback } from "react";
import { useInbox } from "./useInbox";
import type { GroupMode } from "./useInbox";
import { InboxItem } from "./InboxItem";
import { ItemDetail } from "./ItemDetail";
import { archiveItem, batchArchive, batchTrash } from "./api";
import type { ItemSummary } from "./api";
import { listBoards, placeItemOnBoard } from "../boards/api";
import type { BoardSummary } from "../boards/api";
import { showCaptureError, showCaptureSuccess } from "../../stores/toastStore";

/** INB-002 smart-view mode labels. */
const GROUP_MODES: Array<{ value: GroupMode; label: string }> = [
  { value: "date", label: "Date" },
  { value: "source", label: "Source" },
  { value: "domain", label: "Domain" },
  { value: "type", label: "Type" },
  { value: "session", label: "Session" },
];

export function InboxView() {
  const {
    groups,
    groupMode,
    setGroupMode,
    items,
    state,
    error,
    reload,
    toggleFavorite,
    togglePin,
    trash,
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
      await batchArchive([...selectedIds]);
      clearSelection();
      await reload();
    } catch (error) {
      showCaptureError(`Could not archive selected items: ${String(error)}`);
    } finally {
      setBatchBusy(false);
    }
  }, [selectedIds, reload, clearSelection]);

  const handleBatchTrash = useCallback(async () => {
    if (selectedIds.size === 0) return;
    setBatchBusy(true);
    try {
      await batchTrash([...selectedIds]);
      clearSelection();
      await reload();
    } catch (error) {
      showCaptureError(`Could not move selected items to Trash: ${String(error)}`);
    } finally {
      setBatchBusy(false);
    }
  }, [selectedIds, reload, clearSelection]);

  const handleLoadBatchBoards = useCallback(async () => {
    try {
      setBatchBoards(await listBoards());
    } catch (error) {
      setBatchBoards([]);
      showCaptureError(`Could not load boards: ${String(error)}`);
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
      showCaptureSuccess(`${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} added to board`);
    } catch (error) {
      showCaptureError(`Could not add selected items to board: ${String(error)}`);
    } finally {
      setBatchBusy(false);
    }
  }, [batchBoardId, selectedIds, clearSelection]);

  // Listen for open-item events (from dedup toast, search results) — SRC-003
  const handleOpenItem = useCallback(
    (e: Event) => {
      const detail = (e as CustomEvent<{ itemId: string }>).detail;
      const found = items.find((i) => i.id === detail.itemId);
      if (found) {
        setSelectedItem(found);
      }
    },
    [items],
  );

  useEffect(() => {
    window.addEventListener("paste-canvas://open-item", handleOpenItem);
    return () => window.removeEventListener("paste-canvas://open-item", handleOpenItem);
  }, [handleOpenItem]);

  return (
    <div className="inbox-view">
      <header className="inbox-header">
        <h2>Inbox</h2>
        <span className="inbox-count">{items.length} items</span>
        {/* INB-002 smart-view grouping selector */}
        <div className="group-mode-selector" role="tablist" aria-label="Group by">
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
        <button className="btn-refresh" onClick={() => void reload()} title="Refresh inbox" aria-label="Refresh inbox">
          Refresh
        </button>
      </header>

      {selectedIds.size > 0 && (
        <div className="batch-action-bar" role="toolbar" aria-label="Actions for selected items">
          <strong>{selectedIds.size} selected</strong>
          <button type="button" onClick={() => void handleBatchArchive()} disabled={batchBusy}>Archive</button>
          <button type="button" onClick={() => void handleBatchTrash()} disabled={batchBusy}>Move to Trash</button>
          {batchBoards.length === 0 ? (
            <button type="button" onClick={() => void handleLoadBatchBoards()} disabled={batchBusy}>Add to board…</button>
          ) : (
            <>
              <select aria-label="Target board" value={batchBoardId} onChange={(e) => setBatchBoardId(e.target.value)}>
                <option value="">Choose board…</option>
                {batchBoards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}
              </select>
              <button type="button" onClick={() => void handleBatchAddToBoard()} disabled={batchBusy || !batchBoardId}>Add</button>
            </>
          )}
          <button type="button" className="batch-clear" onClick={clearSelection} disabled={batchBusy}>Clear</button>
        </div>
      )}

      {state === "loading" && <div className="inbox-loading">Loading…</div>}

      {state === "error" && (
        <div className="inbox-error" role="alert">
          <p>{error}</p>
          <button onClick={() => void reload()}>Retry</button>
        </div>
      )}

      {state === "success" && items.length === 0 && (
        <div className="inbox-empty">
          <p>No items yet.</p>
          <p className="hint">
            Press <kbd>Ctrl+Shift+V</kbd> to capture your clipboard.
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
                    onArchive={() => void archiveItem(item.id).then(() => reload()).catch((error) => showCaptureError(`Could not archive item: ${String(error)}`))}
                    onSelect={(selected) => toggleSelected(item.id, selected)}
                    selected={selectedIds.has(item.id)}
                    onOpen={() => setSelectedItem(item)}
                  />
                ))}
              </ul>
            </section>
          ))}
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
