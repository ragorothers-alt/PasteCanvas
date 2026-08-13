import { memo, useState } from "react";
import type { ItemSummary } from "./api";
import { listBoards, placeItemOnBoard } from "../boards/api";
import type { BoardSummary } from "../boards/api";
import { setInboxItemDragPayload } from "./dragPayload";
import { ColorCard } from "./ColorCard";
import { PdfCard } from "./PdfCard";
import { FontCard } from "./FontCard";

interface InboxItemProps {
  item: ItemSummary;
  onToggleFavorite: () => void;
  onTogglePin: () => void;
  onTrash: () => void;
  onArchive?: () => void;
  onSelect?: (selected: boolean) => void;
  onOpen?: () => void;
  selected?: boolean;
}

function formatTime(unixMs: number): string {
  const d = new Date(unixMs);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);

  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function typeIcon(itemType: string): string {
  switch (itemType) {
    case "link": return "🔗";
    case "image": return "🖼";
    case "screenshot": return "📷";
    case "pdf": return "📄";
    case "code": return "💻";
    default: return "📝";
  }
}

export const InboxItem = memo(function InboxItem({
  item,
  onToggleFavorite,
  onTogglePin,
  onTrash,
  onArchive,
  onSelect,
  onOpen,
  selected = false,
}: InboxItemProps) {
  const [showBoardPicker, setShowBoardPicker] = useState(false);
  const [boards, setBoards] = useState<BoardSummary[]>([]);
  const [added, setAdded] = useState(false);

  const handleAddToBoard = async () => {
    if (!showBoardPicker) {
      try {
        const b = await listBoards();
        setBoards(b);
      } catch { /* ignore */ }
      setShowBoardPicker(true);
    } else {
      setShowBoardPicker(false);
    }
  };

  const handleSelectBoard = async (boardId: string) => {
    try {
      await placeItemOnBoard(boardId, item.id, 100 + Math.random() * 200, 100 + Math.random() * 200);
      setAdded(true);
      setShowBoardPicker(false);
      setTimeout(() => setAdded(false), 2000);
    } catch { /* ignore */ }
  };

  return (
    <li
      className={`inbox-item ${item.isFavorite ? "is-favorite" : ""} ${selected ? "is-selected" : ""}`}
      draggable
      onDragStart={(e) => setInboxItemDragPayload(e.dataTransfer, item.id)}
      title="Drag onto a board canvas to place this item (or use 📋)"
    >
      {onSelect && (
        <input
          className="item-select"
          type="checkbox"
          aria-label={`Select ${item.title ?? item.textPreview ?? "untitled item"}`}
          checked={selected}
          onChange={(e) => onSelect(e.target.checked)}
        />
      )}
      <div className="item-icon" aria-hidden="true">
        {typeIcon(item.itemType)}
      </div>
      <div className="item-content">
        {item.itemType === "color" && item.textPreview ? (
          <ColorCard colorText={item.textPreview} />
        ) : item.itemType === "pdf" ? (
          <PdfCard
            fileName={item.title ?? "document.pdf"}
            fileSize={0}
            pageCount={null}
          />
        ) : item.itemType === "font" ? (
          <FontCard
            fileName={item.title ?? "font.ttf"}
            fileSize={0}
          />
        ) : (
          <button type="button" className="item-title item-title-button" onClick={onOpen}>
            {item.title ?? item.textPreview ?? "(untitled)"}
          </button>
        )}
        <div className="item-meta">
          <span className="item-type">{item.itemType}</span>
          {item.sourceApp && <span className="item-source">{item.sourceApp}</span>}
          <span className="item-time">{formatTime(item.createdAt)}</span>
        </div>
      </div>
      <div className="item-actions">
        <button
          className={`btn-add-board ${added ? "added" : ""}`}
          onClick={() => void handleAddToBoard()}
          title="Add to board"
        >
          {added ? "✓" : "📋"}
        </button>
        <button
          className={`btn-fav ${item.isFavorite ? "active" : ""}`}
          onClick={onToggleFavorite}
          title={item.isFavorite ? "Remove favorite" : "Add favorite"}
        >
          {item.isFavorite ? "★" : "☆"}
        </button>
        <button
          className={`btn-pin ${item.isPinned ? "active" : ""}`}
          onClick={onTogglePin}
          title={item.isPinned ? "Unpin" : "Pin"}
        >
          {item.isPinned ? "📌" : "📍"}
        </button>
        <button className="btn-trash" onClick={onTrash} title="Move to trash">
          🗑
        </button>
        {onArchive && (
          <button className="btn-archive" onClick={onArchive} title="Archive item">
            Archive
          </button>
        )}
      </div>

      {showBoardPicker && boards.length > 0 && (
        <div className="board-picker">
          <div className="board-picker-title">Add to board:</div>
          {boards.map((b) => (
            <button
              key={b.id}
              className="board-picker-item"
              onClick={() => void handleSelectBoard(b.id)}
            >
              {b.name}
            </button>
          ))}
        </div>
      )}
      {showBoardPicker && boards.length === 0 && (
        <div className="board-picker">
          <div className="board-picker-title">No boards yet. Create one first.</div>
        </div>
      )}
    </li>
  );
});
