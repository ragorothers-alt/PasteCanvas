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
  createBackup,
  createExport,
  validateBackup,
  prepareRestore,
  getAutostart,
  setAutostart,
} from "./api";
import type { Exclusion, HotkeyBinding } from "./api";
import {
  verifySearchIndex,
  rebuildSearchIndex,
  optimizeSearchIndex,
} from "../search/api";
import type { SearchIndexHealth } from "../search/api";
import type { ThemeMode } from "../../lib/theme";
import { Desktop, Moon, Sun } from "@phosphor-icons/react";

const HOTKEY_LABELS: Record<string, string> = {
  capture_clipboard: "擷取剪貼簿",
  quick_capture: "快速擷取",
  screenshot: "螢幕截圖",
  toggle_main_window: "切換主視窗",
  global_search: "全域搜尋",
  toggle_capture_session: "切換擷取工作階段",
};

interface SettingsViewProps {
  theme?: ThemeMode;
  onThemeChange?: (theme: ThemeMode) => void;
}

type SettingsCategory = "general" | "shortcuts" | "integrations" | "search" | "privacy" | "data";

const SETTINGS_CATEGORIES: Array<{ id: SettingsCategory; label: string; hint: string }> = [
  { id: "general", label: "一般", hint: "外觀與視窗行為" },
  { id: "shortcuts", label: "快捷鍵", hint: "快速操作" },
  { id: "integrations", label: "整合", hint: "瀏覽器與檔案總管" },
  { id: "search", label: "搜尋", hint: "索引維護" },
  { id: "privacy", label: "隱私", hint: "擷取排除" },
  { id: "data", label: "資料", hint: "備份與匯出" },
];

