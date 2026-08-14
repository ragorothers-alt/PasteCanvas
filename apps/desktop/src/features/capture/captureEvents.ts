/**
 * CAP-001 capture feedback (PRD compliance).
 *
 * PRD CAP-001: "成功時顯示輕量提示" and "失敗時提供可理解原因與重試".
 * The backend emits capture success/failure events from the global hotkey and
 * tray paths; these helpers turn them into user-visible toasts so no capture
 * outcome is ever silent.
 */
import { useToastStore, showCaptureSuccess } from "../../stores/toastStore";

/** Builds the user-facing failure message, tolerating any payload shape. */
export function captureFailureMessage(reason: unknown): string {
  const detail =
    typeof reason === "string" && reason.trim().length > 0
      ? reason.trim()
      : "未知錯誤";
  return `擷取失敗：${detail}。你可以再試一次。`;
}

/** Shows the failure toast with an understandable reason (CAP-001). */
export function notifyCaptureFailure(reason: unknown): void {
  useToastStore.getState().push({
    message: captureFailureMessage(reason),
    variant: "error",
  });
}

/** Shows the lightweight success toast (CAP-001). */
export function notifyCaptureSuccess(): void {
  showCaptureSuccess("已擷取至收件匣");
}

/** All window event names carrying capture outcomes (backend emitters). */
export const CAPTURE_FAILURE_EVENTS = [
  "capture://failed", // global hotkey path
  "paste-canvas://capture-failed", // tray path
] as const;

export const CAPTURE_SUCCESS_EVENTS = [
  "capture://success", // global hotkey path
  "paste-canvas://capture-success", // tray path
] as const;

/**
 * Subscribes a capture-outcome listener set on window; returns the cleanup
 * function. Kept framework-free so App can mount it in one effect.
 */
export function subscribeCaptureFeedback(): () => void {
  const onFailure = (e: Event) => {
    notifyCaptureFailure((e as CustomEvent).detail);
  };
  const onSuccess = () => notifyCaptureSuccess();

  for (const name of CAPTURE_FAILURE_EVENTS) {
    window.addEventListener(name, onFailure);
  }
  for (const name of CAPTURE_SUCCESS_EVENTS) {
    window.addEventListener(name, onSuccess);
  }
  return () => {
    for (const name of CAPTURE_FAILURE_EVENTS) {
      window.removeEventListener(name, onFailure);
    }
    for (const name of CAPTURE_SUCCESS_EVENTS) {
      window.removeEventListener(name, onSuccess);
    }
  };
}
