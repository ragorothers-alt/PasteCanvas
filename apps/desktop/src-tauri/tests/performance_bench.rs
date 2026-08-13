//! Performance benchmark tests for search and capture operations.
//! Verifies GOAL.md quality targets:
//! - 20,000 Item search: p95 < 150ms
//! - Text capture to persist: p95 < 300ms

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
    use pastecanvas_desktop_lib::domain::entities::item::CaptureMethod;
    use pastecanvas_desktop_lib::domain::value_objects::EntityId;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;
    use std::time::Instant;

    async fn setup() -> AppDatabase {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        db
    }

    /// Inserts N items directly via SQL for benchmarking.
    async fn seed_items(db: &AppDatabase, count: usize) {
        let device_id = "bench-device";
        let now = chrono::Utc::now().timestamp_millis();

        for i in 0..count {
            let id = format!("bench-item-{i:06}");
            let title = format!("Item {i}: test content for searching");
            let body = format!(
                "This is benchmark item number {i}. It contains various words like
                 programming, rust, typescript, database, search, performance, testing.
                 Unique identifier: bench-{i:06}"
            );

            sqlx::query(
                "INSERT INTO items (id, item_type, title, plain_text, inbox_status, is_favorite, is_pinned,
                  capture_method, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
                 VALUES (?, 'plain_text', ?, ?, 'new', 0, 0, 'manual_hotkey', ?, ?, 1, ?, ?, 'local_only')",
            )
            .bind(&id)
            .bind(&title)
            .bind(&body)
            .bind(now - (i as i64 * 1000))
            .bind(now - (i as i64 * 1000))
            .bind(device_id)
            .bind(device_id)
            .execute(db.pool())
            .await
            .expect("insert item");

            // Also insert search document
            sqlx::query(
                "INSERT INTO search_documents (item_id, title, body, url, domain, source_app, tags_text, filename, updated_at)
                 VALUES (?, ?, ?, '', '', 'bench', '', '', ?)",
            )
            .bind(&id)
            .bind(&title)
            .bind(&body)
            .bind(now)
            .execute(db.pool())
            .await
            .expect("insert search doc");
        }
    }

    #[tokio::test]
    async fn search_1000_items_under_50ms() {
        let db = setup().await;
        seed_items(&db, 1000).await;

        // Verify FTS table has data
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_fts_unicode")
            .fetch_one(db.pool())
            .await
            .expect("count");
        println!("FTS unicode entries: {}", count.0);

        // Benchmark (query may return 0 if FTS triggers haven't synced in-memory DB)
        let start = Instant::now();
        let iterations = 10;

        for _ in 0..iterations {
            let _results: Vec<(String,)> = sqlx::query_as(
                "SELECT sd.item_id FROM search_documents sd WHERE sd.body LIKE '%programming%' LIMIT 30",
            )
            .fetch_all(db.pool())
            .await
            .expect("search");
        }

        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

        println!("Search 1000 items (LIKE): avg {avg_ms:.2}ms per query");
        assert!(
            avg_ms < 50.0,
            "Search too slow: {avg_ms:.2}ms (target: <50ms for 1000 items)"
        );
    }

    #[tokio::test]
    async fn search_5000_items_under_100ms() {
        let db = setup().await;
        seed_items(&db, 5000).await;

        // Benchmark
        let start = Instant::now();
        let iterations = 5;

        for _ in 0..iterations {
            let _results: Vec<(String,)> = sqlx::query_as(
                "SELECT sd.item_id FROM search_documents sd WHERE sd.body LIKE '%database%' LIMIT 30",
            )
            .fetch_all(db.pool())
            .await
            .expect("search");
        }

        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

        println!("Search 5000 items (LIKE): avg {avg_ms:.2}ms per query");
        assert!(
            avg_ms < 100.0,
            "Search too slow: {avg_ms:.2}ms (target: <100ms for 5000 items)"
        );
    }

    #[tokio::test]
    async fn capture_text_under_10ms() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        // Warm up
        svc.capture_text(CaptureTextRequest {
            text: "warmup".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        })
        .await
        .expect("warmup");

        // Benchmark
        let start = Instant::now();
        let iterations = 10;

        for i in 0..iterations {
            svc.capture_text(CaptureTextRequest {
                text: format!("benchmark capture {i}"),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");
        }

        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

        println!("Capture text: avg {avg_ms:.2}ms per capture");
        assert!(
            avg_ms < 10.0,
            "Capture too slow: {avg_ms:.2}ms (target: <10ms)"
        );
    }

    #[tokio::test]
    async fn inbox_query_1000_items_under_20ms() {
        let db = setup().await;
        seed_items(&db, 1000).await;

        // Benchmark paginated inbox query
        let start = Instant::now();
        let iterations = 10;

        for _ in 0..iterations {
            let results: Vec<(String,)> = sqlx::query_as(
                "SELECT id FROM items WHERE deleted_at IS NULL ORDER BY is_pinned DESC, created_at DESC LIMIT 50 OFFSET 0",
            )
            .fetch_all(db.pool())
            .await
            .expect("inbox query");

            assert_eq!(results.len(), 50);
        }

        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

        println!("Inbox query 1000 items: avg {avg_ms:.2}ms per query");
        assert!(
            avg_ms < 20.0,
            "Inbox query too slow: {avg_ms:.2}ms (target: <20ms)"
        );
    }

    /// GOAL.md target: "20,000 Item 全域搜尋：目標 p95 小於 150ms"
    #[tokio::test]
    async fn search_20000_items_under_150ms() {
        let db = setup().await;

        // Seed 20,000 items in batches for speed
        let device_id = "bench-device";
        let now = chrono::Utc::now().timestamp_millis();

        for batch in 0..20 {
            let mut tx = db.pool().begin().await.expect("begin");
            for i in 0..1000 {
                let idx = batch * 1000 + i;
                let id = format!("bench20k-{idx:06}");
                let title = format!("Item {idx}: searchable content");
                let body = format!(
                    "Benchmark item {idx}. Words: programming rust typescript database search performance testing unique-{idx:06}"
                );

                sqlx::query(
                    "INSERT INTO items (id, item_type, title, plain_text, inbox_status, is_favorite, is_pinned,
                      capture_method, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
                     VALUES (?, 'plain_text', ?, ?, 'new', 0, 0, 'manual_hotkey', ?, ?, 1, ?, ?, 'local_only')",
                )
                .bind(&id)
                .bind(&title)
                .bind(&body)
                .bind(now - (idx as i64 * 100))
                .bind(now - (idx as i64 * 100))
                .bind(device_id)
                .bind(device_id)
                .execute(&mut *tx)
                .await
                .expect("insert");

                sqlx::query(
                    "INSERT INTO search_documents (item_id, title, body, url, domain, source_app, tags_text, filename, updated_at)
                     VALUES (?, ?, ?, '', '', 'bench', '', '', ?)",
                )
                .bind(&id)
                .bind(&title)
                .bind(&body)
                .bind(now)
                .execute(&mut *tx)
                .await
                .expect("insert doc");
            }
            tx.commit().await.expect("commit");
        }

        // Verify count
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items")
            .fetch_one(db.pool())
            .await
            .expect("count");
        assert_eq!(count.0, 20000);
        println!("Seeded 20,000 items");

        // Benchmark search
        let start = Instant::now();
        let iterations = 5;

        for _ in 0..iterations {
            let _results: Vec<(String,)> = sqlx::query_as(
                "SELECT sd.item_id FROM search_documents sd WHERE sd.body LIKE '%performance%' LIMIT 30",
            )
            .fetch_all(db.pool())
            .await
            .expect("search");
        }

        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

        println!("Search 20,000 items: avg {avg_ms:.2}ms per query");
        assert!(
            avg_ms < 150.0,
            "Search too slow: {avg_ms:.2}ms (GOAL.md target: <150ms for 20,000 items)"
        );

        // PERF-001: benchmark the REAL production query path (FTS MATCH),
        // exactly as commands/search.rs executes it, with p95 measurement.
        let fts_samples = bench_query(db.pool(), 20, || async {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT sd.item_id FROM search_fts_unicode
                 JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
                 JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
                 WHERE search_fts_unicode MATCH ?
                 ORDER BY rank LIMIT 30",
            )
            .bind("\"performance\"")
            .fetch_all(db.pool())
            .await
            .expect("fts unicode match");
            assert!(!rows.is_empty(), "FTS must find seeded items");
        })
        .await;
        let fts_p95 = percentile(&fts_samples, 0.95);
        println!("FTS unicode MATCH 20,000 items: p95 {fts_p95:.2}ms");
        assert!(
            fts_p95 < 150.0,
            "FTS MATCH too slow: p95 {fts_p95:.2}ms (GOAL.md target: <150ms)"
        );

        // Trigram path (short/CJK fallback in production).
        let tri_samples = bench_query(db.pool(), 20, || async {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT sd.item_id FROM search_fts_trigram
                 JOIN search_documents sd ON sd.rowid = search_fts_trigram.rowid
                 WHERE search_fts_trigram MATCH ?
                 LIMIT 30",
            )
            .bind("\"perf\"")
            .fetch_all(db.pool())
            .await
            .expect("fts trigram match");
            assert!(!rows.is_empty(), "trigram must find seeded items");
        })
        .await;
        let tri_p95 = percentile(&tri_samples, 0.95);
        println!("FTS trigram MATCH 20,000 items: p95 {tri_p95:.2}ms");
        assert!(
            tri_p95 < 150.0,
            "trigram MATCH too slow: p95 {tri_p95:.2}ms (GOAL.md target: <150ms)"
        );
    }

    /// PERF-001: FTS MATCH path at 5,000 items (unicode index).
    #[tokio::test]
    async fn fts_match_5000_items_p95_under_50ms() {
        let db = setup().await;
        seed_items(&db, 5000).await;

        let samples = bench_query(db.pool(), 20, || async {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT sd.item_id FROM search_fts_unicode
                 JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
                 WHERE search_fts_unicode MATCH ?
                 LIMIT 30",
            )
            .bind("\"database\"")
            .fetch_all(db.pool())
            .await
            .expect("fts match");
            assert!(!rows.is_empty(), "FTS must find seeded items");
        })
        .await;
        let p95 = percentile(&samples, 0.95);
        println!("FTS MATCH 5,000 items: p95 {p95:.2}ms");
        assert!(p95 < 50.0, "FTS MATCH too slow at 5k: p95 {p95:.2}ms");
    }

    /// Runs `f` n times, returning per-run durations in milliseconds.
    async fn bench_query<F, Fut>(pool: &sqlx::SqlitePool, n: usize, f: F) -> Vec<f64>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let _ = pool; // pool access is captured inside the closure
        let mut samples = Vec::with_capacity(n);
        // Warm-up run (page cache / query plan)
        f().await;
        for _ in 0..n {
            let start = Instant::now();
            f().await;
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        samples
    }

    /// Percentile of a sample set (0.0–1.0). Panics on empty input.
    fn percentile(samples: &[f64], p: f64) -> f64 {
        assert!(!samples.is_empty());
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let idx = ((samples.len() as f64 - 1.0) * p).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}
