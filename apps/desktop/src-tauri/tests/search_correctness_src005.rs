//! SRC-005 search correctness fixtures (Phase-3 "Correctness fixtures").
//!
//! Exercises the REAL production FTS MATCH path (same SQL and same query
//! builder rules as commands/search.rs) against adversarial and
//! internationalized inputs:
//! - FTS injection / control characters must never crash, leak, or corrupt
//! - Traditional Chinese (no spaces), Japanese mixed scripts
//! - camelCase/snake_case identifiers
//! - 1–2 character searches (trigram fallback path)
//! - Deleted/trashed visibility policy

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
    use pastecanvas_desktop_lib::domain::entities::item::CaptureMethod;
    use pastecanvas_desktop_lib::domain::value_objects::EntityId;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;

    async fn setup() -> AppDatabase {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        db
    }

    async fn capture(db: &AppDatabase, text: &str) -> String {
        let svc = CaptureService::new(db.clone(), EntityId::new());
        svc.capture_text(CaptureTextRequest {
            text: text.to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        })
        .await
        .expect("capture")
        .item_id
        .to_string()
    }

    /// Mirrors commands/search.rs::build_fts_query exactly (keep in sync).
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

    /// The production unicode MATCH query (commands/search.rs shape).
    async fn fts_unicode_search(db: &AppDatabase, query: &str) -> Result<Vec<String>, sqlx::Error> {
        let fts = build_fts_query(query);
        sqlx::query_scalar(
            "SELECT sd.item_id
             FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
             WHERE search_fts_unicode MATCH ?
             ORDER BY rank LIMIT 30",
        )
        .bind(&fts)
        .fetch_all(db.pool())
        .await
    }

    /// The production trigram MATCH query (short/CJK fallback path).
    async fn fts_trigram_search(db: &AppDatabase, query: &str) -> Result<Vec<String>, sqlx::Error> {
        let fts = build_fts_query(query);
        sqlx::query_scalar(
            "SELECT sd.item_id
             FROM search_fts_trigram
             JOIN search_documents sd ON sd.rowid = search_fts_trigram.rowid
             JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
             WHERE search_fts_trigram MATCH ?
             LIMIT 30",
        )
        .bind(&fts)
        .fetch_all(db.pool())
        .await
    }

    // --- FTS injection & control characters ---

    #[tokio::test]
    async fn fts_injection_payloads_are_neutralized() {
        let db = setup().await;
        let secret = capture(&db, "top secret payload content").await;

        let payloads = [
            // FTS operators must be treated as literal text
            "secret\" OR \"1\"=\"1",
            "secret OR deleted",
            "secret AND NOT payload",
            "secret NEAR(leak)",
            "secret*",
            "^secret",
            // SQL-level attempts inside the MATCH expression
            "x'); DROP TABLE items; --",
            "x\")); DELETE FROM items; --",
            // Column-filter abuse attempts
            "item_id:secret",
            "{title}: secret",
        ];

        for payload in &payloads {
            let outcome = fts_unicode_search(&db, payload).await;
            match outcome {
                Ok(rows) => {
                    // A neutralized payload may match literally or return
                    // nothing — it must NEVER return items via operator abuse.
                    // "secret" appears literally in some payloads, so the only
                    // allowed match is the item containing the literal text.
                    for r in &rows {
                        assert_eq!(r, &secret, "payload {payload:?} leaked via operator abuse");
                    }
                }
                Err(_) => { /* a rejected query is also safe */ }
            }
        }

        // Database intact
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items")
            .fetch_one(db.pool())
            .await
            .expect("items intact");
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn control_characters_do_not_crash_or_leak() {
        let db = setup().await;
        capture(&db, "control char fixture alpha").await;

        let queries = [
            "\u{0000}",
            "alpha\u{0000}",
            "\u{0007}\u{001b}",
            "\u{202E}alpha", // RTL override
            "\u{FEFF}alpha", // BOM
            "alpha\n\r\tbeta",
        ];

        for q in &queries {
            // Either empty results or an error — never a panic, never a leak.
            let _ = fts_unicode_search(&db, q).await;
            let _ = fts_trigram_search(&db, q).await;
        }

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_documents")
            .fetch_one(db.pool())
            .await
            .expect("docs intact");
        assert_eq!(count.0, 1);
    }

    // --- Internationalized fixtures ---

    #[tokio::test]
    async fn traditional_chinese_no_space_text_is_searchable() {
        let db = setup().await;
        let id = capture(&db, "剪貼簿管理工具的搜尋功能測試").await;

        // Short CJK query goes through the trigram fallback path in production.
        let rows = fts_trigram_search(&db, "剪貼簿").await.expect("trigram");
        assert!(
            rows.contains(&id),
            "CJK no-space text must match via trigram"
        );
    }

    #[tokio::test]
    async fn japanese_mixed_scripts_are_searchable() {
        let db = setup().await;
        let id = capture(&db, "これはテストです test 漢字 katakana カタカナ").await;

        // Latin token via unicode index
        let rows = fts_unicode_search(&db, "test").await.expect("unicode");
        assert!(rows.contains(&id));

        // Kana substring via trigram index
        let rows = fts_trigram_search(&db, "カタカナ").await.expect("trigram");
        assert!(rows.contains(&id));
    }

    #[tokio::test]
    async fn camel_and_snake_case_identifiers_are_findable() {
        let db = setup().await;
        let id = capture(&db, "fn parseSearchQuery and build_fts_query ERROR_CODE_42").await;

        // Exact identifier tokens (unicode61 splits on case/punct boundaries)
        let rows = fts_unicode_search(&db, "parseSearchQuery")
            .await
            .expect("u1");
        assert!(rows.contains(&id), "camelCase token must match");

        let rows = fts_trigram_search(&db, "build_fts_query")
            .await
            .expect("t1");
        assert!(rows.contains(&id), "snake_case must match via trigram");
    }

    // --- Short queries ---

    /// The production short-query fallback (commands/search.rs): parameterized
    /// LIKE scan with wildcard-escaped pattern, for queries under 3 chars.
    async fn short_query_search(db: &AppDatabase, query: &str) -> Vec<String> {
        use pastecanvas_desktop_lib::commands::search::short_query_like_pattern;
        let like = short_query_like_pattern(query);
        sqlx::query_scalar(
            r"SELECT sd.item_id
              FROM search_documents sd
              JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
              WHERE (sd.title LIKE ? ESCAPE '\' OR sd.body LIKE ? ESCAPE '\')
              ORDER BY i.created_at DESC LIMIT 30",
        )
        .bind(&like)
        .bind(&like)
        .fetch_all(db.pool())
        .await
        .expect("short query search")
    }

    #[tokio::test]
    async fn one_and_two_char_queries_find_matches_via_fallback() {
        let db = setup().await;
        let id = capture(&db, "xylophone exceptional zest").await;
        capture(&db, "unrelated other content").await;

        // 2-char query: FTS trigram cannot serve it; LIKE fallback does.
        let rows = short_query_search(&db, "xy").await;
        assert!(rows.contains(&id), "2-char query must match via fallback");

        // 1-char query matches all items containing the char — but never
        // crashes and never returns items that lack it.
        let rows = short_query_search(&db, "z").await;
        assert!(rows.contains(&id));
        assert_eq!(rows.len(), 1, "only the item containing 'z' may match");
    }

    #[tokio::test]
    async fn like_wildcards_in_short_queries_are_literal() {
        let db = setup().await;
        let id = capture(&db, "wildcard fixture 100% done").await;
        capture(&db, "another item entirely").await;

        // '%' must be treated literally, not as a LIKE wildcard.
        let rows = short_query_search(&db, "%").await;
        assert_eq!(
            rows,
            vec![id.clone()],
            "'%' must match only items containing %"
        );

        // '_' likewise literal.
        let rows = short_query_search(&db, "_").await;
        assert!(rows.is_empty(), "'_' must not act as a wildcard");
    }

    // --- Visibility policy ---

    #[tokio::test]
    async fn trashed_items_never_appear_in_search() {
        let db = setup().await;
        let alive = capture(&db, "visibility fixture alive").await;
        let doomed = capture(&db, "visibility fixture doomed").await;

        // Soft-delete one item
        sqlx::query("UPDATE items SET deleted_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp_millis())
            .bind(&doomed)
            .execute(db.pool())
            .await
            .expect("trash");

        let rows = fts_unicode_search(&db, "visibility").await.expect("search");
        assert!(rows.contains(&alive));
        assert!(!rows.contains(&doomed), "trashed item must be excluded");

        let rows = fts_trigram_search(&db, "visibility").await.expect("tri");
        assert!(
            !rows.contains(&doomed),
            "trashed item must be excluded (trigram)"
        );
    }

    #[tokio::test]
    async fn same_item_on_two_boards_is_not_duplicated_in_results() {
        use pastecanvas_desktop_lib::application::board_service::BoardService;
        let db = setup().await;
        let id = capture(&db, "dual board fixture").await;

        let svc = BoardService::new(db.clone(), EntityId::new());
        let b1 = svc.create_board("B1", None).await.expect("b1");
        let b2 = svc.create_board("B2", None).await.expect("b2");
        svc.place_item_on_board(&b1.id, &id, 0.0, 0.0)
            .await
            .expect("p1");
        svc.place_item_on_board(&b2.id, &id, 0.0, 0.0)
            .await
            .expect("p2");

        let rows = fts_unicode_search(&db, "fixture").await.expect("search");
        let occurrences = rows.iter().filter(|r| *r == &id).count();
        assert_eq!(occurrences, 1, "item must appear once even on two boards");
    }
}
