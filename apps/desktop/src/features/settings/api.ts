import { invoke } from "@tauri-apps/api/core";

export interface Exclusion {
  id: string;
  matchType: string;
  matchValue: string;
  enabled: boolean;
  source: string;
}

export interface HotkeyBinding {
  action: string;
  accelerator: string;
  enabled: boolean;
}

export async function getSetting(key: string): Promise<string | null> {
  return invoke<string | null>("get_setting", { key });
}

export async function setSetting(key: string, valueJson: string): Promise<void> {
  return invoke<void>("set_setting", { key, valueJson });
}

export async function listExclusions(): Promise<Exclusion[]> {
  return invoke<Exclusion[]>("list_exclusions");
}

export async function addExclusion(matchType: string, matchValue: string): Promise<Exclusion> {
  return invoke<Exclusion>("add_exclusion", { request: { matchType, matchValue } });
}

export async function toggleExclusion(id: string): Promise<boolean> {
  return invoke<boolean>("toggle_exclusion", { id });
}

export async function removeExclusion(id: string): Promise<void> {
  return invoke<void>("remove_exclusion", { id });
}

export async function listHotkeys(): Promise<HotkeyBinding[]> {
  return invoke<HotkeyBinding[]>("list_hotkeys");
}

export async function updateHotkey(action: string, accelerator: string): Promise<void> {
  return invoke<void>("update_hotkey", { request: { action, accelerator } });
}

/** FND-006: hotkeys that failed to register at startup (drained on read). */
export async function getStartupHotkeyFailures(): Promise<string[]> {
  return invoke<string[]>("get_startup_hotkey_failures");
}

/** Registers the browser extension native messaging host (CAP-008). */
export async function registerBrowserHost(extensionOrigin: string): Promise<void> {
  return invoke<void>("register_browser_host", { request: { extensionOrigin } });
}

/** Unregisters the browser extension native messaging host. */
export async function unregisterBrowserHost(): Promise<void> {
  return invoke<void>("unregister_browser_host");
}

/** Registers Explorer context-menu integration (CAP-007). */
export async function registerExplorerIntegration(): Promise<void> {
  return invoke<void>("register_explorer_integration");
}

/** Unregisters Explorer context-menu integration. */
export async function unregisterExplorerIntegration(): Promise<void> {
  return invoke<void>("unregister_explorer_integration");
}

export interface ExplorerMenuRegistration {
  dllPath: string;
  sparseManifestPath: string | null;
  registered: boolean;
}

/** Registers the IExplorerCommand COM server for the modern Explorer menu (CAP-007). */
export async function registerModernExplorerMenu(): Promise<ExplorerMenuRegistration> {
  return invoke<ExplorerMenuRegistration>("register_modern_explorer_menu");
}

/** Unregisters the IExplorerCommand COM server. */
export async function unregisterModernExplorerMenu(): Promise<void> {
  return invoke<void>("unregister_modern_explorer_menu");
}
