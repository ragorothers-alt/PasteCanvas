use serde::Deserialize;
use tauri::State;

use crate::application::item_service::{InboxQuery, ItemService, ItemSummaryDto};
use crate::bootstrap::app_state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryInboxRequest {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub status: Option<String>,
}

fn default_limit() -> i64 {
    50
}

/// Queries the Inbox with pagination.
#[tauri::command]
pub async fn query_inbox(
    state: State<'_, AppState>,
    request: QueryInboxRequest,
) -> Result<Vec<ItemSummaryDto>, String> {
    let status_filter = request.status.and_then(|s| match s.as_str() {
        "new" => Some(crate::domain::entities::item::InboxStatus::New),
        "triaged" => Some(crate::domain::entities::item::InboxStatus::Triaged),
        "archived" => Some(crate::domain::entities::item::InboxStatus::Archived),
        _ => None,
    });

    let svc = ItemService::new(state.db.clone());
    let items = svc
        .query_inbox(InboxQuery {
            limit: request.limit.min(200),
            offset: request.offset,
            status_filter,
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(items)
}

/// Toggles favorite status on an item.
#[tauri::command]
pub async fn toggle_favorite(state: State<'_, AppState>, item_id: String) -> Result<bool, String> {
    let svc = ItemService::new(state.db.clone());
    svc.toggle_favorite(&item_id)
        .await
        .map_err(|e| e.to_string())
}

/// Toggles pinned status on an item (INB-003).
#[tauri::command]
pub async fn toggle_pin(state: State<'_, AppState>, item_id: String) -> Result<bool, String> {
    let svc = ItemService::new(state.db.clone());
    svc.toggle_pin(&item_id).await.map_err(|e| e.to_string())
}

/// DAT-003 trash impact stats shown before emptying.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashStatsDto {
    pub item_count: i64,
    pub total_bytes: i64,
}

/// Returns trash impact stats (item count + asset bytes) — DAT-003.
#[tauri::command]
pub async fn get_trash_stats(state: State<'_, AppState>) -> Result<TrashStatsDto, String> {
    let svc = ItemService::new(state.db.clone());
    let (item_count, total_bytes) = svc.trash_stats().await.map_err(|e| e.to_string())?;
    Ok(TrashStatsDto {
        item_count,
        total_bytes,
    })
}

/// Lists soft-deleted items (the trash) — DAT-003.
#[tauri::command]
pub async fn query_trash(
    state: State<'_, AppState>,
) -> Result<Vec<crate::application::item_service::ItemSummaryDto>, String> {
    let svc = ItemService::new(state.db.clone());
    svc.query_trash(200).await.map_err(|e| e.to_string())
}

/// Result of emptying the trash.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyTrashDto {
    pub deleted_items: i64,
    pub deleted_assets: i64,
}

/// Permanently deletes trashed items and orphaned assets (DAT-003).
/// Only assets with no remaining references are physically removed.
#[tauri::command]
pub async fn empty_trash(state: State<'_, AppState>) -> Result<EmptyTrashDto, String> {
    let svc = ItemService::new(state.db.clone());
    let (deleted_items, orphan_paths) = svc.empty_trash().await.map_err(|e| e.to_string())?;

    // Physically remove orphaned asset files (SEC-002: checked resolution —
    // a hostile DB path must never delete files outside the store).
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    let mut deleted_assets = 0i64;
    for rel_path in &orphan_paths {
        let path = match asset_store.resolve_path_checked(rel_path) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(event = "orphan_asset_path_rejected", error = %e);
                continue;
            }
        };
        match std::fs::remove_file(&path) {
            Ok(()) => deleted_assets += 1,
            Err(e) => {
                tracing::warn!(event = "orphan_asset_remove_failed", path = %path.display(), error = %e);
            }
        }
    }

    tracing::info!(
        event = "trash_emptied",
        deleted_items = deleted_items,
        deleted_assets = deleted_assets,
    );

    Ok(EmptyTrashDto {
        deleted_items,
        deleted_assets,
    })
}

/// Soft-deletes an item (moves to trash).
#[tauri::command]
pub async fn trash_item(state: State<'_, AppState>, item_id: String) -> Result<(), String> {
    let svc = ItemService::new(state.db.clone());
    svc.trash_item(&item_id).await.map_err(|e| e.to_string())
}

