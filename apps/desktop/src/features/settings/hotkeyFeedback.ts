/**
 * FND-006 hotkey registration failure feedback.
 *
 * PRD acceptance: "Registration failure is visible and recoverable."
 * Startup failures are drained from the backend once and shown as an error
 * toast pointing the user to Settings, where the shortcut can be rebound.
 */
import { useToastStore } from "../../stores/toastStore";
import { getStartupHotkeyFailures } from "./api";

/** Builds the user-facing message for registration failures. */
export function hotkeyFailureMessage(failures: string[]): string {
  return (
    `全域快捷鍵註冊失敗：${failures.join(", ")}。` +
    "其他應用程式可能正在使用此快捷鍵，請至設定重新綁定。"
  );
}

/** Fetches startup hotkey failures (once) and shows a toast when present. */
export async function surfaceStartupHotkeyFailures(): Promise<void> {
  try {
    const failures = await getStartupHotkeyFailures();
    if (failures.length === 0) return;
    useToastStore.getState().push({
      message: hotkeyFailureMessage(failures),
      variant: "error",
    });
  } catch {
    // Not running in Tauri or backend unavailable — nothing to surface.
  }
}
