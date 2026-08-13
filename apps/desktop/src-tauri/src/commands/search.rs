use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteArguments;
use tauri::State;

use crate::application::search_index_service::SearchIndexService;
use crate::bootstrap::app_state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    // SRC-002 filters
    #[serde(default)]
    pub item_type: Option<String>,
    #[serde(default)]
    pub source_app: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub favorite_only: Option<bool>,
    #[serde(default)]
    pub board_id: Option<String>,
    /// Items created at/after this timestamp (ms since epoch). `after:`
    #[serde(default)]
    pub after: Option<i64>,
    /// Items created at/before this timestamp (ms since epoch). `before:`
    #[serde(default)]
    pub before: Option<i64>,
    /// Inbox status filter: new | triaged | archived. `inbox:`
    #[serde(default)]
    pub inbox_status: Option<String>,
    /// Capture session id. `session:`
    #[serde(default)]
    pub session_id: Option<String>,
}

fn default_limit() -> i64 {
    30
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    pub item_id: String,
    pub title: String,
    pub body_snippet: String,
    pub url: String,
    pub domain: String,
    pub source_app: String,
    pub item_type: String,
    pub is_favorite: bool,
    pub created_at: i64,
    pub score: f64,
}

type ResultRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    i32,
    i64,
    f64,
);

/// Builds the filter WHERE fragments and their bind values (SRC-002).
/// All values are bound parameters — never interpolated — preventing SQL injection.
fn build_filters(request: &SearchRequest) -> (String, Vec<String>, Vec<i64>) {
    let mut sql = String::new();
    let mut strings: Vec<String> = Vec::new();
    let mut numbers: Vec<i64> = Vec::new();

    if let Some(ref item_type) = request.item_type {
        strings.push(item_type.clone());
        sql.push_str(" AND i.item_type = ?");
    }
    if let Some(ref source) = request.source_app {
        strings.push(source.clone());
        sql.push_str(" AND sd.source_app = ?");
    }
    if let Some(ref domain) = request.domain {
        strings.push(domain.clone());
        sql.push_str(" AND sd.domain = ?");
    }
    if request.favorite_only == Some(true) {
        sql.push_str(" AND i.is_favorite = 1");
    }
    if let Some(ref board_id) = request.board_id {
        strings.push(board_id.clone());
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM board_nodes bn WHERE bn.item_id = i.id \
             AND bn.board_id = ? AND bn.deleted_at IS NULL)",
        );
    }
    if let Some(after) = request.after {
        numbers.push(after);
        sql.push_str(" AND i.created_at >= ?");
    }
    if let Some(before) = request.before {
        numbers.push(before);
        sql.push_str(" AND i.created_at <= ?");
    }
    if let Some(ref status) = request.inbox_status {
        strings.push(status.clone());
        sql.push_str(" AND i.inbox_status = ?");
    }
    if let Some(ref session_id) = request.session_id {
        strings.push(session_id.clone());
        sql.push_str(" AND i.capture_session_id = ?");
    }

    (sql, strings, numbers)
}

/// Binds fts query + all filter values in the order the SQL expects.
fn bind_all<'q>(
    query: sqlx::query::QueryAs<'q, sqlx::Sqlite, ResultRow, SqliteArguments<'q>>,
    fts_query: Option<&'q str>,
    strings: &'q [String],
    numbers: &'q [i64],
    limit: i64,
    offset: i64,
) -> sqlx::query::QueryAs<'q, sqlx::Sqlite, ResultRow, SqliteArguments<'q>> {
    let mut q = query;
    if let Some(fts) = fts_query {
        q = q.bind(fts);
    }
    for s in strings {
        q = q.bind(s);
    }
    for n in numbers {
        q = q.bind(n);
    }
    q.bind(limit).bind(offset)
}

