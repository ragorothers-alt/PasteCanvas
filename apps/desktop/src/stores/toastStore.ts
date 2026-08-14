/**
 * Toast notification store — lightweight feedback for capture results.
 * Per PRD CAP-001: "成功時顯示輕量提示"
 * Per PRD INB-004: dedup notice offers "open existing item" action.
 *
 * Bounded queue (max 5 toasts) with auto-dismiss timers.
 */
import { create } from "zustand";

export type ToastVariant = "success" | "info" | "dedup" | "error";

export interface ToastAction {
  label: string;
  onAction: () => void;
}

export interface Toast {
  id: string;
  message: string;
  variant: ToastVariant;
  /** Optional action button (e.g., "Open existing item" for dedup). */
  action?: ToastAction;
  /** ID of related item (for dedup navigation). */
  relatedItemId?: string;
}

const MAX_TOASTS = 5;
const DEFAULT_DURATION_MS = 4000;
const DEDUP_DURATION_MS = 7000; // Dedup notices stay longer for user action

interface ToastState {
  toasts: Toast[];
  /** Push a toast; auto-dismisses after duration. */
  push: (toast: Omit<Toast, "id">) => string;
  dismiss: (id: string) => void;
  clearAll: () => void;
}

let toastCounter = 0;

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],

  push: (toast) => {
    const id = `toast-${++toastCounter}-${Date.now()}`;
    const fullToast: Toast = { ...toast, id };

    set((state) => ({
      toasts: [...state.toasts.slice(-(MAX_TOASTS - 1)), fullToast],
    }));

    // Schedule auto-dismiss
    const duration =
      toast.variant === "dedup" ? DEDUP_DURATION_MS : DEFAULT_DURATION_MS;
    setTimeout(() => {
      get().dismiss(id);
    }, duration);

    return id;
  },

  dismiss: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },

  clearAll: () => {
    set({ toasts: [] });
  },
}));

/** Convenience: show a capture-success toast. */
export function showCaptureSuccess(message: string): void {
  useToastStore.getState().push({
    message,
    variant: "success",
  });
}

/** Surface a user-action failure without swallowing the native error. */
export function showCaptureError(message: string): void {
  useToastStore.getState().push({ message, variant: "error" });
}

/** Convenience: WIN-004 — system slept, capture session was paused. */
export function showSuspendNotice(message: string): void {
  useToastStore.getState().push({
    message,
    variant: "info",
  });
}

/** Convenience: show a dedup notice with "open existing" action. */
export function showDedupNotice(existingItemId: string): void {
  useToastStore.getState().push({
    message: "已有相似內容",
    variant: "dedup",
    relatedItemId: existingItemId,
    action: {
      label: "開啟既有項目",
      onAction: () => {
        // Dispatch custom event for navigation handlers to open the existing item
        window.dispatchEvent(
          new CustomEvent("paste-canvas://open-item", {
            detail: { itemId: existingItemId },
          }),
        );
      },
    },
  });
}
