import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  hotkeyFailureMessage,
  surfaceStartupHotkeyFailures,
} from "../features/settings/hotkeyFeedback";
import { useToastStore } from "../stores/toastStore";

const mockGetFailures = vi.fn();
vi.mock("../features/settings/api", () => ({
  getStartupHotkeyFailures: (...args: unknown[]) => mockGetFailures(...args),
}));

describe("hotkeyFeedback (FND-006 visible + recoverable failures)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useToastStore.getState().clearAll();
  });

  it("formats a single failure with recovery guidance", () => {
    const msg = hotkeyFailureMessage(["capture_clipboard (already registered)"]);
    expect(msg).toContain("capture_clipboard");
    expect(msg).toContain("already registered");
    expect(msg).toContain("Settings");
  });

  it("joins multiple failures", () => {
    const msg = hotkeyFailureMessage(["a (x)", "b (y)"]);
    expect(msg).toContain("a (x), b (y)");
  });

  it("shows an error toast when failures exist", async () => {
    mockGetFailures.mockResolvedValue(["capture_clipboard (conflict)"]);
    await surfaceStartupHotkeyFailures();
    const toasts = useToastStore.getState().toasts;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]!.variant).toBe("error");
    expect(toasts[0]!.message).toContain("capture_clipboard (conflict)");
  });

  it("stays silent when there are no failures", async () => {
    mockGetFailures.mockResolvedValue([]);
    await surfaceStartupHotkeyFailures();
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("swallows backend errors (browser-only render)", async () => {
    mockGetFailures.mockRejectedValue(new Error("no tauri"));
    await expect(surfaceStartupHotkeyFailures()).resolves.toBeUndefined();
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });
});
