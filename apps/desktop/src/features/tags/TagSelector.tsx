import { useState, useEffect, useCallback } from "react";
import {
  listTags,
  getItemTags,
  addTagToItem,
  removeTagFromItem,
  createTag,
  renameTag,
  deleteTag,
} from "./api";
import type { Tag } from "./api";

interface TagSelectorProps {
  itemId: string;
}

/**
 * TagSelector — allows adding/removing tags on an item.
 * Shows existing tags as chips, with an input to add new ones.
 */
export function TagSelector({ itemId }: TagSelectorProps) {
  const [itemTags, setItemTags] = useState<Tag[]>([]);
  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [input, setInput] = useState("");
  const [showDropdown, setShowDropdown] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  const loadTags = useCallback(async () => {
    try {
      const [itemT, allT] = await Promise.all([getItemTags(itemId), listTags()]);
      setItemTags(itemT);
      setAllTags(allT);
    } catch { /* ignore */ }
  }, [itemId]);

  useEffect(() => {
    void loadTags();
  }, [loadTags]);

  const handleAdd = useCallback(
    async (tagId: string) => {
      try {
        await addTagToItem(itemId, tagId);
        setInput("");
        setShowDropdown(false);
        void loadTags();
      } catch { /* ignore */ }
    },
    [itemId, loadTags],
  );

  const handleRemove = useCallback(
    async (tagId: string) => {
      try {
        await removeTagFromItem(itemId, tagId);
        void loadTags();
      } catch { /* ignore */ }
    },
    [itemId, loadTags],
  );

  const handleCreateAndAdd = useCallback(async () => {
    const name = input.trim();
    if (!name) return;
    try {
      const newTag = await createTag(name);
      await addTagToItem(itemId, newTag.id);
      setInput("");
      setShowDropdown(false);
      void loadTags();
    } catch { /* ignore */ }
  }, [input, itemId, loadTags]);

  // Tag management (global): rename affects every item carrying the tag.
  const handleStartRename = useCallback((tag: Tag) => {
    setRenamingId(tag.id);
    setRenameValue(tag.name);
  }, []);

  const handleSaveRename = useCallback(async () => {
    if (!renamingId) return;
    const name = renameValue.trim();
    if (name) {
      try {
        await renameTag(renamingId, name);
      } catch { /* ignore */ }
    }
    setRenamingId(null);
    void loadTags();
  }, [renamingId, renameValue, loadTags]);

  const handleDeleteTag = useCallback(
    async (tag: Tag) => {
      const count = tag.itemCount;
      const message =
        count > 0
          ? `刪除標籤「${tag.name}」會同時從 ${count} 個項目移除它。確定嗎？`
          : `刪除標籤「${tag.name}」？`;
      if (!window.confirm(message)) return;
      try {
        await deleteTag(tag.id);
      } catch { /* ignore */ }
      setRenamingId(null);
      void loadTags();
    },
    [loadTags],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        // Check if input matches an existing tag
        const match = allTags.find(
          (t) => t.name.toLowerCase() === input.trim().toLowerCase(),
        );
        if (match) {
          void handleAdd(match.id);
        } else if (input.trim()) {
          void handleCreateAndAdd();
        }
      }
      if (e.key === "Escape") {
        setShowDropdown(false);
        setInput("");
      }
    },
    [allTags, input, handleAdd, handleCreateAndAdd],
  );

  // Filter available tags (not already on item)
  const availableTags = allTags.filter(
    (t) => !itemTags.some((it) => it.id === t.id),
  );
  const filteredTags = input.trim()
    ? availableTags.filter((t) =>
        t.name.toLowerCase().includes(input.trim().toLowerCase()),
      )
    : availableTags;

  return (
    <div className="tag-selector">
      <div className="tag-chips">
        {itemTags.map((tag) => (
          <span key={tag.id} className="tag-chip" style={tag.color ? { borderColor: tag.color } : undefined}>
            {tag.name}
          <button className="tag-remove" onClick={() => void handleRemove(tag.id)} aria-label={`移除標籤 ${tag.name}`}>
              ×
            </button>
          </span>
        ))}
      </div>
      <div className="tag-input-wrapper">
        <input
          className="tag-input"
          placeholder="新增標籤…"
          value={input}
          onChange={(e) => {
            setInput(e.target.value);
            setShowDropdown(true);
          }}
          onFocus={() => setShowDropdown(true)}
          onKeyDown={handleKeyDown}
        />
        {showDropdown && filteredTags.length > 0 && (
          <div className="tag-dropdown">
            {filteredTags.slice(0, 8).map((tag) =>
              renamingId === tag.id ? (
                <div key={tag.id} className="tag-dropdown-item tag-rename-row">
                  <input
                    className="tag-rename-input"
                    value={renameValue}
                    autoFocus
                    onChange={(e) => setRenameValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        void handleSaveRename();
                      }
                      if (e.key === "Escape") {
                        e.stopPropagation();
                        setRenamingId(null);
                      }
                    }}
                    aria-label={`重新命名標籤 ${tag.name}`}
                  />
                  <button
                    className="tag-action"
                    onClick={() => void handleSaveRename()}
                    aria-label="儲存新名稱"
                  >
                    ✓
                  </button>
                </div>
              ) : (
                <div key={tag.id} className="tag-dropdown-item tag-manage-row">
                  <button className="tag-dropdown-pick" onClick={() => void handleAdd(tag.id)}>
                    {tag.name}
                    <span className="tag-count">{tag.itemCount}</span>
                  </button>
                  <span className="tag-manage-actions">
                    <button
                      className="tag-action"
                      onClick={() => handleStartRename(tag)}
                      aria-label={`重新命名標籤 ${tag.name}`}
                      title="重新命名"
                    >
                      ✏
                    </button>
                    <button
                      className="tag-action tag-action-danger"
                      onClick={() => void handleDeleteTag(tag)}
                      aria-label={`刪除標籤 ${tag.name}`}
                      title="刪除標籤"
                    >
                      🗑
                    </button>
                  </span>
                </div>
              ),
            )}
          </div>
        )}
      </div>
    </div>
  );
}
