import { useState, useEffect, useCallback } from "react";
import type { ReactNode } from "react";
import { InboxView } from "./features/inbox/InboxView";
import { TrashView } from "./features/inbox/TrashView";
import { QuickCapture } from "./features/capture/QuickCapture";
import { ToastContainer } from "./features/capture/Toast";
import { SearchBar } from "./features/search/SearchBar";
import { BoardView } from "./features/boards/BoardView";
import { SettingsView } from "./features/settings/SettingsView";
import { SessionControl } from "./features/session/SessionControl";
import {
  listBoards,
  createBoard,
  listFolders,
  createFolder,
  trashFolder,
  renameFolder,
  toggleBoardFavorite,
  getRecentBoards,
  reorderBoards,
  reorderFolders,
  renameBoard,
  trashBoard,
  moveBoard,
} from "./features/boards/api";
import type { BoardSummary, Folder } from "./features/boards/api";
import { buildBoardTree } from "./features/boards/boardTree";
import type { BoardTreeNode } from "./features/boards/boardTree";
import {
  BOARD_DRAG_TYPE,
  FOLDER_DRAG_TYPE,
  computeBoardReorder,
  computeFolderReorder,
} from "./features/boards/boardReorder";
import { surfaceStartupHotkeyFailures } from "./features/settings/hotkeyFeedback";
import { startTauriEventBridge } from "./lib/tauriEvents";
import { subscribeCaptureFeedback } from "./features/capture/captureEvents";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./lib/runtime";
import { applyTheme, getStoredTheme, loadTheme, persistTheme } from "./lib/theme";
import type { ThemeMode } from "./lib/theme";
import { ClockCounterClockwise, GearSix, FolderSimple, MagnifyingGlass, Note, PencilSimple, Plus, Star, Trash } from "@phosphor-icons/react";

type View = "inbox" | "board" | "settings" | "trash";
type SessionState = "active" | "paused" | "finished" | null;

