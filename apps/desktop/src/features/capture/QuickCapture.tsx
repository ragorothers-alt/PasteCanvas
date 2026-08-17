/**
 * Quick Capture (CAP-002).
 * Supports: paste/type text, drag-in files (image/PDF/font/any file),
 * optional target Board, Enter to save, Esc to close the floating window.
 */
import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { currentWindowLabel, isTauri } from "../../lib/runtime";
import { captureText, captureFiles } from "../inbox/api";
import { listBoards, placeItemOnBoard } from "../boards/api";
import type { BoardSummary } from "../boards/api";
import { showCaptureSuccess, showDedupNotice } from "../../stores/toastStore";
import { Check, FloppyDisk, X } from "@phosphor-icons/react";

export function QuickCapture() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [boards, setBoards] = useState<BoardSummary[]>([]);
  const [targetBoard, setTargetBoard] = useState<string>("");
  const [dragOver, setDragOver] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);

  const isFloating = currentWindowLabel() === "quick-capture";

  // CAP-002: close the floating window and return focus to the app the user
  // was in before Quick Capture opened (backend restores the foreground).
  const closeFloating = useCallback(() => {
    if (!isFloating || !isTauri()) return;
    void invoke("close_quick_capture").catch(() => {
      /* window may already be gone — non-fatal */
    });
  }, [isFloating]);

  // Load boards for the optional target selector (CAP-002)
  useEffect(() => {
    void listBoards().then(setBoards).catch(() => setBoards([]));
  }, []);

  useEffect(() => {
    const handler = () => inputRef.current?.focus();
    window.addEventListener("paste-canvas://focus-capture", handler);
    return () => window.removeEventListener("paste-canvas://focus-capture", handler);
  }, []);

  const handleFilesDropped = useCallback(
    async (paths: string[]) => {
      setStatus("saving");
      try {
        await captureFiles(paths, targetBoard || undefined);
        setStatus("saved");
        window.dispatchEvent(new CustomEvent("paste-canvas://item-captured"));
        showCaptureSuccess(
          paths.length === 1 ? "檔案已擷取" : `已擷取 ${paths.length} 個檔案`,
        );
        // CAP-002: brief confirmation, then close + restore previous focus
        setTimeout(() => closeFloating(), 300);
      } catch {
        setStatus("error");
      }
    },
    [targetBoard, closeFloating],
  );

  // File drag-in via Tauri webview drag-drop events (real OS file paths)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    try {
      const webview = getCurrentWebview();
      void webview
        .onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === "enter" || payload.type === "over") {
            setDragOver(true);
          } else if (payload.type === "leave") {
            setDragOver(false);
          } else if (payload.type === "drop") {
            setDragOver(false);
            if (payload.paths.length > 0) {
              void handleFilesDropped(payload.paths);
            }
          }
        })
        .then((fn) => {
          if (cancelled) {
            fn();
          } else {
            unlisten = fn;
          }
        });
    } catch {
      // Drag-drop unavailable (e.g., non-Tauri test environment)
    }

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [targetBoard]);

  const handleSubmit = useCallback(async (keepOpen = false) => {
    if (!text.trim()) return;
    setStatus("saving");
    try {
      const result = await captureText({ text: text.trim() });
      // CAP-002: optional target Board — place the captured item on it
      if (targetBoard) {
        try {
          await placeItemOnBoard(targetBoard, result.itemId, 100, 100);
        } catch {
          // Placement failure should not fail the capture itself
        }
      }
      setText("");
      setStatus("saved");
      window.dispatchEvent(new CustomEvent("paste-canvas://item-captured"));

      if (result.isDuplicate && result.duplicateOf) {
        showDedupNotice(result.duplicateOf);
      } else {
        showCaptureSuccess("已擷取");
      }
      // CAP-002: brief confirmation, then close + restore previous focus.
      // Inline (main-window) capture keeps the panel open.
      if (isFloating && !keepOpen) {
        setTimeout(() => closeFloating(), 300);
      } else {
        setTimeout(() => setStatus("idle"), 1500);
      }
    } catch {
      setStatus("error");
    }
  }, [text, targetBoard, isFloating, closeFloating]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // CAP-002: Enter saves, Esc closes the floating window (focus returns
      // to the previous application via close_quick_capture).
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        // Ctrl/Cmd+Enter saves but keeps Quick Capture open for a second item.
        void handleSubmit(e.ctrlKey || e.metaKey);
      }
      if (e.key === "Escape" && isFloating) {
        closeFloating();
      }
    },
    [handleSubmit, isFloating, closeFloating],
  );

  return (
    <main className={`quick-capture-window ${isFloating ? "floating-quick-capture" : "inline-quick-capture"} ${dragOver ? "drag-over" : ""}`} aria-label="快速擷取">
      {isFloating && <>
        <div className="capture-ribbon" aria-hidden="true" />
        <div className="quick-capture-heading">
          <div>
            <p className="eyebrow">PasteCanvas</p>
            <h1>快速擷取</h1>
          </div>
          <button className="btn-close-capture" type="button" onClick={closeFloating} aria-label="關閉快速擷取">
            <X aria-hidden="true" /> Esc
          </button>
        </div>
      </>}
      <div className="quick-capture">
      <textarea
        ref={inputRef}
        className="capture-input"
        placeholder={
          dragOver
            ? "放開檔案即可擷取…"
            : "貼上文字、連結、圖片或檔案…"
        }
        aria-label="擷取內容"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        rows={2}
      />
      <div className="capture-actions">
        {boards.length > 0 && (
          <select
            className="capture-board-select"
            value={targetBoard}
            onChange={(e) => setTargetBoard(e.target.value)}
            title="選填的目標看板"
          >
            <option value="">僅收件匣</option>
            {boards.map((b) => (
              <option key={b.id} value={b.id}>
                → {b.name}
              </option>
            ))}
          </select>
        )}
        <button
          className="btn-capture"
          onClick={() => void handleSubmit()}
          disabled={!text.trim() || status === "saving"}
        >
          {status === "saving" ? "儲存中…" : <><FloppyDisk aria-hidden="true" /> 擷取</>}
        </button>
        {status === "saved" && <span className="capture-success"><Check aria-hidden="true" /> 已儲存</span>}
        {status === "error" && (
          <span className="capture-error" role="alert">
            儲存失敗。<button type="button" onClick={() => void handleSubmit()}>再試一次</button>
          </span>
        )}
      </div>
      {isFloating && <p className="capture-footnote"><kbd>Enter</kbd> 儲存 <span aria-hidden="true">·</span> <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>Enter</kbd> 儲存並保持開啟 <span aria-hidden="true">·</span> <kbd>Esc</kbd> 關閉</p>}
      </div>
    </main>
  );
}
