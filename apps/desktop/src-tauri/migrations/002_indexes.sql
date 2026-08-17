-- Migration 002: production indexes for trash, edges, jobs, sessions.
-- Additive only; 001 remains immutable.

CREATE INDEX IF NOT EXISTS ix_items_deleted
ON items(deleted_at)
WHERE deleted_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS ix_items_session
ON items(capture_session_id)
WHERE capture_session_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_board_nodes_item_all
ON board_nodes(item_id)
WHERE item_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS ix_board_edges_source
ON board_edges(source_node_id);

CREATE INDEX IF NOT EXISTS ix_board_edges_target
ON board_edges(target_node_id);

CREATE INDEX IF NOT EXISTS ix_board_edges_board
ON board_edges(board_id)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_jobs_running_lease
ON jobs(state, lease_until)
WHERE state = 'running';

CREATE INDEX IF NOT EXISTS ix_item_assets_asset
ON item_assets(asset_id);

CREATE INDEX IF NOT EXISTS ix_item_assets_item
ON item_assets(item_id);

CREATE INDEX IF NOT EXISTS ix_capture_sessions_active
ON capture_sessions(state, updated_at)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_boards_updated
ON boards(updated_at DESC)
WHERE deleted_at IS NULL;
