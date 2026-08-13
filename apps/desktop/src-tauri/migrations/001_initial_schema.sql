-- Migration 001: Initial schema
-- PasteCanvas foundation schema — immutable once released.

CREATE TABLE schema_metadata (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT INTO schema_metadata(key, value) VALUES
  ('schema_version', '1'),
  ('created_at', '2026-08-02');

CREATE TABLE devices (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  platform TEXT NOT NULL,
  is_current INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_seen_at INTEGER
);

CREATE UNIQUE INDEX ux_devices_current
ON devices(is_current)
WHERE is_current = 1;

CREATE TABLE folders (
  id TEXT PRIMARY KEY,
  parent_folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 255),
  sort_key TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  origin_device_id TEXT NOT NULL,
  last_modified_device_id TEXT NOT NULL,
  sync_state TEXT NOT NULL DEFAULT 'local_only'
    CHECK (sync_state IN ('local_only','pending_upload','synced','conflict','error','remote_only')),
  deleted_at INTEGER,
  CHECK (parent_folder_id IS NULL OR parent_folder_id <> id)
);

CREATE INDEX ix_folders_parent_sort
ON folders(parent_folder_id, sort_key)
WHERE deleted_at IS NULL;

CREATE TABLE boards (
  id TEXT PRIMARY KEY,
  folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 255),
  description TEXT,
  icon TEXT,
  is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
  sort_key TEXT NOT NULL,
  viewport_x REAL NOT NULL DEFAULT 0,
  viewport_y REAL NOT NULL DEFAULT 0,
  viewport_zoom REAL NOT NULL DEFAULT 1 CHECK (viewport_zoom > 0),
  board_revision INTEGER NOT NULL DEFAULT 1 CHECK (board_revision >= 1),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  origin_device_id TEXT NOT NULL,
  last_modified_device_id TEXT NOT NULL,
  sync_state TEXT NOT NULL DEFAULT 'local_only'
    CHECK (sync_state IN ('local_only','pending_upload','synced','conflict','error','remote_only')),
  deleted_at INTEGER
);

CREATE INDEX ix_boards_folder_sort
ON boards(folder_id, sort_key)
WHERE deleted_at IS NULL;

CREATE TABLE capture_sessions (
  id TEXT PRIMARY KEY,
  name TEXT CHECK (name IS NULL OR length(name) <= 255),
  state TEXT NOT NULL CHECK (state IN ('active','paused','finished')),
  started_at INTEGER NOT NULL,
  paused_at INTEGER,
  ended_at INTEGER,
  idle_timeout_ms INTEGER CHECK (idle_timeout_ms IS NULL OR idle_timeout_ms >= 0),
  source_filter_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  origin_device_id TEXT NOT NULL,
  last_modified_device_id TEXT NOT NULL,
  sync_state TEXT NOT NULL DEFAULT 'local_only'
    CHECK (sync_state IN ('local_only','pending_upload','synced','conflict','error','remote_only')),
  deleted_at INTEGER,
  CHECK ((state = 'finished' AND ended_at IS NOT NULL) OR state <> 'finished')
);

CREATE UNIQUE INDEX ux_capture_sessions_one_unfinished
ON capture_sessions((1))
WHERE state IN ('active','paused') AND deleted_at IS NULL;

