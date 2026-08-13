import { describe, it, expect } from "vitest";
import { cssToPhysicalRegion } from "../features/capture/RegionSelect";

describe("cssToPhysicalRegion (CAP-006 multi-monitor/DPI)", () => {
  it("identity at scale 1 with origin (0,0)", () => {
    const region = cssToPhysicalRegion(100, 200, 300, 400, 0, 0, 1.0);
    expect(region).toEqual({ x: 100, y: 200, width: 300, height: 400 });
  });

  it("applies 2x DPI scaling", () => {
    const region = cssToPhysicalRegion(100, 50, 200, 100, 0, 0, 2.0);
    expect(region).toEqual({ x: 200, y: 100, width: 400, height: 200 });
  });

  it("applies monitor origin offset (second monitor)", () => {
    // Monitor starting at physical x=1920 (right of primary)
    const region = cssToPhysicalRegion(10, 20, 100, 100, 1920, 0, 1.0);
    expect(region).toEqual({ x: 1930, y: 20, width: 100, height: 100 });
  });

  it("handles negative origin (monitor left of primary)", () => {
    // Multi-monitor: left monitor has negative virtual coordinates
    const region = cssToPhysicalRegion(50, 50, 200, 200, -1920, 0, 1.0);
    expect(region).toEqual({ x: -1870, y: 50, width: 200, height: 200 });
  });

  it("combines negative origin with DPI scaling", () => {
    // Left monitor at -2560 physical, 1.5x scale
    const region = cssToPhysicalRegion(100, 100, 400, 300, -2560, -140, 1.5);
    expect(region).toEqual({
      x: Math.round(-2560 + 150),
      y: Math.round(-140 + 150),
      width: 600,
      height: 450,
    });
  });

  it("rounds fractional results", () => {
    const region = cssToPhysicalRegion(1, 1, 3, 3, 0, 0, 1.25);
    expect(region).toEqual({ x: 1, y: 1, width: 4, height: 4 });
  });
});
