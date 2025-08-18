use crate::application::AppState;
use crate::infrastructure::{
    config::{ConfigManager, csa_iot},
    html_parser::MatterDataExtractor,
};
use serde::Serialize;
use sqlx::Row;
use std::collections::{BTreeMap, HashMap};
use tauri::{AppHandle, State};

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
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Optional site meta for context (not strictly needed for checks)
    let (total_pages_site, items_on_last_page) = {
        let cfg_manager = ConfigManager::new()
            .map_err(|e| format!("Config manager init failed: {e}"))?;
        let app_config = cfg_manager
            .load_config()
            .await
            .map_err(|e| format!("Config load failed: {e}"))?;
        let http = app_config
            .create_http_client()
            .map_err(|e| e.to_string())?;
        let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
        // Fetch first page to estimate total_pages; tolerate failures silently
        let newest_url = csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
        let mut total_pages: Option<u32> = None;
        let mut last_count: Option<u32> = None;
        if let Ok(resp) = http.fetch_response(&newest_url).await {
            if let Ok(html) = resp.text().await {
                total_pages = extractor.extract_total_pages(&html).ok();
                if total_pages == Some(1) {
                    last_count = extractor
                        .extract_product_urls_from_content(&html)
                        .ok()
                        .map(|v| v.len() as u32);
                } else if let Some(tp) = total_pages {
                    let oldest_url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
                        .replace("{}", &tp.to_string());
                    if let Ok(resp2) = http.fetch_response(&oldest_url).await {
                        if let Ok(html2) = resp2.text().await {
                            last_count = extractor
                                .extract_product_urls_from_content(&html2)
                                .ok()
                                .map(|v| v.len() as u32);
                        }
                    }
                }
            }
        }
        (total_pages, last_count)
    };

    // Load all relevant rows
    let mut total_products: u64 = 0;
    if let Ok(c) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await
    {
        total_products = c as u64;
    }

    // Fetch url, page_id, index_in_page; ignore rows with NULL url
    let rows = sqlx::query(
        "SELECT url, page_id, index_in_page FROM products WHERE url IS NOT NULL",
    )
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
        });
    }

    let max_page_id_db = *by_pid.keys().max().unwrap();
    let mut group_summaries: Vec<GroupSummary> = Vec::new();
    let mut duplicate_positions: Vec<DuplicatePosition> = Vec::new();

    for (pid, items) in by_pid.iter() {
        let count = items.len() as u32;
        let terminal = *pid == max_page_id_db;
        let expected_count = if terminal { count } else { 12 };
        let expected_full = !terminal;
        let current_page_number = total_pages_site.map(|tp| tp.saturating_sub(*pid as u32));

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
            (0..12)
                .filter(|ix| !index_map.contains_key(ix))
                .collect()
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

    Ok(DbPaginationMismatchReport {
        total_products,
        max_page_id_db: Some(max_page_id_db),
        total_pages_site,
        items_on_last_page,
        group_summaries,
        duplicate_positions,
    })
}
