/**
 * WIN-005: bridges backend Tauri events into window CustomEvents so feature
 * components can stay framework-agnostic (and E2E-testable in a plain
 * browser). Mounted once in App; outside the Tauri runtime it is a no-op.
 */
import { isTauri } from "./runtime";

const FORWARDED_EVENTS = [
  "paste-canvas://show-settings",
  "paste-canvas://focus-search",
  "paste-canvas://toggle-capture-pause",
  // CAP-009: out-of-process captures (tray / explorer / browser) refresh Inbox
  "pastecanvas:item-captured",
  // CAP-001: capture outcome feedback (hotkey + tray emitters)
  "capture://success",
  "capture://failed",
  "paste-canvas://capture-success",
  "paste-canvas://capture-failed",
] as const;

/**
 * Starts forwarding Tauri events to window listeners.
 * Returns a cleanup function that removes all listeners.
 */
export async function startTauriEventBridge(): Promise<() => void> {
  if (!isTauri()) return () => {};

  const { listen } = await import("@tauri-apps/api/event");
  const unlisteners = await Promise.all(
    FORWARDED_EVENTS.map((name) =>
      listen<unknown>(name, (e) => {
        window.dispatchEvent(new CustomEvent(name, { detail: e.payload }));
      }),
    ),
  );
  return () => {
    for (const unlisten of unlisteners) unlisten();
  };
}
