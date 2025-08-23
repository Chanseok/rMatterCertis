use crate::application::AppState;
use crate::application::shared_state::SharedStateCache;
// (no additional infrastructure imports needed)
use serde::Serialize;
use sqlx::Row;
use std::collections::{BTreeMap, HashMap};
use tauri::{AppHandle, State};
use tauri::Manager; // for try_state
use tracing::{debug, info};

#[derive(Debug, Serialize)]
pub struct DuplicatePosition {
    pub page_id: i32,
    pub current_page_number: Option<u32>,
    pub index_in_page: i32,
    pub urls: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GroupSummary {
    pub page_id: i32,
    pub current_page_number: Option<u32>,
    pub count: u32,
    pub distinct_indices: u32,
    pub min_index: Option<i32>,
    pub max_index: Option<i32>,
    pub expected_full: bool, // true if group is expected to be full (12)
    pub expected_count: u32,
    pub missing_indices: Vec<i32>,
    pub duplicate_indices: Vec<i32>,
    pub out_of_range_count: u32,
    pub status: String, // ok | duplicates | holes | sparse_nonterminal | out_of_range | mixed
}

#[derive(Debug, Serialize)]
pub struct DbPaginationMismatchReport {
    pub total_products: u64,
    pub max_page_id_db: Option<i32>,
    pub total_pages_site: Option<u32>,
    pub items_on_last_page: Option<u32>,
    pub group_summaries: Vec<GroupSummary>,
    pub duplicate_positions: Vec<DuplicatePosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepass: Option<PrepassSummary>,
}

#[derive(Debug, Serialize, Default)]
pub struct PrepassSummary {
    pub details_aligned: u64,
    pub products_id_backfilled: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_align_skipped_due_to_slot_taken: Option<u64>,
}

/// Scan local DB for pagination invariants without mutating anything.
/// Invariants checked per page_id group:
/// - For non-terminal groups (page_id < max_page_id_db): count must be 12 and indices must be 0..11 (no holes/dupes)
/// - For terminal group (page_id == max_page_id_db): indices must be contiguous starting from 0 (0..count-1)
/// - index_in_page must be within [0, 11]
#[tauri::command(async)]
pub async fn scan_db_pagination_mismatches(
    _app: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<DbPaginationMismatchReport, String> {
    info!(target: "db_diagnostics", "scan_db_pagination_mismatches: start");
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

        // === Pre-pass: align product_details positions/ids by products.url, then backfill products.id from details ===
        let mut prepass = PrepassSummary::default();
    {
                let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
                // 1) Align product_details.page_id/index_in_page and recompute product_details.id from products
                //    Only when products has non-null page_id/index_in_page.
                                // Count how many rows would collide with existing target slot
                                let res0 = sqlx::query_scalar::<_, i64>(
                                                r#"
                                                SELECT COUNT(*) FROM product_details pd
                                                WHERE EXISTS (SELECT 1 FROM products WHERE products.url = pd.url)
                                                    AND (SELECT page_id FROM products WHERE products.url = pd.url) IS NOT NULL
                                                    AND (SELECT index_in_page FROM products WHERE products.url = pd.url) IS NOT NULL
                                                    AND EXISTS (
                                                            SELECT 1 FROM product_details AS pd2
                                                            WHERE pd2.page_id = (SELECT page_id FROM products WHERE products.url = pd.url)
                                                                AND pd2.index_in_page = (SELECT index_in_page FROM products WHERE products.url = pd.url)
                                                    )
                                                    AND (
                                                            COALESCE(pd.page_id, -1) != COALESCE((SELECT page_id FROM products WHERE products.url = pd.url), -1)
                                                            OR COALESCE(pd.index_in_page, -1) != COALESCE((SELECT index_in_page FROM products WHERE products.url = pd.url), -1)
                                                            OR pd.id != printf('p%04di%02d',
                                                                            (SELECT page_id FROM products WHERE products.url = pd.url),
                                                                            (SELECT index_in_page FROM products WHERE products.url = pd.url))
                                                    )
                                                "#,
                                )
                                .fetch_one(&mut *tx)
                                .await
                                .unwrap_or(0);

                                let res1 = sqlx::query(
                        r#"
                        UPDATE product_details
                        SET
                            page_id = (SELECT page_id FROM products WHERE products.url = product_details.url),
                            index_in_page = (SELECT index_in_page FROM products WHERE products.url = product_details.url),
                            id = printf('p%04di%02d',
                                        (SELECT page_id FROM products WHERE products.url = product_details.url),
                                        (SELECT index_in_page FROM products WHERE products.url = product_details.url))
                        WHERE
                            EXISTS (SELECT 1 FROM products WHERE products.url = product_details.url)
                            AND (SELECT page_id FROM products WHERE products.url = product_details.url) IS NOT NULL
                            AND (SELECT index_in_page FROM products WHERE products.url = product_details.url) IS NOT NULL
                            -- Choose a single canonical row per URL to avoid multiple rows racing for the same target slot
                            AND product_details.rowid = (
                                SELECT MIN(rowid) FROM product_details AS pdsame WHERE pdsame.url = product_details.url
                            )
                            -- Do not update if the target slot is already occupied by any row (avoid UNIQUE violation)
                            AND NOT EXISTS (
                                SELECT 1 FROM product_details AS pd2
                                WHERE pd2.page_id = (SELECT page_id FROM products WHERE products.url = product_details.url)
                                  AND pd2.index_in_page = (SELECT index_in_page FROM products WHERE products.url = product_details.url)
                            )
                            AND (
                                COALESCE(product_details.page_id, -1) != COALESCE((SELECT page_id FROM products WHERE products.url = product_details.url), -1)
                                OR COALESCE(product_details.index_in_page, -1) != COALESCE((SELECT index_in_page FROM products WHERE products.url = product_details.url), -1)
                                OR product_details.id != printf('p%04di%02d',
                                        (SELECT page_id FROM products WHERE products.url = product_details.url),
                                        (SELECT index_in_page FROM products WHERE products.url = product_details.url))
                            )
                        "#,
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Prepass alignment failed: {e}"))?;
                prepass.details_aligned = res1.rows_affected();
                prepass.details_align_skipped_due_to_slot_taken = Some(res0 as u64);
                debug!(target: "db_diagnostics", details_aligned = prepass.details_aligned, "prepass: details aligned");

                // 2) Backfill products.id from product_details.id when NULL/empty
                let res2 = sqlx::query(
                        r#"
                        UPDATE products
                        SET id = (SELECT id FROM product_details WHERE product_details.url = products.url)
                        WHERE (id IS NULL OR id = '')
                            AND EXISTS (
                                SELECT 1 FROM product_details 
                                WHERE product_details.url = products.url 
                                    AND product_details.id IS NOT NULL 
                                    AND product_details.id <> ''
                            )
                        "#,
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Prepass products.id backfill failed: {e}"))?;
                prepass.products_id_backfilled = res2.rows_affected();
                debug!(target: "db_diagnostics", products_id_backfilled = prepass.products_id_backfilled, "prepass: products.id backfilled");

                tx.commit().await.map_err(|e| e.to_string())?;
        }

    // Skip network calls in diagnostics to avoid stalling; derive site meta from cache/config only.
    // 1) Prefer SharedStateCache.site_analysis (if present and fresh)
    // 2) Fallback to AppConfig.app_managed.last_known_max_page (no items_on_last_page available)
    let mut total_pages_site: Option<u32> = None;
    let mut items_on_last_page: Option<u32> = None;

    if let Some(cache_state) = _app.try_state::<SharedStateCache>() {
        if let Some(site) = cache_state.get_valid_site_analysis_async(Some(10)).await {
            total_pages_site = Some(site.total_pages);
            items_on_last_page = Some(site.products_on_last_page);
        }
    }
    if total_pages_site.is_none() {
        // Fallback to persisted config values without any network calls
        let cfg = { app_state.config.read().await.clone() };
        total_pages_site = cfg.app_managed.last_known_max_page;
        // items_on_last_page not available from config; leave as None
    }

    // Load all relevant rows
    let mut total_products: u64 = 0;
    if let Ok(c) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await
    {
        total_products = c as u64;
    }

    // Fetch url, page_id, index_in_page; ignore rows with NULL url
    let rows =
        sqlx::query("SELECT url, page_id, index_in_page FROM products WHERE url IS NOT NULL")
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;

    // Organize by page_id
    let mut by_pid: BTreeMap<i32, Vec<(String, Option<i32>)>> = BTreeMap::new();
    let mut out_of_range_count_by_pid: HashMap<i32, u32> = HashMap::new();
    for r in rows.into_iter() {
        let url: String = r.try_get("url").unwrap_or_default();
        let pid_opt: Option<i64> = r.try_get("page_id").ok();
        let idx_opt: Option<i64> = r.try_get("index_in_page").ok();
        let pid = pid_opt.unwrap_or(-1) as i32;
        let idx = idx_opt.map(|v| v as i32);
        let entry = by_pid.entry(pid).or_default();
        entry.push((url, idx));
        // track out-of-range
        if let Some(ix) = idx {
            if ix < 0 || ix > 11 {
                *out_of_range_count_by_pid.entry(pid).or_insert(0) += 1;
            }
        } else {
            *out_of_range_count_by_pid.entry(pid).or_insert(0) += 1;
        }
    }

    if by_pid.is_empty() {
        return Ok(DbPaginationMismatchReport {
            total_products,
            max_page_id_db: None,
            total_pages_site,
            items_on_last_page,
            group_summaries: vec![],
            duplicate_positions: vec![],
            prepass: Some(prepass),
        });
    }

    let max_page_id_db = *by_pid.keys().max().unwrap();
    // If no site meta available from cache/config, fall back to DB-derived total pages
    if total_pages_site.is_none() {
        total_pages_site = Some((max_page_id_db as u32).saturating_add(1));
    }
    let mut group_summaries: Vec<GroupSummary> = Vec::new();
    let mut duplicate_positions: Vec<DuplicatePosition> = Vec::new();

    for (pid, items) in by_pid.iter() {
        let count = items.len() as u32;
        let terminal = *pid == max_page_id_db;
        let expected_count = if terminal { count } else { 12 };
        let expected_full = !terminal;
        let current_page_number = if *pid >= 0 {
            total_pages_site.map(|tp| tp.saturating_sub(*pid as u32))
        } else {
            None
        };

        // Build map index -> urls
        let mut index_map: BTreeMap<i32, Vec<&str>> = BTreeMap::new();
        let mut indices: Vec<i32> = Vec::new();
        for (url, idx_opt) in items.iter() {
            if let Some(ix) = *idx_opt {
                indices.push(ix);
                index_map.entry(ix).or_default().push(url.as_str());
            }
        }
        let distinct_indices = index_map.len() as u32;
        let min_index = indices.iter().min().cloned();
        let max_index = indices.iter().max().cloned();

        // Detect duplicates and missing
        let mut dup_indices: Vec<i32> = Vec::new();
        for (ix, urls) in index_map.iter() {
            if urls.len() > 1 {
                dup_indices.push(*ix);
                duplicate_positions.push(DuplicatePosition {
                    page_id: *pid,
                    current_page_number,
                    index_in_page: *ix,
                    urls: urls.iter().map(|s| s.to_string()).collect(),
                });
            }
        }
        // Missing indices
        let missing_indices: Vec<i32> = if expected_full {
            (0..12).filter(|ix| !index_map.contains_key(ix)).collect()
        } else {
            // terminal group expected contiguous from 0..(distinct_indices-1)
            (0..(distinct_indices as i32))
                .filter(|ix| !index_map.contains_key(ix))
                .collect()
        };
        let out_of_range_count = *out_of_range_count_by_pid.get(pid).unwrap_or(&0);

        // Status aggregation
        let mut status_parts: Vec<&str> = Vec::new();
        if !dup_indices.is_empty() {
            status_parts.push("duplicates");
        }
        if !missing_indices.is_empty() {
            // Only label sparse_nonterminal if non-terminal and count != 12
            if expected_full && count != 12 {
                status_parts.push("sparse_nonterminal");
            } else {
                status_parts.push("holes");
            }
        }
        if out_of_range_count > 0 {
            status_parts.push("out_of_range");
        }
        let status = if status_parts.is_empty() {
            "ok".to_string()
        } else if status_parts.len() == 1 {
            status_parts[0].to_string()
        } else {
            "mixed".to_string()
        };

        group_summaries.push(GroupSummary {
            page_id: *pid,
            current_page_number,
            count,
            distinct_indices,
            min_index,
            max_index,
            expected_full,
            expected_count,
            missing_indices,
            duplicate_indices: dup_indices,
            out_of_range_count,
            status,
        });
    }

    let report = DbPaginationMismatchReport {
        total_products,
        max_page_id_db: Some(max_page_id_db),
        total_pages_site,
        items_on_last_page,
        group_summaries,
        duplicate_positions,
        prepass: Some(prepass),
    };

    info!(target: "db_diagnostics", total_products = report.total_products, groups = report.group_summaries.len(), dup_positions = report.duplicate_positions.len(), "scan_db_pagination_mismatches: done");
    Ok(report)
}
