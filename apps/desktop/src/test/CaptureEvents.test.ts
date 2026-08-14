import { describe, it, expect, beforeEach } from "vitest";
import {
  captureFailureMessage,
  notifyCaptureFailure,
  notifyCaptureSuccess,
  subscribeCaptureFeedback,
} from "../features/capture/captureEvents";
import { useToastStore } from "../stores/toastStore";

describe("captureEvents (CAP-001 PRD feedback)", () => {
  beforeEach(() => {
    useToastStore.getState().clearAll();
  });

  describe("captureFailureMessage", () => {
    it("includes the backend reason", () => {
      expect(captureFailureMessage("Clipboard is empty")).toBe(
        "擷取失敗：Clipboard is empty。你可以再試一次。",
      );
    });

    it("trims whitespace-only reasons into a fallback", () => {
      expect(captureFailureMessage("   ")).toContain("未知錯誤");
    });

    it("tolerates non-string payloads", () => {
      expect(captureFailureMessage(undefined)).toContain("未知錯誤");
      expect(captureFailureMessage({ code: 5 })).toContain("未知錯誤");
    });
  });

  it("notifyCaptureFailure pushes an error toast", () => {
    notifyCaptureFailure("Clipboard read failed");
    const toasts = useToastStore.getState().toasts;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]!.variant).toBe("error");
    expect(toasts[0]!.message).toContain("Clipboard read failed");
  });

  it("notifyCaptureSuccess pushes a success toast", () => {
    notifyCaptureSuccess();
    const toasts = useToastStore.getState().toasts;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]!.variant).toBe("success");
  });

  it("subscribes to both hotkey and tray outcome events", () => {
    const stop = subscribeCaptureFeedback();

    // Hotkey failure path
    window.dispatchEvent(
      new CustomEvent("capture://failed", { detail: "Clipboard is empty" }),
    );
    // Tray failure path
    window.dispatchEvent(
      new CustomEvent("paste-canvas://capture-failed", { detail: "read error" }),
    );
    // Hotkey success path
    window.dispatchEvent(new CustomEvent("capture://success", { detail: "item-1" }));
    // Tray success path
    window.dispatchEvent(new CustomEvent("paste-canvas://capture-success", { detail: "item-2" }));

    const toasts = useToastStore.getState().toasts;
    expect(toasts).toHaveLength(4);
    expect(toasts.filter((t) => t.variant === "error")).toHaveLength(2);
    expect(toasts.filter((t) => t.variant === "success")).toHaveLength(2);

    stop();

    // After cleanup, no further toasts appear
    window.dispatchEvent(new CustomEvent("capture://failed", { detail: "late" }));
    expect(useToastStore.getState().toasts).toHaveLength(4);
  });
});
