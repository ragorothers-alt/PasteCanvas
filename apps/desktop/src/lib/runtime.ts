/**
 * Runtime environment helpers — safe access to Tauri APIs so the UI shell can
 * render both inside the Tauri WebView and in a plain browser (E2E / preview).
 *
 * These do NOT mock functionality: they only detect whether the Tauri runtime is
 * present. Real commands still go through the typed api.ts invoke wrappers.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Returns true when running inside the Tauri WebView (runtime internals present). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Returns the current Tauri window label, or null outside the Tauri runtime.
 * Safe to call at module/component top level without crashing in a plain browser.
 */
export function currentWindowLabel(): string | null {
  if (!isTauri()) return null;
  try {
    return getCurrentWindow().label;
  } catch {
    return null;
  }
}
