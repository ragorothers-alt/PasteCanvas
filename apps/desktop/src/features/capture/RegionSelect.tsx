/**
 * Region selection overlay (CAP-006).
 * Renders in the borderless transparent "region-overlay" window.
 * User drags to select a region; release captures, Esc cancels.
 *
 * Per PRD CAP-006: 支援取消、多螢幕與不同 DPI 下座標正確.
 * Coordinate conversion: CSS px → physical px (scale factor) → virtual screen.
 */
import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface RegionContext {
  originX: number;
  originY: number;
  scaleFactor: number;
}

interface DragState {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  dragging: boolean;
}

/**
 * Converts a CSS-pixel selection rectangle to virtual-screen physical coordinates.
 * Pure function — unit tested for multi-monitor/DPI correctness (CAP-006).
 */
export function cssToPhysicalRegion(
  cssX: number,
  cssY: number,
  cssW: number,
  cssH: number,
  originX: number,
  originY: number,
  scaleFactor: number,
): { x: number; y: number; width: number; height: number } {
  return {
    x: Math.round(originX + cssX * scaleFactor),
    y: Math.round(originY + cssY * scaleFactor),
    width: Math.round(cssW * scaleFactor),
    height: Math.round(cssH * scaleFactor),
  };
}

export function RegionSelect() {
  const [context, setContext] = useState<RegionContext | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const capturing = useRef(false);

  // Load monitor context for coordinate conversion
  useEffect(() => {
    void invoke<RegionContext>("get_region_capture_context").then(setContext);
  }, []);

  // Esc cancels (CAP-006: 支援取消)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !capturing.current) {
        void invoke("cancel_region_capture");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (capturing.current) return;
    setDrag({
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
      dragging: true,
    });
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    setDrag((prev) =>
      prev && prev.dragging
        ? { ...prev, currentX: e.clientX, currentY: e.clientY }
        : prev,
    );
  }, []);

  const handleMouseUp = useCallback(() => {
    if (!drag || !drag.dragging || !context || capturing.current) return;

    const x = Math.min(drag.startX, drag.currentX);
    const y = Math.min(drag.startY, drag.currentY);
    const w = Math.abs(drag.currentX - drag.startX);
    const h = Math.abs(drag.currentY - drag.startY);

    // Ignore tiny selections (accidental clicks)
    if (w < 5 || h < 5) {
      setDrag(null);
      return;
    }

    capturing.current = true;

    // Convert CSS px → virtual-screen physical coordinates
    const region = cssToPhysicalRegion(
      x,
      y,
      w,
      h,
      context.originX,
      context.originY,
      context.scaleFactor,
    );

    void invoke("capture_region_screenshot", {
      request: region,
    }).catch(() => {
      void invoke("cancel_region_capture");
    });
  }, [drag, context]);

  // Compute selection rectangle for rendering
  let selectionRect: React.CSSProperties | null = null;
  if (drag && drag.dragging) {
    const x = Math.min(drag.startX, drag.currentX);
    const y = Math.min(drag.startY, drag.currentY);
    const w = Math.abs(drag.currentX - drag.startX);
    const h = Math.abs(drag.currentY - drag.startY);
    selectionRect = {
      left: `${x}px`,
      top: `${y}px`,
      width: `${w}px`,
      height: `${h}px`,
    };
  }

  return (
    <div
      className="region-overlay"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      role="presentation"
    >
      <div className="region-hint">
        拖曳以選取區域 · Esc 取消
      </div>
      {selectionRect && <div className="region-selection" style={selectionRect} />}
    </div>
  );
}
