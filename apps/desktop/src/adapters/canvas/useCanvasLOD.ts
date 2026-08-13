/**
 * Canvas Level of Detail (LOD) system.
 * Per 03_TECHNICAL_ARCHITECTURE.md §4.4:
 * - zoom < 0.35: thumbnail/icon + title only
 * - 0.35–0.75: summary content
 * - > 0.75: richer preview
 * - editors mount only when selected and editing
 * - offscreen or collapsed group content may be hidden
 *
 * Per GOAL.md: "500 個可見簡單 Node 的畫布拖動：目標維持接近 60fps"
 */
import { useViewport } from "@xyflow/react";
import { useMemo } from "react";

export type DetailLevel = "minimal" | "summary" | "full";

export interface LODThresholds {
  /** Below this zoom: minimal (icon + title only) */
  minimal: number;
  /** Below this zoom: summary (title + preview snippet) */
  summary: number;
  /** Above this zoom: full (rich preview, editors) */
  full: number;
}

export const DEFAULT_LOD_THRESHOLDS: LODThresholds = {
  minimal: 0.35,
  summary: 0.75,
  full: 0.75,
};

/** Determines the detail level for a given zoom value. */
export function getDetailLevel(
  zoom: number,
  thresholds: LODThresholds = DEFAULT_LOD_THRESHOLDS,
): DetailLevel {
  if (zoom < thresholds.minimal) return "minimal";
  if (zoom < thresholds.summary) return "summary";
  return "full";
}

/**
 * Hook that returns the current LOD detail level based on canvas zoom.
 * Memoized to avoid unnecessary re-renders.
 */
export function useCanvasLOD(
  thresholds: LODThresholds = DEFAULT_LOD_THRESHOLDS,
): DetailLevel {
  const { zoom } = useViewport();
  return useMemo(
    () => getDetailLevel(zoom, thresholds),
    [zoom, thresholds],
  );
}

/**
 * Determines whether expensive content (editors, PDF viewers) should mount.
 * Per AGENTS.md: "editors mount only when selected and editing"
 */
export function shouldMountEditor(
  isSelected: boolean,
  isEditing: boolean,
  detailLevel: DetailLevel,
): boolean {
  return isSelected && isEditing && detailLevel === "full";
}

/**
 * Determines whether a node should render its preview content.
 * Offscreen nodes and collapsed groups can skip rendering.
 */
export function shouldRenderPreview(
  detailLevel: DetailLevel,
  isCollapsed: boolean = false,
): boolean {
  if (isCollapsed) return false;
  return detailLevel !== "minimal";
}
