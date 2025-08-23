use crate::application::AppState;
use serde::Serialize;
use sqlx::sqlite::SqliteQueryResult;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize)]
pub struct UrlDedupCleanupReport {
    // URL-based dedup results
    pub products_removed: u64,
    pub product_details_removed: u64,
    pub remaining_duplicates_products: u64,
    pub remaining_duplicates_product_details: u64,
    // Slot-based dedup results (page_id, index_in_page)
    pub slot_products_removed: u64,
    pub slot_product_details_removed: u64,
    pub remaining_slot_duplicates_products: u64,
    pub remaining_slot_duplicates_product_details: u64,
}

async fn delete_dupes_in_table(pool: &sqlx::SqlitePool, table: &str) -> Result<u64, String> {
    // Delete rows whose url duplicates exist, keeping the lowest rowid per url
    let sql = format!(
        r#"
        WITH dupes AS (
            SELECT url, MIN(rowid) AS keep_rowid
            FROM {table}
            WHERE url IS NOT NULL
            GROUP BY url
            HAVING COUNT(*) > 1
        )
        DELETE FROM {table}
        WHERE url IN (SELECT url FROM dupes)
          AND rowid NOT IN (SELECT keep_rowid FROM dupes);
        "#,
        table = table
    );

    let res: SqliteQueryResult = sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.rows_affected())
}

async fn count_remaining_dupes(pool: &sqlx::SqlitePool, table: &str) -> Result<u64, String> {
    let sql = format!(
        r#"
        SELECT COALESCE(SUM(cnt - 1), 0) AS remain
        FROM (
            SELECT url, COUNT(*) AS cnt
            FROM {table}
            WHERE url IS NOT NULL
            GROUP BY url
            HAVING COUNT(*) > 1
        ) t;
        "#,
        table = table
    );
    let remain: i64 = sqlx::query_scalar(&sql)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
        .unwrap_or(0);
    Ok(remain as u64)
}

async fn delete_slot_dupes_in_table(pool: &sqlx::SqlitePool, table: &str) -> Result<u64, String> {
    // Delete rows that collide on (page_id, index_in_page), keep the lowest rowid per slot
    let sql = format!(
        r#"
        WITH kept AS (
            SELECT MIN(rowid) AS keep_rowid
            FROM {table}
            WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
            GROUP BY page_id, index_in_page
        )
        DELETE FROM {table}
        WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
          AND rowid NOT IN (SELECT keep_rowid FROM kept);
        "#,
        table = table
    );

    let res: SqliteQueryResult = sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.rows_affected())
}

async fn count_remaining_slot_dupes(pool: &sqlx::SqlitePool, table: &str) -> Result<u64, String> {
    let sql = format!(
        r#"
        SELECT COALESCE(SUM(cnt - 1), 0) AS remain
        FROM (
            SELECT page_id, index_in_page, COUNT(*) AS cnt
            FROM {table}
            WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
            GROUP BY page_id, index_in_page
            HAVING COUNT(*) > 1
        ) t;
        "#,
        table = table
    );
    let remain: i64 = sqlx::query_scalar(&sql)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
        .unwrap_or(0);
    Ok(remain as u64)
}

/// Remove duplicate rows by exact URL for both products and product_details.
/// Keeps the first inserted row (by rowid) and deletes the rest.
#[tauri::command(async)]
pub async fn cleanup_duplicate_urls(
    _app: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<UrlDedupCleanupReport, String> {
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Pass 1: URL-based dedup
    let products_removed = delete_dupes_in_table(&pool, "products").await?;
    let product_details_removed = delete_dupes_in_table(&pool, "product_details").await?;

    // Pass 2: Slot-based dedup (page_id, index_in_page)
    let slot_products_removed = delete_slot_dupes_in_table(&pool, "products").await?;
    let slot_product_details_removed = delete_slot_dupes_in_table(&pool, "product_details").await?;

    // Remaining duplicate counts (post-commit)
    let remaining_duplicates_products = count_remaining_dupes(&pool, "products").await?;
    let remaining_duplicates_product_details =
        count_remaining_dupes(&pool, "product_details").await?;

    let remaining_slot_duplicates_products = count_remaining_slot_dupes(&pool, "products").await?;
    let remaining_slot_duplicates_product_details =
        count_remaining_slot_dupes(&pool, "product_details").await?;

    Ok(UrlDedupCleanupReport {
        products_removed,
        product_details_removed,
        remaining_duplicates_products,
        remaining_duplicates_product_details,
        slot_products_removed,
        slot_product_details_removed,
        remaining_slot_duplicates_products,
        remaining_slot_duplicates_product_details,
    })
}
