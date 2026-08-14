import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  useToastStore,
  showCaptureSuccess,
  showDedupNotice,
} from "../stores/toastStore";

describe("toastStore", () => {
  beforeEach(() => {
    useToastStore.getState().clearAll();
  });

  describe("push and dismiss", () => {
    it("pushes a toast and assigns id", () => {
      const id = useToastStore.getState().push({
        message: "test",
        variant: "info",
      });
      expect(id).toBeTruthy();
      expect(useToastStore.getState().toasts).toHaveLength(1);
    });

    it("dismisses a toast by id", () => {
      const id = useToastStore.getState().push({
        message: "to dismiss",
        variant: "info",
      });
      expect(useToastStore.getState().toasts).toHaveLength(1);

      useToastStore.getState().dismiss(id);
      expect(useToastStore.getState().toasts).toHaveLength(0);
    });

    it("bounds toasts to max 5", () => {
      for (let i = 0; i < 8; i++) {
        useToastStore.getState().push({ message: `toast ${i}`, variant: "info" });
      }
      expect(useToastStore.getState().toasts.length).toBeLessThanOrEqual(5);
    });

    it("clears all toasts", () => {
      useToastStore.getState().push({ message: "a", variant: "info" });
      useToastStore.getState().push({ message: "b", variant: "info" });
      expect(useToastStore.getState().toasts).toHaveLength(2);

      useToastStore.getState().clearAll();
      expect(useToastStore.getState().toasts).toHaveLength(0);
    });
  });

  describe("convenience helpers", () => {
    it("showCaptureSuccess creates success toast", () => {
      showCaptureSuccess("Captured");
      const toasts = useToastStore.getState().toasts;
      expect(toasts).toHaveLength(1);
      expect(toasts[0]!.variant).toBe("success");
      expect(toasts[0]!.message).toBe("Captured");
    });

    it("showDedupNotice creates dedup toast with action", () => {
      const existingId = "item-abc-123";
      showDedupNotice(existingId);

      const toasts = useToastStore.getState().toasts;
      expect(toasts).toHaveLength(1);
      const toast = toasts[0]!;
      expect(toast.variant).toBe("dedup");
      expect(toast.relatedItemId).toBe(existingId);
      expect(toast.action).toBeDefined();
      expect(toast.action!.label).toBe("開啟既有項目");
    });

    it("dedup action dispatches open-item event", () => {
      const existingId = "item-xyz-789";
      showDedupNotice(existingId);

      const toast = useToastStore.getState().toasts[0]!;
      const dispatchSpy = vi.spyOn(window, "dispatchEvent");

      toast.action!.onAction();

      expect(dispatchSpy).toHaveBeenCalled();
      const event = dispatchSpy.mock.calls[0]![0] as CustomEvent;
      expect(event.type).toBe("paste-canvas://open-item");
      expect(event.detail.itemId).toBe(existingId);

      dispatchSpy.mockRestore();
    });
  });
});