CREATE TABLE source_contexts (
  id TEXT PRIMARY KEY,
  process_name TEXT,
  process_path_hash TEXT,
  application_name TEXT,
  window_title TEXT,
  source_url TEXT,
  source_domain TEXT,
  original_file_path TEXT,
  monitor_id TEXT,
  extra_json TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX ix_source_contexts_app
ON source_contexts(application_name, created_at DESC);

CREATE TABLE items (
  id TEXT PRIMARY KEY,
  item_type TEXT NOT NULL CHECK (item_type IN (
    'plain_text','rich_text','code','image','screenshot','link','video_link',
    'pdf','color','font','svg','file'
  )),
  title TEXT CHECK (title IS NULL OR length(title) <= 2000),
  plain_text TEXT,
  rich_text_json TEXT,
  sanitized_html TEXT,
  code_language TEXT CHECK (code_language IS NULL OR length(code_language) <= 100),
  normalized_url TEXT,
  display_url TEXT,
  content_hash TEXT,
  inbox_status TEXT NOT NULL DEFAULT 'new'
    CHECK (inbox_status IN ('new','triaged','archived')),
  is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
  is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
  capture_method TEXT NOT NULL CHECK (capture_method IN (
    'manual_hotkey','quick_capture','capture_session','canvas_paste','drag_drop',
    'screenshot','explorer','browser','import','manual_create'
  )),
  capture_session_id TEXT REFERENCES capture_sessions(id) ON DELETE SET NULL,
  source_context_id TEXT REFERENCES source_contexts(id) ON DELETE SET NULL,
  metadata_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  origin_device_id TEXT NOT NULL,
  last_modified_device_id TEXT NOT NULL,
  sync_state TEXT NOT NULL DEFAULT 'local_only'
    CHECK (sync_state IN ('local_only','pending_upload','synced','conflict','error','remote_only')),
  deleted_at INTEGER
);

CREATE INDEX ix_items_inbox
ON items(inbox_status, is_pinned DESC, created_at DESC)
WHERE deleted_at IS NULL;

CREATE INDEX ix_items_type_date
ON items(item_type, created_at DESC)
WHERE deleted_at IS NULL;

CREATE INDEX ix_items_content_hash
ON items(content_hash)
WHERE content_hash IS NOT NULL AND deleted_at IS NULL;

CREATE TABLE assets (
  id TEXT PRIMARY KEY,
  sha256 TEXT NOT NULL UNIQUE CHECK (length(sha256) = 64),
  media_kind TEXT NOT NULL CHECK (media_kind IN ('image','pdf','font','svg','generic')),
  mime_type TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  relative_path TEXT NOT NULL UNIQUE,
  original_filename TEXT,
  width INTEGER CHECK (width IS NULL OR width > 0),
  height INTEGER CHECK (height IS NULL OR height > 0),
  page_count INTEGER CHECK (page_count IS NULL OR page_count > 0),
  duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
  metadata_json TEXT,
  created_at INTEGER NOT NULL,
  verified_at INTEGER,
  deleted_at INTEGER
);

CREATE TABLE asset_variants (
  id TEXT PRIMARY KEY,
  asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  variant_kind TEXT NOT NULL CHECK (variant_kind IN ('thumb_sm','thumb_md','preview','pdf_page_1')),
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  mime_type TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  width INTEGER CHECK (width IS NULL OR width > 0),
  height INTEGER CHECK (height IS NULL OR height > 0),
  relative_path TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  UNIQUE(asset_id, variant_kind)
);

CREATE TABLE item_assets (
  item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  role TEXT NOT NULL CHECK (role IN ('primary','favicon','attachment','poster')),
  sort_order INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(item_id, asset_id, role)
);

CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  normalized_name TEXT NOT NULL UNIQUE,
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER
);

CREATE TABLE item_tags (
  item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(item_id, tag_id)
);

CREATE TABLE board_nodes (
  id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  node_kind TEXT NOT NULL CHECK (node_kind IN ('item','group','board_link')),
  item_id TEXT REFERENCES items(id) ON DELETE RESTRICT,
  target_board_id TEXT REFERENCES boards(id) ON DELETE RESTRICT,
  parent_node_id TEXT REFERENCES board_nodes(id) ON DELETE SET NULL,
  x REAL NOT NULL,
  y REAL NOT NULL,
  width REAL NOT NULL CHECK (width > 0),
  height REAL NOT NULL CHECK (height > 0),
  z_index INTEGER NOT NULL DEFAULT 0,
  rotation REAL NOT NULL DEFAULT 0,
  locked INTEGER NOT NULL DEFAULT 0 CHECK (locked IN (0, 1)),
  collapsed INTEGER NOT NULL DEFAULT 0 CHECK (collapsed IN (0, 1)),
  title_override TEXT,
  style_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  origin_device_id TEXT NOT NULL,
  last_modified_device_id TEXT NOT NULL,
  sync_state TEXT NOT NULL DEFAULT 'local_only'
    CHECK (sync_state IN ('local_only','pending_upload','synced','conflict','error','remote_only')),
  deleted_at INTEGER,
  CHECK (
    (node_kind = 'item' AND item_id IS NOT NULL AND target_board_id IS NULL) OR
    (node_kind = 'board_link' AND item_id IS NULL AND target_board_id IS NOT NULL) OR
    (node_kind = 'group' AND item_id IS NULL AND target_board_id IS NULL)
  ),
  CHECK (parent_node_id IS NULL OR parent_node_id <> id),
  CHECK (target_board_id IS NULL OR target_board_id <> board_id)
);

CREATE INDEX ix_board_nodes_board_z
ON board_nodes(board_id, z_index, id)
WHERE deleted_at IS NULL;

CREATE INDEX ix_board_nodes_item
ON board_nodes(item_id)
WHERE item_id IS NOT NULL AND deleted_at IS NULL;

CREATE TABLE board_edges (
  id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  source_node_id TEXT NOT NULL REFERENCES board_nodes(id) ON DELETE CASCADE,
  target_node_id TEXT NOT NULL REFERENCES board_nodes(id) ON DELETE CASCADE,
  edge_type TEXT NOT NULL DEFAULT 'default',
  label TEXT,
  style_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  origin_device_id TEXT NOT NULL,
  last_modified_device_id TEXT NOT NULL,
  sync_state TEXT NOT NULL DEFAULT 'local_only'
    CHECK (sync_state IN ('local_only','pending_upload','synced','conflict','error','remote_only')),
  deleted_at INTEGER,
  CHECK (source_node_id <> target_node_id)
);

