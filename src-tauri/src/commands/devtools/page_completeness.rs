use crate::application::AppState;
use serde::Serialize;

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct PageCompletenessGap {
    pub logical_page_id: i64,
    pub latest_attempt_no: i64,
    pub product_count: i64,
    pub distinct_indices: i64,
    pub count_mismatch: bool,
    pub index_mismatch: bool,
    pub last_error_code: Option<String>,
    pub last_error_detail: Option<String>,
    pub is_terminal_guess: bool,
    pub success_final: bool,
    pub last_fetched_at: String,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct PageCompletenessSummary {
    pub total_pages_tracked: usize,
    pub problematic_pages: usize,
    pub gaps: Vec<PageCompletenessGap>,
}

/// Scan for pages whose latest attempt did not reach completeness success.
#[tauri::command(async)]
pub async fn scan_page_completeness_gaps(app_state: tauri::State<'_, AppState>) -> Result<PageCompletenessSummary, String> {
    let pool = app_state.get_database_pool().await.map_err(|e| e.to_string())?;

    // Query latest attempt for each page and filter those not success_final
    let rows = sqlx::query!(
        r#"SELECT p.logical_page_id, p.attempt_no AS latest_attempt_no, p.product_count, p.distinct_indices, p.count_mismatch, p.index_mismatch, p.error_code, p.error_detail, p.is_terminal_guess, p.success_final, p.fetched_at
            FROM v_page_latest_attempt p
            WHERE p.success_final = 0
            ORDER BY p.logical_page_id ASC"#
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let gaps: Vec<PageCompletenessGap> = rows.into_iter().map(|r| PageCompletenessGap {
        logical_page_id: r.logical_page_id,
        latest_attempt_no: r.latest_attempt_no,
        product_count: r.product_count,
        distinct_indices: r.distinct_indices,
        count_mismatch: r.count_mismatch != 0,
        index_mismatch: r.index_mismatch != 0,
        last_error_code: r.error_code,
        last_error_detail: r.error_detail,
        is_terminal_guess: r.is_terminal_guess != 0,
        success_final: r.success_final != 0,
        last_fetched_at: r.fetched_at,
    }).collect();

    // Count total tracked pages (including success) via page_fetch_attempts distinct
    let total_pages_tracked: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT logical_page_id) FROM page_fetch_attempts")
        .fetch_one(&pool).await.unwrap_or(0);

    Ok(PageCompletenessSummary { total_pages_tracked: total_pages_tracked as usize, problematic_pages: gaps.len(), gaps })
}
