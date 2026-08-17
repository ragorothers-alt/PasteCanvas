use serde::Deserialize;
use tauri::State;

use crate::application::board_service::{BoardService, BoardSnapshotDto, BoardSummaryDto};
use crate::bootstrap::app_state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBoardRequest {
    pub name: String,
    #[serde(default)]
    pub folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceItemRequest {
    pub board_id: String,
    pub item_id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveNodesRequest {
    pub board_id: String,
    pub moves: Vec<NodeMove>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMove {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveViewportRequest {
    pub board_id: String,
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[tauri::command]
pub async fn create_board(
    state: State<'_, AppState>,
    request: CreateBoardRequest,
) -> Result<BoardSummaryDto, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.create_board(&request.name, request.folder_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_boards(state: State<'_, AppState>) -> Result<Vec<BoardSummaryDto>, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.list_boards().await.map_err(|e| e.to_string())
}

/// BRD-007: most recently used boards for the sidebar "Recent" section.
#[tauri::command]
pub async fn get_recent_boards(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<BoardSummaryDto>, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.get_recent_boards(limit.unwrap_or(5))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_board_snapshot(
    state: State<'_, AppState>,
    board_id: String,
) -> Result<BoardSnapshotDto, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.get_board_snapshot(&board_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn place_item_on_board(
    state: State<'_, AppState>,
    request: PlaceItemRequest,
) -> Result<String, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.place_item_on_board(&request.board_id, &request.item_id, request.x, request.y)
        .await
        .map_err(|e| e.to_string())
}

/// A board node placement for an item (SRC-003 locate-on-board).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBoardNodeDto {
    pub board_id: String,
    pub board_name: String,
    pub node_id: String,
}

/// SRC-003: returns the boards (and node ids) that contain the given item.
#[tauri::command]
pub async fn get_item_board_nodes(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Vec<ItemBoardNodeDto>, String> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT bn.board_id, b.name, bn.id
         FROM board_nodes bn
         JOIN boards b ON b.id = bn.board_id AND b.deleted_at IS NULL
         WHERE bn.item_id = ? AND bn.deleted_at IS NULL",
    )
    .bind(&item_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(board_id, board_name, node_id)| ItemBoardNodeDto {
            board_id,
            board_name,
            node_id,
        })
        .collect())
}

#[tauri::command]
pub async fn move_nodes(
    state: State<'_, AppState>,
    request: MoveNodesRequest,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    let moves: Vec<(String, f64, f64)> = request
        .moves
        .into_iter()
        .map(|m| (m.node_id, m.x, m.y))
        .collect();
    svc.move_nodes(&request.board_id, &moves)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeNodesRequest {
    pub board_id: String,
    pub resizes: Vec<NodeResize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeResize {
    pub node_id: String,
    pub width: f64,
    pub height: f64,
}

/// BRD-010: persists node resize (batch, on resize end).
#[tauri::command]
pub async fn resize_nodes(
    state: State<'_, AppState>,
    request: ResizeNodesRequest,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    let resizes: Vec<(String, f64, f64)> = request
        .resizes
        .into_iter()
        .map(|r| (r.node_id, r.width, r.height))
        .collect();
    svc.resize_nodes(&request.board_id, &resizes)
        .await
        .map_err(|e| e.to_string())
}

/// BRD-010: locks/unlocks a node (locked nodes resist move/resize).
#[tauri::command]
pub async fn set_node_locked(
    state: State<'_, AppState>,
    board_id: String,
    node_id: String,
    locked: bool,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.set_node_locked(&board_id, &node_id, locked)
        .await
        .map_err(|e| e.to_string())
}

/// BRD-010: collapses/expands a group node.
#[tauri::command]
pub async fn set_node_collapsed(
    state: State<'_, AppState>,
    board_id: String,
    node_id: String,
    collapsed: bool,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.set_node_collapsed(&board_id, &node_id, collapsed)
        .await
        .map_err(|e| e.to_string())
}

/// BRD-010: raises a node above all others (z-order).
#[tauri::command]
pub async fn bring_node_to_front(
    state: State<'_, AppState>,
    board_id: String,
    node_id: String,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.bring_node_to_front(&board_id, &node_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_viewport(
    state: State<'_, AppState>,
    request: SaveViewportRequest,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.save_viewport(&request.board_id, request.x, request.y, request.zoom)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_board(
    state: State<'_, AppState>,
    board_id: String,
    name: String,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.rename_board(&board_id, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trash_board(state: State<'_, AppState>, board_id: String) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.trash_board(&board_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn move_board(
    state: State<'_, AppState>,
    board_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.move_board(&board_id, folder_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_node(
    state: State<'_, AppState>,
    board_id: String,
    node_id: String,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.remove_node(&board_id, &node_id)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupNodesRequest {
    pub board_id: String,
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[tauri::command]
pub async fn group_nodes(
    state: State<'_, AppState>,
    request: GroupNodesRequest,
) -> Result<String, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.group_nodes(
        &request.board_id,
        &request.node_ids,
        request.title.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ungroup_nodes(
    state: State<'_, AppState>,
    board_id: String,
    group_id: String,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.ungroup_nodes(&board_id, &group_id)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceBoardLinkRequest {
    pub board_id: String,
    pub target_board_id: String,
    pub x: f64,
    pub y: f64,
}

/// Places a board_link node on a board (nested board entry point).
#[tauri::command]
pub async fn place_board_link(
    state: State<'_, AppState>,
    request: PlaceBoardLinkRequest,
) -> Result<String, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.place_board_link(
        &request.board_id,
        &request.target_board_id,
        request.x,
        request.y,
    )
    .await
    .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEdgeRequest {
    pub board_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    #[serde(default)]
    pub label: Option<String>,
}

/// Creates an edge between two nodes on a board (BRD-005).
#[tauri::command]
pub async fn create_edge(
    state: State<'_, AppState>,
    request: CreateEdgeRequest,
) -> Result<String, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.create_edge(
        &request.board_id,
        &request.source_node_id,
        &request.target_node_id,
        request.label.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Soft-deletes an edge (BRD-005).
#[tauri::command]
pub async fn delete_edge(
    state: State<'_, AppState>,
    board_id: String,
    edge_id: String,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.delete_edge(&board_id, &edge_id)
        .await
        .map_err(|e| e.to_string())
}

// --- BRD-006: board restore/favorite/reorder + folder hierarchy ---

/// Restores a trashed board.
#[tauri::command]
pub async fn restore_board(state: State<'_, AppState>, board_id: String) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.restore_board(&board_id)
        .await
        .map_err(|e| e.to_string())
}

/// Toggles a board's favorite flag. Returns the new state.
#[tauri::command]
pub async fn toggle_board_favorite(
    state: State<'_, AppState>,
    board_id: String,
) -> Result<bool, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.toggle_board_favorite(&board_id)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderRequest {
    pub ordered_ids: Vec<String>,
}

/// Assigns explicit sort order to boards.
#[tauri::command]
pub async fn reorder_boards(
    state: State<'_, AppState>,
    request: ReorderRequest,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.reorder_boards(&request.ordered_ids)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderRequest {
    pub name: String,
    #[serde(default)]
    pub parent_folder_id: Option<String>,
}

/// Lists all live folders (sidebar tree).
#[tauri::command]
pub async fn list_folders(
    state: State<'_, AppState>,
) -> Result<Vec<crate::application::board_service::FolderDto>, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.list_folders().await.map_err(|e| e.to_string())
}

/// Creates a folder, optionally nested under a parent.
#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    request: CreateFolderRequest,
) -> Result<crate::application::board_service::FolderDto, String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.create_folder(&request.name, request.parent_folder_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Renames a folder.
#[tauri::command]
pub async fn rename_folder(
    state: State<'_, AppState>,
    folder_id: String,
    name: String,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.rename_folder(&folder_id, &name)
        .await
        .map_err(|e| e.to_string())
}

/// Moves a folder under a new parent (None = root). Cycle-safe.
#[tauri::command]
pub async fn move_folder(
    state: State<'_, AppState>,
    folder_id: String,
    new_parent_id: Option<String>,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.move_folder(&folder_id, new_parent_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Soft-deletes a folder; contained boards become unfiled, child folders are
/// re-parented to the trashed folder's parent.
#[tauri::command]
pub async fn trash_folder(state: State<'_, AppState>, folder_id: String) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.trash_folder(&folder_id)
        .await
        .map_err(|e| e.to_string())
}

/// Restores a trashed folder.
#[tauri::command]
pub async fn restore_folder(state: State<'_, AppState>, folder_id: String) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.restore_folder(&folder_id)
        .await
        .map_err(|e| e.to_string())
}

/// Assigns explicit sort order to folders.
#[tauri::command]
pub async fn reorder_folders(
    state: State<'_, AppState>,
    request: ReorderRequest,
) -> Result<(), String> {
    let svc = BoardService::new(state.db.clone(), state.device_id.clone());
    svc.reorder_folders(&request.ordered_ids)
        .await
        .map_err(|e| e.to_string())
}