CREATE TABLE capture_exclusions (
  id TEXT PRIMARY KEY,
  match_type TEXT NOT NULL CHECK (match_type IN ('process_name','executable_path','signer','window_title')),
  match_value TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  source TEXT NOT NULL CHECK (source IN ('user','built_in')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(match_type, match_value)
);

CREATE TABLE hotkey_bindings (
  action TEXT PRIMARY KEY CHECK (action IN (
    'capture_clipboard','quick_capture','screenshot','toggle_main_window','global_search','toggle_capture_session'
  )),
  accelerator TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  updated_at INTEGER NOT NULL
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
  updated_at INTEGER NOT NULL
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL CHECK (job_type IN (
    'fetch_link_metadata','generate_image_thumbnail','render_pdf_thumbnail',
    'extract_pdf_metadata','rebuild_search_document','cleanup_orphan_asset','build_backup'
  )),
  payload_json TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'queued'
    CHECK (state IN ('queued','running','succeeded','failed','cancelled')),
  priority INTEGER NOT NULL DEFAULT 0,
  available_at INTEGER NOT NULL,
  lease_until INTEGER,
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
  last_error_code TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX ix_jobs_claim
ON jobs(state, available_at, priority DESC, created_at)
WHERE state = 'queued';

CREATE TABLE operation_log (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  action TEXT NOT NULL,
  base_revision INTEGER,
  result_revision INTEGER,
  payload_json TEXT NOT NULL,
  device_id TEXT NOT NULL,
  occurred_at INTEGER NOT NULL
);

CREATE INDEX ix_operation_log_entity
ON operation_log(entity_type, entity_id, occurred_at, id);

CREATE TABLE event_outbox (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  dispatched_at INTEGER,
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0)
);

CREATE INDEX ix_event_outbox_pending
ON event_outbox(created_at)
WHERE dispatched_at IS NULL;

CREATE TABLE search_documents (
  item_id TEXT NOT NULL UNIQUE REFERENCES items(id) ON DELETE CASCADE,
  title TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL DEFAULT '',
  url TEXT NOT NULL DEFAULT '',
  domain TEXT NOT NULL DEFAULT '',
  source_app TEXT NOT NULL DEFAULT '',
  tags_text TEXT NOT NULL DEFAULT '',
  filename TEXT NOT NULL DEFAULT '',
  updated_at INTEGER NOT NULL
);

CREATE VIRTUAL TABLE search_fts_unicode USING fts5(
  title, body, url, domain, source_app, tags_text, filename,
  content='search_documents',
  content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE search_fts_trigram USING fts5(
  title, body, url, domain, source_app, tags_text, filename,
  content='search_documents',
  content_rowid='rowid',
  tokenize='trigram'
);

CREATE TRIGGER search_documents_ai AFTER INSERT ON search_documents BEGIN
  INSERT INTO search_fts_unicode(rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES (new.rowid, new.title, new.body, new.url, new.domain, new.source_app, new.tags_text, new.filename);
  INSERT INTO search_fts_trigram(rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES (new.rowid, new.title, new.body, new.url, new.domain, new.source_app, new.tags_text, new.filename);
END;

CREATE TRIGGER search_documents_ad AFTER DELETE ON search_documents BEGIN
  INSERT INTO search_fts_unicode(search_fts_unicode, rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES ('delete', old.rowid, old.title, old.body, old.url, old.domain, old.source_app, old.tags_text, old.filename);
  INSERT INTO search_fts_trigram(search_fts_trigram, rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES ('delete', old.rowid, old.title, old.body, old.url, old.domain, old.source_app, old.tags_text, old.filename);
END;

CREATE TRIGGER search_documents_au AFTER UPDATE ON search_documents BEGIN
  INSERT INTO search_fts_unicode(search_fts_unicode, rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES ('delete', old.rowid, old.title, old.body, old.url, old.domain, old.source_app, old.tags_text, old.filename);
  INSERT INTO search_fts_unicode(rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES (new.rowid, new.title, new.body, new.url, new.domain, new.source_app, new.tags_text, new.filename);
  INSERT INTO search_fts_trigram(search_fts_trigram, rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES ('delete', old.rowid, old.title, old.body, old.url, old.domain, old.source_app, old.tags_text, old.filename);
  INSERT INTO search_fts_trigram(rowid, title, body, url, domain, source_app, tags_text, filename)
  VALUES (new.rowid, new.title, new.body, new.url, new.domain, new.source_app, new.tags_text, new.filename);
END;
