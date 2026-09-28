/**
 * WIN-005: bridges backend Tauri events into window CustomEvents so feature
 * components can stay framework-agnostic (and E2E-testable in a plain
 * browser). Mounted once in App; outside the Tauri runtime it is a no-op.
 */
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "./runtime";

const FORWARDED_EVENTS = [
  "paste-canvas://show-settings",
  "paste-canvas://focus-search",
  "paste-canvas://toggle-capture-pause",
  "paste-canvas://item-captured",
  "paste-canvas://capture-success",
  "paste-canvas://capture-failed",
  "paste-canvas://session-suspended",
] as const;

/**
 * Starts forwarding Tauri events to window listeners.
 * Returns a cleanup function that removes all listeners.
 */
export async function startTauriEventBridge(): Promise<() => void> {
  if (!isTauri()) return () => {};

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