export function App() {
  const [view, setView] = useState<View>("inbox");
  const [sessionState, setSessionState] = useState<SessionState>(null);
  const [theme, setTheme] = useState<ThemeMode>(() => getStoredTheme());
  const [boards, setBoards] = useState<BoardSummary[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [recentBoards, setRecentBoards] = useState<BoardSummary[]>([]);
  // BRD-007: inline folder rename state
  const [editingFolderId, setEditingFolderId] = useState<string | null>(null);
  const [editingFolderName, setEditingFolderName] = useState("");
  const [editingBoardId, setEditingBoardId] = useState<string | null>(null);
  const [editingBoardName, setEditingBoardName] = useState("");
  const [pendingOpenItemId, setPendingOpenItemId] = useState<string | null>(null);
  // BRD-004: navigation stack for nested boards (last = current)
  const [boardStack, setBoardStack] = useState<BoardSummary[]>([]);
  const activeBoard = boardStack.length > 0 ? boardStack[boardStack.length - 1] : null;

  const loadBoards = useCallback(async () => {
    try {
      const [b, f] = await Promise.all([
        listBoards(),
        listFolders().catch(() => [] as Folder[]),
      ]);
      setBoards(b);
      setFolders(f);
      return b;
    } catch {
      return [] as BoardSummary[];
    }
  }, []);

  useEffect(() => { void loadBoards(); }, [loadBoards]);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const handleThemeChange = useCallback((nextTheme: ThemeMode) => {
    setTheme(nextTheme);
    applyTheme(nextTheme);
    void persistTheme(nextTheme);
  }, []);

  useEffect(() => {
    void loadTheme().then((mode) => { setTheme(mode); applyTheme(mode); });
  }, []);

  // BRD-007: sidebar "Recent" section (best-effort; backend-only feature)
  const loadRecent = useCallback(async () => {
    try {
      setRecentBoards(await getRecentBoards(5));
    } catch {
      setRecentBoards([]);
    }
  }, []);

  useEffect(() => { void loadRecent(); }, [loadRecent]);

  // WIN-001: tray "Settings" emits this event to switch the view
  useEffect(() => {
    const handler = () => setView("settings");
    window.addEventListener("paste-canvas://show-settings", handler);
    return () => window.removeEventListener("paste-canvas://show-settings", handler);
  }, []);

  useEffect(() => {
    const handler = (event: Event) => {
      const state = (event as CustomEvent<{ state: SessionState }>).detail?.state ?? null;
      setSessionState(state);
    };
    window.addEventListener("paste-canvas://session-state", handler);
    return () => window.removeEventListener("paste-canvas://session-state", handler);
  }, []);

  // WIN-005: forward backend Tauri events (tray/hotkeys) to window listeners
  useEffect(() => {
    let stop: (() => void) | null = null;
    let disposed = false;
    void startTauriEventBridge().then((cleanup) => {
      if (disposed) cleanup();
      else stop = cleanup;
    });
    return () => {
      disposed = true;
      if (stop) stop();
    };
  }, []);

  // CAP-001 (PRD): capture outcomes are never silent — success shows a light
  // toast, failure shows an understandable reason.
  useEffect(() => subscribeCaptureFeedback(), []);

  // FND-006: surface hotkey registration failures once (visible + recoverable).
  useEffect(() => {
    void surfaceStartupHotkeyFailures();
  }, []);

  // SRC-003: "locate on board" emits this event to open the target board
  useEffect(() => {
    const handler = (e: Event) => {
      const { boardId } = (e as CustomEvent<{ boardId: string }>).detail;
      void listBoards().then((all) => {
        const board = all.find((b) => b.id === boardId);
        if (board) {
          setBoards(all);
          setBoardStack([board]);
          setView("board");
        }
      });
    };
    window.addEventListener("paste-canvas://open-board", handler);
    return () => window.removeEventListener("paste-canvas://open-board", handler);
  }, []);

  useEffect(() => {
    const handler = (e: Event) => {
      const itemId = (e as CustomEvent<{ itemId: string }>).detail?.itemId;
      if (!itemId) return;
      setView("inbox");
      setPendingOpenItemId(itemId);
    };
    window.addEventListener("paste-canvas://open-item", handler);
    return () => window.removeEventListener("paste-canvas://open-item", handler);
  }, []);

  const handleCreateBoard = useCallback(async () => {
    try {
      const board = await createBoard(`看板 ${boards.length + 1}`);
      setBoards((prev) => [...prev, board]);
      setBoardStack([board]);
      setView("board");
    } catch { /* ignore */ }
  }, [boards.length]);

  // BRD-006: folder organization
  const handleCreateFolder = useCallback(async () => {
    try {
      await createFolder(`資料夾 ${folders.length + 1}`);
      void loadBoards();
    } catch { /* ignore */ }
  }, [folders.length, loadBoards]);

  const handleTrashFolder = useCallback(
    async (folderId: string) => {
      try {
        await trashFolder(folderId);
        void loadBoards();
      } catch { /* ignore */ }
    },
    [loadBoards],
  );

  // BRD-007: folder rename (backend command from BRD-006)
  const handleSaveFolderName = useCallback(
    async (folderId: string) => {
      const name = editingFolderName.trim();
      if (name) {
        try {
          await renameFolder(folderId, name);
        } catch { /* ignore */ }
      }
      setEditingFolderId(null);
      void loadBoards();
    },
    [editingFolderName, loadBoards],
  );

  const handleSaveBoardName = useCallback(
    async (boardId: string) => {
      const name = editingBoardName.trim();
      if (name) {
        try {
          await renameBoard(boardId, name);
        } catch { /* ignore */ }
      }
      setEditingBoardId(null);
      void loadBoards();
    },
    [editingBoardName, loadBoards],
  );

  const handleTrashBoard = useCallback(
    async (boardId: string) => {
      try {
        await trashBoard(boardId);
        setBoardStack((prev) => prev.filter((b) => b.id !== boardId));
        setView((current) => (activeBoard?.id === boardId ? "inbox" : current));
        void loadBoards();
        void loadRecent();
      } catch { /* ignore */ }
    },
    [activeBoard?.id, loadBoards, loadRecent],
  );

  const handleToggleFavorite = useCallback(
    async (boardId: string) => {
      try {
        await toggleBoardFavorite(boardId);
        void loadBoards();
      } catch { /* ignore */ }
    },
    [loadBoards],
  );

  // BRD-008: drag-reorder boards in the sidebar (persisted via sort_key)
  const handleBoardDrop = useCallback(
    async (targetId: string, e: React.DragEvent) => {
      const draggedId = e.dataTransfer.getData(BOARD_DRAG_TYPE);
      if (!draggedId) return;
      e.preventDefault();
      const order = computeBoardReorder(boards, draggedId, targetId);
      if (!order) return;
      try {
        await reorderBoards(order);
        void loadBoards();
      } catch { /* ignore */ }
    },
    [boards, loadBoards],
  );

  // BRD-009: drag-reorder folders in the sidebar (persisted via sort_key)
  const handleFolderDrop = useCallback(
    async (targetId: string, e: React.DragEvent) => {
      const draggedId = e.dataTransfer.getData(FOLDER_DRAG_TYPE);
      if (!draggedId) return;
      e.preventDefault();
      e.stopPropagation();
      const order = computeFolderReorder(folders, draggedId, targetId);
      if (!order) return;
      try {
        await reorderFolders(order);
        void loadBoards();
      } catch { /* ignore */ }
    },
    [folders, loadBoards],
  );

  const handleSelectBoard = useCallback((board: BoardSummary) => {
    setBoardStack([board]);
    setView("board");
    void loadRecent(); // BRD-007: opening refreshes the Recent list
  }, [loadRecent]);

  // BRD-004: enter a nested board (double-click on board_link node)
  const handleEnterBoard = useCallback(
    async (targetBoardId: string) => {
      // Resolve the target from the cached list, refreshing if needed
      let target = boards.find((b) => b.id === targetBoardId);
      if (!target) {
        const fresh = await loadBoards();
        target = fresh.find((b) => b.id === targetBoardId);
      }
      if (target) {
        setBoardStack((prev) =>
          prev.some((b) => b.id === target!.id) ? prev : [...prev, target!],
        );
      }
    },
    [boards, loadBoards],
  );

  // BRD-004: breadcrumb navigation back up the stack
  const handleBreadcrumbClick = useCallback((index: number) => {
    setBoardStack((prev) => prev.slice(0, index + 1));
  }, []);

  // BRD-006: recursive sidebar tree rendering
  const renderBoardRow = (b: BoardSummary, depth: number): ReactNode => (
    <span
      key={b.id}
      className={`nav-item nav-board ${view === "board" && activeBoard?.id === b.id ? "active" : ""}`}
      style={{ paddingLeft: `${12 + depth * 14}px` }}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData(BOARD_DRAG_TYPE, b.id);
        e.dataTransfer.effectAllowed = "move";
      }}
      onDragOver={(e) => {
        if (Array.from(e.dataTransfer.types).includes(BOARD_DRAG_TYPE)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        }
      }}
      onDrop={(e) => void handleBoardDrop(b.id, e)}
    >
      {editingBoardId === b.id ? (
        <input
          className="exclusion-input nav-folder-rename"
          value={editingBoardName}
          autoFocus
          onChange={(e) => setEditingBoardName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void handleSaveBoardName(b.id);
            if (e.key === "Escape") setEditingBoardId(null);
          }}
          onBlur={() => void handleSaveBoardName(b.id)}
          onClick={(e) => e.stopPropagation()}
        />
      ) : (
        <span className="nav-board-name" onClick={() => handleSelectBoard(b)}>
          {b.name}
        </span>
      )}
      <span className="nav-folder-actions">
        <button
          className={`btn-board-fav ${b.isFavorite ? "fav" : ""}`}
          title={b.isFavorite ? "取消收藏看板" : "收藏看板"}
          aria-label={b.isFavorite ? "取消收藏看板" : "收藏看板"}
          onClick={(e) => {
            e.stopPropagation();
            void handleToggleFavorite(b.id);
          }}
        >
          <Star weight={b.isFavorite ? "fill" : "regular"} aria-hidden="true" />
        </button>
        <button
          className="btn-folder-rename"
          title="重新命名看板"
          onClick={(e) => {
            e.stopPropagation();
            setEditingBoardId(b.id);
            setEditingBoardName(b.name);
          }}
        >
          <PencilSimple aria-hidden="true" />
        </button>
        <button
          className="btn-folder-trash"
          title="刪除看板"
          onClick={(e) => {
            e.stopPropagation();
            void handleTrashBoard(b.id);
          }}
        >
          <Trash aria-hidden="true" />
        </button>
      </span>
    </span>
  );

  const renderTreeNode = (node: BoardTreeNode, depth: number): ReactNode[] => {
    const out: ReactNode[] = [];
    if (node.folder) {
      const folder = node.folder;
      out.push(
        <span
          key={`folder-${folder.id}`}
          className="nav-item nav-folder"
          style={{ paddingLeft: `${12 + depth * 14}px` }}
          title={folder.name}
          draggable
          onDragStart={(e) => {
            e.dataTransfer.setData(FOLDER_DRAG_TYPE, folder.id);
            e.dataTransfer.effectAllowed = "move";
          }}
          onDragOver={(e) => {
            const types = Array.from(e.dataTransfer.types);
            if (types.includes(FOLDER_DRAG_TYPE) || types.includes(BOARD_DRAG_TYPE)) {
              e.preventDefault();
              e.stopPropagation();
              e.dataTransfer.dropEffect = "move";
            }
          }}
          onDrop={(e) => {
            const boardId = e.dataTransfer.getData(BOARD_DRAG_TYPE);
            if (boardId) {
              e.preventDefault();
              e.stopPropagation();
              void moveBoard(boardId, folder.id).then(() => loadBoards()).catch(() => undefined);
              return;
            }
            void handleFolderDrop(folder.id, e);
          }}
        >
              {editingFolderId === folder.id ? (
            <input
              className="exclusion-input nav-folder-rename"
              value={editingFolderName}
              autoFocus
              onChange={(e) => setEditingFolderName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSaveFolderName(folder.id);
                if (e.key === "Escape") setEditingFolderId(null);
              }}
              onBlur={() => void handleSaveFolderName(folder.id)}
            />
          ) : (
              <span
              className="nav-folder-name"
              onDoubleClick={() => {
                setEditingFolderId(folder.id);
                setEditingFolderName(folder.name);
              }}
              >
              <FolderSimple aria-hidden="true" /> {folder.name}
            </span>
          )}
          <span className="nav-folder-actions">
            <button
              className="btn-folder-rename"
              title="重新命名資料夾"
              onClick={() => {
                setEditingFolderId(folder.id);
                setEditingFolderName(folder.name);
              }}
            >
              <PencilSimple aria-hidden="true" />
            </button>
            <button
              className="btn-folder-trash"
              title="刪除資料夾（保留看板）"
              onClick={() => void handleTrashFolder(folder.id)}
            >
              <Trash aria-hidden="true" />
            </button>
          </span>
        </span>,
      );
    }
    for (const b of node.boards) out.push(renderBoardRow(b, node.folder ? depth + 1 : depth));
    for (const child of node.children) out.push(...renderTreeNode(child, depth + 1));
    return out;
  };

  const tree = buildBoardTree(folders, boards);

  return (
    <main className="app-root">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <p className="brand-kicker">本機工作區</p>
          <h1>PasteCanvas</h1>
          <p className="brand-note">先收集，再整理。</p>
        </div>
        <nav className="sidebar-nav">
          <button
            type="button"
            aria-label="收件匣"
            className={`nav-item ${view === "inbox" ? "active" : ""}`}
            onClick={() => setView("inbox")}
          >
            <Note className="nav-glyph" aria-hidden="true" /><span>收件匣</span>
          </button>
          <button type="button" className={`nav-item ${view === "inbox" ? "" : ""}`} onClick={() => {
            setView("inbox");
            window.dispatchEvent(new CustomEvent("paste-canvas://search-command", { detail: { query: "favorite:true" } }));
          }} title="顯示收藏項目">
            <Star className="nav-glyph" aria-hidden="true" /><span>收藏</span>
          </button>
          <button type="button" aria-label="搜尋" className="nav-item nav-search-link" onClick={() => {
            setView("inbox");
            window.dispatchEvent(new Event("paste-canvas://focus-search"));
          }}>
            <MagnifyingGlass className="nav-glyph" aria-hidden="true" /><span>搜尋</span><kbd>Ctrl K</kbd>
          </button>
          {recentBoards.length > 0 && (
            <div className="nav-section nav-recent">
              <div className="nav-section-header">
                <span>最近看板</span>
              </div>
              {recentBoards.map((b) => (
                <button
                  type="button"
                  key={`recent-${b.id}`}
                  className={`nav-item nav-board ${view === "board" && activeBoard?.id === b.id ? "active" : ""}`}
                  onClick={() => handleSelectBoard(b)}
                >
                  <ClockCounterClockwise className="nav-glyph" aria-hidden="true" /><span>{b.name}</span>
                </button>
              ))}
            </div>
          )}
          <div className="nav-section">
            <div className="nav-section-header">
              <span>看板</span>
              <span className="nav-section-actions">
                <button type="button" className="btn-add-board btn-wide" title="新增資料夾" onClick={() => void handleCreateFolder()}><FolderSimple size={15} /> <span>資料夾</span></button>
                <button type="button" className="btn-add-board" title="新增看板" onClick={() => void handleCreateBoard()}><Plus size={16} /></button>
              </span>
            </div>
            {tree.flatMap((node) => renderTreeNode(node, 0))}
          </div>
          <div className="nav-footer">
            <SessionControl />
            <button
              type="button"
              aria-label="垃圾桶"
              className={`nav-item ${view === "trash" ? "active" : ""}`}
              onClick={() => setView("trash")}
            >
              <Trash className="nav-glyph" aria-hidden="true" /><span>垃圾桶</span>
            </button>
            <button
              type="button"
              aria-label="設定"
              className={`nav-item ${view === "settings" ? "active" : ""}`}
              onClick={() => setView("settings")}
            >
              <GearSix className="nav-glyph" aria-hidden="true" /><span>設定</span>
            </button>
          </div>
        </nav>
      </aside>

      <div className="main-content">
        <header className="command-bar">
          <div className="command-context">
            <span className="context-mark" aria-hidden="true" />
            <div>
              <p className="eyebrow">工作區</p>
              <p className="command-title">{view === "board" && activeBoard ? activeBoard.name : view === "settings" ? "設定" : view === "trash" ? "垃圾桶" : "收件匣"}</p>
            </div>
          </div>
          <SearchBar />
          <div className="command-status" aria-label="擷取狀態">
            <span className={`status-dot ${sessionState ?? "idle"}`} aria-hidden="true" />
            <span>{sessionState === "active" ? "收集中 · 工作階段進行中" : sessionState === "paused" ? "工作階段已暫停" : "僅限本機 · 未監看"}</span>
          </div>
          <button type="button" className="command-capture" onClick={() => {
            setView("inbox");
            if (isTauri()) {
              void invoke("show_quick_capture").catch(() => {
                window.dispatchEvent(new Event("paste-canvas://focus-capture"));
              });
            } else {
              window.dispatchEvent(new Event("paste-canvas://focus-capture"));
            }
          }} aria-label="開啟快速擷取">
            <Plus aria-hidden="true" /> 擷取
          </button>
        </header>
        {view === "inbox" && (
          <>
            <QuickCapture />
            <InboxView
              openItemId={pendingOpenItemId}
              onOpenItemHandled={() => setPendingOpenItemId(null)}
            />
          </>
        )}
        {view === "board" && activeBoard && (
          <>
            {boardStack.length > 1 && (
              <nav className="board-breadcrumb" aria-label="看板導覽">
                {boardStack.map((b, i) => (
                  <span key={b.id} className="breadcrumb-item">
                    {i > 0 && <span className="breadcrumb-sep">›</span>}
                    <button
                      className={`breadcrumb-link ${i === boardStack.length - 1 ? "current" : ""}`}
                      onClick={() => handleBreadcrumbClick(i)}
                    >
                      {b.name}
                    </button>
                  </span>
                ))}
              </nav>
            )}
            <BoardView
              key={activeBoard.id}
              board={activeBoard}
              onEnterBoard={(id) => void handleEnterBoard(id)}
            />
          </>
        )}
        {view === "settings" && <SettingsView theme={theme} onThemeChange={handleThemeChange} />}
        {view === "trash" && <TrashView />}
      </div>

      <ToastContainer />
    </main>
  );
}
