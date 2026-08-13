/**
 * CAP-004 / WIN-001 capture-pause toggle.
 *
 * The tray menu's "Pause / Resume Session" emits a Tauri event that the event
 * bridge forwards to a window event; this module implements the toggle
 * semantics so the behavior is unit-testable without a Tauri runtime.
 */
import { getSession, pauseSession, resumeSession } from "./api";
import type { SessionInfo } from "./api";

/** Window event name forwarded by the event bridge from the tray. */
export const TOGGLE_CAPTURE_PAUSE_EVENT = "paste-canvas://toggle-capture-pause";

/**
 * Toggle logic (pure): returns the action to take for a session state.
 * - active → pause
 * - paused → resume
 * - no session / finished → nothing (the user must explicitly start a session;
 *   privacy-by-default — a stray tray click never STARTS monitoring)
 */
export function decideCaptureToggle(
  session: SessionInfo | null,
): "pause" | "resume" | "none" {
  if (!session) return "none";
  if (session.state === "active") return "pause";
  if (session.state === "paused") return "resume";
  return "none";
}

/** Executes the toggle against the backend. Errors are non-fatal. */
export async function toggleCapturePause(): Promise<void> {
  try {
    const session = await getSession();
    switch (decideCaptureToggle(session)) {
      case "pause":
        await pauseSession();
        break;
      case "resume":
        await resumeSession();
        break;
      case "none":
        break;
    }
  } catch {
    // Tray toggles must never surface errors; state is visible in the UI.
  }
}

/** Subscribes the window listener; returns the cleanup function. */
export function subscribeCapturePauseToggle(): () => void {
  const handler = () => {
    void toggleCapturePause();
  };
  window.addEventListener(TOGGLE_CAPTURE_PAUSE_EVENT, handler);
  return () => window.removeEventListener(TOGGLE_CAPTURE_PAUSE_EVENT, handler);
}
