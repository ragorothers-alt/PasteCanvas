use serde::{Deserialize, Serialize};
use tauri::State;

use crate::bootstrap::app_state::AppState;
use crate::domain::value_objects::UnixMs;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub item_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagRequest {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameTagRequest {
    pub tag_id: String,
    pub name: String,
}

/// Creates a new tag.
#[tauri::command]
pub async fn create_tag(
    state: State<'_, AppState>,
    request: CreateTagRequest,
) -> Result<TagDto, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() || name.len() > 100 {
        return Err("tag name must be 1-100 characters".to_string());
    }

    let normalized = name.to_lowercase();
    let id = uuid::Uuid::now_v7().to_string();
    let now = UnixMs::now().as_millis();

    sqlx::query(
        "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&name)
    .bind(&normalized)
    .bind(&request.color)
    .bind(now)
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            format!("tag '{}' already exists", name)
        } else {
            e.to_string()
        }
    })?;

    Ok(TagDto {
        id,
        name,
        color: request.color,
        item_count: 0,
    })
}

/// Lists all tags with item counts.
#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagDto>, String> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64)>(
        "SELECT t.id, t.name, t.color, COUNT(it.item_id) as item_count
         FROM tags t
         LEFT JOIN item_tags it ON it.tag_id = t.id
         WHERE t.deleted_at IS NULL
         GROUP BY t.id
         ORDER BY t.name ASC",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(id, name, color, item_count)| TagDto {
            id,
            name,
            color,
            item_count,
        })
        .collect())
}

/// Renames a tag.
#[tauri::command]
pub async fn rename_tag(
    state: State<'_, AppState>,
    request: RenameTagRequest,
) -> Result<(), String> {
    let name = request.name.trim().to_string();
    if name.is_empty() || name.len() > 100 {
        return Err("tag name must be 1-100 characters".to_string());
    }

    let normalized = name.to_lowercase();
    let now = UnixMs::now().as_millis();

    let result = sqlx::query(
        "UPDATE tags SET name = ?, normalized_name = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&name)
    .bind(&normalized)
    .bind(now)
    .bind(&request.tag_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("tag not found".to_string());
    }

    // Update search documents for items with this tag
    update_tags_text_for_tag(state.db.pool(), &request.tag_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Soft-deletes a tag.
#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, tag_id: String) -> Result<(), String> {
    delete_tag_in_pool(state.db.pool(), &tag_id).await
}

/// Soft-deletes a tag and rebuilds search_documents.tags_text for every
/// previously tagged item, in one transaction.
pub async fn delete_tag_in_pool(pool: &sqlx::SqlitePool, tag_id: &str) -> Result<(), String> {
    let now = UnixMs::now().as_millis();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let item_ids: Vec<(String,)> = sqlx::query_as("SELECT item_id FROM item_tags WHERE tag_id = ?")
        .bind(tag_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM item_tags WHERE tag_id = ?")
        .bind(tag_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("UPDATE tags SET deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(now)
        .bind(tag_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for (item_id,) in item_ids {
        update_tags_text_for_item_tx(&mut tx, &item_id)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Adds a tag to an item.
#[tauri::command]
pub async fn add_tag_to_item(
    state: State<'_, AppState>,
    item_id: String,
    tag_id: String,
) -> Result<(), String> {
    let now = UnixMs::now().as_millis();

    sqlx::query("INSERT OR IGNORE INTO item_tags (item_id, tag_id, created_at) VALUES (?, ?, ?)")
        .bind(&item_id)
        .bind(&tag_id)
        .bind(now)
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;

    // Update search document
    update_tags_text_for_item(state.db.pool(), &item_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Removes a tag from an item.
#[tauri::command]
pub async fn remove_tag_from_item(
    state: State<'_, AppState>,
    item_id: String,
    tag_id: String,
) -> Result<(), String> {
    sqlx::query("DELETE FROM item_tags WHERE item_id = ? AND tag_id = ?")
        .bind(&item_id)
        .bind(&tag_id)
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;

    // Update search document
    update_tags_text_for_item(state.db.pool(), &item_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Gets tags for an item.
#[tauri::command]
pub async fn get_item_tags(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Vec<TagDto>, String> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT t.id, t.name, t.color FROM tags t
         JOIN item_tags it ON it.tag_id = t.id
         WHERE it.item_id = ? AND t.deleted_at IS NULL
         ORDER BY t.name",
    )
    .bind(&item_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(id, name, color)| TagDto {
            id,
            name,
            color,
            item_count: 0,
        })
        .collect())
}

/// Updates the tags_text field in search_documents for a specific item.
async fn update_tags_text_for_item(
    pool: &sqlx::SqlitePool,
    item_id: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    update_tags_text_for_item_tx(&mut tx, item_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn update_tags_text_for_item_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    item_id: &str,
) -> Result<(), sqlx::Error> {
    let tags: Vec<(String,)> = sqlx::query_as(
        "SELECT t.name FROM tags t JOIN item_tags it ON it.tag_id = t.id WHERE it.item_id = ? AND t.deleted_at IS NULL",
    )
    .bind(item_id)
    .fetch_all(&mut **tx)
    .await?;

    let tags_text = tags
        .into_iter()
        .map(|(name,)| name)
        .collect::<Vec<_>>()
        .join(" ");

    let now = UnixMs::now().as_millis();
    sqlx::query("UPDATE search_documents SET tags_text = ?, updated_at = ? WHERE item_id = ?")
        .bind(&tags_text)
        .bind(now)
        .bind(item_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Updates tags_text for all items that have a specific tag (used after rename).
async fn update_tags_text_for_tag(
    pool: &sqlx::SqlitePool,
    tag_id: &str,
) -> Result<(), sqlx::Error> {
    let items: Vec<(String,)> = sqlx::query_as("SELECT item_id FROM item_tags WHERE tag_id = ?")
        .bind(tag_id)
        .fetch_all(pool)
        .await?;

    for (item_id,) in items {
        update_tags_text_for_item(pool, &item_id).await?;
    }

    Ok(())
}