/// Archives an item (moves out of inbox new/triaged).
#[tauri::command]
pub async fn archive_item(state: State<'_, AppState>, item_id: String) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "UPDATE items SET inbox_status = 'archived', updated_at = ?, revision = revision + 1 WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(&item_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Batch archive multiple items.
#[tauri::command]
pub async fn batch_archive(
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<u64, String> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut affected = 0u64;

    for id in &item_ids {
        let result = sqlx::query(
            "UPDATE items SET inbox_status = 'archived', updated_at = ?, revision = revision + 1 WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(id)
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;
        affected += result.rows_affected();
    }

    Ok(affected)
}

/// Batch delete (soft) multiple items.
#[tauri::command]
pub async fn batch_trash(state: State<'_, AppState>, item_ids: Vec<String>) -> Result<u64, String> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut affected = 0u64;

    for id in &item_ids {
        let result = sqlx::query(
            "UPDATE items SET deleted_at = ?, updated_at = ?, revision = revision + 1 WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;
        affected += result.rows_affected();
    }

    Ok(affected)
}

/// INB-003: reveals an item's managed asset in the OS file explorer.
/// Returns the absolute asset path (or an error if the item has no asset).
#[tauri::command]
pub async fn reveal_item_asset(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<String, String> {
    // Look up the item's primary asset relative path
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT a.relative_path FROM item_assets ia
         JOIN assets a ON a.id = ia.asset_id
         WHERE ia.item_id = ? AND ia.role = 'primary'
         LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let rel = row.ok_or_else(|| "item has no asset".to_string())?;
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    // SEC-002: refuse to reveal anything outside the store root.
    let abs = asset_store
        .resolve_path_checked(&rel.0)
        .map_err(|e| e.to_string())?;
    if !abs.exists() {
        return Err("asset file not found on disk".to_string());
    }

    // Open the containing folder with the file selected (Windows)
    #[cfg(target_os = "windows")]
    {
        let arg = format!("/select,{}", abs.display());
        let _ = std::process::Command::new("explorer.exe").arg(&arg).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = &abs;
    }

    Ok(abs.display().to_string())
}

/// Restores an item from trash (clears deleted_at).
#[tauri::command]
pub async fn restore_item(state: State<'_, AppState>, item_id: String) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    let result = sqlx::query(
        "UPDATE items SET deleted_at = NULL, inbox_status = 'new', updated_at = ?, revision = revision + 1 WHERE id = ? AND deleted_at IS NOT NULL",
    )
    .bind(now)
    .bind(&item_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("item not found in trash".to_string());
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditItemRequest {
    pub item_id: String,
    pub expected_revision: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub inbox_status: Option<String>,
}

/// Edits an item with optimistic concurrency control (revision check).
/// Per 04_DATA_MODEL.md §16: zero affected rows means REVISION_CONFLICT.
#[tauri::command]
pub async fn edit_item(
    state: State<'_, AppState>,
    request: EditItemRequest,
) -> Result<i64, String> {
    let now = chrono::Utc::now().timestamp_millis();

    // Build dynamic update based on provided fields
    let mut sets = vec![
        "updated_at = ?".to_string(),
        "revision = revision + 1".to_string(),
    ];
    if request.title.is_some() {
        sets.push("title = ?".to_string());
    }
    if request.inbox_status.is_some() {
        sets.push("inbox_status = ?".to_string());
    }

    let sql = format!(
        "UPDATE items SET {} WHERE id = ? AND revision = ? AND deleted_at IS NULL",
        sets.join(", ")
    );

    let mut query = sqlx::query(&sql).bind(now);
    if let Some(ref title) = request.title {
        query = query.bind(title);
    }
    if let Some(ref status) = request.inbox_status {
        query = query.bind(status);
    }
    query = query.bind(&request.item_id).bind(request.expected_revision);

    let result = query
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        // Check if item exists to distinguish NOT_FOUND from REVISION_CONFLICT
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM items WHERE id = ? AND deleted_at IS NULL")
                .bind(&request.item_id)
                .fetch_optional(state.db.pool())
                .await
                .map_err(|e| e.to_string())?;

        return match exists {
            Some((current_rev,)) => Err(format!(
                "REVISION_CONFLICT: expected {}, current {}",
                request.expected_revision, current_rev
            )),
            None => Err("NOT_FOUND: item does not exist".to_string()),
        };
    }

    // Return new revision
    let new_rev: (i64,) = sqlx::query_as("SELECT revision FROM items WHERE id = ?")
        .bind(&request.item_id)
        .fetch_one(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;

    Ok(new_rev.0)
}

/// Full content of an item for SRC-003 actions (copy / open source).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemContentDto {
    pub item_id: String,
    pub item_type: String,
    pub plain_text: Option<String>,
    pub display_url: Option<String>,
    pub title: Option<String>,
}

/// Returns full item content (plain_text + URL) for copy/open actions.
#[tauri::command]
pub async fn get_item_content(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<ItemContentDto, String> {
    #[derive(sqlx::FromRow)]
    struct ContentRow {
        item_type: String,
        plain_text: Option<String>,
        display_url: Option<String>,
        title: Option<String>,
    }

    let row: Option<ContentRow> = sqlx::query_as(
        "SELECT item_type, plain_text, display_url, title FROM items WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&item_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    match row {
        Some(r) => Ok(ItemContentDto {
            item_id,
            item_type: r.item_type,
            plain_text: r.plain_text,
            display_url: r.display_url,
            title: r.title,
        }),
        None => Err("NOT_FOUND: item does not exist".to_string()),
    }
}

/// Opens an item's source (URL or file) with the system default handler (SRC-003).
/// Security: only http(s) URLs and existing files are allowed (see platform::windows::shell).
#[tauri::command]
pub async fn open_source(target: String) -> Result<(), String> {
    crate::platform::windows::shell::open_with_shell(&target)
}

/// Asset binary payload returned as base64 (for PDF/image rendering).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDataDto {
    pub mime_type: String,
    pub data_base64: String,
    pub byte_size: i64,
}

/// Maximum asset size returned to the frontend (25 MB) to avoid huge IPC payloads.
const MAX_ASSET_READ_BYTES: i64 = 25 * 1024 * 1024;

/// Returns the primary asset bytes for an item as base64 (PDF/image preview).
#[tauri::command]
pub async fn get_item_asset_base64(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<AssetDataDto, String> {
    // Look up the item's primary asset
    #[derive(sqlx::FromRow)]
    struct AssetRow {
        relative_path: String,
        mime_type: String,
        byte_size: i64,
    }

    let row: Option<AssetRow> = sqlx::query_as(
        "SELECT a.relative_path, a.mime_type, a.byte_size
         FROM item_assets ia
         JOIN assets a ON a.id = ia.asset_id
         WHERE ia.item_id = ? AND ia.role = 'primary'
         LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let asset = row.ok_or_else(|| "NOT_FOUND: item has no asset".to_string())?;

    if asset.byte_size > MAX_ASSET_READ_BYTES {
        return Err("asset too large to preview".to_string());
    }

    // Read the content-addressed asset file (SEC-002: checked resolution —
    // a hostile DB path must never read files outside the store).
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    let path = asset_store
        .resolve_path_checked(&asset.relative_path)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| format!("read asset: {e}"))?;

    use base64::Engine;
    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(AssetDataDto {
        mime_type: asset.mime_type,
        data_base64,
        byte_size: asset.byte_size,
    })
}

/// Maximum source size we are willing to decode for thumbnail generation.
const MAX_THUMB_SOURCE_BYTES: i64 = 50 * 1024 * 1024;
/// Largest fallback (original-as-thumbnail) payload returned to the frontend.
const MAX_THUMB_FALLBACK_BYTES: i64 = 5 * 1024 * 1024;

/// Returns a bounded PNG thumbnail for an image item (P1-CAP-007).
/// Generates and caches the variant on first use under
/// `variants/<sha256>_thumb_<max>.png`; already-small or non-PNG-decodable
/// assets fall back to the original bytes when small, else error.
#[tauri::command]
pub async fn get_item_thumbnail(
    state: State<'_, AppState>,
    item_id: String,
    max_dimension: Option<u32>,
) -> Result<AssetDataDto, String> {
    let max_dim = max_dimension.unwrap_or(320).clamp(16, 1024);

    #[derive(sqlx::FromRow)]
    struct AssetRow {
        sha256: String,
        relative_path: String,
        byte_size: i64,
    }

    let row: Option<AssetRow> = sqlx::query_as(
        "SELECT a.sha256, a.relative_path, a.byte_size
         FROM item_assets ia
         JOIN assets a ON a.id = ia.asset_id
         WHERE ia.item_id = ? AND ia.role = 'primary'
         LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let asset = row.ok_or_else(|| "NOT_FOUND: item has no asset".to_string())?;

    let store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    // Variant path is built from the content hash + bounded int — never from
    // user input — and still goes through the checked resolver.
    let variant_rel = format!("variants/{}_thumb_{}.png", asset.sha256, max_dim);
    let variant_path = store
        .resolve_variant_path_checked(&variant_rel)
        .map_err(|e| e.to_string())?;

    // Cached variant? Serve it directly.
    if variant_path.exists() {
        let bytes = std::fs::read(&variant_path).map_err(|e| format!("read variant: {e}"))?;
        use base64::Engine;
        return Ok(AssetDataDto {
            mime_type: "image/png".to_string(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
            byte_size: bytes.len() as i64,
        });
    }

    if asset.byte_size > MAX_THUMB_SOURCE_BYTES {
        return Err("source too large to thumbnail".to_string());
    }

    let src_path = store
        .resolve_path_checked(&asset.relative_path)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&src_path).map_err(|e| format!("read asset: {e}"))?;

    let thumb = match crate::infrastructure::thumbnail::downscale_png(&bytes, max_dim) {
        Ok(t) => t,
        Err(_) => {
            // Already small (or not PNG-decodable): fall back to the original
            // when it is small enough to serve as its own thumbnail.
            if asset.byte_size <= MAX_THUMB_FALLBACK_BYTES {
                use base64::Engine;
                return Ok(AssetDataDto {
                    mime_type: "image/png".to_string(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                    byte_size: asset.byte_size,
                });
            }
            return Err("thumbnail not available for this asset".to_string());
        }
    };

    // Cache the variant (best effort; the store dir may not exist yet).
    if let Some(parent) = variant_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&variant_path, &thumb);

    use base64::Engine;
    Ok(AssetDataDto {
        mime_type: "image/png".to_string(),
        data_base64: base64::engine::general_purpose::STANDARD.encode(&thumb),
        byte_size: thumb.len() as i64,
    })
}
