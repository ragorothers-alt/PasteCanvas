import { useState, useEffect, useCallback } from "react";
import {
  listExclusions,
  addExclusion,
  toggleExclusion,
  removeExclusion,
  listHotkeys,
  updateHotkey,
  getSetting,
  setSetting,
  registerBrowserHost,
  unregisterBrowserHost,
  registerExplorerIntegration,
  unregisterExplorerIntegration,
  registerModernExplorerMenu,
  unregisterModernExplorerMenu,
} from "./api";
import type { Exclusion, HotkeyBinding } from "./api";
import {
  verifySearchIndex,
  rebuildSearchIndex,
  optimizeSearchIndex,
} from "../search/api";
import type { SearchIndexHealth } from "../search/api";

export function SettingsView() {
  const [exclusions, setExclusions] = useState<Exclusion[]>([]);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [newExclusion, setNewExclusion] = useState("");
  const [extensionOrigin, setExtensionOrigin] = useState("");
  const [browserMsg, setBrowserMsg] = useState<string | null>(null);
  const [explorerMsg, setExplorerMsg] = useState<string | null>(null);
  const [modernMsg, setModernMsg] = useState<string | null>(null);
  const [indexHealth, setIndexHealth] = useState<SearchIndexHealth | null>(null);
  const [indexMsg, setIndexMsg] = useState<string | null>(null);
  const [editingHotkey, setEditingHotkey] = useState<string | null>(null);
  const [editingValue, setEditingValue] = useState("");
  const [hotkeyMsg, setHotkeyMsg] = useState<string | null>(null);
  const [closeToTray, setCloseToTray] = useState(true);

  const load = useCallback(async () => {
    try {
      const [exc, hk, ctt] = await Promise.all([
        listExclusions(),
        listHotkeys(),
        getSetting("ui.close_to_tray").catch(() => null),
      ]);
      setExclusions(exc);
      setHotkeys(hk);
      // Stored as JSON ("true"/"false"); default true when unset.
      setCloseToTray(ctt === null ? true : ctt !== "false");
    } catch { /* ignore */ }
  }, []);

  // WIN-001: close-button behavior (hide to tray vs exit)
  const handleToggleCloseToTray = useCallback(async () => {
    const next = !closeToTray;
    setCloseToTray(next);
    try {
      await setSetting("ui.close_to_tray", next ? "true" : "false");
    } catch { /* ignore */ }
  }, [closeToTray]);

  useEffect(() => { void load(); }, [load]);

  const handleAddExclusion = useCallback(async () => {
    if (!newExclusion.trim()) return;
    try {
      await addExclusion("process_name", newExclusion.trim());
      setNewExclusion("");
      void load();
    } catch { /* ignore */ }
  }, [newExclusion, load]);

  const handleToggle = useCallback(async (id: string) => {
    try {
      await toggleExclusion(id);
      void load();
    } catch { /* ignore */ }
  }, [load]);

  const handleRemove = useCallback(async (id: string) => {
    try {
      await removeExclusion(id);
      void load();
    } catch { /* ignore */ }
  }, [load]);

  // WIN-005: hotkey editing — changes apply immediately (backend re-registers)
  const handleSaveHotkey = useCallback(
    async (action: string) => {
      setHotkeyMsg(null);
      try {
        await updateHotkey(action, editingValue.trim());
        setEditingHotkey(null);
        setHotkeyMsg("Hotkey updated — active immediately, no restart needed.");
        void load();
      } catch (e) {
        setHotkeyMsg(`Hotkey update failed: ${String(e)}`);
      }
    },
    [editingValue, load],
  );

  const handleRegisterBrowser = useCallback(async () => {
    setBrowserMsg(null);
    if (!extensionOrigin.trim()) {
      setBrowserMsg("Enter the extension origin (chrome-extension://<id>/).");
      return;
    }
    try {
      await registerBrowserHost(extensionOrigin.trim());
      setBrowserMsg("Browser capture registered for Chrome and Edge.");
    } catch (e) {
      setBrowserMsg(`Registration failed: ${String(e)}`);
    }
  }, [extensionOrigin]);

  const handleUnregisterBrowser = useCallback(async () => {
    setBrowserMsg(null);
    try {
      await unregisterBrowserHost();
      setBrowserMsg("Browser capture unregistered.");
    } catch (e) {
      setBrowserMsg(`Unregister failed: ${String(e)}`);
    }
  }, []);

  const handleRegisterExplorer = useCallback(async () => {
    setExplorerMsg(null);
    try {
      await registerExplorerIntegration();
      setExplorerMsg("Explorer context menu registered (Inbox + last Board).");
    } catch (e) {
      setExplorerMsg(`Registration failed: ${String(e)}`);
    }
  }, []);

  const handleUnregisterExplorer = useCallback(async () => {
    setExplorerMsg(null);
    try {
      await unregisterExplorerIntegration();
      setExplorerMsg("Explorer context menu removed.");
    } catch (e) {
      setExplorerMsg(`Unregister failed: ${String(e)}`);
    }
  }, []);

  const handleRegisterModern = useCallback(async () => {
    setModernMsg(null);
    try {
      const res = await registerModernExplorerMenu();
      const script = res.sparseManifestPath
        ? ` Sparse-package script written for the Windows 11 modern menu.`
        : "";
      setModernMsg(`IExplorerCommand registered.${script}`);
    } catch (e) {
      setModernMsg(`Registration failed: ${String(e)}`);
    }
  }, []);

  const handleUnregisterModern = useCallback(async () => {
    setModernMsg(null);
    try {
      await unregisterModernExplorerMenu();
      setModernMsg("Modern Explorer menu unregistered.");
    } catch (e) {
      setModernMsg(`Unregister failed: ${String(e)}`);
    }
  }, []);

  const handleVerifyIndex = useCallback(async () => {
    setIndexMsg(null);
    try {
      const health = await verifySearchIndex();
      setIndexHealth(health);
      setIndexMsg(health.healthy ? "Search index is healthy." : "Issues found — run Repair.");
    } catch (e) {
      setIndexMsg(`Verify failed: ${String(e)}`);
    }
  }, []);

  const handleRebuildIndex = useCallback(async () => {
    setIndexMsg(null);
    try {
      const res = await rebuildSearchIndex();
      setIndexMsg(
        `Repaired: ${res.rebuilt} document(s) restored, ${res.removedOrphans} orphan(s) removed.`,
      );
      const health = await verifySearchIndex();
      setIndexHealth(health);
    } catch (e) {
      setIndexMsg(`Repair failed: ${String(e)}`);
    }
  }, []);

  const handleOptimizeIndex = useCallback(async () => {
    setIndexMsg(null);
    try {
      await optimizeSearchIndex();
      setIndexMsg("Search index optimized.");
    } catch (e) {
      setIndexMsg(`Optimize failed: ${String(e)}`);
    }
  }, []);

  return (
    <div className="settings-view">
      <h2>Settings</h2>

      <section className="settings-section">
        <h3>Window</h3>
        <label className="settings-toggle-row">
          <input
            type="checkbox"
            checked={closeToTray}
            onChange={() => void handleToggleCloseToTray()}
          />
          <span>
            Close button hides to the system tray (uncheck to exit on close)
          </span>
        </label>
      </section>

      <section className="settings-section">
        <h3>Keyboard Shortcuts</h3>
        <p className="settings-hint">
          Click a shortcut to change it (e.g., Ctrl+Shift+C). Empty disables the
          action. Changes take effect immediately.
        </p>
        <div className="hotkey-list">
          {hotkeys.length === 0 && (
            <div className="hotkey-row empty-state">No shortcuts configured</div>
          )}
          {hotkeys.map((hk) => (
            <div key={hk.action} className="hotkey-row">
              <span className="hotkey-action">{hk.action.replace(/_/g, " ")}</span>
              {editingHotkey === hk.action ? (
                <span className="hotkey-edit">
                  <input
                    className="exclusion-input"
                    value={editingValue}
                    autoFocus
                    placeholder="Ctrl+Shift+… or empty to disable"
                    onChange={(e) => setEditingValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") void handleSaveHotkey(hk.action);
                      if (e.key === "Escape") setEditingHotkey(null);
                    }}
                  />
                  <button className="btn-add-exclusion" onClick={() => void handleSaveHotkey(hk.action)}>
                    Save
                  </button>
                  <button className="btn-add-exclusion" onClick={() => setEditingHotkey(null)}>
                    Cancel
                  </button>
                </span>
              ) : (
                <kbd
                  className="hotkey-accelerator hotkey-editable"
                  title="Click to change"
                  onClick={() => {
                    setEditingHotkey(hk.action);
                    setEditingValue(hk.accelerator);
                  }}
                >
                  {hk.accelerator || "(disabled)"}
                </kbd>
              )}
            </div>
          ))}
        </div>
        {hotkeyMsg && <p className="settings-hint">{hotkeyMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>Browser Extension</h3>
        <p className="settings-hint">
          Register the local capture host so the PasteCanvas browser extension can
          save pages, selections, and links. Everything stays on this machine.
        </p>
        <div className="exclusion-add">
          <input
            className="exclusion-input"
            placeholder="chrome-extension://<extension-id>/"
            value={extensionOrigin}
            onChange={(e) => setExtensionOrigin(e.target.value)}
          />
          <button className="btn-add-exclusion" onClick={() => void handleRegisterBrowser()}>
            Register
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleUnregisterBrowser()}>
            Unregister
          </button>
        </div>
        {browserMsg && <p className="settings-hint">{browserMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>Windows Explorer</h3>
        <p className="settings-hint">
          Add right-click entries to send files to the PasteCanvas Inbox or to your
          most recently used board. Runs as a background process so Explorer never blocks.
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleRegisterExplorer()}>
            Register
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleUnregisterExplorer()}>
            Unregister
          </button>
        </div>
        {explorerMsg && <p className="settings-hint">{explorerMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>Modern Explorer Menu (Windows 11)</h3>
        <p className="settings-hint">
          Registers the IExplorerCommand COM server for the native right-click menu.
          Also generates a sparse-package script for the Windows 11 modern menu.
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleRegisterModern()}>
            Register Modern Menu
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleUnregisterModern()}>
            Unregister
          </button>
        </div>
        {modernMsg && <p className="settings-hint">{modernMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>Search Index (SRC-004)</h3>
        <p className="settings-hint">
          Search runs on a derived index of your captured items. Verify checks its
          integrity; Repair restores any missing entries from the original data
          (nothing is ever lost); Optimize keeps queries fast.
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleVerifyIndex()}>
            Verify
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleRebuildIndex()}>
            Repair
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleOptimizeIndex()}>
            Optimize
          </button>
        </div>
        {indexHealth && (
          <p className="settings-hint">
            {indexHealth.liveItems} item(s) · {indexHealth.indexedDocs} indexed ·{" "}
            {indexHealth.missingDocs} missing · {indexHealth.orphanDocs} orphan ·
            FTS unicode {indexHealth.ftsUnicodeOk ? "OK" : "FAIL"} · trigram{" "}
            {indexHealth.ftsTrigramOk ? "OK" : "FAIL"}
          </p>
        )}
        {indexMsg && <p className="settings-hint">{indexMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>Capture Exclusions</h3>
        <p className="settings-hint">
          Clipboard capture is skipped for these applications (privacy protection).
        </p>

        <div className="exclusion-add">
          <input
            className="exclusion-input"
            placeholder="Process name (e.g., MyPasswordApp.exe)"
            value={newExclusion}
            onChange={(e) => setNewExclusion(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void handleAddExclusion(); }}
          />
          <button className="btn-add-exclusion" onClick={() => void handleAddExclusion()}>
            Add
          </button>
        </div>

        <ul className="exclusion-list">
          {exclusions.length === 0 && (
            <li className="exclusion-item empty-state">No exclusions configured</li>
          )}
          {exclusions.map((exc) => (
            <li key={exc.id} className={`exclusion-item ${!exc.enabled ? "disabled" : ""}`}>
              <span className="exclusion-value">{exc.matchValue}</span>
              <span className="exclusion-source">{exc.source}</span>
              <div className="exclusion-actions">
                <button onClick={() => void handleToggle(exc.id)}>
                  {exc.enabled ? "Disable" : "Enable"}
                </button>
                {exc.source === "user" && (
                  <button onClick={() => void handleRemove(exc.id)}>Remove</button>
                )}
              </div>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
