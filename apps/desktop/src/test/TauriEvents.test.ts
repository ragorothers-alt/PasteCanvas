import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const mockListen = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: unknown;
}

describe("startTauriEventBridge (WIN-005 / CAP-009)", () => {
  beforeEach(() => {
    (window as TauriWindow).__TAURI_INTERNALS__ = {};
    mockListen.mockImplementation(() => Promise.resolve(() => {}));
  });

  afterEach(() => {
    delete (window as TauriWindow).__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("subscribes to all forwarded events inside the Tauri runtime", async () => {
    const { startTauriEventBridge } = await import("../lib/tauriEvents");
    const stop = await startTauriEventBridge();

    const names = mockListen.mock.calls.map((c) => c[0]);
    expect(names).toContain("paste-canvas://show-settings");
    expect(names).toContain("paste-canvas://focus-search");
    expect(names).toContain("pastecanvas:item-captured");
    stop();
  });

  it("forwards a backend capture event to window listeners", async () => {
    type Handler = (e: { payload: unknown }) => void;
    let captureHandler: Handler | null = null;
    mockListen.mockImplementation((name: string, handler: Handler) => {
      if (name === "pastecanvas:item-captured") captureHandler = handler;
      return Promise.resolve(() => {});
    });

    const { startTauriEventBridge } = await import("../lib/tauriEvents");
    const stop = await startTauriEventBridge();

    const received = new Promise<string>((resolve) => {
      window.addEventListener(
        "pastecanvas:item-captured",
        (e) => resolve(e.type),
        { once: true },
      );
    });
    expect(captureHandler).not.toBeNull();
    captureHandler!({ payload: null });
    expect(await received).toBe("pastecanvas:item-captured");
    stop();
  });

  it("is a no-op outside the Tauri runtime (browser/E2E)", async () => {
    delete (window as TauriWindow).__TAURI_INTERNALS__;
    const { startTauriEventBridge } = await import("../lib/tauriEvents");
    const stop = await startTauriEventBridge();
    expect(mockListen).not.toHaveBeenCalled();
    stop();
  });
});
