/**
 * Global keyboard shortcuts hook.
 * Implements the keyboard-first principle from GOAL.md.
 *
 * Shortcuts:
 * - Ctrl+Shift+V: Capture clipboard (handled by Rust global hotkey)
 * - Ctrl+K: Focus search
 * - Ctrl+Z: Undo (canvas)
 * - Ctrl+Shift+Z: Redo (canvas)
 * - Escape: Close modals/panels
 */
import { useEffect } from "react";

interface KeyboardShortcuts {
  onFocusSearch?: () => void;
  onEscape?: () => void;
  onNewBoard?: () => void;
}

export function useKeyboardShortcuts({
  onFocusSearch,
  onEscape,
  onNewBoard,
}: KeyboardShortcuts) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isCtrl = e.ctrlKey || e.metaKey;

      // Ctrl+K: Focus search
      if (isCtrl && e.key === "k") {
        e.preventDefault();
        onFocusSearch?.();
        return;
      }

      // Ctrl+Shift+B: New board
      if (isCtrl && e.shiftKey && e.key === "b") {
        e.preventDefault();
        onNewBoard?.();
        return;
      }

      // Escape: Close modals
      if (e.key === "Escape") {
        onEscape?.();
        return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onFocusSearch, onEscape, onNewBoard]);
}
