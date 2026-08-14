import { useState, useEffect, useCallback } from "react";
import { queryInbox, toggleFavorite, togglePin, trashItem } from "./api";
import type { ItemSummary } from "./api";
import { isTauri } from "../../lib/runtime";
import { showCaptureError } from "../../stores/toastStore";

type LoadState = "loading" | "success" | "error";

/** INB-002 smart-view grouping dimensions. */
export type GroupMode = "date" | "source" | "domain" | "type" | "session";

export interface ItemGroup {
  label: string;
  items: ItemSummary[];
}

function startOfToday(): number {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
}

/** Groups items by date: Today / Yesterday / Last 7 days / Earlier. */
function groupByDate(items: ItemSummary[]): ItemGroup[] {
  const todayStart = startOfToday();
  const yesterdayStart = todayStart - 86400000;
  const weekStart = todayStart - 7 * 86400000;

  const buckets: Record<string, ItemSummary[]> = {
    Today: [],
    Yesterday: [],
    "Last 7 days": [],
    Earlier: [],
  };

  for (const item of items) {
    if (item.createdAt >= todayStart) buckets.Today!.push(item);
    else if (item.createdAt >= yesterdayStart) buckets.Yesterday!.push(item);
    else if (item.createdAt >= weekStart) buckets["Last 7 days"]!.push(item);
    else buckets.Earlier!.push(item);
  }

  return Object.entries(buckets)
    .filter(([, list]) => list.length > 0)
    .map(([label, list]) => ({ label, items: list }));
}

/** Groups items by an arbitrary key extractor (source/domain/type/session). */
function groupByKey(
  items: ItemSummary[],
  keyFn: (item: ItemSummary) => string | null,
  noneLabel: string,
): ItemGroup[] {
  const map = new Map<string, ItemSummary[]>();
  for (const item of items) {
    const key = keyFn(item) ?? noneLabel;
    const list = map.get(key);
    if (list) list.push(item);
    else map.set(key, [item]);
  }
  return Array.from(map.entries())
    .sort((a, b) => b[1].length - a[1].length)
    .map(([label, list]) => ({ label, items: list }));
}

/** Dispatches to the correct grouping strategy (INB-002). */
export function groupItems(items: ItemSummary[], mode: GroupMode): ItemGroup[] {
  switch (mode) {
    case "date":
      return groupByDate(items);
    case "source":
      return groupByKey(items, (i) => i.sourceApp, "(no source)");
    case "domain":
      return groupByKey(items, (i) => i.domain, "(no domain)");
    case "type":
      return groupByKey(items, (i) => i.itemType, "(unknown)");
    case "session":
      return groupByKey(
        items,
        (i) => i.captureSessionId,
        "(no session)",
      );
  }
}

export function useInbox() {
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [state, setState] = useState<LoadState>("loading");
  const [error, setError] = useState<string | null>(null);
  const [groupMode, setGroupMode] = useState<GroupMode>("date");

  const load = useCallback(async () => {
    setState("loading");
    setError(null);
    // Outside the Tauri runtime (browser preview / E2E) there is no backend;
    // show a clean empty state instead of an error.
    if (!isTauri()) {
      setItems([]);
      setState("success");
      return;
    }
    try {
      const result = await queryInbox({ limit: 100, offset: 0 });
      setItems(result);
      setState("success");
    } catch (err) {
      setError(String(err));
      setState("error");
    }
  }, []);

  useEffect(() => {
    void load();
    // Listen for capture events to auto-refresh
    const handler = () => void load();
    window.addEventListener("pastecanvas:item-captured", handler);
    return () => window.removeEventListener("pastecanvas:item-captured", handler);
  }, [load]);

  const handleToggleFavorite = useCallback(async (itemId: string) => {
    try {
      const newState = await toggleFavorite(itemId);
      setItems((prev) =>
        prev.map((item) =>
          item.id === itemId ? { ...item, isFavorite: newState } : item,
        ),
      );
    } catch (err) {
      showCaptureError(`更新收藏失敗：${String(err)}`);
    }
  }, []);

  const handleTogglePin = useCallback(async (itemId: string) => {
    try {
      const newState = await togglePin(itemId);
      setItems((prev) =>
        prev.map((item) =>
          item.id === itemId ? { ...item, isPinned: newState } : item,
        ),
      );
    } catch (err) {
      showCaptureError(`更新釘選失敗：${String(err)}`);
    }
  }, []);

  const handleTrash = useCallback(async (itemId: string) => {
    try {
      await trashItem(itemId);
      setItems((prev) => prev.filter((item) => item.id !== itemId));
    } catch (err) {
      showCaptureError(`移至垃圾桶失敗：${String(err)}`);
    }
  }, []);

  const groups = groupItems(items, groupMode);

  return {
    items,
    groups,
    groupMode,
    setGroupMode,
    state,
    error,
    reload: load,
    toggleFavorite: handleToggleFavorite,
    togglePin: handleTogglePin,
    trash: handleTrash,
  };
}
