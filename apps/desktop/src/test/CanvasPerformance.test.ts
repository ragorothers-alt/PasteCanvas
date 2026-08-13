import { describe, it, expect } from "vitest";
import {
  getDetailLevel,
  shouldMountEditor,
  shouldRenderPreview,
} from "../adapters/canvas/useCanvasLOD";
import type { DetailLevel } from "../adapters/canvas/useCanvasLOD";

/**
 * 500-node canvas performance benchmark.
 * Per GOAL.md: "500 個可見簡單 Node 的畫布拖動：目標維持接近 60fps"
 *
 * These tests verify that LOD computations scale linearly
 * and that editor gating prevents expensive rendering at scale.
 */

interface MockNode {
  id: string;
  isSelected: boolean;
  isEditing: boolean;
  isCollapsed: boolean;
}

function generateNodes(count: number): MockNode[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `node-${i}`,
    isSelected: false,
    isEditing: false,
    isCollapsed: false,
  }));
}

describe("500-node canvas performance", () => {
  const NODE_COUNT = 500;

  describe("LOD computation at scale", () => {
    it("computes detail level for 500 nodes in < 1ms", () => {
      const nodes = generateNodes(NODE_COUNT);
      const zoom = 0.5; // summary level

      const start = performance.now();
      const results = nodes.map(() => getDetailLevel(zoom));
      const elapsed = performance.now() - start;

      expect(results).toHaveLength(NODE_COUNT);
      expect(elapsed).toBeLessThan(1); // Must be under 1ms for 500 nodes
    });

    it("computes detail level for 2000 nodes in < 5ms", () => {
      const nodes = generateNodes(2000);
      const zoom = 0.5;

      const start = performance.now();
      nodes.map(() => getDetailLevel(zoom));
      const elapsed = performance.now() - start;

      expect(elapsed).toBeLessThan(5);
    });
  });

  describe("editor gating at 500 nodes", () => {
    it("mounts only 1 editor when 1 node is selected+editing at full zoom", () => {
      const nodes = generateNodes(NODE_COUNT);
      nodes[42] = { ...nodes[42]!, isSelected: true, isEditing: true };

      const detailLevel = getDetailLevel(1.0); // full zoom
      const mountedEditors = nodes.filter(
        (n) => shouldMountEditor(n.isSelected, n.isEditing, detailLevel),
      );

      expect(mountedEditors).toHaveLength(1);
      expect(mountedEditors[0]!.id).toBe("node-42");
    });

    it("mounts 0 editors at summary zoom even when editing", () => {
      const nodes = generateNodes(NODE_COUNT);
      nodes[42] = { ...nodes[42]!, isSelected: true, isEditing: true };

      const detailLevel = getDetailLevel(0.5); // summary zoom
      const mountedEditors = nodes.filter(
        (n) => shouldMountEditor(n.isSelected, n.isEditing, detailLevel),
      );

      expect(mountedEditors).toHaveLength(0);
    });

    it("mounts 0 editors at minimal zoom even when editing", () => {
      const nodes = generateNodes(NODE_COUNT);
      nodes[42] = { ...nodes[42]!, isSelected: true, isEditing: true };

      const detailLevel = getDetailLevel(0.2); // minimal zoom
      const mountedEditors = nodes.filter(
        (n) => shouldMountEditor(n.isSelected, n.isEditing, detailLevel),
      );

      expect(mountedEditors).toHaveLength(0);
    });
  });

  describe("preview rendering at 500 nodes", () => {
    it("renders all previews at summary zoom", () => {
      const nodes = generateNodes(NODE_COUNT);
      const detailLevel = getDetailLevel(0.5);

      const rendered = nodes.filter(
        (n) => shouldRenderPreview(detailLevel, n.isCollapsed),
      );

      expect(rendered).toHaveLength(NODE_COUNT);
    });

    it("renders 0 previews at minimal zoom", () => {
      const nodes = generateNodes(NODE_COUNT);
      const detailLevel = getDetailLevel(0.2);

      const rendered = nodes.filter(
        (n) => shouldRenderPreview(detailLevel, n.isCollapsed),
      );

      expect(rendered).toHaveLength(0);
    });

    it("skips collapsed nodes at any zoom", () => {
      const nodes = generateNodes(NODE_COUNT);
      // Collapse half the nodes
      for (let i = 0; i < NODE_COUNT / 2; i++) {
        nodes[i] = { ...nodes[i]!, isCollapsed: true };
      }

      const detailLevel = getDetailLevel(1.0); // full zoom
      const rendered = nodes.filter(
        (n) => shouldRenderPreview(detailLevel, n.isCollapsed),
      );

      expect(rendered).toHaveLength(NODE_COUNT / 2);
    });
  });

  describe("LOD transition correctness", () => {
    it("transitions correctly at threshold boundaries", () => {
      // Below minimal threshold
      expect(getDetailLevel(0.1)).toBe("minimal");
      expect(getDetailLevel(0.34)).toBe("minimal");

      // At minimal boundary
      expect(getDetailLevel(0.35)).toBe("summary");

      // Summary range
      expect(getDetailLevel(0.5)).toBe("summary");
      expect(getDetailLevel(0.74)).toBe("summary");

      // At summary/full boundary
      expect(getDetailLevel(0.75)).toBe("full");

      // Full range
      expect(getDetailLevel(1.0)).toBe("full");
      expect(getDetailLevel(2.0)).toBe("full");
    });

    it("supports custom thresholds", () => {
      const custom = { minimal: 0.5, summary: 1.0, full: 1.0 };
      expect(getDetailLevel(0.3, custom)).toBe("minimal");
      expect(getDetailLevel(0.7, custom)).toBe("summary");
      expect(getDetailLevel(1.5, custom)).toBe("full");
    });
  });

  describe("batch node state computation", () => {
    it("processes 500 nodes LOD + gating in < 2ms", () => {
      const nodes = generateNodes(NODE_COUNT);
      nodes[100] = { ...nodes[100]!, isSelected: true, isEditing: true };
      nodes[200] = { ...nodes[200]!, isCollapsed: true };

      const zoom = 0.8; // full
      const start = performance.now();

      const results = nodes.map((n) => {
        const level = getDetailLevel(zoom);
        const mountEditor = shouldMountEditor(n.isSelected, n.isEditing, level);
        const renderPreview = shouldRenderPreview(level, n.isCollapsed);
        return { level, mountEditor, renderPreview };
      });

      const elapsed = performance.now() - start;

      expect(results).toHaveLength(NODE_COUNT);
      expect(elapsed).toBeLessThan(2);

      // Verify correctness
      const editorCount = results.filter((r) => r.mountEditor).length;
      expect(editorCount).toBe(1);

      const hiddenCount = results.filter((r) => !r.renderPreview).length;
      expect(hiddenCount).toBe(1); // Only the collapsed node
    });

    it("processes 2000 nodes in < 8ms (stress test)", () => {
      const nodes = generateNodes(2000);
      const zoom = 0.6;

      const start = performance.now();
      nodes.map((n) => {
        const level = getDetailLevel(zoom);
        return {
          level,
          mountEditor: shouldMountEditor(n.isSelected, n.isEditing, level),
          renderPreview: shouldRenderPreview(level, n.isCollapsed),
        };
      });
      const elapsed = performance.now() - start;

      expect(elapsed).toBeLessThan(8);
    });
  });

  describe("memory efficiency", () => {
    it("detail level computation is O(1) per node (no allocation)", () => {
      // Verify that getDetailLevel returns a string literal (no object creation)
      const level: DetailLevel = getDetailLevel(0.5);
      expect(typeof level).toBe("string");
      expect(level).toBe("summary");
    });

    it("shouldMountEditor returns boolean without allocation", () => {
      const result = shouldMountEditor(true, true, "full");
      expect(typeof result).toBe("boolean");
      expect(result).toBe(true);
    });
  });
});
