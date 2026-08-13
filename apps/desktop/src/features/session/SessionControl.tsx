import { useState, useEffect, useCallback } from "react";
import { getSession, startSession, pauseSession, resumeSession, finishSession } from "./api";
import type { SessionInfo } from "./api";
import { isTauri } from "../../lib/runtime";
import { showSuspendNotice } from "../../stores/toastStore";
import { TOGGLE_CAPTURE_PAUSE_EVENT, toggleCapturePause } from "./sessionToggle";

export function SessionControl() {
  const [session, setSession] = useState<SessionInfo | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await getSession();
      setSession(s);
      window.dispatchEvent(new CustomEvent("pastecanvas://session-state", {
        detail: { state: s?.state ?? null },
      }));
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    void refresh();
    const interval = setInterval(() => void refresh(), 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  // WIN-004: backend pauses capture sessions when the system sleeps;
  // reflect it immediately and tell the user capture did NOT auto-resume.
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | null = null;
    import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<number>("paste-canvas://session-suspended", () => {
          void refresh();
          showSuspendNotice("System slept — capture session paused. Resume when ready.");
        }),
      )
      .then((u) => {
        if (disposed) {
          u();
        } else {
          unlisten = u;
        }
      })
      .catch(() => { /* browser-only render */ });
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [refresh]);

  const handleStart = useCallback(async () => {
    try {
      await startSession();
      void refresh();
    } catch { /* ignore */ }
  }, [refresh]);

  const handlePause = useCallback(async () => {
    try {
      await pauseSession();
      void refresh();
    } catch { /* ignore */ }
  }, [refresh]);

  const handleResume = useCallback(async () => {
    try {
      await resumeSession();
      void refresh();
    } catch { /* ignore */ }
  }, [refresh]);

  const handleFinish = useCallback(async () => {
    try {
      await finishSession();
      void refresh();
    } catch { /* ignore */ }
  }, [refresh]);

  // WIN-001/CAP-004: tray "Pause / Resume Session" — forwarded by the event
  // bridge; toggle then refresh so the indicator reflects the new state.
  useEffect(() => {
    const handler = () => {
      void toggleCapturePause().then(() => refresh());
    };
    window.addEventListener(TOGGLE_CAPTURE_PAUSE_EVENT, handler);
    return () => window.removeEventListener(TOGGLE_CAPTURE_PAUSE_EVENT, handler);
  }, [refresh]);

  if (!session) {
    return (
      <div className="session-control">
        <button className="btn-session-start" onClick={() => void handleStart()} title="Start capture session">
          ● Start Session
        </button>
      </div>
    );
  }

  return (
    <div className={`session-control session-${session.state}`}>
      <div className="session-indicator">
        <span className={`session-dot ${session.state}`} />
        <span className="session-label">
          {session.state === "active" ? "Recording" : "Paused"}
          {session.itemCount > 0 && ` (${session.itemCount})`}
        </span>
      </div>
      <div className="session-actions">
        {session.state === "active" && (
          <button className="btn-session-pause" onClick={() => void handlePause()}>
            ⏸
          </button>
        )}
        {session.state === "paused" && (
          <button className="btn-session-resume" onClick={() => void handleResume()}>
            ▶
          </button>
        )}
        <button className="btn-session-stop" onClick={() => void handleFinish()}>
          ⏹
        </button>
      </div>
    </div>
  );
}