/// Full-text search across items using FTS5 with optional filters (SRC-001, SRC-002).
/// Supports filter-only search (empty text query) for browsing by filter.
#[tauri::command]
pub async fn search_items(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<Vec<SearchResultDto>, String> {
    let query = request.query.trim();
    let limit = request.limit.min(100);
    let (filter_sql, strings, numbers) = build_filters(&request);

    let rows: Vec<ResultRow> = if query.is_empty() {
        // Filter-only browse path (no FTS). Requires at least one filter.
        if strings.is_empty() && numbers.is_empty() && request.favorite_only != Some(true) {
            return Ok(vec![]);
        }
        let sql = format!(
            r"SELECT sd.item_id, COALESCE(sd.title,''), COALESCE(sd.body,''), COALESCE(sd.url,''),
                     COALESCE(sd.domain,''), COALESCE(sd.source_app,''),
                     i.item_type, i.is_favorite, i.created_at, 0.0 as rank
              FROM search_documents sd
              JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
              WHERE 1=1{filter_sql}
              ORDER BY i.created_at DESC LIMIT ? OFFSET ?"
        );
        let q = sqlx::query_as::<_, ResultRow>(&sql);
        bind_all(q, None, &strings, &numbers, limit, request.offset)
            .fetch_all(state.db.pool())
            .await
            .map_err(|e| e.to_string())?
    } else {
        let fts_query = build_fts_query(query);
        let sql = format!(
            r"SELECT sd.item_id, sd.title, sd.body, sd.url, sd.domain, sd.source_app,
                     i.item_type, i.is_favorite, i.created_at, rank
              FROM search_fts_unicode
              JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
              JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
              WHERE search_fts_unicode MATCH ?{filter_sql}
              ORDER BY rank LIMIT ? OFFSET ?"
        );
        let q = sqlx::query_as::<_, ResultRow>(&sql);
        bind_all(
            q,
            Some(&fts_query),
            &strings,
            &numbers,
            limit,
            request.offset,
        )
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| {
            tracing::debug!(event = "fts_query_failed", error = %e);
            e.to_string()
        })?
    };

    let now_ms = chrono::Utc::now().timestamp_millis();

    let mut dtos: Vec<SearchResultDto> = rows
        .into_iter()
        .map(
            |(
                item_id,
                title,
                body,
                url,
                domain,
                source_app,
                item_type,
                is_fav,
                created_at,
                score,
            )| {
                let snippet: String = body.chars().take(200).collect();

                // Ranking boosts (lower score = better in FTS5)
                let mut boosted = score;
                if is_fav == 1 {
                    boosted -= 2.0; // Favorite boost
                }
                let age_days = ((now_ms - created_at) as f64) / 86_400_000.0;
                if age_days < 1.0 {
                    boosted -= 1.5; // Today boost
                } else if age_days < 7.0 {
                    boosted -= 0.5; // This week boost
                }
                if !query.is_empty() && title.to_lowercase() == query.to_lowercase() {
                    boosted -= 3.0; // Exact title match
                }

                SearchResultDto {
                    item_id,
                    title,
                    body_snippet: snippet,
                    url,
                    domain,
                    source_app,
                    item_type,
                    is_favorite: is_fav == 1,
                    created_at,
                    score: boosted,
                }
            },
        )
        .collect();

    // Re-sort by boosted score
    dtos.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Trigram fallback: if unicode61 returned nothing and query is short (CJK/substring)
    if dtos.is_empty() && !query.is_empty() && query.chars().count() <= 4 {
        let fts_query = build_fts_query(query);
        let sql = format!(
            r"SELECT sd.item_id, sd.title, sd.body, sd.url, sd.domain, sd.source_app,
                     i.item_type, i.is_favorite, i.created_at, rank
              FROM search_fts_trigram
              JOIN search_documents sd ON sd.rowid = search_fts_trigram.rowid
              JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
              WHERE search_fts_trigram MATCH ?{filter_sql}
              ORDER BY rank LIMIT ? OFFSET ?"
        );
        let q = sqlx::query_as::<_, ResultRow>(&sql);
        let trigram_results = bind_all(
            q,
            Some(&fts_query),
            &strings,
            &numbers,
            limit,
            request.offset,
        )
        .fetch_all(state.db.pool())
        .await;

        if let Ok(rows) = trigram_results {
            dtos = rows
                .into_iter()
                .map(
                    |(
                        item_id,
                        title,
                        body,
                        url,
                        domain,
                        source_app,
                        item_type,
                        is_fav,
                        created_at,
                        score,
                    )| {
                        SearchResultDto {
                            item_id,
                            title,
                            body_snippet: body.chars().take(200).collect(),
                            url,
                            domain,
                            source_app,
                            item_type,
                            is_favorite: is_fav == 1,
                            created_at,
                            score,
                        }
                    },
                )
                .collect();
        }
    }

    // SRC-005: FTS5 trigram phrases need ≥3 characters, so 1–2 char queries
    // still found nothing — fall back to a parameterized substring scan so
    // short searches remain usable (Phase-3 "1–2 character search" fixture).
    if dtos.is_empty() && !query.is_empty() && query.chars().count() < 3 {
        let like = short_query_like_pattern(query);
        let rows: Vec<ResultRow> = sqlx::query_as(
            r"SELECT sd.item_id, COALESCE(sd.title,''), COALESCE(sd.body,''), COALESCE(sd.url,''),
                     COALESCE(sd.domain,''), COALESCE(sd.source_app,''),
                     i.item_type, i.is_favorite, i.created_at, 0.0 as rank
              FROM search_documents sd
              JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
              WHERE (sd.title LIKE ? ESCAPE '\' OR sd.body LIKE ? ESCAPE '\')
              ORDER BY i.created_at DESC LIMIT ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;

        dtos = rows
            .into_iter()
            .map(
                |(
                    item_id,
                    title,
                    body,
                    url,
                    domain,
                    source_app,
                    item_type,
                    is_fav,
                    created_at,
                    score,
                )| {
                    SearchResultDto {
                        item_id,
                        title,
                        body_snippet: body.chars().take(200).collect(),
                        url,
                        domain,
                        source_app,
                        item_type,
                        is_favorite: is_fav == 1,
                        created_at,
                        score,
                    }
                },
            )
            .collect();
    }

    Ok(dtos)
}

/// SRC-005: builds a LIKE pattern for 1–2 char queries, escaping LIKE
/// wildcards (`%`, `_`, `\`) in user input so short searches stay literal.
pub fn short_query_like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

// --- SRC-004: search index maintenance (verify / rebuild / optimize) ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexHealthDto {
    pub live_items: i64,
    pub indexed_docs: i64,
    pub missing_docs: i64,
    pub orphan_docs: i64,
    pub fts_unicode_ok: bool,
    pub fts_trigram_ok: bool,
    pub healthy: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexRebuildDto {
    pub rebuilt: i64,
    pub removed_orphans: i64,
}

/// Checks the search projection against canonical data (read-only).
#[tauri::command]
pub async fn verify_search_index(
    state: State<'_, AppState>,
) -> Result<SearchIndexHealthDto, String> {
    let svc = SearchIndexService::new(state.db.clone());
    let h = svc.verify().await.map_err(|e| e.to_string())?;
    Ok(SearchIndexHealthDto {
        live_items: h.live_items,
        indexed_docs: h.indexed_docs,
        missing_docs: h.missing_docs,
        orphan_docs: h.orphan_docs,
        fts_unicode_ok: h.fts_unicode_ok,
        fts_trigram_ok: h.fts_trigram_ok,
        healthy: h.is_healthy(),
    })
}

/// Repairs the search projection: restores missing documents from canonical
/// item data and removes orphaned documents. Canonical data is never touched.
#[tauri::command]
pub async fn rebuild_search_index(
    state: State<'_, AppState>,
) -> Result<SearchIndexRebuildDto, String> {
    let svc = SearchIndexService::new(state.db.clone());
    let (rebuilt, removed_orphans) = svc.rebuild().await.map_err(|e| e.to_string())?;
    Ok(SearchIndexRebuildDto {
        rebuilt,
        removed_orphans,
    })
}

/// Runs the FTS5 'optimize' command on both indexes.
#[tauri::command]
pub async fn optimize_search_index(state: State<'_, AppState>) -> Result<(), String> {
    let svc = SearchIndexService::new(state.db.clone());
    svc.optimize().await.map_err(|e| e.to_string())
}

/// Builds a safe FTS5 query from user input.
fn build_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> SearchRequest {
        SearchRequest {
            query: String::new(),
            limit: 30,
            offset: 0,
            item_type: None,
            source_app: None,
            domain: None,
            favorite_only: None,
            board_id: None,
            after: None,
            before: None,
            inbox_status: None,
            session_id: None,
        }
    }

    #[test]
    fn build_fts_query_simple() {
        assert_eq!(build_fts_query("hello world"), "\"hello\" OR \"world\"");
    }

    #[test]
    fn build_fts_query_escapes_quotes() {
        assert_eq!(build_fts_query(r#"say "hi""#), "\"say\" OR \"\"\"hi\"\"\"");
    }

    #[test]
    fn build_fts_query_empty() {
        assert_eq!(build_fts_query(""), "");
    }

    #[test]
    fn build_fts_query_cjk() {
        assert_eq!(build_fts_query("剪貼簿"), "\"剪貼簿\"");
    }

    #[test]
    fn build_filters_none_produces_empty_sql() {
        let req = base_request();
        let (sql, strings, numbers) = build_filters(&req);
        assert!(sql.is_empty());
        assert!(strings.is_empty());
        assert!(numbers.is_empty());
    }

    #[test]
    fn build_filters_uses_placeholders_not_interpolation() {
        let mut req = base_request();
        req.item_type = Some("link'. DROP TABLE items; --".to_string());
        req.domain = Some("example.com".to_string());
        req.after = Some(1_000);
        req.before = Some(2_000);
        req.inbox_status = Some("new".to_string());
        req.session_id = Some("sess-1".to_string());
        req.board_id = Some("board-1".to_string());

        let (sql, strings, numbers) = build_filters(&req);
        // No user content in the SQL text itself — injection payload stays in bind values
        assert!(!sql.contains("DROP"));
        assert!(sql.contains("i.item_type = ?"));
        assert!(sql.contains("sd.domain = ?"));
        assert!(sql.contains("i.created_at >= ?"));
        assert!(sql.contains("i.created_at <= ?"));
        assert!(sql.contains("i.inbox_status = ?"));
        assert!(sql.contains("i.capture_session_id = ?"));
        assert!(sql.contains("bn.board_id = ?"));
        assert_eq!(numbers, vec![1_000, 2_000]);
        assert_eq!(strings.len(), 5);
    }

    #[test]
    fn build_filters_favorite_only_no_binds() {
        let mut req = base_request();
        req.favorite_only = Some(true);
        let (sql, strings, numbers) = build_filters(&req);
        assert_eq!(sql, " AND i.is_favorite = 1");
        assert!(strings.is_empty());
        assert!(numbers.is_empty());
    }
}
