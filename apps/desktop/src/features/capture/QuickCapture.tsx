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
    window.addEventListener("pastecanvas://focus-capture", handler);
    return () => window.removeEventListener("pastecanvas://focus-capture", handler);
  }, []);

  const handleFilesDropped = useCallback(
    async (paths: string[]) => {
      setStatus("saving");
      try {
        await captureFiles(paths, targetBoard || undefined);
        setStatus("saved");
        window.dispatchEvent(new CustomEvent("pastecanvas:item-captured"));
        showCaptureSuccess(
          paths.length === 1 ? "File captured" : `${paths.length} files captured`,
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
      window.dispatchEvent(new CustomEvent("pastecanvas:item-captured"));

      if (result.isDuplicate && result.duplicateOf) {
        showDedupNotice(result.duplicateOf);
      } else {
        showCaptureSuccess("Captured");
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
    <main className={`quick-capture-window ${isFloating ? "floating-quick-capture" : "inline-quick-capture"} ${dragOver ? "drag-over" : ""}`} aria-label="Quick Capture">
      <div className="capture-ribbon" aria-hidden="true" />
      <div className="quick-capture-heading">
        <div>
          <p className="eyebrow">PasteCanvas</p>
          <h1>Quick Capture</h1>
        </div>
        {isFloating && (
          <button className="btn-close-capture" type="button" onClick={closeFloating} aria-label="Close Quick Capture">
            Esc
          </button>
        )}
      </div>
      <div className="quick-capture">
      <textarea
        ref={inputRef}
        className="capture-input"
        placeholder={
          dragOver
            ? "Drop files to capture…"
            : "Paste text, a link, image, or file…"
        }
        aria-label="Capture content"
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
            title="Optional target board"
          >
            <option value="">Inbox only</option>
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
          {status === "saving" ? "Saving…" : "Capture"}
        </button>
        {status === "saved" && <span className="capture-success">✓ Saved</span>}
        {status === "error" && (
          <span className="capture-error" role="alert">
            Save failed. <button type="button" onClick={() => void handleSubmit()}>Try again</button>
          </span>
        )}
      </div>
      <p className="capture-footnote"><kbd>Enter</kbd> save <span aria-hidden="true">·</span> <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>Enter</kbd> save and keep open <span aria-hidden="true">·</span> <kbd>Esc</kbd> close</p>
      </div>
    </main>
  );
}
