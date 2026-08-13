import { invoke } from "@tauri-apps/api/core";

export interface SearchResult {
  itemId: string;
  title: string;
  bodySnippet: string;
  url: string;
  domain: string;
  sourceApp: string;
  itemType: string;
  isFavorite: boolean;
  createdAt: number;
  score: number;
}

export interface SearchFilters {
  itemType?: string;
  sourceApp?: string;
  domain?: string;
  favoriteOnly?: boolean;
  boardId?: string;
  after?: number;
  before?: number;
  inboxStatus?: string;
  sessionId?: string;
}

export async function searchItems(
  query: string,
  limit = 30,
  filters?: SearchFilters,
): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search_items", {
    request: { query, limit, offset: 0, ...filters },
  });
}

// --- SRC-004: search index maintenance ---

export interface SearchIndexHealth {
  liveItems: number;
  indexedDocs: number;
  missingDocs: number;
  orphanDocs: number;
  ftsUnicodeOk: boolean;
  ftsTrigramOk: boolean;
  healthy: boolean;
}

export interface SearchIndexRebuildResult {
  rebuilt: number;
  removedOrphans: number;
}

export async function verifySearchIndex(): Promise<SearchIndexHealth> {
  return invoke<SearchIndexHealth>("verify_search_index");
}

export async function rebuildSearchIndex(): Promise<SearchIndexRebuildResult> {
  return invoke<SearchIndexRebuildResult>("rebuild_search_index");
}

export async function optimizeSearchIndex(): Promise<void> {
  return invoke<void>("optimize_search_index");
}