export function SettingsView({ theme = "system", onThemeChange }: SettingsViewProps) {
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
  const [autostart, setAutostartState] = useState(false);
  const [dataMsg, setDataMsg] = useState<string | null>(null);
  const [restorePath, setRestorePath] = useState("");
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>("general");

  const load = useCallback(async () => {
    try {
      const [exc, hk, ctt, auto] = await Promise.all([
        listExclusions(),
        listHotkeys(),
        getSetting("ui.close_to_tray").catch(() => null),
        getAutostart().catch(() => false),
      ]);
      setExclusions(exc);
      setHotkeys(hk);
      // Stored as JSON ("true"/"false"); default true when unset.
      setCloseToTray(ctt === null ? true : ctt !== "false");
      setAutostartState(Boolean(auto));
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

  const handleToggleAutostart = useCallback(async () => {
    const next = !autostart;
    setAutostartState(next);
    try {
      await setAutostart(next);
    } catch {
      setAutostartState(!next);
    }
  }, [autostart]);

  const handleCreateBackup = useCallback(async () => {
    setDataMsg(null);
    try {
      const res = await createBackup();
      setDataMsg(`已備份 ${res.itemCount} 個項目到 ${res.backupPath}`);
    } catch (e) {
      setDataMsg(`備份失敗：${String(e)}`);
    }
  }, []);

  const handleCreateExport = useCallback(async () => {
    setDataMsg(null);
    try {
      const res = await createExport();
      setDataMsg(`已匯出 ${res.itemCount} 個項目到 ${res.exportPath}`);
    } catch (e) {
      setDataMsg(`匯出失敗：${String(e)}`);
    }
  }, []);

  const handlePrepareRestore = useCallback(async () => {
    const path = restorePath.trim();
    if (!path) {
      setDataMsg("請輸入備份資料夾路徑。");
      return;
    }
    setDataMsg(null);
    try {
      const check = await validateBackup(path);
      if (!check.valid) {
        setDataMsg(`備份無效：${check.reason ?? "未知原因"}`);
        return;
      }
      const staged = await prepareRestore(path);
      if (!staged.valid) {
        setDataMsg(`無法準備還原：${staged.reason ?? "未知原因"}`);
        return;
      }
      setDataMsg(`已排程還原（${staged.itemCount} 個項目）。下次啟動時套用。`);
    } catch (e) {
      setDataMsg(`還原失敗：${String(e)}`);
    }
  }, [restorePath]);

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
        setHotkeyMsg("快捷鍵已更新，立即生效，不需要重新啟動。");
        void load();
      } catch (e) {
        setHotkeyMsg(`快捷鍵更新失敗：${String(e)}`);
      }
    },
    [editingValue, load],
  );

  const handleRegisterBrowser = useCallback(async () => {
    setBrowserMsg(null);
    if (!extensionOrigin.trim()) {
        setBrowserMsg("請輸入擴充功能來源（chrome-extension://<id>/）。");
      return;
    }
    try {
      await registerBrowserHost(extensionOrigin.trim());
      setBrowserMsg("已為 Chrome 與 Edge 註冊瀏覽器擷取。");
    } catch (e) {
      setBrowserMsg(`註冊失敗：${String(e)}`);
    }
  }, [extensionOrigin]);

  const handleUnregisterBrowser = useCallback(async () => {
    setBrowserMsg(null);
    try {
      await unregisterBrowserHost();
      setBrowserMsg("已取消註冊瀏覽器擷取。");
    } catch (e) {
      setBrowserMsg(`取消註冊失敗：${String(e)}`);
    }
  }, []);

  const handleRegisterExplorer = useCallback(async () => {
    setExplorerMsg(null);
    try {
      await registerExplorerIntegration();
      setExplorerMsg("已註冊檔案總管右鍵選單（收件匣＋最近看板）。");
    } catch (e) {
      setExplorerMsg(`註冊失敗：${String(e)}`);
    }
  }, []);

  const handleUnregisterExplorer = useCallback(async () => {
    setExplorerMsg(null);
    try {
      await unregisterExplorerIntegration();
      setExplorerMsg("已移除檔案總管右鍵選單。");
    } catch (e) {
      setExplorerMsg(`取消註冊失敗：${String(e)}`);
    }
  }, []);

  const handleRegisterModern = useCallback(async () => {
    setModernMsg(null);
    try {
      const res = await registerModernExplorerMenu();
      const script = res.sparseManifestPath
        ? ` Windows 11 精簡選單不會自動出現，請用系統管理員執行：${res.sparseManifestPath}`
        : "";
      setModernMsg(
        `已接上「顯示更多選項」右鍵（ExplorerCommandHandler）。${script}`,
      );
    } catch (e) {
      setModernMsg(`註冊失敗：${String(e)}`);
    }
  }, []);

  const handleUnregisterModern = useCallback(async () => {
    setModernMsg(null);
    try {
      await unregisterModernExplorerMenu();
      setModernMsg("已取消註冊新版檔案總管選單。");
    } catch (e) {
      setModernMsg(`取消註冊失敗：${String(e)}`);
    }
  }, []);

  const handleVerifyIndex = useCallback(async () => {
    setIndexMsg(null);
    try {
      const health = await verifySearchIndex();
      setIndexHealth(health);
      setIndexMsg(health.healthy ? "搜尋索引狀態正常。" : "發現問題，請執行修復。");
    } catch (e) {
      setIndexMsg(`驗證失敗：${String(e)}`);
    }
  }, []);

  const handleRebuildIndex = useCallback(async () => {
    setIndexMsg(null);
    try {
      const res = await rebuildSearchIndex();
      setIndexMsg(
        `已修復：還原 ${res.rebuilt} 份文件，移除 ${res.removedOrphans} 個孤立項目。`,
      );
      const health = await verifySearchIndex();
      setIndexHealth(health);
    } catch (e) {
      setIndexMsg(`修復失敗：${String(e)}`);
    }
  }, []);

  const handleOptimizeIndex = useCallback(async () => {
    setIndexMsg(null);
    try {
      await optimizeSearchIndex();
      setIndexMsg("搜尋索引已最佳化。");
    } catch (e) {
      setIndexMsg(`最佳化失敗：${String(e)}`);
    }
  }, []);

  return (
    <div className="settings-view">
      <div className="settings-intro">
        <p className="eyebrow">工作區偏好設定</p>
        <h2>設定</h2>
        <p className="settings-lede">調整 PasteCanvas 的視窗、快捷鍵與本機整合方式。</p>
      </div>

      <div className="settings-layout">
        <nav className="settings-category-rail" aria-label="設定分類">
          {SETTINGS_CATEGORIES.map((category) => (
            <button
              type="button"
              key={category.id}
              className={`settings-category ${activeCategory === category.id ? "active" : ""}`}
              aria-pressed={activeCategory === category.id}
              onClick={() => setActiveCategory(category.id)}
            >
              <span>{category.label}</span>
              <small>{category.hint}</small>
            </button>
          ))}
        </nav>
        <div className="settings-panel">
      {activeCategory === "general" && <>
      <section className="settings-section theme-section">
        <h3>外觀</h3>
        <p className="settings-hint">選擇介面在不同時間與環境中的顯示方式。</p>
        <div className="theme-options" role="radiogroup" aria-label="介面主題">
          {(["light", "dark", "system"] as ThemeMode[]).map((mode) => (
            <button
              type="button"
              key={mode}
              className={`theme-option ${theme === mode ? "active" : ""}`}
              role="radio"
              aria-checked={theme === mode}
              onClick={() => onThemeChange?.(mode)}
            >
              {mode === "light" ? <Sun className="theme-option-icon" aria-hidden="true" /> : mode === "dark" ? <Moon className="theme-option-icon" aria-hidden="true" /> : <Desktop className="theme-option-icon" aria-hidden="true" />}
              <span>{mode === "light" ? "淺色" : mode === "dark" ? "深色" : "跟隨系統"}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <h3>視窗</h3>
        <label className="settings-toggle-row">
          <input
            type="checkbox"
            checked={closeToTray}
            onChange={() => void handleToggleCloseToTray()}
          />
          <span>
            關閉按鈕會縮到系統匣（取消勾選則在關閉時結束程式）
          </span>
        </label>
        <label className="settings-toggle-row">
          <input
            type="checkbox"
            checked={autostart}
            onChange={() => void handleToggleAutostart()}
          />
          <span>開機時啟動 PasteCanvas</span>
        </label>
      </section>
      </>}

      {activeCategory === "shortcuts" && <section className="settings-section">
        <h3>鍵盤快捷鍵</h3>
        <p className="settings-hint">
          點選快捷鍵即可修改（例如 Ctrl+Shift+C）。留白可停用動作，修改會立即生效。
        </p>
        <div className="hotkey-list">
          {hotkeys.length === 0 && (
            <div className="hotkey-row empty-state">尚未設定快捷鍵</div>
          )}
          {hotkeys.map((hk) => (
            <div key={hk.action} className="hotkey-row">
              <span className="hotkey-action">{HOTKEY_LABELS[hk.action] ?? "快捷鍵動作"}</span>
              {editingHotkey === hk.action ? (
                <span className="hotkey-edit">
                  <input
                    className="exclusion-input"
                    value={editingValue}
                    autoFocus
                    placeholder="Ctrl+Shift+…，留白可停用"
                    onChange={(e) => setEditingValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") void handleSaveHotkey(hk.action);
                      if (e.key === "Escape") setEditingHotkey(null);
                    }}
                  />
                  <button className="btn-add-exclusion" onClick={() => void handleSaveHotkey(hk.action)}>
                    儲存
                  </button>
                  <button className="btn-add-exclusion" onClick={() => setEditingHotkey(null)}>
                    取消
                  </button>
                </span>
              ) : (
                <kbd
                  className="hotkey-accelerator hotkey-editable"
                  title="點選即可修改"
                  onClick={() => {
                    setEditingHotkey(hk.action);
                    setEditingValue(hk.accelerator);
                  }}
                >
                  {hk.accelerator || "（已停用）"}
                </kbd>
              )}
            </div>
          ))}
        </div>
        {hotkeyMsg && <p className="settings-hint">{hotkeyMsg}</p>}
      </section>}

      {activeCategory === "integrations" && <>
      <section className="settings-section">
        <h3>瀏覽器擴充功能</h3>
        <p className="settings-hint">
          註冊本機擷取主機，讓 PasteCanvas 瀏覽器擴充功能儲存網頁、選取內容與連結。所有資料都留在這台電腦。
        </p>
        <div className="exclusion-add">
          <input
            className="exclusion-input"
            placeholder="chrome-extension://<extension-id>/"
            value={extensionOrigin}
            onChange={(e) => setExtensionOrigin(e.target.value)}
          />
          <button className="btn-add-exclusion" onClick={() => void handleRegisterBrowser()}>
            註冊
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleUnregisterBrowser()}>
            取消註冊
          </button>
        </div>
        {browserMsg && <p className="settings-hint">{browserMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>Windows 檔案總管</h3>
        <p className="settings-hint">
          新增右鍵選單，將檔案送到 PasteCanvas 收件匣或最近使用的看板。以背景程序執行，不會阻塞檔案總管。
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleRegisterExplorer()}>
            註冊
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleUnregisterExplorer()}>
            取消註冊
          </button>
        </div>
        {explorerMsg && <p className="settings-hint">{explorerMsg}</p>}
      </section>

      <section className="settings-section">
        <h3>新版檔案總管選單（Windows 11）</h3>
        <p className="settings-hint">
          為「顯示更多選項」註冊 IExplorerCommand。Windows 11 精簡選單還需要另外執行寫出的稀疏套件指令碼，不會在按註冊時假裝已完成。
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleRegisterModern()}>
            註冊新版選單
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleUnregisterModern()}>
            取消註冊
          </button>
        </div>
        {modernMsg && <p className="settings-hint">{modernMsg}</p>}
      </section>
      </>}

      {activeCategory === "search" && <section className="settings-section">
        <h3>搜尋索引（SRC-004）</h3>
        <p className="settings-hint">
          搜尋會使用擷取項目的衍生索引。驗證可檢查完整性；修復會從原始資料還原遺漏項目（不會遺失資料）；最佳化可維持查詢速度。
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleVerifyIndex()}>
            驗證
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleRebuildIndex()}>
            修復
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleOptimizeIndex()}>
            最佳化
          </button>
        </div>
        {indexHealth && (
          <p className="settings-hint">
            即時資料 {indexHealth.liveItems} 項 · 已建立索引 {indexHealth.indexedDocs} 份 ·{" "}
            遺漏 {indexHealth.missingDocs} 份 · 孤立 {indexHealth.orphanDocs} 份 ·{" "}
            FTS Unicode {indexHealth.ftsUnicodeOk ? "正常" : "失敗"} · 三元組{" "}
            {indexHealth.ftsTrigramOk ? "正常" : "失敗"}
          </p>
        )}
        {indexMsg && <p className="settings-hint">{indexMsg}</p>}
      </section>}
      {activeCategory === "privacy" && <section className="settings-section">
        <h3>擷取排除清單</h3>
        <p className="settings-hint">
          這些應用程式不會擷取剪貼簿內容（隱私保護）。
        </p>

        <div className="exclusion-add">
          <input
            className="exclusion-input"
            placeholder="程序名稱（例如 MyPasswordApp.exe）"
            value={newExclusion}
            onChange={(e) => setNewExclusion(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void handleAddExclusion(); }}
          />
          <button className="btn-add-exclusion" onClick={() => void handleAddExclusion()}>
            新增
          </button>
        </div>

        <ul className="exclusion-list">
          {exclusions.length === 0 && (
            <li className="exclusion-item empty-state">尚未設定排除項目</li>
          )}
          {exclusions.map((exc) => (
            <li key={exc.id} className={`exclusion-item ${!exc.enabled ? "disabled" : ""}`}>
              <span className="exclusion-value">{exc.matchValue}</span>
              <span className="exclusion-source">{exc.source}</span>
              <div className="exclusion-actions">
                <button onClick={() => void handleToggle(exc.id)}>
                  {exc.enabled ? "停用" : "啟用"}
                </button>
                {exc.source === "user" && (
                  <button onClick={() => void handleRemove(exc.id)}>移除</button>
                )}
              </div>
            </li>
          ))}
        </ul>
      </section>}
      {activeCategory === "data" && <section className="settings-section">
        <h3>備份與匯出</h3>
        <p className="settings-hint">
          備份會保存資料庫與素材；匯出產生可讀的 JSON／Markdown。還原會在下次啟動時套用。
        </p>
        <div className="exclusion-add">
          <button className="btn-add-exclusion" onClick={() => void handleCreateBackup()}>
            立即備份
          </button>
          <button className="btn-add-exclusion" onClick={() => void handleCreateExport()}>
            匯出
          </button>
        </div>
        <div className="exclusion-add">
          <input
            className="exclusion-input"
            placeholder="備份資料夾路徑"
            value={restorePath}
            onChange={(e) => setRestorePath(e.target.value)}
          />
          <button className="btn-add-exclusion" onClick={() => void handlePrepareRestore()}>
            排程還原
          </button>
        </div>
        {dataMsg && <p className="settings-hint">{dataMsg}</p>}
      </section>}
        </div>
      </div>
    </div>
  );
}
