import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  decideCaptureToggle,
  toggleCapturePause,
  subscribeCapturePauseToggle,
  TOGGLE_CAPTURE_PAUSE_EVENT,
} from "../features/session/sessionToggle";
import type { SessionInfo } from "../features/session/api";

const mockGetSession = vi.fn();
const mockPauseSession = vi.fn();
const mockResumeSession = vi.fn();
vi.mock("../features/session/api", () => ({
  getSession: (...args: unknown[]) => mockGetSession(...args),
  pauseSession: (...args: unknown[]) => mockPauseSession(...args),
  resumeSession: (...args: unknown[]) => mockResumeSession(...args),
}));

function session(state: string): SessionInfo {
  return { id: "s1", name: null, state, startedAt: 1, itemCount: 0 };
}

describe("sessionToggle (WIN-001 / CAP-004 tray pause-resume)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetSession.mockResolvedValue(null);
    mockPauseSession.mockResolvedValue(session("paused"));
    mockResumeSession.mockResolvedValue(session("active"));
  });

  describe("decideCaptureToggle (pure)", () => {
    it("pauses an active session", () => {
      expect(decideCaptureToggle(session("active"))).toBe("pause");
    });

    it("resumes a paused session", () => {
      expect(decideCaptureToggle(session("paused"))).toBe("resume");
    });

    it("never starts monitoring from a tray toggle (privacy-by-default)", () => {
      expect(decideCaptureToggle(null)).toBe("none");
      expect(decideCaptureToggle(session("finished"))).toBe("none");
    });
  });

  describe("toggleCapturePause", () => {
    it("pauses when active", async () => {
      mockGetSession.mockResolvedValue(session("active"));
      await toggleCapturePause();
      expect(mockPauseSession).toHaveBeenCalledTimes(1);
      expect(mockResumeSession).not.toHaveBeenCalled();
    });

    it("resumes when paused", async () => {
      mockGetSession.mockResolvedValue(session("paused"));
      await toggleCapturePause();
      expect(mockResumeSession).toHaveBeenCalledTimes(1);
      expect(mockPauseSession).not.toHaveBeenCalled();
    });

    it("does nothing without a session", async () => {
      mockGetSession.mockResolvedValue(null);
      await toggleCapturePause();
      expect(mockPauseSession).not.toHaveBeenCalled();
      expect(mockResumeSession).not.toHaveBeenCalled();
    });

    it("swallows backend errors (tray must not surface failures)", async () => {
      mockGetSession.mockRejectedValue(new Error("db locked"));
      await expect(toggleCapturePause()).resolves.toBeUndefined();
    });
  });

  it("subscribe wires the window event and cleans up", async () => {
    mockGetSession.mockResolvedValue(session("active"));
    const stop = subscribeCapturePauseToggle();

    window.dispatchEvent(new CustomEvent(TOGGLE_CAPTURE_PAUSE_EVENT));
    // event handler runs the async toggle; let microtasks settle
    await new Promise((r) => setTimeout(r, 0));
    expect(mockPauseSession).toHaveBeenCalledTimes(1);

    stop();
    window.dispatchEvent(new CustomEvent(TOGGLE_CAPTURE_PAUSE_EVENT));
    await new Promise((r) => setTimeout(r, 0));
    expect(mockPauseSession).toHaveBeenCalledTimes(1); // unchanged after cleanup
  });
});
