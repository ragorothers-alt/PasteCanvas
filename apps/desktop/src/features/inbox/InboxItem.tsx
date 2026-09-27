import { memo, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Archive, Check, ClipboardText, File, FileCode, FilePdf, Image, MapPin, PushPin, Star, Trash } from "@phosphor-icons/react";
import type { ItemSummary } from "./api";
import { listBoards, placeItemOnBoard } from "../boards/api";
import type { BoardSummary } from "../boards/api";
import { setInboxItemDragPayload } from "./dragPayload";
import { showCaptureError } from "../../stores/toastStore";
import { ColorCard } from "./ColorCard";
import { PdfCard } from "./PdfCard";
import { FontCard } from "./FontCard";
import { ImagePreview } from "./ImagePreview";

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

  if (diffMin < 1) return "剛剛";
  if (diffMin < 60) return `${diffMin} 分鐘前`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr} 小時前`;
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function typeIcon(itemType: string): ReactNode {
  switch (itemType) {
    case "link": return <MapPin aria-hidden="true" />;
    case "image":
    case "screenshot": return <Image aria-hidden="true" />;
    case "pdf": return <FilePdf aria-hidden="true" />;
    case "code": return <FileCode aria-hidden="true" />;
    default: return <File aria-hidden="true" />;
  }
}

function typeLabel(itemType: string): string {
  const labels: Record<string, string> = {
    plain_text: "文字", link: "連結", image: "圖片", screenshot: "螢幕截圖",
    pdf: "PDF", code: "程式碼", color: "色彩", font: "字型", file: "檔案", svg: "SVG",
  };
  return labels[itemType] ?? itemType;
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
  const pickerRef = useRef<HTMLDivElement | null>(null);

  // Dismiss the picker on Escape / outside click (keyboard + pointer parity).
  useEffect(() => {
    if (!showBoardPicker) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShowBoardPicker(false);
    };
    const onPointer = (e: PointerEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setShowBoardPicker(false);
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
  }, [showBoardPicker]);

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

  const handleSelectBoard = async (boardId: string, index: number) => {
    try {
      // Deterministic grid slot (matches InboxView batch placement) instead
      // of a random scatter — repeated placements stay predictable.
      await placeItemOnBoard(boardId, item.id, 100 + (index % 4) * 220, 100 + Math.floor(index / 4) * 170);
      setAdded(true);
      setShowBoardPicker(false);
      setTimeout(() => setAdded(false), 2000);
    } catch (error) {
      showCaptureError(`無法加入看板：${String(error)}`);
    }
  };

  return (
    <li
      className={`inbox-item ${item.isFavorite ? "is-favorite" : ""} ${selected ? "is-selected" : ""}`}
      draggable
      onDragStart={(e) => setInboxItemDragPayload(e.dataTransfer, item.id)}
      title="拖曳到看板畫布即可放置項目"
    >
      {onSelect && (
        <input
          className="item-select"
          type="checkbox"
          aria-label={`選取 ${item.title ?? item.textPreview ?? "未命名項目"}`}
          checked={selected}
          onChange={(e) => onSelect(e.target.checked)}
        />
      )}
      <div className="item-icon" aria-hidden="true">
        {typeIcon(item.itemType)}
      </div>
      <div className="item-content">
        {(item.itemType === "image" || item.itemType === "screenshot") && (
          <ImagePreview itemId={item.id} maxDimension={640} />
        )}
        {item.itemType === "color" && item.textPreview ? (
          <ColorCard colorText={item.textPreview} />
        ) : item.itemType === "pdf" ? (
          <PdfCard
            fileName={item.title ?? "document.pdf"}
            fileSize={item.assetByteSize ?? 0}
            pageCount={null}
          />
        ) : item.itemType === "font" ? (
          <FontCard
            fileName={item.title ?? "font.ttf"}
            fileSize={item.assetByteSize ?? 0}
          />
        ) : (
          <button type="button" className="item-title item-title-button" onClick={onOpen}>
            {item.title ?? item.textPreview ?? "（未命名）"}
          </button>
        )}
        <div className="item-meta">
          <span className="item-type">{typeLabel(item.itemType)}</span>
          {item.sourceApp && <span className="item-source">{item.sourceApp}</span>}
          <span className="item-time">{formatTime(item.createdAt)}</span>
        </div>
      </div>
      <div className="item-actions">
        <button
          className={`btn-add-board ${added ? "added" : ""}`}
          onClick={() => void handleAddToBoard()}
          title={added ? "已加入看板" : "加入看板"}
          aria-label={added ? "已加入看板" : "加入看板"}
          aria-live="polite"
        >
          {added ? <Check aria-hidden="true" /> : <ClipboardText aria-hidden="true" />}
        </button>
        <button
          className={`btn-fav ${item.isFavorite ? "active" : ""}`}
          onClick={onToggleFavorite}
          title={item.isFavorite ? "取消收藏" : "加入收藏"}
          aria-label={item.isFavorite ? "取消收藏" : "加入收藏"}
        >
          <Star weight={item.isFavorite ? "fill" : "regular"} aria-hidden="true" />
        </button>
        <button
          className={`btn-pin ${item.isPinned ? "active" : ""}`}
          onClick={onTogglePin}
          title={item.isPinned ? "取消釘選" : "釘選"}
          aria-label={item.isPinned ? "取消釘選" : "釘選"}
        >
          <PushPin weight={item.isPinned ? "fill" : "regular"} aria-hidden="true" />
        </button>
        <button className="btn-trash" onClick={onTrash} title="移到垃圾桶" aria-label="移到垃圾桶">
          <Trash aria-hidden="true" />
        </button>
        {onArchive && (
          <button className="btn-archive" onClick={onArchive} title="封存項目" aria-label="封存項目">
            <Archive aria-hidden="true" /><span className="sr-only">封存</span>
          </button>
        )}
      </div>

      {showBoardPicker && boards.length > 0 && (
        <div className="board-picker" ref={pickerRef} role="menu" aria-label="選擇目標看板">
          <div className="board-picker-title">加入看板：</div>
          {boards.map((b, index) => (
            <button
              key={b.id}
              type="button"
              role="menuitem"
              className="board-picker-item"
              onClick={() => void handleSelectBoard(b.id, index)}
            >
              {b.name}
            </button>
          ))}
        </div>
      )}
      {showBoardPicker && boards.length === 0 && (
        <div className="board-picker">
          <div className="board-picker-title">還沒有看板，請先建立一個。</div>
        </div>
      )}
    </li>
  );
});
