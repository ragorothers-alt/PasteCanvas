-- Migration 003: dedup lookup indexes for the capture hot path.
-- Additive only; 001/002 remain immutable.

-- URL dedup runs on every captured link (find_item_by_normalized_url);
-- without an index this is a full scan of items per capture.
-- (assets.sha256 needs no index: the UNIQUE constraint in 001 already
-- carries an implicit unique index, verified via EXPLAIN QUERY PLAN.)
CREATE INDEX IF NOT EXISTS ix_items_normalized_url
ON items(normalized_url)
WHERE normalized_url IS NOT NULL AND deleted_at IS NULL;
