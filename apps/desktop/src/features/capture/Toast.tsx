/**
 * Toast notification container — renders the global toast stack.
 * Per PRD CAP-001: lightweight capture feedback.
 * Per PRD INB-004: dedup notice with "open existing" action.
 */
import { useToastStore } from "../../stores/toastStore";

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-container" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast-${toast.variant}`} role={toast.variant === "error" ? "alert" : undefined}>
          <span className="toast-message">{toast.message}</span>
          {toast.action && (
            <button
              className="toast-action"
              onClick={() => {
                toast.action!.onAction();
                dismiss(toast.id);
              }}
            >
              {toast.action.label}
            </button>
          )}
          <button
            className="toast-close"
            onClick={() => dismiss(toast.id)}
            aria-label="關閉通知"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
