import { useState, useEffect, useCallback } from "react";
import { getSession, startSession, pauseSession, resumeSession, finishSession } from "./api";
import type { SessionInfo } from "./api";
import { showSuspendNotice } from "../../stores/toastStore";
import { TOGGLE_CAPTURE_PAUSE_EVENT, toggleCapturePause } from "./sessionToggle";
import { Pause, Play, Stop, Circle } from "@phosphor-icons/react";

export function SessionControl() {
  const [session, setSession] = useState<SessionInfo | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await getSession();
      setSession(s);
      window.dispatchEvent(new CustomEvent("paste-canvas://session-state", {
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
  // Listens on the unified window bridge (tauriEvents) instead of a direct
  // Tauri listen — single channel for all backend events (DP6).
  useEffect(() => {
    const handler = () => {
      void refresh();
      showSuspendNotice("系統休眠，擷取工作階段已暫停。準備好後請手動恢復。");
    };
    window.addEventListener("paste-canvas://session-suspended", handler);
    return () => window.removeEventListener("paste-canvas://session-suspended", handler);
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
        <button className="btn-session-start" onClick={() => void handleStart()} title="開始擷取工作階段">
          <Circle weight="fill" aria-hidden="true" /> 開始工作階段
        </button>
      </div>
    );
  }

  return (
    <div className={`session-control session-${session.state}`}>
      <div className="session-indicator" role="status" aria-live="polite">
        <span className={`session-dot ${session.state}`} aria-hidden="true" />
        <span className="session-label">
          {session.state === "active"
            ? `擷取中${session.itemCount > 0 ? ` · 已收集 ${session.itemCount} 個項目` : ""}`
            : session.state === "paused"
              ? "已暫停"
              : "已結束"}
        </span>
      </div>
      <div className="session-actions">
        {session.state === "active" && (
          <button className="btn-session-pause" onClick={() => void handlePause()} title="暫停擷取" aria-label="暫停擷取">
            <Pause aria-hidden="true" />
          </button>
        )}
        {session.state === "paused" && (
          <button className="btn-session-resume" onClick={() => void handleResume()} title="恢復擷取" aria-label="恢復擷取">
            <Play aria-hidden="true" />
          </button>
        )}
        <button className="btn-session-stop" onClick={() => void handleFinish()} title="結束工作階段" aria-label="結束工作階段">
            <Stop aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
