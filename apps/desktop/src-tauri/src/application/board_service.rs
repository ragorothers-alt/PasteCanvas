use crate::domain::value_objects::{EntityId, UnixMs};
use crate::infrastructure::database::pool::AppDatabase;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoardServiceError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("board not found: {0}")]
    NotFound(String),

    #[error("name too long (max 255)")]
    NameTooLong,

    #[error("name cannot be empty")]
    NameEmpty,

    #[error("edge source and target cannot be the same node")]
    SelfLoop,

    #[error("folder not found: {0}")]
    FolderNotFound(String),

    #[error("move would create a folder cycle")]
    FolderCycle,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_key: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSummaryDto {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub is_favorite: bool,
    pub sort_key: String,
    pub node_count: i64,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub viewport_zoom: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardNodeDto {
    pub id: String,
    pub board_id: String,
    pub node_kind: String,
    pub item_id: Option<String>,
    pub target_board_id: Option<String>,
    pub parent_node_id: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z_index: i64,
    pub locked: bool,
    pub collapsed: bool,
    pub title_override: Option<String>,
    // Joined item data for display
    pub item_title: Option<String>,
    pub item_type: Option<String>,
    pub item_text_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardEdgeDto {
    pub id: String,
    pub board_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub edge_type: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSnapshotDto {
    pub board: BoardSummaryDto,
    pub nodes: Vec<BoardNodeDto>,
    pub edges: Vec<BoardEdgeDto>,
}

pub struct BoardService {
    db: AppDatabase,
    device_id: EntityId,
}

impl BoardService {
    pub fn new(db: AppDatabase, device_id: EntityId) -> Self {
        Self { db, device_id }
    }

    /// Creates a new board.
    pub async fn create_board(
        &self,
        name: &str,
        folder_id: Option<&str>,
    ) -> Result<BoardSummaryDto, BoardServiceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BoardServiceError::NameEmpty);
        }
        if name.len() > 255 {
            return Err(BoardServiceError::NameTooLong);
        }

        let id = EntityId::new();
        let now = UnixMs::now();
        let sort_key = format!("{}", now.as_millis());

        sqlx::query(
            "INSERT INTO boards (id, folder_id, name, sort_key, viewport_x, viewport_y, viewport_zoom,
              board_revision, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, ?, ?, 0, 0, 1, 1, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(id.as_str())
        .bind(folder_id)
        .bind(name)
        .bind(&sort_key)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(self.db.pool())
        .await?;

        Ok(BoardSummaryDto {
            id: id.to_string(),
            folder_id: folder_id.map(String::from),
            name: name.to_string(),
            description: None,
            is_favorite: false,
            sort_key,
            node_count: 0,
            viewport_x: 0.0,
            viewport_y: 0.0,
            viewport_zoom: 1.0,
            created_at: now.as_millis(),
            updated_at: now.as_millis(),
        })
    }

    /// Lists all boards (non-deleted), ordered by sort_key.
    pub async fn list_boards(&self) -> Result<Vec<BoardSummaryDto>, BoardServiceError> {
        self.query_board_summaries("ORDER BY b.sort_key ASC", None)
            .await
    }

    /// BRD-007: most recently used boards (sidebar "Recent" section).
    /// Opening a board touches `updated_at` (see get_board_snapshot).
    pub async fn get_recent_boards(
        &self,
        limit: i64,
    ) -> Result<Vec<BoardSummaryDto>, BoardServiceError> {
        let limit = limit.clamp(1, 20);
        self.query_board_summaries("ORDER BY b.updated_at DESC", Some(limit))
            .await
    }

    /// Shared board-summary query. `order_by` is always a compile-time constant.
    async fn query_board_summaries(
        &self,
        order_by: &str,
        limit: Option<i64>,
    ) -> Result<Vec<BoardSummaryDto>, BoardServiceError> {
        let sql = format!(
            "SELECT b.id, b.folder_id, b.name, b.description, b.is_favorite, b.sort_key,
                    b.viewport_x, b.viewport_y, b.viewport_zoom, b.created_at, b.updated_at
             FROM boards b
             WHERE b.deleted_at IS NULL
             {order_by}{}",
            if limit.is_some() { " LIMIT ?" } else { "" }
        );
        let mut query = sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                String,
                Option<String>,
                i32,
                String,
                f64,
                f64,
                f64,
                i64,
                i64,
            ),
        >(&sql);
        if let Some(l) = limit {
            query = query.bind(l);
        }
        let rows = query.fetch_all(self.db.pool()).await?;

        let mut boards = Vec::new();
        for (id, folder_id, name, description, is_fav, sort_key, vx, vy, vz, created, updated) in
            rows
        {
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
            )
            .bind(&id)
            .fetch_one(self.db.pool())
            .await?;

            boards.push(BoardSummaryDto {
                id,
                folder_id,
                name,
                description,
                is_favorite: is_fav == 1,
                sort_key,
                node_count: count.0,
                viewport_x: vx,
                viewport_y: vy,
                viewport_zoom: vz,
                created_at: created,
                updated_at: updated,
            });
        }

        Ok(boards)
    }

    /// Gets a full board snapshot (board + all nodes with item data).
    pub async fn get_board_snapshot(
        &self,
        board_id: &str,
    ) -> Result<BoardSnapshotDto, BoardServiceError> {
        let board_row = sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                String,
                Option<String>,
                i32,
                String,
                f64,
                f64,
                f64,
                i64,
                i64,
            ),
        >(
            "SELECT id, folder_id, name, description, is_favorite, sort_key,
                    viewport_x, viewport_y, viewport_zoom, created_at, updated_at
             FROM boards WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(board_id)
        .fetch_optional(self.db.pool())
        .await?;

        let (id, folder_id, name, description, is_fav, sort_key, vx, vy, vz, created, _) =
            board_row.ok_or_else(|| BoardServiceError::NotFound(board_id.to_string()))?;

        // BRD-007: opening a board counts as "use" for the Recent list.
        // Activity tracking only — content revision is not bumped.
        let touch = UnixMs::now();
        sqlx::query("UPDATE boards SET updated_at = ? WHERE id = ?")
            .bind(touch.as_millis())
            .bind(board_id)
            .execute(self.db.pool())
            .await?;
        let updated = touch.as_millis();

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
        )
        .bind(board_id)
        .fetch_one(self.db.pool())
        .await?;

        let board = BoardSummaryDto {
            id: id.clone(),
            folder_id,
            name,
            description,
            is_favorite: is_fav == 1,
            sort_key,
            node_count: count.0,
            viewport_x: vx,
            viewport_y: vy,
            viewport_zoom: vz,
            created_at: created,
            updated_at: updated,
        };

        // Fetch nodes with joined item data
        #[derive(sqlx::FromRow)]
        struct NodeRow {
            id: String,
            board_id: String,
            node_kind: String,
            item_id: Option<String>,
            target_board_id: Option<String>,
            parent_node_id: Option<String>,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            z_index: i64,
            locked: i32,
            collapsed: i32,
            title_override: Option<String>,
            item_title: Option<String>,
            item_type: Option<String>,
            plain_text: Option<String>,
        }

        let node_rows = sqlx::query_as::<_, NodeRow>(
            "SELECT n.id, n.board_id, n.node_kind, n.item_id, n.target_board_id, n.parent_node_id,
                    n.x, n.y, n.width, n.height, n.z_index, n.locked, n.collapsed, n.title_override,
                    i.title as item_title, i.item_type, i.plain_text
             FROM board_nodes n
             LEFT JOIN items i ON i.id = n.item_id
             WHERE n.board_id = ? AND n.deleted_at IS NULL
             ORDER BY n.z_index ASC, n.id ASC",
        )
        .bind(board_id)
        .fetch_all(self.db.pool())
        .await?;

        let nodes = node_rows
            .into_iter()
            .map(|r| {
                let item_text_preview = r.plain_text.map(|t| t.chars().take(200).collect());
                BoardNodeDto {
                    id: r.id,
                    board_id: r.board_id,
                    node_kind: r.node_kind,
                    item_id: r.item_id,
                    target_board_id: r.target_board_id,
                    parent_node_id: r.parent_node_id,
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                    z_index: r.z_index,
                    locked: r.locked == 1,
                    collapsed: r.collapsed == 1,
                    title_override: r.title_override,
                    item_title: r.item_title,
                    item_type: r.item_type,
                    item_text_preview,
                }
            })
            .collect();

        // Fetch edges (Board-local, per BRD-005)
        #[derive(sqlx::FromRow)]
        struct EdgeRow {
            id: String,
            board_id: String,
            source_node_id: String,
            target_node_id: String,
            edge_type: String,
            label: Option<String>,
        }

        let edge_rows = sqlx::query_as::<_, EdgeRow>(
            "SELECT id, board_id, source_node_id, target_node_id, edge_type, label
             FROM board_edges WHERE board_id = ? AND deleted_at IS NULL
             ORDER BY created_at ASC, id ASC",
        )
        .bind(board_id)
        .fetch_all(self.db.pool())
        .await?;

        let edges = edge_rows
            .into_iter()
            .map(|r| BoardEdgeDto {
                id: r.id,
                board_id: r.board_id,
                source_node_id: r.source_node_id,
                target_node_id: r.target_node_id,
                edge_type: r.edge_type,
                label: r.label,
            })
            .collect();

        Ok(BoardSnapshotDto {
            board,
            nodes,
            edges,
        })
    }

    /// Places an item on a board at a given position.
    pub async fn place_item_on_board(
        &self,
        board_id: &str,
        item_id: &str,
        x: f64,
        y: f64,
    ) -> Result<String, BoardServiceError> {
        let node_id = EntityId::new();
        let now = UnixMs::now();

        // Get next z_index
        let max_z: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(z_index), 0) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
        )
        .bind(board_id)
        .fetch_one(self.db.pool())
        .await?;

        sqlx::query(
            "INSERT INTO board_nodes (id, board_id, node_kind, item_id, x, y, width, height, z_index,
              created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, 'item', ?, ?, ?, 280, 180, ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(node_id.as_str())
        .bind(board_id)
        .bind(item_id)
        .bind(x)
        .bind(y)
        .bind(max_z.0 + 1)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(self.db.pool())
        .await?;

        // INB-003 (PRD): the FIRST placement onto any Board transitions a
        // 'new' item to 'triaged'. Later placements are no-ops thanks to the
        // inbox_status = 'new' guard.
        sqlx::query(
            "UPDATE items SET inbox_status = 'triaged', updated_at = ?, revision = revision + 1
             WHERE id = ? AND inbox_status = 'new' AND deleted_at IS NULL",
        )
        .bind(now.as_millis())
        .bind(item_id)
        .execute(self.db.pool())
        .await?;

        // Bump board revision
        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(self.db.pool())
        .await?;

        Ok(node_id.to_string())
    }

    /// Moves nodes to new positions (batch, on drag stop).
    pub async fn move_nodes(
        &self,
        board_id: &str,
        moves: &[(String, f64, f64)],
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        let mut tx = self.db.pool().begin().await?;

        for (node_id, x, y) in moves {
            sqlx::query(
                "UPDATE board_nodes SET x = ?, y = ?, updated_at = ?, revision = revision + 1
                 WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
            )
            .bind(x)
            .bind(y)
            .bind(now.as_millis())
            .bind(node_id)
            .bind(board_id)
            .execute(&mut *tx)
            .await?;
        }

        // Bump board revision
        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// BRD-010: resizes nodes (persisted in batch on drag/resize end).
    /// Enforces the schema minimums (width/height > 0) with a usable floor.
    pub async fn resize_nodes(
        &self,
        board_id: &str,
        resizes: &[(String, f64, f64)],
    ) -> Result<(), BoardServiceError> {
        const MIN_DIM: f64 = 40.0;
        let now = UnixMs::now();
        let mut tx = self.db.pool().begin().await?;

        for (node_id, width, height) in resizes {
            if !width.is_finite() || !height.is_finite() {
                return Err(BoardServiceError::NotFound("invalid resize".to_string()));
            }
            let w = width.max(MIN_DIM);
            let h = height.max(MIN_DIM);
            sqlx::query(
                "UPDATE board_nodes SET width = ?, height = ?, updated_at = ?, revision = revision + 1
                 WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
            )
            .bind(w)
            .bind(h)
            .bind(now.as_millis())
            .bind(node_id)
            .bind(board_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// BRD-010: toggles the locked flag on a node (locked nodes cannot be
    /// moved/resized by the canvas).
    pub async fn set_node_locked(
        &self,
        board_id: &str,
        node_id: &str,
        locked: bool,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE board_nodes SET locked = ?, updated_at = ?, revision = revision + 1
             WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
        )
        .bind(locked)
        .bind(now.as_millis())
        .bind(node_id)
        .bind(board_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::NotFound(node_id.to_string()));
        }
        Ok(())
    }

    /// BRD-010: toggles group collapse (visual; children stay in place).
    pub async fn set_node_collapsed(
        &self,
        board_id: &str,
        node_id: &str,
        collapsed: bool,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE board_nodes SET collapsed = ?, updated_at = ?, revision = revision + 1
             WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
        )
        .bind(collapsed)
        .bind(now.as_millis())
        .bind(node_id)
        .bind(board_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::NotFound(node_id.to_string()));
        }
        Ok(())
    }

    /// BRD-010: raises a node above all others on its board (z-order).
    pub async fn bring_node_to_front(
        &self,
        board_id: &str,
        node_id: &str,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        let mut tx = self.db.pool().begin().await?;

        let max_z: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(z_index), 0) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
        )
        .bind(board_id)
        .fetch_one(&mut *tx)
        .await?;

        let result = sqlx::query(
            "UPDATE board_nodes SET z_index = ?, updated_at = ?, revision = revision + 1
             WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
        )
        .bind(max_z.0 + 1)
        .bind(now.as_millis())
        .bind(node_id)
        .bind(board_id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::NotFound(node_id.to_string()));
        }

        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Saves viewport state.
    pub async fn save_viewport(
        &self,
        board_id: &str,
        x: f64,
        y: f64,
        zoom: f64,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        sqlx::query(
            "UPDATE boards SET viewport_x = ?, viewport_y = ?, viewport_zoom = ?, updated_at = ? WHERE id = ?",
        )
        .bind(x)
        .bind(y)
        .bind(zoom)
        .bind(now.as_millis())
        .bind(board_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Renames a board.
    pub async fn rename_board(&self, board_id: &str, name: &str) -> Result<(), BoardServiceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BoardServiceError::NameEmpty);
        }
        if name.len() > 255 {
            return Err(BoardServiceError::NameTooLong);
        }
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE boards SET name = ?, updated_at = ?, revision = revision + 1 WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(name)
        .bind(now.as_millis())
        .bind(board_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(BoardServiceError::NotFound(board_id.to_string()));
        }
        Ok(())
    }

    /// Soft-deletes a board.
    pub async fn trash_board(&self, board_id: &str) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        sqlx::query("UPDATE boards SET deleted_at = ?, updated_at = ? WHERE id = ?")
            .bind(now.as_millis())
            .bind(now.as_millis())
            .bind(board_id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Restores a trashed board (BRD-006).
    pub async fn restore_board(&self, board_id: &str) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE boards SET deleted_at = NULL, updated_at = ?, revision = revision + 1
             WHERE id = ? AND deleted_at IS NOT NULL",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::NotFound(board_id.to_string()));
        }
        Ok(())
    }

    /// Toggles a board's favorite flag (BRD-006). Returns the new state.
    pub async fn toggle_board_favorite(&self, board_id: &str) -> Result<bool, BoardServiceError> {
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE boards SET is_favorite = 1 - is_favorite, updated_at = ?, revision = revision + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::NotFound(board_id.to_string()));
        }
        let (is_fav,): (i32,) = sqlx::query_as("SELECT is_favorite FROM boards WHERE id = ?")
            .bind(board_id)
            .fetch_one(self.db.pool())
            .await?;
        Ok(is_fav == 1)
    }

    /// Assigns explicit sort order to boards (BRD-006 reorder).
    /// IDs not in the list keep their existing sort_key.
    pub async fn reorder_boards(&self, ordered_ids: &[String]) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        for (idx, id) in ordered_ids.iter().enumerate() {
            let key = format!("{:020}", idx);
            sqlx::query("UPDATE boards SET sort_key = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&key)
                .bind(now.as_millis())
                .bind(id)
                .execute(self.db.pool())
                .await?;
        }
        Ok(())
    }

    // --- BRD-006: Folder hierarchy management ---

    fn validate_name(name: &str) -> Result<&str, BoardServiceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BoardServiceError::NameEmpty);
        }
        if name.len() > 255 {
            return Err(BoardServiceError::NameTooLong);
        }
        Ok(name)
    }

    /// Lists all live folders ordered by sort_key.
    pub async fn list_folders(&self) -> Result<Vec<FolderDto>, BoardServiceError> {
        let rows: Vec<(String, Option<String>, String, String, i64, i64)> = sqlx::query_as(
            "SELECT id, parent_folder_id, name, sort_key, created_at, updated_at
             FROM folders WHERE deleted_at IS NULL ORDER BY sort_key ASC",
        )
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, parent_folder_id, name, sort_key, created_at, updated_at)| FolderDto {
                    id,
                    parent_folder_id,
                    name,
                    sort_key,
                    created_at,
                    updated_at,
                },
            )
            .collect())
    }

    /// Creates a folder, optionally nested under a live parent folder.
    pub async fn create_folder(
        &self,
        name: &str,
        parent_folder_id: Option<&str>,
    ) -> Result<FolderDto, BoardServiceError> {
        let name = Self::validate_name(name)?;
        if let Some(parent) = parent_folder_id {
            self.require_live_folder(parent).await?;
        }

        let id = EntityId::new();
        let now = UnixMs::now();
        let sort_key = format!("{}", now.as_millis());

        sqlx::query(
            "INSERT INTO folders (id, parent_folder_id, name, sort_key, created_at, updated_at,
              revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(id.as_str())
        .bind(parent_folder_id)
        .bind(name)
        .bind(&sort_key)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(self.db.pool())
        .await?;

        Ok(FolderDto {
            id: id.to_string(),
            parent_folder_id: parent_folder_id.map(String::from),
            name: name.to_string(),
            sort_key,
            created_at: now.as_millis(),
            updated_at: now.as_millis(),
        })
    }

    /// Renames a folder.
    pub async fn rename_folder(
        &self,
        folder_id: &str,
        name: &str,
    ) -> Result<(), BoardServiceError> {
        let name = Self::validate_name(name)?;
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE folders SET name = ?, updated_at = ?, revision = revision + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(name)
        .bind(now.as_millis())
        .bind(folder_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::FolderNotFound(folder_id.to_string()));
        }
        Ok(())
    }

    /// Moves a folder under a new parent (or to the root when None).
    /// Cycle-safe: rejected when the new parent is the folder itself or one of
    /// its descendants.
    pub async fn move_folder(
        &self,
        folder_id: &str,
        new_parent_id: Option<&str>,
    ) -> Result<(), BoardServiceError> {
        self.require_live_folder(folder_id).await?;

        if let Some(parent) = new_parent_id {
            if parent == folder_id {
                return Err(BoardServiceError::FolderCycle);
            }
            self.require_live_folder(parent).await?;
            // Walk up from the proposed parent; hitting folder_id means a cycle.
            let mut cursor: Option<String> = Some(parent.to_string());
            while let Some(cur) = cursor {
                if cur == folder_id {
                    return Err(BoardServiceError::FolderCycle);
                }
                let row: Option<(Option<String>,)> =
                    sqlx::query_as("SELECT parent_folder_id FROM folders WHERE id = ?")
                        .bind(&cur)
                        .fetch_optional(self.db.pool())
                        .await?;
                cursor = row.and_then(|(p,)| p);
            }
        }

        let now = UnixMs::now();
        sqlx::query(
            "UPDATE folders SET parent_folder_id = ?, updated_at = ?, revision = revision + 1 WHERE id = ?",
        )
        .bind(new_parent_id)
        .bind(now.as_millis())
        .bind(folder_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Soft-deletes a folder (BRD-006). Explicit child behavior: contained
    /// boards are preserved and become unfiled (folder_id → NULL), matching the
    /// schema's ON DELETE SET NULL semantics. Child folders are re-parented to
    /// the trashed folder's parent.
    pub async fn trash_folder(&self, folder_id: &str) -> Result<(), BoardServiceError> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT parent_folder_id FROM folders WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(folder_id)
        .fetch_optional(self.db.pool())
        .await?;
        let (parent,) =
            row.ok_or_else(|| BoardServiceError::FolderNotFound(folder_id.to_string()))?;

        let now = UnixMs::now();
        let mut tx = self.db.pool().begin().await?;

        sqlx::query("UPDATE folders SET deleted_at = ?, updated_at = ? WHERE id = ?")
            .bind(now.as_millis())
            .bind(now.as_millis())
            .bind(folder_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE boards SET folder_id = NULL, updated_at = ? WHERE folder_id = ?")
            .bind(now.as_millis())
            .bind(folder_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE folders SET parent_folder_id = ?, updated_at = ? WHERE parent_folder_id = ?",
        )
        .bind(parent.as_deref())
        .bind(now.as_millis())
        .bind(folder_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Restores a trashed folder (boards stay wherever trash_folder left them).
    pub async fn restore_folder(&self, folder_id: &str) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE folders SET deleted_at = NULL, updated_at = ?, revision = revision + 1
             WHERE id = ? AND deleted_at IS NOT NULL",
        )
        .bind(now.as_millis())
        .bind(folder_id)
        .execute(self.db.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(BoardServiceError::FolderNotFound(folder_id.to_string()));
        }
        Ok(())
    }

    /// Assigns explicit sort order to folders (BRD-006 reorder).
    pub async fn reorder_folders(&self, ordered_ids: &[String]) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        for (idx, id) in ordered_ids.iter().enumerate() {
            let key = format!("{:020}", idx);
            sqlx::query("UPDATE folders SET sort_key = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&key)
                .bind(now.as_millis())
                .bind(id)
                .execute(self.db.pool())
                .await?;
        }
        Ok(())
    }

    async fn require_live_folder(&self, folder_id: &str) -> Result<(), BoardServiceError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT id FROM folders WHERE id = ? AND deleted_at IS NULL")
                .bind(folder_id)
                .fetch_optional(self.db.pool())
                .await?;
        if row.is_none() {
            return Err(BoardServiceError::FolderNotFound(folder_id.to_string()));
        }
        Ok(())
    }

    /// Removes a node placement (does NOT delete the item).
    pub async fn remove_node(
        &self,
        board_id: &str,
        node_id: &str,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        sqlx::query(
            "UPDATE board_nodes SET deleted_at = ?, updated_at = ? WHERE id = ? AND board_id = ?",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(node_id)
        .bind(board_id)
        .execute(self.db.pool())
        .await?;

        // Per BRD-005: cascade soft-delete edges connected to the removed node.
        sqlx::query(
            "UPDATE board_edges SET deleted_at = ?, updated_at = ?
             WHERE board_id = ? AND deleted_at IS NULL
               AND (source_node_id = ? OR target_node_id = ?)",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(board_id)
        .bind(node_id)
        .bind(node_id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Creates an edge between two nodes on a board (BRD-005).
    /// Edges are Board-local. Self-loops are rejected.
    pub async fn create_edge(
        &self,
        board_id: &str,
        source_node_id: &str,
        target_node_id: &str,
        label: Option<&str>,
    ) -> Result<String, BoardServiceError> {
        if source_node_id == target_node_id {
            return Err(BoardServiceError::SelfLoop);
        }

        let edge_id = EntityId::new();
        let now = UnixMs::now();

        sqlx::query(
            "INSERT INTO board_edges (id, board_id, source_node_id, target_node_id, edge_type, label,
              created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, ?, ?, 'default', ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(edge_id.as_str())
        .bind(board_id)
        .bind(source_node_id)
        .bind(target_node_id)
        .bind(label)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(self.db.pool())
        .await?;

        Ok(edge_id.to_string())
    }

    /// Soft-deletes an edge (BRD-005 cascade: deleting a node removes its edges).
    pub async fn delete_edge(
        &self,
        board_id: &str,
        edge_id: &str,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();
        sqlx::query(
            "UPDATE board_edges SET deleted_at = ?, updated_at = ? WHERE id = ? AND board_id = ?",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(edge_id)
        .bind(board_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Groups selected nodes under a new group node.
    /// Converts child absolute coordinates to relative (child - group_origin).
    pub async fn group_nodes(
        &self,
        board_id: &str,
        node_ids: &[String],
        title: Option<&str>,
    ) -> Result<String, BoardServiceError> {
        if node_ids.is_empty() {
            return Err(BoardServiceError::NotFound("no nodes to group".into()));
        }

        let now = UnixMs::now();
        let group_id = EntityId::new();

        // Compute group bounds from selected nodes
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for nid in node_ids {
            let row: Option<(f64, f64, f64, f64)> = sqlx::query_as(
                "SELECT x, y, width, height FROM board_nodes WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
            )
            .bind(nid)
            .bind(board_id)
            .fetch_optional(self.db.pool())
            .await?;

            if let Some((x, y, w, h)) = row {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + w);
                max_y = max_y.max(y + h);
            }
        }

        if min_x == f64::MAX {
            return Err(BoardServiceError::NotFound("no valid nodes found".into()));
        }

        let group_w = max_x - min_x;
        let group_h = max_y - min_y;

        let mut tx = self.db.pool().begin().await?;

        let max_z: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(z_index), 0) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
        )
        .bind(board_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO board_nodes (id, board_id, node_kind, x, y, width, height, z_index,
              title_override, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, 'group', ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(group_id.as_str())
        .bind(board_id)
        .bind(min_x)
        .bind(min_y)
        .bind(group_w)
        .bind(group_h)
        .bind(max_z.0 + 1)
        .bind(title)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(&mut *tx)
        .await?;

        // Convert children to relative coordinates
        for nid in node_ids {
            sqlx::query(
                "UPDATE board_nodes SET x = x - ?, y = y - ?, parent_node_id = ?, updated_at = ?, revision = revision + 1
                 WHERE id = ? AND board_id = ? AND deleted_at IS NULL",
            )
            .bind(min_x)
            .bind(min_y)
            .bind(group_id.as_str())
            .bind(now.as_millis())
            .bind(nid)
            .bind(board_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(group_id.to_string())
    }

    /// Ungroups: converts children back to absolute coordinates, deletes group.
    pub async fn ungroup_nodes(
        &self,
        board_id: &str,
        group_id: &str,
    ) -> Result<(), BoardServiceError> {
        let now = UnixMs::now();

        let group_pos: Option<(f64, f64)> = sqlx::query_as(
            "SELECT x, y FROM board_nodes WHERE id = ? AND board_id = ? AND node_kind = 'group' AND deleted_at IS NULL",
        )
        .bind(group_id)
        .bind(board_id)
        .fetch_optional(self.db.pool())
        .await?;

        let (gx, gy) =
            group_pos.ok_or_else(|| BoardServiceError::NotFound(group_id.to_string()))?;

        let mut tx = self.db.pool().begin().await?;

        sqlx::query(
            "UPDATE board_nodes SET x = x + ?, y = y + ?, parent_node_id = NULL, updated_at = ?, revision = revision + 1
             WHERE parent_node_id = ? AND board_id = ? AND deleted_at IS NULL",
        )
        .bind(gx)
        .bind(gy)
        .bind(now.as_millis())
        .bind(group_id)
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE board_nodes SET deleted_at = ?, updated_at = ? WHERE id = ? AND board_id = ?",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(group_id)
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Places a board_link node (nested board entry point).
    pub async fn place_board_link(
        &self,
        board_id: &str,
        target_board_id: &str,
        x: f64,
        y: f64,
    ) -> Result<String, BoardServiceError> {
        // Prevent self-reference
        if board_id == target_board_id {
            return Err(BoardServiceError::NotFound(
                "cannot link board to itself".into(),
            ));
        }

        let node_id = EntityId::new();
        let now = UnixMs::now();

        let max_z: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(z_index), 0) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
        )
        .bind(board_id)
        .fetch_one(self.db.pool())
        .await?;

        // Get target board name for title_override
        let target_name: Option<(String,)> =
            sqlx::query_as("SELECT name FROM boards WHERE id = ? AND deleted_at IS NULL")
                .bind(target_board_id)
                .fetch_optional(self.db.pool())
                .await?;

        let title = target_name.map(|(n,)| n);

        sqlx::query(
            "INSERT INTO board_nodes (id, board_id, node_kind, target_board_id, x, y, width, height, z_index,
              title_override, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, 'board_link', ?, ?, ?, 200, 120, ?, ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(node_id.as_str())
        .bind(board_id)
        .bind(target_board_id)
        .bind(x)
        .bind(y)
        .bind(max_z.0 + 1)
        .bind(&title)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            "UPDATE boards SET board_revision = board_revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(board_id)
        .execute(self.db.pool())
        .await?;

        Ok(node_id.to_string())
    }
}
