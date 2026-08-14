import { useState, useCallback, useRef, useEffect } from "react";
import { searchItems } from "./api";
import type { SearchResult, SearchFilters } from "./api";
import { parseSearchQuery, hasActiveFilters } from "./queryParser";
import {
  openItemDetail,
  copyItemContent,
  openItemSource,
  favoriteItem,
  locateItemBoards,
  navigateToBoard,
} from "./searchActions";
import { showCaptureSuccess } from "../../stores/toastStore";
import { ArrowSquareOut, Copy, MapPin, MagnifyingGlass, Star } from "@phosphor-icons/react";

const TYPE_FILTERS = ["plain_text", "link", "image", "pdf", "code"] as const;
const TYPE_LABELS: Record<string, string> = { plain_text: "文字", link: "連結", image: "圖片", pdf: "PDF", code: "程式碼" };

export function SearchBar() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [hasSearched, setHasSearched] = useState(false);
  const [activeFilter, setActiveFilter] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // WIN-005: the global_search hotkey focuses this input via a backend event.
  useEffect(() => {
    const handler = () => inputRef.current?.focus();
    window.addEventListener("paste-canvas://focus-search", handler);
    return () => window.removeEventListener("paste-canvas://focus-search", handler);
  }, []);

  const doSearch = useCallback(async (q: string, filter?: string | null) => {
    // Parse inline filter syntax (SRC-002): "rust type:code after:2024-01-01"
    const { freeText, filters: parsedFilters } = parseSearchQuery(q);

    // Chip filter (type) overrides/merges with parsed
    const filters: SearchFilters = { ...parsedFilters };
    if (filter) filters.itemType = filter;

    // Allow filter-only search (no free text) when filters are present
    if (!freeText.trim() && !hasActiveFilters(filters)) {
      setResults([]);
      setHasSearched(false);
      return;
    }
    setSearching(true);
    try {
      const r = await searchItems(freeText.trim(), 30, filters);
      setResults(r);
      setHasSearched(true);
    } catch {
      setResults([]);
      setHasSearched(true);
    } finally {
      setSearching(false);
    }
  }, []);

  const handleChange = useCallback(
    (value: string) => {
      setQuery(value);
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => void doSearch(value, activeFilter), 200);
    },
    [doSearch, activeFilter],
  );

  const handleFilterClick = useCallback(
    (filter: string) => {
      const next = activeFilter === filter ? null : filter;
      setActiveFilter(next);
      void doSearch(query, next);
    },
    [activeFilter, doSearch, query],
  );

  // Sidebar commands are intentionally handled here so they use the same
  // parser/API path as typed searches instead of pretending to navigate to a
  // different Inbox that is not actually filtered.
  useEffect(() => {
    const handler = (event: Event) => {
      const requestedQuery = (event as CustomEvent<{ query?: string }>).detail?.query;
      if (!requestedQuery) return;
      setQuery(requestedQuery);
      setActiveFilter(null);
      inputRef.current?.focus();
      void doSearch(requestedQuery, null);
    };
    window.addEventListener("paste-canvas://search-command", handler);
    return () => window.removeEventListener("paste-canvas://search-command", handler);
  }, [doSearch]);

  return (
    <div className="search-container">
      <div className="search-bar">
        <MagnifyingGlass className="search-icon" aria-hidden="true" />
        <input
          ref={inputRef}
          className="search-input"
          type="text"
          placeholder="搜尋…（支援 type: source: domain: board: after: before: favorite: inbox: session:）"
          value={query}
          onChange={(e) => handleChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void doSearch(query, activeFilter);
          }}
        />
        {searching && <span className="search-spinner">…</span>}
      </div>

      <div className="search-filters">
        {TYPE_FILTERS.map((t) => (
          <button
            key={t}
            className={`filter-chip ${activeFilter === t ? "active" : ""}`}
            onClick={() => handleFilterClick(t)}
          >
            {TYPE_LABELS[t] ?? t}
          </button>
        ))}
      </div>

      {hasSearched && (
        <div className="search-results">
          {results.length === 0 && (
            <div className="search-empty">找不到結果。</div>
          )}
          {results.map((r) => (
            <div key={r.itemId} className="search-result-item">
              <div
                className="result-main"
                onClick={() => openItemDetail(r.itemId)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter") openItemDetail(r.itemId);
                }}
              >
                <div className="result-title">
                  {r.isFavorite && <Star weight="fill" aria-hidden="true" />}
                  {r.title || r.bodySnippet || "(未命名)"}
                </div>
                <div className="result-meta">
                  <span className="result-type">{TYPE_LABELS[r.itemType] ?? r.itemType}</span>
                  {r.domain && <span className="result-domain">{r.domain}</span>}
                  {r.sourceApp && <span className="result-source">{r.sourceApp}</span>}
                </div>
              </div>
              {/* SRC-003 result actions */}
              <div className="result-actions">
                <button
                  className="result-action"
                  title="複製內容"
                  aria-label="複製內容"
                  onClick={() => {
                    void copyItemContent(r.itemId).then((copied) => {
                      if (copied !== null) showCaptureSuccess("已複製");
                    });
                  }}
                >
                  <Copy aria-hidden="true" />
                </button>
                <button
                  className="result-action"
                  title={r.isFavorite ? "取消收藏" : "加入收藏"}
                  aria-label={r.isFavorite ? "取消收藏" : "加入收藏"}
                  onClick={() => {
                    void favoriteItem(r.itemId).then(() => void doSearch(query, activeFilter));
                  }}
                >
                  <Star weight={r.isFavorite ? "fill" : "regular"} aria-hidden="true" />
                </button>
                {r.url && (
                  <button
                  className="result-action"
                  title="開啟來源"
                  aria-label="開啟來源"
                    onClick={() => {
                      void openItemSource(r.itemId);
                    }}
                  >
                    <ArrowSquareOut aria-hidden="true" />
                  </button>
                )}
                <button
                  className="result-action"
                  title="在看板中定位"
                  aria-label="在看板中定位"
                  onClick={() => {
                    void locateItemBoards(r.itemId).then((boards) => {
                      if (boards.length > 0) {
                        navigateToBoard(boards[0]!.boardId);
                      } else {
                        showCaptureSuccess("尚未加入任何看板");
                      }
                    });
                  }}
                >
                  <MapPin aria-hidden="true" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
