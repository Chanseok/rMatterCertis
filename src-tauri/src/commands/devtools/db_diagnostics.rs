// Diagnostics now enabled also for release builds (was gated by dev-tools/debug).
use crate::application::AppState;
use crate::infrastructure::database_connection::legacy_slot_unique_index_present;
use crate::infrastructure::write_lock_tracker; // detect active write tx to avoid intrusive probes
use crate::application::shared_state::SharedStateCache;
// (no additional infrastructure imports needed)
use serde::Serialize;
use sqlx::Row;
use std::collections::{BTreeMap, HashMap};
use tauri::Manager; // for try_state
use tauri::{AppHandle, State};
use tracing::{debug, info};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct DbConnectionDiagnostics {
    pub timestamp_utc: String,
    pub pool_closed: bool,
    pub acquire_timeout_ms: u64,
    pub simple_select_ok: bool,
    pub concurrent_connections: Option<u32>,
    pub immediate_select_ok: bool,
    pub write_probe_ok: bool,
    pub write_probe_elapsed_ms: Option<u64>,
    pub write_probe_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_probe_error_classification: Option<String>,
    pub slot_unique_index_present: bool,
    pub notes: Vec<String>,
}
/// Lightweight DB connection health check.
/// - Attempts immediate PRAGMA busy_timeout=1 then a SELECT 1.
/// - Reports whether pool is closed and captures active connection count if possible.
// (Removed malformed fragment from previous bad merge)
    #[tauri::command(async)]
    pub async fn diagnose_database_connection(app_state: tauri::State<'_, AppState>) -> Result<DbConnectionDiagnostics, String> {
        let t_start = std::time::Instant::now();
        info!(target: "db_diag", "🔍 diagnose_database_connection invoked");
        let pool = app_state
            .get_database_pool()
            .await
            .map_err(|e| format!("db pool error: {e}"))?;

        let mut notes = Vec::new();
        let pool_closed = pool.is_closed();
        if pool_closed { notes.push("Pool is marked closed".into()); }

        // 전용 커넥션을 따로 획득하여 busy_timeout 변형을 그 안에만 국한
        let mut dedicated_conn = match pool.acquire().await {
            Ok(c) => c,
            Err(e) => return Err(format!("acquire failed: {e}")),
        };

        // Set extremely small timeout to probe for immediate lock contention (read) - isolated
        let _ = sqlx::query("PRAGMA busy_timeout=1").execute(&mut *dedicated_conn).await;
        let immediate_select_ok = match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&mut *dedicated_conn).await {
            Ok(_) => true,
            Err(e) => { notes.push(format!("Immediate SELECT failed: {e}")); false }
        };

        // Restore normal timeout on the dedicated connection only
        let _ = sqlx::query("PRAGMA busy_timeout=5000").execute(&mut *dedicated_conn).await;

        // Run a second simple select to confirm operational
        let simple_select_ok = match sqlx::query_scalar::<_, i64>("SELECT 42").fetch_one(&mut *dedicated_conn).await {
            Ok(v) => v == 42,
            Err(e) => { notes.push(format!("Second SELECT failed: {e}")); false }
        };

        // Active connection count not directly available for SQLite (leave None for now)
        let concurrent_connections: Option<u32> = None;

        // --- Write probe (BEGIN IMMEDIATE -> fallback to BEGIN) opt-in ---
        let mut write_probe_ok = false;
    let mut write_probe_elapsed_ms: Option<u64> = None;
    let mut write_probe_error: Option<String> = None;
    let mut write_probe_error_classification: Option<String> = None;
        let write_probe_enabled = std::env::var("MC_DIAGNOSTICS_WRITE_PROBE")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        let active_writes = write_lock_tracker::snapshot();
        if !write_probe_enabled {
            notes.push("Write probe disabled by default (set MC_DIAGNOSTICS_WRITE_PROBE=1 to enable)".into());
        } else if !active_writes.is_empty() {
            let now = Utc::now();
            let ls: Vec<String> = active_writes
                .into_iter()
                .map(|i| format!("id={} label={} age_ms={}", i.id, i.label, (now - i.started_at).num_milliseconds()))
                .collect();
            notes.push(format!("Write probe skipped: active write(s) detected -> {}", ls.join("; ")));
        } else {
            let start = std::time::Instant::now();
            let max_attempts = 3u8; // allow one fallback attempt (attempt 3)
            let immediate_limit: u8 = 2; // first 2 attempts: IMMEDIATE, then fallback
            for attempt in 1..=max_attempts {
                match pool.acquire().await {
                    Ok(mut conn) => {
                        let _ = sqlx::query("PRAGMA busy_timeout=80").execute(&mut *conn).await;
                        let use_deferred = attempt > immediate_limit;
                        if use_deferred && attempt == immediate_limit + 1 { notes.push("Write probe fallback: switching to DEFERRED".into()); }
                        let begin_sql = if use_deferred { "BEGIN" } else { "BEGIN IMMEDIATE" };
                        match sqlx::query(begin_sql).execute(&mut *conn).await {
                            Ok(_) => {
                                write_probe_ok = true;
                                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await; // release lock immediately
                                let elapsed = start.elapsed().as_millis() as u64;
                                write_probe_elapsed_ms = Some(elapsed);
                                notes.push(format!("Write probe success mode={} attempt={} elapsed_ms={}", if use_deferred {"deferred"} else {"immediate"}, attempt, elapsed));
                                let _ = sqlx::query("PRAGMA busy_timeout=5000").execute(&mut *conn).await; // restore
                                break;
                            }
                            Err(e) => {
                                let err_str = e.to_string();
                                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                                let lower = err_str.to_lowercase();
                                let is_locked = lower.contains("locked") || lower.contains("busy");
                                let is_unique_slot = lower.contains("unique constraint failed: products.page_id, products.index_in_page");
                                if attempt == max_attempts {
                                    write_probe_error = Some(err_str.clone());
                                    let elapsed_total = start.elapsed().as_millis() as u64;
                                    write_probe_elapsed_ms = Some(elapsed_total);
                                    write_probe_error_classification = if is_locked { Some("lock_busy".into()) } else if is_unique_slot { Some("unique_slot_conflict".into()) } else { Some("other".into()) };
                                    match write_probe_error_classification.as_deref() {
                                        Some("lock_busy") => notes.push(format!("Write probe busy after {} attempts mode_last={} total_elapsed_ms={}", attempt, if use_deferred {"deferred"} else {"immediate"}, elapsed_total)),
                                        Some("unique_slot_conflict") => notes.push(format!("Write probe failed due to unique slot conflict attempts={} total_elapsed_ms={} err={}", attempt, elapsed_total, err_str)),
                                        _ => notes.push(format!("Write probe failed non-lock error attempts={} err={}", attempt, err_str)),
                                    }
                                } else {
                                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let err_str = format!("acquire failed: {e}");
                        if attempt == max_attempts {
                            write_probe_error = Some(err_str.clone());
                            let elapsed_total = start.elapsed().as_millis() as u64;
                            write_probe_elapsed_ms = Some(elapsed_total);
                            notes.push(format!("Write probe aborted: connection acquire failed after {} attempts ({} ms): {}", attempt, elapsed_total, err_str));
                        } else {
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        }
                    }
                }
                if write_probe_ok { break; }
            }
            if write_probe_ok && write_probe_error.is_none() && write_probe_elapsed_ms.is_none() {
                write_probe_elapsed_ms = Some(start.elapsed().as_millis() as u64);
            }
        }

        let elapsed_total = t_start.elapsed().as_millis() as u64;
        info!(target: "db_diag", immediate_select_ok, simple_select_ok, write_probe_ok, write_probe_elapsed_ms = write_probe_elapsed_ms.unwrap_or(0), write_probe_error = write_probe_error.as_deref().unwrap_or(""), elapsed_ms = elapsed_total, "diagnose_database_connection completed");
        Ok(DbConnectionDiagnostics {
            timestamp_utc: DateTime::<Utc>::from(Utc::now()).to_rfc3339(),
            pool_closed,
            acquire_timeout_ms: 1,
            simple_select_ok,
            concurrent_connections,
            immediate_select_ok,
            write_probe_ok,
            write_probe_elapsed_ms,
            write_probe_error,
            write_probe_error_classification,
            slot_unique_index_present: legacy_slot_unique_index_present(),
            notes,
        })
    }

    // ===== Pagination mismatch diagnostics (legacy + enhanced) =====

    #[derive(Debug, Serialize, ts_rs::TS)]
    #[ts(export, export_to = "../../../../generated-types/")]
    pub struct DbPaginationMismatchReport {
        pub total_products: u64, // 로컬 DB 총 제품 수 (좌표 NULL 포함)
        pub total_products_with_coords: u64, // 좌표가 있는 제품 수
        pub total_products_without_coords: u64, // 좌표가 NULL인 제품 수
        pub total_products_site: Option<u64>, // 사이트 총 제품 수
        pub max_page_id_db: Option<i32>,
        pub total_pages_site: Option<u32>,
        pub items_on_last_page: Option<u32>,
        pub group_summaries: Vec<GroupSummary>,
        pub duplicate_positions: Vec<DuplicatePosition>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub prepass: Option<PrepassSummary>,
        // Newly exposed coordinate reconciliation diagnostics
        #[serde(skip_serializing_if = "Option::is_none")]
        pub coord_mismatch: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub details_missing_coords: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub products_missing_coords: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub coord_mismatch_samples: Option<Vec<CoordMismatchSample>>,
        // Missing page sequence detection
        pub missing_pages: Vec<PageSequenceGap>,
        pub total_missing_pages: u32,
    }

    #[derive(Debug, Serialize, ts_rs::TS)]
    #[ts(export, export_to = "../../../../generated-types/")]
    pub struct GroupSummary {
        pub page_id: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub current_page_number: Option<u32>,
        pub count: u32,
        pub distinct_indices: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub min_index: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_index: Option<i32>,
        pub expected_full: bool,
        pub expected_count: u32,
        pub missing_indices: Vec<i32>,
        pub duplicate_indices: Vec<i32>,
        pub out_of_range_count: u32,
        pub status: String,
    }

    #[derive(Debug, Serialize, ts_rs::TS)]
    #[ts(export, export_to = "../../../../generated-types/")]
    pub struct DuplicatePosition {
        pub page_id: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub current_page_number: Option<u32>,
        pub index_in_page: i32,
        pub urls: Vec<String>,
    }
// (Removed misplaced struct field fragments from previous bad merge)

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct CoordMismatchSample {
    pub url: String,
    pub d_pid: Option<i32>,
    pub d_idx: Option<i32>,
    pub p_pid: Option<i32>,
    pub p_idx: Option<i32>,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct PageSequenceGap {
    pub start_page: i32,
    pub end_page: i32,
    pub missing_count: u32,
    pub gap_type: String, // "single" or "range"
    pub start_physical_page: Option<u32>,
    pub end_physical_page: Option<u32>,
}

#[derive(Debug, Serialize, Default, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
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
/// # Errors
/// Returns an error string if the database pool cannot be obtained or a query fails.
/// # Panics
/// Panics if internal aggregation assumes at least one page group while `by_pid` is empty; guarded by early return.
pub async fn scan_db_pagination_mismatches(
    app: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<DbPaginationMismatchReport, String> {
    info!(target: "db_diagnostics", "scan_db_pagination_mismatches: start");
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // 🔧 Quick DB lock test (전용 커넥션 사용) - 풀의 기본 timeout 오염 방지
    let mut diag_conn = match pool.acquire().await {
        Ok(c) => c,
        Err(e) => return Err(format!("acquire failed: {e}")),
    };
    let _ = sqlx::query("PRAGMA busy_timeout=10").execute(&mut *diag_conn).await; // Very short timeout
    match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&mut *diag_conn).await {
        Ok(_) => {
            // DB is available, continue with diagnostics
        }
        Err(e) if e.to_string().contains("locked") || e.to_string().contains("busy") => {
            info!(target: "db_diagnostics", "DB is currently locked - skipping diagnostics to avoid interfering with crawling");
            // busy_timeout 조정은 전용 커넥션에만 적용되었으므로 별도 복구 불필요
            return Ok(DbPaginationMismatchReport {
                total_products: 0,
                total_products_with_coords: 0,
                total_products_without_coords: 0,
                total_products_site: None,
                max_page_id_db: None,
                total_pages_site: None,
                items_on_last_page: None,
                group_summaries: vec![],
                duplicate_positions: vec![],
                prepass: None,
                coord_mismatch: None,
                details_missing_coords: None,
                products_missing_coords: None,
                coord_mismatch_samples: None,
                missing_pages: vec![],
                total_missing_pages: 0,
            });
        }
        Err(e) => return Err(e.to_string()),
    }

    // === Pre-pass (best effort). 쓰기 작업은 환경 변수로 명시적으로 허용된 경우에만 수행. ===
    let mut prepass = PrepassSummary::default();
    let prepass_writes_enabled = std::env::var("MC_DIAGNOSTICS_PREPASS_WRITE").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
    if prepass_writes_enabled {
        // 전용 커넥션 위에서만 timeout 조정
        let prepass_conn = match pool.acquire().await { Ok(c) => Some(c), Err(e) => { info!(target="db_diagnostics", "prepass acquire failed: {e}"); None } };
        if let Some(mut prepass_conn) = prepass_conn {
            let _ = sqlx::query("PRAGMA busy_timeout=300").execute(&mut *prepass_conn).await;
            // Manual transaction (BEGIN/COMMIT) to avoid needing trait-based begin()
            if let Ok(_) = sqlx::query("BEGIN").execute(&mut *prepass_conn).await {
                // Wrap the whole aligning logic so any lock/busy error just skips
                let tx_result = async {
                    let res0 = sqlx::query_scalar::<_, i64>(r"
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
                    ").fetch_one(&mut *prepass_conn).await.unwrap_or(0);

                    let res1 = if prepass_writes_enabled { sqlx::query(r"
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
                            AND product_details.rowid = (
                                SELECT MIN(rowid) FROM product_details AS pdsame WHERE pdsame.url = product_details.url
                            )
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
                    ").execute(&mut *prepass_conn).await? } else { sqlx::query("SELECT 0").execute(&mut *prepass_conn).await? };
                    if prepass_writes_enabled { prepass.details_aligned = res1.rows_affected(); } else { prepass.details_aligned = 0; }
                    prepass.details_align_skipped_due_to_slot_taken = Some(u64::try_from(res0).unwrap_or_default());
                    debug!(target: "db_diagnostics", details_aligned = prepass.details_aligned, "prepass: details aligned");

                    let res2 = if prepass_writes_enabled { sqlx::query(r"
                        UPDATE products
                        SET id = (SELECT id FROM product_details WHERE product_details.url = products.url)
                        WHERE (id IS NULL OR id = '')
                          AND EXISTS (
                                SELECT 1 FROM product_details 
                                WHERE product_details.url = products.url 
                                  AND product_details.id IS NOT NULL 
                                  AND product_details.id <> ''
                          )
                    ").execute(&mut *prepass_conn).await? } else { sqlx::query("SELECT 0").execute(&mut *prepass_conn).await? };
                    if prepass_writes_enabled { prepass.products_id_backfilled = res2.rows_affected(); }
                    debug!(target: "db_diagnostics", products_id_backfilled = prepass.products_id_backfilled, "prepass: products.id backfilled");
                    Ok::<(), sqlx::Error>(())
                }.await;

                match tx_result {
                    Ok(_) => { let _ = sqlx::query("COMMIT").execute(&mut *prepass_conn).await; },
                    Err(e) => {
                        if e.to_string().contains("locked") || e.to_string().contains("busy") {
                            info!(target: "db_diagnostics", "prepass skipped due to busy/lock: {e}");
                            let _ = sqlx::query("ROLLBACK").execute(&mut *prepass_conn).await;
                            prepass = PrepassSummary::default();
                        } else {
                            info!(target: "db_diagnostics", error = %e, "prepass encountered non-lock error; continuing without abort");
                            let _ = sqlx::query("ROLLBACK").execute(&mut *prepass_conn).await;
                        }
                    }
                }
            } else {
                info!(target: "db_diagnostics", "prepass BEGIN failed (possibly locked); skipping prepass mutations");
            }
        } else {
            info!(target: "db_diagnostics", "prepass transaction begin failed (possibly locked or acquire failed); skipping prepass mutations");
        }
    } else {
        info!(target="db_diagnostics", "prepass writes disabled (MC_DIAGNOSTICS_PREPASS_WRITE != 1)");
    }

    // Skip network calls in diagnostics to avoid stalling; derive site meta from cache/config only.
    // 1) Prefer SharedStateCache.site_analysis (if present and fresh)
    // 2) Fallback to AppConfig.app_managed.last_known_max_page (no items_on_last_page available)
    let mut total_pages_site: Option<u32> = None;
    let mut items_on_last_page: Option<u32> = None;

    if let Some(cache_state) = app.try_state::<SharedStateCache>() {
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

    // Load all relevant rows - 기본 busy_timeout (풀 설정보다 변경 X); 필요시 전용 커넥션 사용 고려 가능
    
    let mut total_products: u64 = 0;
    let mut total_products_with_coords: u64 = 0;
    let mut total_products_without_coords: u64 = 0;
    
    // 총 제품 수
    if let Ok(c) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await
    {
        total_products = u64::try_from(c).unwrap_or_default();
    }
    
    // 좌표가 있는 제품 수
    if let Ok(c) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL")
        .fetch_one(&pool)
        .await
    {
        total_products_with_coords = u64::try_from(c).unwrap_or_default();
    }
    
    // 좌표가 없는 제품 수
    if let Ok(c) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products WHERE page_id IS NULL OR index_in_page IS NULL")
        .fetch_one(&pool)
        .await
    {
        total_products_without_coords = u64::try_from(c).unwrap_or_default();
    }
    
    // 사이트 총 제품 수 계산 (총 페이지 수 * 12 - 마지막 페이지 여분)
    let total_products_site: Option<u64> = if let (Some(total_pages), Some(last_items)) = (total_pages_site, items_on_last_page) {
        let expected = (total_pages as u64) * 12;
        let excess = 12u64.saturating_sub(last_items as u64);
        Some(expected.saturating_sub(excess))
    } else if let Some(total_pages) = total_pages_site {
        // items_on_last_page가 없으면 대략 계산
        Some((total_pages as u64) * 12)
    } else {
        None
    };

    // Fetch url, page_id, index_in_page; ignore rows with NULL url
    // If database is busy (locked by crawling), return early with minimal report
    let rows = match sqlx::query("SELECT url, page_id, index_in_page FROM products WHERE url IS NOT NULL")
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) if e.to_string().contains("locked") || e.to_string().contains("busy") => {
            info!(target: "db_diagnostics", "Main query skipped due to DB lock - crawling in progress");
            // Restore default busy_timeout before returning.
            let _ = sqlx::query("PRAGMA busy_timeout=5000").execute(&pool).await;
            return Ok(DbPaginationMismatchReport {
                total_products,
                total_products_with_coords,
                total_products_without_coords,
                total_products_site,
                max_page_id_db: None,
                total_pages_site,
                items_on_last_page,
                group_summaries: vec![],
                duplicate_positions: vec![],
                prepass: Some(prepass),
                coord_mismatch: None,
                details_missing_coords: None,
                products_missing_coords: None,
                coord_mismatch_samples: None,
                missing_pages: vec![],
                total_missing_pages: 0,
            });
        },
        Err(e) => return Err(e.to_string()),
    };

    // Organize by page_id
    let mut by_pid: BTreeMap<i32, Vec<(String, Option<i32>)>> = BTreeMap::new();
    let mut out_of_range_count_by_pid: HashMap<i32, u32> = HashMap::new();
    for r in rows {
        let url: String = r.try_get("url").unwrap_or_default();
        let pid_opt: Option<i64> = r.try_get("page_id").ok();
        let idx_opt: Option<i64> = r.try_get("index_in_page").ok();
        let pid = pid_opt.and_then(|v| i32::try_from(v).ok()).unwrap_or(-1);
        let idx = idx_opt.and_then(|v| i32::try_from(v).ok());
        let entry = by_pid.entry(pid).or_default();
        entry.push((url, idx));
        // track out-of-range
        if let Some(ix) = idx {
            if !(0..=11).contains(&ix) {
                *out_of_range_count_by_pid.entry(pid).or_insert(0) += 1;
            }
        } else {
            *out_of_range_count_by_pid.entry(pid).or_insert(0) += 1;
        }
    }

    if by_pid.is_empty() {
        // Restore default busy_timeout before returning.
        let _ = sqlx::query("PRAGMA busy_timeout=5000").execute(&pool).await;
        return Ok(DbPaginationMismatchReport {
            total_products,
            total_products_with_coords,
            total_products_without_coords,
            total_products_site,
            max_page_id_db: None,
            total_pages_site,
            items_on_last_page,
            group_summaries: vec![],
            duplicate_positions: vec![],
            prepass: Some(prepass),
            coord_mismatch: None,
            details_missing_coords: None,
            products_missing_coords: None,
            coord_mismatch_samples: None,
            missing_pages: vec![],
            total_missing_pages: 0,
        });
    }

    let max_page_id_db = *by_pid.keys().max().unwrap();
    // If no site meta available from cache/config, fall back to DB-derived total pages
    if total_pages_site.is_none() {
        total_pages_site = u32::try_from(max_page_id_db)
            .ok()
            .map(|v| v.saturating_add(1));
    }
    let mut group_summaries: Vec<GroupSummary> = Vec::new();
    let mut duplicate_positions: Vec<DuplicatePosition> = Vec::new();

    for (pid, items) in &by_pid {
        let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
        let terminal = *pid == max_page_id_db;
        let expected_count = if terminal { count } else { 12 };
        let expected_full = !terminal;
        let current_page_number = if *pid >= 0 {
            total_pages_site.and_then(|tp| u32::try_from(*pid).ok().map(|pp| tp.saturating_sub(pp)))
        } else {
            None
        };

        // Build map index -> urls
        let mut index_map: BTreeMap<i32, Vec<&str>> = BTreeMap::new();
        let mut indices: Vec<i32> = Vec::new();
        for (url, idx_opt) in items {
            if let Some(ix) = *idx_opt {
                indices.push(ix);
                index_map.entry(ix).or_default().push(url.as_str());
            }
        }
        let distinct_indices = u32::try_from(index_map.len()).unwrap_or(u32::MAX);
        let min_index = indices.iter().min().copied();
        let max_index = indices.iter().max().copied();

        // Detect duplicates and missing
        let mut dup_indices: Vec<i32> = Vec::new();
        for (ix, urls) in &index_map {
            if urls.len() > 1 {
                dup_indices.push(*ix);
                duplicate_positions.push(DuplicatePosition {
                    page_id: *pid,
                    current_page_number,
                    index_in_page: *ix,
                    urls: urls.iter().map(|s| (*s).to_string()).collect(),
                });
            }
        }
        // Missing indices
        let missing_indices: Vec<i32> = if expected_full {
            (0..12).filter(|ix| !index_map.contains_key(ix)).collect()
        } else {
            // terminal group expected contiguous from 0..(distinct_indices-1)
            {
                #[allow(clippy::used_underscore_binding)]
                let upper: i32 = i32::try_from(distinct_indices).unwrap_or(i32::MAX);
                0..upper
            }
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

    // Detect page sequence gaps
    let existing_pages: std::collections::HashSet<i32> = by_pid.keys().copied().collect();
    let mut missing_pages = Vec::new();
    let mut total_missing_pages = 0u32;
    
    // Find gaps in page sequence from 0 to max_page_id_db
    let mut current = 0i32;
    while current <= max_page_id_db {
        if !existing_pages.contains(&current) {
            // Found a gap, determine the range
            let start_gap = current;
            while current <= max_page_id_db && !existing_pages.contains(&current) {
                current += 1;
                total_missing_pages += 1;
            }
            let end_gap = current - 1;
            
            // Calculate physical page numbers for missing pages
            let start_physical = total_pages_site.and_then(|tp| {
                if start_gap >= 0 {
                    u32::try_from(start_gap).ok().map(|pg| tp.saturating_sub(pg))
                } else {
                    None
                }
            });
            let end_physical = total_pages_site.and_then(|tp| {
                if end_gap >= 0 {
                    u32::try_from(end_gap).ok().map(|pg| tp.saturating_sub(pg))
                } else {
                    None
                }
            });

            missing_pages.push(PageSequenceGap {
                start_page: start_gap,
                end_page: end_gap,
                missing_count: u32::try_from(end_gap - start_gap + 1).unwrap_or(0),
                gap_type: if start_gap == end_gap { "single".to_string() } else { "range".to_string() },
                start_physical_page: start_physical,
                end_physical_page: end_physical,
            });
        } else {
            current += 1;
        }
    }

    let mut report = DbPaginationMismatchReport {
        total_products,
        total_products_with_coords,
        total_products_without_coords,
        total_products_site,
        max_page_id_db: Some(max_page_id_db),
        total_pages_site,
        items_on_last_page,
        group_summaries,
        duplicate_positions,
        prepass: Some(prepass),
        missing_pages,
        total_missing_pages,
        coord_mismatch: None,
        details_missing_coords: None,
        products_missing_coords: None,
        coord_mismatch_samples: None,
    };

    // 추가 요약 로깅: 좌표 누락 / mismatch 상황 집계 (전용 커넥션 사용)
    if let Ok(mut summary_conn) = pool.acquire().await {
        let _ = sqlx::query("PRAGMA busy_timeout=50").execute(&mut *summary_conn).await;
        if let Ok(coord_mismatch) = sqlx::query_scalar::<_, i64>(r"
        SELECT COUNT(*) FROM product_details d
        LEFT JOIN products p ON p.url = d.url
        WHERE p.url IS NULL
           OR p.page_id IS NULL OR p.index_in_page IS NULL
           OR (p.page_id != d.page_id OR p.index_in_page != d.index_in_page)
    ").fetch_one(&mut *summary_conn).await {
        info!(target: "db_diagnostics", coord_mismatch, "coord_mismatch_summary");
        report.coord_mismatch = u64::try_from(coord_mismatch).ok();
    }
        if let Ok(details_without_coords) = sqlx::query_scalar::<_, i64>(r"
        SELECT COUNT(*) FROM product_details WHERE page_id IS NULL OR index_in_page IS NULL
    ").fetch_one(&mut *summary_conn).await {
        info!(target: "db_diagnostics", details_without_coords, "details_missing_coords_summary");
        report.details_missing_coords = u64::try_from(details_without_coords).ok();
    }
        if let Ok(products_missing_coords) = sqlx::query_scalar::<_, i64>(r"
        SELECT COUNT(*) FROM products WHERE page_id IS NULL OR index_in_page IS NULL
    ").fetch_one(&mut *summary_conn).await {
        info!(target: "db_diagnostics", products_missing_coords, "products_missing_coords_summary");
        report.products_missing_coords = u64::try_from(products_missing_coords).ok();
    }

    // Sample detail holes: pick up to 5 URLs where detail has coords but product missing or mismatch
        if let Ok(rows) = sqlx::query(r"
        SELECT d.url, d.page_id as d_pid, d.index_in_page as d_idx, p.page_id as p_pid, p.index_in_page as p_idx
        FROM product_details d
        LEFT JOIN products p ON p.url = d.url
        WHERE p.url IS NULL
           OR p.page_id IS NULL OR p.index_in_page IS NULL
           OR (p.page_id != d.page_id OR p.index_in_page != d.index_in_page)
        LIMIT 5
    ").fetch_all(&mut *summary_conn).await {
        let mut samples: Vec<CoordMismatchSample> = Vec::new();
        for row in rows {
            let sample = CoordMismatchSample {
                url: row.get("url"),
                d_pid: row.get("d_pid"),
                d_idx: row.get("d_idx"),
                p_pid: row.get("p_pid"),
                p_idx: row.get("p_idx"),
            };
            info!(target="db_diagnostics", url = sample.url, d_pid = sample.d_pid, d_idx = sample.d_idx, p_pid = sample.p_pid, p_idx = sample.p_idx, "coord_mismatch_sample");
            samples.push(sample);
        }
        if !samples.is_empty() { report.coord_mismatch_samples = Some(samples); }
        }
    } // summary_conn scope 종료

    // Log page sequence gaps for visibility
    if !report.missing_pages.is_empty() {
        info!(target: "db_diagnostics", 
              total_missing_pages = report.total_missing_pages, 
              gap_count = report.missing_pages.len(), 
              "page_sequence_gaps_detected");
        for gap in &report.missing_pages {
            if gap.gap_type == "single" {
                info!(target: "db_diagnostics", missing_page = gap.start_page, "single_page_gap");
            } else {
                info!(target: "db_diagnostics", 
                      gap_start = gap.start_page, 
                      gap_end = gap.end_page, 
                      gap_size = gap.missing_count, 
                      "page_range_gap");
            }
        }
    }

    info!(target: "db_diagnostics", 
          total_products = report.total_products, 
          groups = report.group_summaries.len(), 
          dup_positions = report.duplicate_positions.len(),
          missing_pages = report.total_missing_pages,
          "scan_db_pagination_mismatches: done");

    // 🔧 Enhanced connection cleanup for diagnostics
    // Reset busy timeout to default and run a lightweight query to force connection release
    let _ = sqlx::query("PRAGMA busy_timeout=5000").execute(&pool).await;
    let _ = sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&pool).await;
    
    // Small delay to ensure any background operations complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    info!(target: "db_diagnostics", "Connection cleanup completed after diagnostics");
    
    Ok(report)
}

// END scan_db_pagination_mismatches

// ===== NULL Coordinates Management =====

#[derive(Debug, Serialize, ts_rs::TS, Clone)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct ProductWithoutCoordinates {
    pub url: String,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub url_exists: bool, // URL 접근 가능 여부
    pub checked_at: Option<String>, // 마지막 확인 시간
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct NullCoordinatesReport {
    pub total_count: u32,
    pub verified_exists: Vec<ProductWithoutCoordinates>,
    pub verified_missing: Vec<ProductWithoutCoordinates>,
    pub not_verified: Vec<ProductWithoutCoordinates>,
}

/// NULL 좌표를 가진 제품 목록 조회 및 URL 검증
#[tauri::command(async)]
pub async fn get_products_without_coordinates(
    app_state: State<'_, AppState>,
    skip_verification: Option<bool>,
) -> Result<NullCoordinatesReport, String> {
    let skip_verification = skip_verification.unwrap_or(false);
    info!(target: "db_diagnostics", skip_verification, "get_products_without_coordinates: start");
    
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // NULL 좌표 제품 조회
    let rows = sqlx::query(
        r"
        SELECT 
            p.url,
            pd.model,
            pd.manufacturer
        FROM products p
        LEFT JOIN product_details pd ON p.url = pd.url
        WHERE p.page_id IS NULL OR p.index_in_page IS NULL
        ORDER BY p.url
        "
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut products = Vec::new();
    
    for row in rows {
        let url: String = row.try_get("url").unwrap_or_default();
        let model: Option<String> = row.try_get("model").ok().flatten();
        let manufacturer: Option<String> = row.try_get("manufacturer").ok().flatten();
        
        products.push(ProductWithoutCoordinates {
            url,
            model,
            manufacturer,
            url_exists: false,
            checked_at: None,
        });
    }

    info!(target: "db_diagnostics", total = products.len(), "Loaded products without coordinates");

    let mut verified_exists = Vec::new();
    let mut verified_missing = Vec::new();
    let mut not_verified = Vec::new();

    // URL 검증을 건너뛰는 경우 모두 not_verified로 분류
    if skip_verification {
        info!(target: "db_diagnostics", "Skipping URL verification as requested");
        not_verified = products;
    } else {
        // HTTP 클라이언트로 URL 존재 여부 확인 (타임아웃 증가)
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .connect_timeout(std::time::Duration::from_secs(1))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

        // URL 검증 (최대 50개까지만, 너무 많으면 시간 초과)
        let max_verify = 50;
        for (idx, mut product) in products.into_iter().enumerate() {
            if idx >= max_verify {
                // 나머지는 미검증으로 분류
                not_verified.push(product);
                continue;
            }
            
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                client.head(&product.url).send()
            ).await {
                Ok(Ok(response)) => {
                    product.url_exists = response.status().is_success();
                    product.checked_at = Some(Utc::now().to_rfc3339());
                    
                    if product.url_exists {
                        info!(target: "db_diagnostics", url = %product.url, "URL exists");
                        verified_exists.push(product);
                    } else {
                        info!(target: "db_diagnostics", url = %product.url, status = %response.status(), "URL not found");
                        verified_missing.push(product);
                    }
                }
                Ok(Err(e)) => {
                    // HTTP 요청 오류
                    info!(target: "db_diagnostics", url = %product.url, error = %e, "HTTP request failed");
                    not_verified.push(product);
                }
                Err(_) => {
                    // 타임아웃
                    info!(target: "db_diagnostics", url = %product.url, "URL verification timeout");
                    not_verified.push(product);
                }
            }
            
            // 너무 빠른 요청 방지
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    let total_count = u32::try_from(verified_exists.len() + verified_missing.len() + not_verified.len())
        .unwrap_or(u32::MAX);

    info!(
        target: "db_diagnostics",
        total = total_count,
        exists = verified_exists.len(),
        missing = verified_missing.len(),
        not_verified = not_verified.len(),
        "get_products_without_coordinates: done"
    );

    Ok(NullCoordinatesReport {
        total_count,
        verified_exists,
        verified_missing,
        not_verified,
    })
}

/// NULL 좌표를 가진 제품들 삭제 (products 테이블에서 삭제 → FK CASCADE로 product_details도 자동 삭제)
#[tauri::command(async)]
pub async fn delete_products_without_coordinates(
    app_state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<u32, String> {
    info!(target: "db_diagnostics", count = urls.len(), "delete_products_without_coordinates: start");
    
    if urls.is_empty() {
        return Ok(0);
    }

    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    let placeholders = urls.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    
    // products 테이블에서 삭제 → FK CASCADE로 product_details도 자동 삭제됨
    let query_str = format!("DELETE FROM products WHERE url IN ({})", placeholders);
    let mut query = sqlx::query(&query_str);
    for url in &urls {
        query = query.bind(url);
    }
    
    let result = query.execute(&pool).await.map_err(|e| e.to_string())?;
    let deleted = result.rows_affected();
    
    let deleted_count = u32::try_from(deleted).unwrap_or(u32::MAX);

    info!(
        target: "db_diagnostics",
        requested = urls.len(),
        products_deleted = deleted_count,
        "delete_products_without_coordinates: done (product_details also deleted via FK CASCADE)"
    );

    Ok(deleted_count)
}

/// 테이블 일관성 체크: products와 product_details 간 불일치 감지
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableInconsistencyReport {
    pub products_count: i64,
    pub details_count: i64,
    pub only_in_products: Vec<OrphanProduct>,
    pub only_in_details: Vec<OrphanProduct>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OrphanProduct {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
}

#[tauri::command(async)]
pub async fn check_table_consistency(
    app_state: State<'_, AppState>,
) -> Result<TableInconsistencyReport, String> {
    info!(target: "db_diagnostics", "check_table_consistency: start");
    
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // 1. 전체 카운트 확인
    let products_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;
    
    let details_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    info!(
        target: "db_diagnostics",
        products_count,
        details_count,
        "Table counts"
    );

    // 2. products에만 있는 제품 (고아 레코드 - product_details 없음)
    let only_in_products_rows = sqlx::query(
        r"
        SELECT p.url, p.manufacturer, p.model, p.page_id, p.index_in_page
        FROM products p
        LEFT JOIN product_details pd ON p.url = pd.url
        WHERE pd.url IS NULL
        ORDER BY p.page_id DESC, p.index_in_page ASC
        LIMIT 500
        "
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let only_in_products: Vec<OrphanProduct> = only_in_products_rows
        .into_iter()
        .map(|row| OrphanProduct {
            url: row.get("url"),
            manufacturer: row.get("manufacturer"),
            model: row.get("model"),
            page_id: row.get("page_id"),
            index_in_page: row.get("index_in_page"),
        })
        .collect();

    // 3. product_details에만 있는 제품 (고아 레코드 - products 없음)
    let only_in_details_rows = sqlx::query(
        r"
        SELECT pd.url, pd.manufacturer, pd.model, pd.page_id, pd.index_in_page
        FROM product_details pd
        LEFT JOIN products p ON pd.url = p.url
        WHERE p.url IS NULL
        ORDER BY pd.page_id DESC, pd.index_in_page ASC
        LIMIT 500
        "
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let only_in_details: Vec<OrphanProduct> = only_in_details_rows
        .into_iter()
        .map(|row| OrphanProduct {
            url: row.get("url"),
            manufacturer: row.get("manufacturer"),
            model: row.get("model"),
            page_id: row.get("page_id"),
            index_in_page: row.get("index_in_page"),
        })
        .collect();

    info!(
        target: "db_diagnostics",
        only_in_products_count = only_in_products.len(),
        only_in_details_count = only_in_details.len(),
        "check_table_consistency: done"
    );

    Ok(TableInconsistencyReport {
        products_count,
        details_count,
        only_in_products,
        only_in_details,
    })
}

/// 고아 레코드 삭제 (한쪽 테이블에만 있는 레코드 정리)
#[tauri::command(async)]
pub async fn delete_orphan_records(
    app_state: State<'_, AppState>,
    urls: Vec<String>,
    table: String, // "products" or "product_details"
) -> Result<u32, String> {
    info!(
        target: "db_diagnostics",
        count = urls.len(),
        table = %table,
        "delete_orphan_records: start"
    );
    
    if urls.is_empty() {
        return Ok(0);
    }

    if table != "products" && table != "product_details" {
        return Err("Invalid table name. Must be 'products' or 'product_details'".to_string());
    }

    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    let placeholders = urls.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query_str = format!("DELETE FROM {} WHERE url IN ({})", table, placeholders);
    
    let mut query = sqlx::query(&query_str);
    for url in &urls {
        query = query.bind(url);
    }
    
    let result = query.execute(&pool).await.map_err(|e| e.to_string())?;
    let deleted = result.rows_affected();
    
    let deleted_count = u32::try_from(deleted).unwrap_or(u32::MAX);

    info!(
        target: "db_diagnostics",
        requested = urls.len(),
        deleted = deleted_count,
        table = %table,
        "delete_orphan_records: done"
    );

    Ok(deleted_count)
}
