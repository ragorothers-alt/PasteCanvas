import { describe, it, expect } from "vitest";
import {
  getDetailLevel,
  shouldMountEditor,
  shouldRenderPreview,
  DEFAULT_LOD_THRESHOLDS,
} from "../adapters/canvas/useCanvasLOD";

describe("Canvas LOD", () => {
  describe("getDetailLevel", () => {
    it("returns minimal for very low zoom", () => {
      expect(getDetailLevel(0.1)).toBe("minimal");
      expect(getDetailLevel(0.2)).toBe("minimal");
      expect(getDetailLevel(0.34)).toBe("minimal");
    });

    it("returns summary for medium zoom", () => {
      expect(getDetailLevel(0.35)).toBe("summary");
      expect(getDetailLevel(0.5)).toBe("summary");
      expect(getDetailLevel(0.74)).toBe("summary");
    });

    it("returns full for high zoom", () => {
      expect(getDetailLevel(0.75)).toBe("full");
      expect(getDetailLevel(1.0)).toBe("full");
      expect(getDetailLevel(2.0)).toBe("full");
      expect(getDetailLevel(4.0)).toBe("full");
    });

    it("respects custom thresholds", () => {
      const custom = { minimal: 0.5, summary: 1.0, full: 1.0 };
      expect(getDetailLevel(0.3, custom)).toBe("minimal");
      expect(getDetailLevel(0.6, custom)).toBe("summary");
      expect(getDetailLevel(1.5, custom)).toBe("full");
    });

    it("handles boundary values", () => {
      expect(getDetailLevel(DEFAULT_LOD_THRESHOLDS.minimal)).toBe("summary");
      expect(getDetailLevel(DEFAULT_LOD_THRESHOLDS.summary)).toBe("full");
    });
  });

  describe("shouldMountEditor", () => {
    it("mounts editor only when selected, editing, and full detail", () => {
      expect(shouldMountEditor(true, true, "full")).toBe(true);
    });

    it("does not mount when not selected", () => {
      expect(shouldMountEditor(false, true, "full")).toBe(false);
    });

    it("does not mount when not editing", () => {
      expect(shouldMountEditor(true, false, "full")).toBe(false);
    });

    it("does not mount at summary detail level", () => {
      expect(shouldMountEditor(true, true, "summary")).toBe(false);
    });

    it("does not mount at minimal detail level", () => {
      expect(shouldMountEditor(true, true, "minimal")).toBe(false);
    });
  });

  describe("shouldRenderPreview", () => {
    it("renders preview at summary level", () => {
      expect(shouldRenderPreview("summary")).toBe(true);
    });

    it("renders preview at full level", () => {
      expect(shouldRenderPreview("full")).toBe(true);
    });

    it("does not render preview at minimal level", () => {
      expect(shouldRenderPreview("minimal")).toBe(false);
    });

    it("does not render preview when collapsed", () => {
      expect(shouldRenderPreview("full", true)).toBe(false);
      expect(shouldRenderPreview("summary", true)).toBe(false);
    });
  });
});
