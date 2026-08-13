/**
 * Search query syntax parser — extracts `key:value` filter tokens (SRC-002).
 * Per PRD SRC-002: supports type:, source:, domain:, board:, after:, before:,
 * favorite:, inbox:, session: filters alongside free-text terms.
 *
 * Example: "rust type:code after:2024-01-01 favorite:true"
 * → freeText: "rust", filters: { itemType: "code", after: <epoch>, favoriteOnly: true }
 */
import type { SearchFilters } from "./api";

export interface ParsedQuery {
  /** Remaining free-text search terms (joined). */
  freeText: string;
  /** Extracted structured filters. */
  filters: SearchFilters;
}

const FILTER_KEYS = new Set([
  "type",
  "source",
  "domain",
  "board",
  "after",
  "before",
  "favorite",
  "inbox",
  "session",
]);

/** Parses a date string (ISO or epoch-ms) to epoch milliseconds. Returns undefined if invalid. */
function parseDateToMs(value: string): number | undefined {
  // Epoch milliseconds directly
  if (/^\d{13}$/.test(value)) {
    return parseInt(value, 10);
  }
  // Epoch seconds
  if (/^\d{10}$/.test(value)) {
    return parseInt(value, 10) * 1000;
  }
  // ISO date string
  const d = new Date(value);
  if (!Number.isNaN(d.getTime())) {
    return d.getTime();
  }
  return undefined;
}

/** Parses a raw search query into free text + structured filters. */
export function parseSearchQuery(raw: string): ParsedQuery {
  const filters: SearchFilters = {};
  const freeTerms: string[] = [];

  // Match key:value tokens (value may be quoted for spaces)
  const tokenRegex = /(\w+):("[^"]*"|\S+)/g;
  let match: RegExpExecArray | null;
  const consumed: Array<[number, number]> = [];

  while ((match = tokenRegex.exec(raw)) !== null) {
    const key = match[1]!.toLowerCase();
    if (!FILTER_KEYS.has(key)) {
      continue; // Not a recognized filter — leave as free text
    }
    let value = match[2]!;
    if (value.startsWith('"') && value.endsWith('"')) {
      value = value.slice(1, -1);
    }
    consumed.push([match.index, match.index + match[0].length]);

    switch (key) {
      case "type":
        filters.itemType = value;
        break;
      case "source":
        filters.sourceApp = value;
        break;
      case "domain":
        filters.domain = value;
        break;
      case "board":
        filters.boardId = value;
        break;
      case "after": {
        const ms = parseDateToMs(value);
        if (ms !== undefined) filters.after = ms;
        break;
      }
      case "before": {
        const ms = parseDateToMs(value);
        if (ms !== undefined) filters.before = ms;
        break;
      }
      case "favorite":
        filters.favoriteOnly = value === "true" || value === "1";
        break;
      case "inbox":
        filters.inboxStatus = value;
        break;
      case "session":
        filters.sessionId = value;
        break;
    }
  }

  // Collect free text = original string minus consumed filter tokens
  let cursor = 0;
  for (const [start, end] of consumed) {
    freeTerms.push(raw.slice(cursor, start));
    cursor = end;
  }
  freeTerms.push(raw.slice(cursor));

  const freeText = freeTerms
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();

  return { freeText, filters };
}

/** Returns true if any filter is set. */
export function hasActiveFilters(filters: SearchFilters): boolean {
  return (
    filters.itemType !== undefined ||
    filters.sourceApp !== undefined ||
    filters.domain !== undefined ||
    filters.favoriteOnly !== undefined ||
    filters.boardId !== undefined ||
    filters.after !== undefined ||
    filters.before !== undefined ||
    filters.inboxStatus !== undefined ||
    filters.sessionId !== undefined
  );
}
