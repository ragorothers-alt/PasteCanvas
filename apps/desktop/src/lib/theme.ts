import { getSetting, setSetting } from "../features/settings/api";

export type ThemeMode = "light" | "dark" | "system";
export const THEME_STORAGE_KEY = "pastecanvas.theme";

function isThemeMode(value: string | null | undefined): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

export function getStoredTheme(): ThemeMode {
  try {
    const local = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeMode(local) ? local : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(mode: ThemeMode): void {
  document.documentElement.dataset.theme = mode;
  document.documentElement.style.colorScheme = mode === "system" ? "light dark" : mode;
}

export async function loadTheme(): Promise<ThemeMode> {
  const local = getStoredTheme();
  try {
    const stored = await getSetting("ui.theme");
    if (isThemeMode(stored)) {
      try { window.localStorage.setItem(THEME_STORAGE_KEY, stored); } catch { /* browser policy */ }
      return stored;
    }
  } catch { /* browser/Vite fallback */ }
  return local;
}

export async function persistTheme(mode: ThemeMode): Promise<void> {
  try { window.localStorage.setItem(THEME_STORAGE_KEY, mode); } catch { /* browser policy */ }
  try { await setSetting("ui.theme", mode); } catch { /* native settings unavailable */ }
}
