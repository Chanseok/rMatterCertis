use crate::application::AppState;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize)]
pub struct ProductsToDetailsSyncReport {
    pub total_products: u64,
    pub total_details: u64,
    pub updated_product_ids: u64,
    pub inserted_details: u64,
    pub updated_coordinates: u64,
    pub updated_ids: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicates_neutralized: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_align_skipped_due_to_slot_taken: Option<u64>,
}

/// Synchronize product_details.page_id, .index_in_page, and .id based on products table by URL.
/// - Inserts missing product_details rows for products URLs
/// - Updates coordinates when they differ or are NULL
/// - Regenerates id as p%04di%02d when page_id/index_in_page are set and id is NULL or mismatched
#[tauri::command(async)]
pub async fn sync_product_details_coordinates(
    _app: AppHandle,
    app_state: State<'_, AppState>,
)
-> Result<ProductsToDetailsSyncReport, String> {
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Use a transaction for atomicity across steps
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // Counts for context
    let total_products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
    let total_details: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

        // -1) Neutralize duplicate product_details rows per URL (keep canonical MIN(rowid))
        let neutralize_dups_sql = r#"
                UPDATE product_details
                SET page_id = NULL, index_in_page = NULL, id = NULL
                WHERE rowid NOT IN (SELECT MIN(rowid) FROM product_details GROUP BY url)
                    AND url IN (
                        SELECT url FROM product_details GROUP BY url HAVING COUNT(*) > 1
                    );
        "#;
        let duplicates_neutralized = sqlx::query(neutralize_dups_sql)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?
                .rows_affected();

        // 0) First, regenerate products.id from page_id/index_in_page (canonical form) when needed
    let update_product_ids_sql = r#"
        UPDATE products
        SET id = 'p' || printf('%04d', page_id) || 'i' || printf('%02d', index_in_page)
        WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
          AND (
                id IS NULL OR id = ''
             OR id != 'p' || printf('%04d', page_id) || 'i' || printf('%02d', index_in_page)
          );
    "#;
    let updated_product_ids = sqlx::query(update_product_ids_sql)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();

    // 1) Insert missing details from products (anti-join). Use NULL coords/id to avoid unique conflicts now.
    let insert_sql = r#"
        INSERT INTO product_details (url, page_id, index_in_page, id)
        SELECT p.url,
               NULL,
               NULL,
               NULL
        FROM products p
        LEFT JOIN product_details d ON d.url = p.url
        WHERE d.url IS NULL;
    "#;
    let inserted_details = sqlx::query(insert_sql)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();

    // 2) Update coordinates for canonical detail rows only, avoiding occupied target slots
    let update_coords_sql = r#"
        UPDATE product_details AS d
        SET page_id = (
                SELECT p.page_id FROM products p WHERE p.url = d.url LIMIT 1
            ),
            index_in_page = (
                SELECT p.index_in_page FROM products p WHERE p.url = d.url LIMIT 1
            )
        WHERE d.rowid = (
                SELECT MIN(rowid) FROM product_details AS pdsame WHERE pdsame.url = d.url
            )
          AND EXISTS (SELECT 1 FROM products p WHERE p.url = d.url)
          AND (SELECT p.page_id FROM products p WHERE p.url = d.url) IS NOT NULL
          AND (SELECT p.index_in_page FROM products p WHERE p.url = d.url) IS NOT NULL
          AND (
                d.page_id IS NULL
             OR d.index_in_page IS NULL
             OR d.page_id != (SELECT p2.page_id FROM products p2 WHERE p2.url = d.url LIMIT 1)
             OR d.index_in_page != (SELECT p3.index_in_page FROM products p3 WHERE p3.url = d.url LIMIT 1)
          )
          AND NOT EXISTS (
                SELECT 1 FROM product_details AS pd2
                WHERE pd2.rowid != d.rowid
                  AND pd2.page_id = (SELECT p4.page_id FROM products p4 WHERE p4.url = d.url LIMIT 1)
                  AND pd2.index_in_page = (SELECT p5.index_in_page FROM products p5 WHERE p5.url = d.url LIMIT 1)
          );
    "#;
    let updated_coordinates = sqlx::query(update_coords_sql)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();

    // 2b) Count updates skipped due to target slot being taken (for reporting)
    let skipped_sql = r#"
        SELECT COUNT(*) FROM product_details AS d
        WHERE d.rowid = (
                SELECT MIN(rowid) FROM product_details AS pdsame WHERE pdsame.url = d.url
            )
          AND EXISTS (SELECT 1 FROM products p WHERE p.url = d.url)
          AND (SELECT p.page_id FROM products p WHERE p.url = d.url) IS NOT NULL
          AND (SELECT p.index_in_page FROM products p WHERE p.url = d.url) IS NOT NULL
          AND (
                d.page_id IS NULL
             OR d.index_in_page IS NULL
             OR d.page_id != (SELECT p2.page_id FROM products p2 WHERE p2.url = d.url LIMIT 1)
             OR d.index_in_page != (SELECT p3.index_in_page FROM products p3 WHERE p3.url = d.url LIMIT 1)
          )
          AND EXISTS (
                SELECT 1 FROM product_details AS pd2
                WHERE pd2.rowid != d.rowid
                  AND pd2.page_id = (SELECT p4.page_id FROM products p4 WHERE p4.url = d.url LIMIT 1)
                  AND pd2.index_in_page = (SELECT p5.index_in_page FROM products p5 WHERE p5.url = d.url LIMIT 1)
          );
    "#;
    let details_align_skipped_due_to_slot_taken: i64 = sqlx::query_scalar(skipped_sql)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

    // 3) Mirror id in details from products.id when NULL/mismatched (canonical rows only)
    let update_ids_sql = r#"
        UPDATE product_details AS d
        SET id = (
            SELECT p.id FROM products p WHERE p.url = d.url LIMIT 1
        )
        WHERE d.rowid = (
                SELECT MIN(rowid) FROM product_details AS pdsame WHERE pdsame.url = d.url
            )
          AND EXISTS (SELECT 1 FROM products p WHERE p.url = d.url)
          AND (
                d.id IS NULL OR d.id = ''
             OR d.id != (SELECT p2.id FROM products p2 WHERE p2.url = d.url LIMIT 1)
          );
    "#;
    let updated_ids = sqlx::query(update_ids_sql)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(ProductsToDetailsSyncReport {
        total_products: total_products as u64,
        total_details: total_details as u64,
        updated_product_ids,
        inserted_details,
        updated_coordinates,
        updated_ids,
        duplicates_neutralized: Some(duplicates_neutralized as u64),
        details_align_skipped_due_to_slot_taken: Some(details_align_skipped_due_to_slot_taken as u64),
    })
}
