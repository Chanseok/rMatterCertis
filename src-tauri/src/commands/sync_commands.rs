#![allow(clippy::used_underscore_binding, clippy::unnecessary_map_or)]
use crate::application::AppState;
use crate::crawl_engine::actors::types::{AppEvent, SyncAnomalyEntry};
use crate::domain::pagination::CanonicalPageIdCalculator;
use crate::infrastructure::{
    config::csa_iot, html_parser::MatterDataExtractor, simple_http_client::RequestOptions,
};
use chrono::Utc;
use scraper::Html;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, State};
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

// Reuse helper to emit events
use super::validation_commands::emit_actor_event;

// Minimal summary returned by sync commands
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncSummary {
    pub pages_processed: u32,
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
    pub failed: u32,
    pub duration_ms: u64,
}

// Minimal local diagnostic structs (placeholder until unified types module)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticPageInput {
    pub physical_page: u32,
    pub miss_indices: Vec<u32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticSnapshotInput {
    pub total_pages: u32,
    pub items_on_last_page: u32,
}

// ---- Temporary stubs to satisfy lib.rs registrations (fast-build path) ----
#[tauri::command(async)]
pub async fn start_partial_sync(_app: AppHandle, _app_state: State<'_, AppState>, _expr: String, _dry_run: Option<bool>) -> Result<SyncSummary, String> {
    Err("start_partial_sync temporarily disabled".into())
}

#[tauri::command(async)]
pub async fn start_batched_sync(_app: AppHandle, _app_state: State<'_, AppState>, _ranges: String, _dry_run: Option<bool>) -> Result<SyncSummary, String> {
    Err("start_batched_sync temporarily disabled".into())
}

#[tauri::command(async)]
pub async fn start_repair_sync(_app: AppHandle, _app_state: State<'_, AppState>, _ranges: String, _dry_run: Option<bool>) -> Result<SyncSummary, String> {
    Err("start_repair_sync temporarily disabled".into())
}

#[tauri::command(async)]
pub async fn retry_failed_details(_app: AppHandle, _app_state: State<'_, AppState>, _limit: Option<u32>) -> Result<SyncSummary, String> {
    Err("retry_failed_details temporarily disabled".into())
}

/// Run the basic 4-stage crawling engine for an explicit set of physical page numbers
/// using the new page_filter path (avoids delegating to partial sync).
///
/// # Errors
/// Returns `Err(String)` if no pages are provided, or if HTTP fetch/parse or DB access fails.
#[tauri::command(async)]
pub async fn start_basic_sync_pages(
    app: AppHandle,
    app_state: State<'_, AppState>,
    mut pages: Vec<u32>,
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    // Errors
    // - Returns Err(String) if no pages provided, HTTP fetch/parse fails, or DB access fails.
    if pages.is_empty() {
        return Err("No pages provided".into());
    }

    // Normalize page set: newest → oldest
    pages.sort_unstable();
    pages.dedup();
    pages.reverse();

    // Infra
    let app_config = app_state.config.read().await.clone();
    let http = app_state.get_http_client().await?;
    let sync_ua = app_config.user.crawling.workers.user_agent_sync.clone();
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Schema capability: does products have an 'id' column?
    let products_has_id_column: bool = sqlx::query("PRAGMA table_info(products)")
        .fetch_all(&pool)
        .await
        .is_ok_and(|cols| {
            cols.iter().any(|r| {
                let name: String = r.try_get("name").unwrap_or_default();
                name == "id"
            })
        });

    // Discover site meta (Stage 1-equivalent)
    let newest_url = csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
    let newest_html = match http
        .fetch_response_with_options(
            &newest_url,
            &RequestOptions {
                user_agent_override: sync_ua.clone(),
                referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                skip_robots_check: false,
                attempt: None,
                max_attempts: None,
            },
        )
        .await
    {
        Ok(resp) => resp.text().await.map_err(|e| e.to_string())?,
        Err(e) => return Err(e.to_string()),
    };
    let total_pages = extractor
        .extract_total_pages(&newest_html)
        .unwrap_or(1)
        .max(1);
    let oldest_page = total_pages;
    let oldest_html = if oldest_page == 1 {
        newest_html.clone()
    } else {
        let oldest_url =
            csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &oldest_page.to_string());
        match http
            .fetch_response_with_options(
                &oldest_url,
                &RequestOptions {
                    user_agent_override: sync_ua.clone(),
                    referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                    skip_robots_check: false,
                    attempt: None,
                    max_attempts: None,
                },
            )
            .await
        {
            Ok(resp) => resp.text().await.map_err(|e| e.to_string())?,
            Err(e) => return Err(e.to_string()),
        }
    };
    let items_on_last_page = extractor
        .extract_product_urls_from_content(&oldest_html)
        .map_err(|e| e.to_string())?
        .len();
    let calculator = CanonicalPageIdCalculator::new(total_pages, items_on_last_page);

    // Emit SyncStarted with explicit pages as singleton ranges
    let session_id = format!("basic-{}", Utc::now().format("%Y%m%d%H%M%S"));
    emit_actor_event(
        &app,
        &AppEvent::SyncStarted {
            session_id: session_id.clone(),
            ranges: pages.iter().map(|p| (*p, *p)).collect(),
            rate_limit: Some(app_config.user.crawling.workers.max_requests_per_second),
            timestamp: Utc::now(),
        },
    );

    // Concurrency and counters
    let max_concurrent = app_config
        .user
        .crawling
        .workers
        .list_page_max_concurrent
        .max(1);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    let pages_processed = Arc::new(AtomicU32::new(0));
    let inserted = Arc::new(AtomicU32::new(0));
    let updated = Arc::new(AtomicU32::new(0));
    let skipped = Arc::new(AtomicU32::new(0));
    let failed = Arc::new(AtomicU32::new(0));

    // Retry configs
    let list_retry_count: u32 = app_config.user.crawling.product_list_retry_count.max(1);
    let detail_retry_count: u32 = app_config.user.crawling.product_detail_retry_count.max(1);
    let is_dry_run = dry_run.unwrap_or(false);

    let started = std::time::Instant::now();

    let mut handles = Vec::with_capacity(pages.len());
    for physical_page in pages {
        // Bound to site limits just in case
        if physical_page < 1 || physical_page > total_pages {
            continue;
        }

        let permit = semaphore.clone().acquire_owned();
        let app = app.clone();
        let session_id = session_id.clone();
        let pool = pool.clone();
        let http = http.clone();
        let extractor = extractor.clone();
        let calculator = calculator.clone();
        let newest_html_clone = newest_html.clone();
        let oldest_html_clone = oldest_html.clone();
        let pages_processed_c = pages_processed.clone();
        let inserted_c = inserted.clone();
        let updated_c = updated.clone();
        let skipped_c = skipped.clone();
        let failed_c = failed.clone();
    let products_has_id_col = products_has_id_column;
    let sync_ua = sync_ua.clone();
        let max_list_retries = list_retry_count;
        let max_detail_retries_cfg = detail_retry_count;

        let handle = tokio::spawn(async move {
            // Acquire slot
            let _permit = match permit.await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to acquire semaphore: {}", e);
                    return;
                }
            };

            emit_actor_event(
                &app,
                &AppEvent::SyncPageStarted {
                    session_id: session_id.clone(),
                    physical_page,
                    timestamp: Utc::now(),
                },
            );

            // Fetch + parse product list with retries
            let expected_count = if physical_page == oldest_page {
                u32::try_from(items_on_last_page).unwrap_or(u32::MAX)
            } else {
                12u32
            };
            let mut attempt = 0u32;
            let mut product_urls: Vec<String> = Vec::new();
            let mut last_err_msg: Option<String> = None;
            loop {
                let use_cache =
                    attempt == 0 && (physical_page == oldest_page || physical_page == 1);
                let page_html = if use_cache {
                    if physical_page == oldest_page {
                        oldest_html_clone.clone()
                    } else {
                        newest_html_clone.clone()
                    }
                } else {
                    let url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
                        .replace("{}", &physical_page.to_string());
                    match http
                        .fetch_response_with_options(
                            &url,
                            &RequestOptions {
                                user_agent_override: sync_ua.clone(),
                                referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                                skip_robots_check: false,
                                attempt: Some(std::cmp::max(1, attempt + 1)),
                                max_attempts: Some(std::cmp::max(1, max_list_retries + 1)),
                            },
                        )
                        .await
                    {
                        Ok(resp) => match resp.text().await {
                            Ok(t) => t,
                            Err(e) => {
                                last_err_msg = Some(format!("read_body_failed: {}", e));
                                String::new()
                            }
                        },
                        Err(e) => {
                            last_err_msg = Some(format!("fetch_failed: {}", e));
                            String::new()
                        }
                    }
                };

                if !page_html.is_empty() {
                    match extractor.extract_product_urls_from_content(&page_html) {
                        Ok(v) => {
                            product_urls = v;
                            if u32::try_from(product_urls.len()).unwrap_or(u32::MAX)
                                == expected_count
                            {
                                break;
                            }
                            last_err_msg = Some(format!(
                                "count_mismatch: expected {} got {}",
                                expected_count,
                                product_urls.len()
                            ));
                        }
                        Err(e) => {
                            last_err_msg = Some(format!("parse_failed: {}", e));
                        }
                    }
                }

                if attempt >= max_list_retries {
                    break;
                }
                // Emit retrying event
                emit_actor_event(
                    &app,
                    &AppEvent::SyncRetrying {
                        session_id: session_id.clone(),
                        scope: "list_page".into(),
                        physical_page: Some(physical_page),
                        url: None,
                        attempt: attempt + 1,
                        max_attempts: max_list_retries,
                        reason: last_err_msg.clone(),
                        timestamp: Utc::now(),
                    },
                );
                let backoff_ms = 200u64 * (1u64 << attempt);
                tokio::time::sleep(std::time::Duration::from_millis(
                    backoff_ms + (u64::from(physical_page) % 37),
                ))
                .await;
                attempt += 1;
            }

            if u32::try_from(product_urls.len()).unwrap_or(u32::MAX) != expected_count {
                if let Some(msg) = &last_err_msg {
                    emit_actor_event(
                        &app,
                        &AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "count_mismatch".into(),
                            detail: format!(
                                "page {}: {} (got {} of {})",
                                physical_page,
                                msg,
                                product_urls.len(),
                                expected_count
                            ),
                            timestamp: Utc::now(),
                        },
                    );
                }
            }

            // Begin transaction (page-level) with write lock tracking
            let tx_guard = crate::infrastructure::write_lock_tracker::register("sync_page","page write tx");
            let tx_begin_instant = std::time::Instant::now();
            let mut tx = match pool.begin().await {
                Ok(t) => t,
                Err(e) => {
                    let elapsed = tx_begin_instant.elapsed().as_millis();
                    tracing::warn!(target="write_lock_tracker", page=physical_page, elapsed_ms=%elapsed, guard_id=%tx_guard.id(), error=%e, "sync_page_tx_begin_failed");
                    failed_c.fetch_add(1, Ordering::SeqCst);
                    emit_actor_event(
                        &app,
                        AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "tx_begin_failed".into(),
                            detail: format!("page {}: {}", physical_page, e),
                            timestamp: Utc::now(),
                        },
                    );
                    return;
                }
            };

            let mut page_inserted = 0u32;
            let mut page_updated = 0u32;
            let mut page_skipped = 0u32;
            let mut page_failed = 0u32;
            let page_start = std::time::Instant::now();

            for (i, url) in product_urls.iter().enumerate() {
                let calc = calculator.calculate(physical_page, i);
                if is_dry_run {
                    page_skipped += 1;
                    emit_actor_event(
                        &app,
                        &AppEvent::SyncUpsertProgress {
                            session_id: session_id.clone(),
                            physical_page,
                            inserted: page_inserted,
                            updated: page_updated,
                            skipped: page_skipped,
                            failed: page_failed,
                            timestamp: Utc::now(),
                        },
                    );
                    continue;
                }

                // Record observed
                let _ = sqlx::query(
                    "INSERT INTO sync_observed(session_id, url, page_id, index_in_page) VALUES(?, ?, ?, ?) \
                     ON CONFLICT(session_id, url) DO UPDATE SET page_id=excluded.page_id, index_in_page=excluded.index_in_page",
                )
                .bind(&session_id)
                .bind(url)
                .bind(calc.page_id)
                .bind(calc.index_in_page)
                .execute(&mut *tx)
                .await;

                let row = match sqlx::query(
                    "SELECT page_id, index_in_page FROM products WHERE url = ? LIMIT 1",
                )
                .bind(url)
                .fetch_optional(&mut *tx)
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        page_failed += 1;
                        failed_c.fetch_add(1, Ordering::SeqCst);
                        emit_actor_event(
                            &app,
                            &AppEvent::SyncWarning {
                                session_id: session_id.clone(),
                                code: "select_failed".into(),
                                detail: format!("{}: {}", url, e),
                                timestamp: Utc::now(),
                            },
                        );
                        continue;
                    }
                };

                match row {
                    None => {
                        // Insert product
                        if calc.page_id > 0 && calc.index_in_page > 0 {
                            match sqlx::query("INSERT INTO products (url, page_id, index_in_page) VALUES (?, ?, ?)")
                                .bind(url)
                                .bind(calc.page_id)
                                .bind(calc.index_in_page)
                                .execute(&mut *tx).await {
                                    Ok(_) => { page_inserted += 1; inserted_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, &AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_inserted".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); },
                                    Err(e) => { page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, &AppEvent::SyncWarning { session_id: session_id.clone(), code: "insert_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); continue; }
                                }
                        }
                        // Ensure product_details placeholder with synthetic id
                        let synthetic_id =
                            format!("p{:04}i{:02}", calc.page_id, calc.index_in_page);
                        let _ = sqlx::query(
                            r"INSERT INTO product_details (url, page_id, index_in_page, id)
                                    VALUES (?, ?, ?, ?)
                                    ON CONFLICT(url) DO UPDATE SET
                                        page_id = COALESCE(excluded.page_id, product_details.page_id),
                                        index_in_page = COALESCE(excluded.index_in_page, product_details.index_in_page),
                                        id = COALESCE(product_details.id, excluded.id),
                                        updated_at = CURRENT_TIMESTAMP",
                        )
                        .bind(url)
                        .bind(calc.page_id)
                        .bind(calc.index_in_page)
                        .bind(synthetic_id)
                        .execute(&mut *tx)
                        .await;
                    }
                    Some(r) => {
                        let db_pid: Option<i64> = r.get("page_id");
                        let db_idx: Option<i64> = r.get("index_in_page");
                        let needs_update = match (db_pid, db_idx) {
                            (Some(p), Some(ix)) => match (i32::try_from(p), i32::try_from(ix)) {
                                (Ok(pp), Ok(ii)) => pp != calc.page_id || ii != calc.index_in_page,
                                // If conversion fails (negative or out of range), force update to repair
                                _ => true,
                            },
                            _ => true,
                        };
                        if needs_update {
                            match sqlx::query("UPDATE products SET page_id = ?, index_in_page = ?, updated_at = CURRENT_TIMESTAMP WHERE url = ?")
                                .bind(calc.page_id)
                                .bind(calc.index_in_page)
                                .bind(url)
                                .execute(&mut *tx)
                                .await {
                                    Ok(_) => { page_updated += 1; updated_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, &AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_updated".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); },
                                    Err(e) => { page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, &AppEvent::SyncWarning { session_id: session_id.clone(), code: "update_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); emit_actor_event(&app, &AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_update_failed".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); }
                                }
                        } else {
                            page_skipped += 1;
                            skipped_c.fetch_add(1, Ordering::SeqCst);
                            emit_actor_event(
                                &app,
                                &AppEvent::ProductLifecycle {
                                    session_id: session_id.clone(),
                                    batch_id: None,
                                    page_number: Some(physical_page),
                                    product_ref: url.clone(),
                                    status: "product_skipped_nochange".into(),
                                    retry: None,
                                    duration_ms: None,
                                    metrics: None,
                                    timestamp: Utc::now(),
                                },
                            );
                        }

                        // Keep details in sync and ensure id if missing
                        let synthetic_id =
                            format!("p{:04}i{:02}", calc.page_id, calc.index_in_page);
                        let _ = sqlx::query(
                            r"INSERT INTO product_details (url, page_id, index_in_page, id)
                                    VALUES (?, ?, ?, ?)
                                    ON CONFLICT(url) DO UPDATE SET
                                        page_id = COALESCE(excluded.page_id, product_details.page_id),
                                        index_in_page = COALESCE(excluded.index_in_page, product_details.index_in_page),
                                        id = COALESCE(product_details.id, excluded.id),
                                        updated_at = CURRENT_TIMESTAMP",
                        )
                        .bind(url)
                        .bind(calc.page_id)
                        .bind(calc.index_in_page)
                        .bind(synthetic_id)
                        .execute(&mut *tx)
                        .await;

                        // If details missing, try fetch with retries
                        let details_missing = (sqlx::query_scalar::<_, i64>(
                            "SELECT 1 FROM product_details WHERE url = ? LIMIT 1",
                        )
                        .bind(url)
                        .fetch_optional(&mut *tx)
                        .await)
                            .map_or(false, |opt| opt.is_none());
                        if details_missing && !is_dry_run {
                            let mut success = false;
                            for attempt in 1..=max_detail_retries_cfg {
                                let referer_url = if physical_page == 1 {
                                    csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string()
                                } else {
                                    csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
                                        .replace("{}", &physical_page.to_string())
                                };
                                if let Ok(resp) = http
                                    .fetch_response_with_options(
                                        url,
                                        &RequestOptions {
                                            user_agent_override: sync_ua.clone(),
                                            referer: Some(referer_url),
                                            skip_robots_check: false,
                                            attempt: Some(attempt),
                                            max_attempts: Some(max_detail_retries_cfg),
                                        },
                                    )
                                    .await
                                {
                                    if let Ok(body) = resp.text().await {
                                        let extracted = {
                                            let doc = Html::parse_document(&body);
                                            extractor.extract_product_detail(&doc, url.clone())
                                        };
                                        if let Ok(mut detail) = extracted {
                                            detail.page_id = Some(calc.page_id);
                                            detail.index_in_page = Some(calc.index_in_page);
                                            if detail.id.is_none() {
                                                detail.id = Some(format!(
                                                    "p{:04}i{:02}",
                                                    calc.page_id, calc.index_in_page
                                                ));
                                            }
                                            let program_type = Some(
                                                detail
                                                    .program_type
                                                    .unwrap_or_else(|| "Matter".to_string()),
                                            );
                                            // clone fields for backfill
                                            let man_c = detail.manufacturer.clone();
                                            let model_c = detail.model.clone();
                                            let cert_c = detail.certificate_id.clone();
                                            let detail_id_clone = detail.id.clone();
                                            if sqlx::query(
                                            r"INSERT INTO product_details (
                                                url, page_id, index_in_page, id, manufacturer, model, device_type,
                                                certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                                specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                                tis_trp_tested, transport_interface, application_categories,
                                                description, compliance_document_url, program_type
                                            ) VALUES (
                                                ?, ?, ?, ?, ?, ?, ?,
                                                ?, ?, ?, ?, ?,
                                                ?, ?, ?, ?, ?, ?,
                                                ?, ?, ?, ?,
                                                ?, ?, ?
                                            ) ON CONFLICT(url) DO UPDATE SET
                                                page_id=COALESCE(excluded.page_id, product_details.page_id),
                                                index_in_page=COALESCE(excluded.index_in_page, product_details.index_in_page),
                                                id=COALESCE(excluded.id, product_details.id),
                                                manufacturer=COALESCE(excluded.manufacturer, product_details.manufacturer),
                                                model=COALESCE(excluded.model, product_details.model),
                                                device_type=COALESCE(excluded.device_type, product_details.device_type),
                                                certificate_id=COALESCE(excluded.certificate_id, product_details.certificate_id),
                                                certification_date=COALESCE(excluded.certification_date, product_details.certification_date),
                                                software_version=COALESCE(excluded.software_version, product_details.software_version),
                                                hardware_version=COALESCE(excluded.hardware_version, product_details.hardware_version),
                                                firmware_version=COALESCE(excluded.firmware_version, product_details.firmware_version),
                                                specification_version=COALESCE(excluded.specification_version, product_details.specification_version),
                                                vid=COALESCE(excluded.vid, product_details.vid),
                                                pid=COALESCE(excluded.pid, product_details.pid),
                                                family_sku=COALESCE(excluded.family_sku, product_details.family_sku),
                                                family_variant_sku=COALESCE(excluded.family_variant_sku, product_details.family_variant_sku),
                                                family_id=COALESCE(excluded.family_id, product_details.family_id),
                                                tis_trp_tested=COALESCE(excluded.tis_trp_tested, product_details.tis_trp_tested),
                                                transport_interface=COALESCE(excluded.transport_interface, product_details.transport_interface),
                                                -- primary_device_type_id removed
                                                application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                                description=COALESCE(excluded.description, product_details.description),
                                                compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                                program_type=COALESCE(excluded.program_type, product_details.program_type),
                                                updated_at=CURRENT_TIMESTAMP
                                        ",
                                        )
                                        .bind(&detail.url)
                                        .bind(detail.page_id)
                                        .bind(detail.index_in_page)
                                        .bind(detail_id_clone.clone())
                                        .bind(detail.manufacturer)
                                        .bind(detail.model)
                                        .bind(detail.device_type)
                                        .bind(detail.certificate_id)
                                        .bind(detail.certification_date)
                                        .bind(detail.software_version)
                                        .bind(detail.hardware_version)
                                        .bind(detail.firmware_version)
                                        .bind(detail.specification_version)
                                        .bind(detail.vid)
                                        .bind(detail.pid)
                                        .bind(detail.family_sku)
                                        .bind(detail.family_variant_sku)
                                        .bind(detail.family_id)
                                        .bind(detail.tis_trp_tested)
                                        .bind(detail.transport_interface)
                                        .bind(detail.application_categories)
                                        .bind(detail.description)
                                        .bind(detail.compliance_document_url)
                                        .bind(program_type)
                                        .execute(&mut *tx)
                                        .await
                                        .is_ok()
                                        {
                                            // backfill products core fields
                                            let _ = sqlx::query(
                                                r"UPDATE products SET
                                                    manufacturer = COALESCE(?, manufacturer),
                                                    model = COALESCE(?, model),
                                                    certificate_id = COALESCE(?, certificate_id),
                                                    updated_at = CURRENT_TIMESTAMP
                                                WHERE url = ?",
                                            )
                                            .bind(&man_c)
                                            .bind(&model_c)
                                            .bind(&cert_c)
                                            .bind(&detail.url)
                                            .execute(&mut *tx)
                                            .await;

                                            // Optionally backfill products.id
                                            if products_has_id_col {
                                                let _ = sqlx::query(
                                                    r"UPDATE products SET id = CASE WHEN id IS NULL OR id = '' THEN ? ELSE id END WHERE url = ?",
                                                )
                                                .bind(&detail_id_clone)
                                                .bind(&detail.url)
                                                .execute(&mut *tx)
                                                .await;
                                            }
                                            success = true;
                                            break;
                                        }
                                        }
                                    } else { /* read failed */
                                    }
                                } else { /* fetch failed */
                                }
                                if attempt < max_detail_retries_cfg && !success {
                                    emit_actor_event(
                                        &app,
                                        AppEvent::SyncRetrying {
                                            session_id: session_id.clone(),
                                            scope: "product_detail".into(),
                                            physical_page: Some(physical_page),
                                            url: Some(url.clone()),
                                            attempt,
                                            max_attempts: max_detail_retries_cfg,
                                            reason: None,
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    let shift = attempt - 1;
                                    let backoff_ms = 200u64 * (1u64 << shift);
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        backoff_ms + (u64::from(physical_page) % 29),
                                    ))
                                    .await;
                                }
                            }
                            if !success {
                                failed_c.fetch_add(1, Ordering::SeqCst);
                                page_failed += 1;
                            }
                        }
                    }
                }

                if (page_inserted + page_updated + page_skipped + page_failed) % 10 == 0 {
                    emit_actor_event(
                        &app,
                        AppEvent::SyncUpsertProgress {
                            session_id: session_id.clone(),
                            physical_page,
                            inserted: page_inserted,
                            updated: page_updated,
                            skipped: page_skipped,
                            failed: page_failed,
                            timestamp: Utc::now(),
                        },
                    );
                }
            }

            // Commit transaction
            if let Err(e) = tx.commit().await {
                page_failed += 1;
                failed_c.fetch_add(1, Ordering::SeqCst);
                emit_actor_event(
                    &app,
                    &AppEvent::SyncWarning {
                        session_id: session_id.clone(),
                        code: "tx_commit_failed".into(),
                        detail: format!("page {}: {}", physical_page, e),
                        timestamp: Utc::now(),
                    },
                );
            } else {
                let dur = tx_begin_instant.elapsed().as_millis();
                tracing::info!(target="write_lock_tracker", page=physical_page, duration_ms=%dur, guard_id=%tx_guard.id(), "sync_page_tx_committed");
            }

            let ms = u64::try_from(page_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            pages_processed_c.fetch_add(1, Ordering::SeqCst);
            emit_actor_event(
                &app,
                &AppEvent::SyncPageCompleted {
                    session_id: session_id.clone(),
                    physical_page,
                    inserted: page_inserted,
                    updated: page_updated,
                    skipped: page_skipped,
                    failed: page_failed,
                    ms,
                    timestamp: Utc::now(),
                },
            );
            debug!(
                "Sync page completed: p{} ins={} upd={} skip={} fail={} ({}ms)",
                physical_page, page_inserted, page_updated, page_skipped, page_failed, ms
            );
        });
        handles.push(handle);
    }

    // Await all page tasks
    for h in handles {
        let _ = h.await;
    }
    // Global safety sweep: backfill products.id across the DB (NULL/empty), regardless of page coverage
    if products_has_id_column {
        match sqlx::query(
            r"UPDATE products AS p
               SET id = CASE WHEN p.id IS NULL OR p.id = ''
                              THEN (SELECT d.id FROM product_details d WHERE d.url = p.url)
                              ELSE p.id END,
                   updated_at = CURRENT_TIMESTAMP
               WHERE p.id IS NULL OR p.id = ''",
        )
        .execute(&pool)
        .await
        {
            Ok(res) => {
                let affected = res.rows_affected();
                debug!(
                    "Global products.id backfill sweep affected {} rows",
                    affected
                );
                // Emit a lightweight event for FE visibility
                emit_actor_event(
                    &app,
                    &AppEvent::SyncWarning {
                        session_id: session_id.clone(),
                        code: "global_products_id_backfill_sweep".into(),
                        detail: format!("affected_rows={}", affected),
                        timestamp: Utc::now(),
                    },
                );
            }
            Err(e) => {
                emit_actor_event(
                    &app,
                    &AppEvent::SyncWarning {
                        session_id: session_id.clone(),
                        code: "global_products_id_backfill_failed".into(),
                        detail: format!("{}", e),
                        timestamp: Utc::now(),
                    },
                );
            }
        }
    }

    let pages_processed = pages_processed.load(Ordering::SeqCst);
    let inserted = inserted.load(Ordering::SeqCst);
    let updated = updated.load(Ordering::SeqCst);
    let skipped = skipped.load(Ordering::SeqCst);
    let failed = failed.load(Ordering::SeqCst);

    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    // Phase-2: bounded sweep for pages covered in this session
    // Only if not a dry_run and some pages were processed
    let mut deleted_total: u32 = 0;
    if !dry_run.unwrap_or(false) && pages_processed > 0 {
        // Merge and normalize ranges again for safety
    let sweep_ranges: Vec<(u32, u32)> = Vec::new();
        // parse_ranges removed (temporarily disabled)

        // Sweep only within ranges, but additionally limit to page_ids actually observed in this session,
        // and delete rows whose URL wasn't observed (URL-only match).
        for (start_oldest, end_newest) in sweep_ranges {
            let phys_start = start_oldest;
            let phys_end = end_newest;
            // Delete products within [e..s] whose url not observed in this session
            // Use page_id BETWEEN (total_pages - s) and (total_pages - e) mapping is not needed here
            // because we stored canonical page_id in observed during calculation.
            let pid_start = calculator.calculate(phys_start, 0).page_id;
            let pid_end = calculator.calculate(phys_end, 0).page_id;
            let low = pid_start.min(pid_end);
            let high = pid_start.max(pid_end);
            match sqlx::query(
                "DELETE FROM products p
                 WHERE p.page_id BETWEEN ? AND ?
                   AND p.page_id IN (
                       SELECT o.page_id FROM (
                           SELECT page_id, COUNT(*) AS cnt
                           FROM sync_observed
                           WHERE session_id = ?
                           GROUP BY page_id
                       ) o
                       WHERE o.cnt = 12
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM sync_observed o2
                       WHERE o2.session_id = ? AND o2.url = p.url
                   )",
            )
            .bind(low)
            .bind(high)
            .bind(&session_id)
            .bind(&session_id)
            .bind(&session_id)
            .execute(&pool)
            .await
            {
                Ok(res) => {
                    let affected = u32::try_from(res.rows_affected()).unwrap_or(u32::MAX);
                    if affected > 0 {
                        deleted_total = deleted_total.saturating_add(affected);
                        debug!(
                            "Sweep deleted {} rows in phys range {}-{} (pid {}-{})",
                            affected, phys_start, phys_end, low, high
                        );
                    }
                }
                Err(err) => {
                    emit_actor_event(
                        &app,
                        AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "sweep_failed".into(),
                            detail: format!(
                                "range {}-{} (pid {}-{}): {}",
                                phys_start, phys_end, low, high, err
                            ),
                            timestamp: Utc::now(),
                        },
                    );
                }
            }
        }
    }

    // Mark session completed
    if let Err(e) = sqlx::query(
        "UPDATE sync_sessions SET status='completed', finished_at=CURRENT_TIMESTAMP WHERE session_id = ?",
    )
    .bind(&session_id)
    .execute(&pool)
    .await
    {
        error!("Failed to mark sync session completed: {}", e);
    }

    // Build anomaly summary for observability (page_id groups with cnt != 12)
    let mut anomalies: Vec<SyncAnomalyEntry> = Vec::new();
    if let Ok(rows) = sqlx::query("WITH c AS (SELECT page_id, COUNT(*) AS cnt FROM products GROUP BY page_id) SELECT page_id, cnt FROM c WHERE cnt != 12 ORDER BY page_id")
        .fetch_all(&pool)
        .await
    {
        for r in rows {
            let pid: Option<i64> = r.try_get("page_id").ok();
            let cnt: Option<i64> = r.try_get("cnt").ok();
            if let (Some(page_id), Some(count)) = (pid, cnt) {
                // current physical page number = total_pages - page_id
                let current_page_number = total_pages.saturating_sub(u32::try_from(page_id).unwrap_or(0));
                anomalies.push(SyncAnomalyEntry {
                    page_id: i32::try_from(page_id).unwrap_or_default(),
                    count,
                    current_page_number,
                });
            }
        }
    }
    emit_actor_event(
        &app,
        &AppEvent::SyncCompleted {
            session_id: session_id.clone(),
            pages_processed,
            inserted,
            updated,
            skipped,
            failed,
            duration_ms,
            deleted: if deleted_total > 0 {
                Some(deleted_total)
            } else {
                None
            },
            total_pages: Some(total_pages),
            items_on_last_page: Some(u32::try_from(items_on_last_page).unwrap_or(0)),
            anomalies: if anomalies.is_empty() {
                None
            } else {
                Some(anomalies)
            },
            timestamp: Utc::now(),
        },
    );
    info!(
        "Sync completed: session_id={} pages={} ins={} upd={} skip={} fail={} duration_ms={}",
        session_id, pages_processed, inserted, updated, skipped, failed, duration_ms
    );
    Ok(SyncSummary {
        pages_processed,
        inserted,
        updated,
        skipped,
        failed,
        duration_ms,
    })
}

/// Run sync for an explicit set of physical page numbers.
/// This builds a comma-separated list of single-page ranges (no merges)
/// to avoid policy-based span clamping in partial sync, then delegates to `start_partial_sync`.
#[tauri::command(async)]
/// # Errors
/// Returns an error string if the `pages` list is empty or if the delegated partial sync fails.
pub async fn start_sync_pages(
    app: AppHandle,
    app_state: State<'_, AppState>,
    mut pages: Vec<u32>,
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    if pages.is_empty() {
        return Err("No pages provided".into());
    }
    // Deduplicate and sort descending (newest first, consistent with ranges parse ordering)
    pages.sort_unstable();
    pages.dedup();
    pages.reverse();
    // parse_ranges removed in diagnostic refactor; sweep disabled here.
    return Err("Partial sync path temporarily unavailable".into());
}

/// Run a diagnostic-driven sync for specific pages and slot indices.
/// Only the specified indices on each page will be processed (precise repair).
#[tauri::command(async)]
/// # Errors
/// Returns an error string if no diagnostic pages were provided or all miss_indices are empty.
pub async fn start_diagnostic_sync(
    app: AppHandle,
    app_state: State<'_, AppState>,
    pages: Vec<DiagnosticPageInput>,
    snapshot: Option<DiagnosticSnapshotInput>,
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    if pages.is_empty() {
        return Err("No diagnostic pages provided".into());
    }

    // Build page -> indices map and a sorted page list (desc)
    let mut index_map: HashMap<u32, HashSet<usize>> = HashMap::new();
    for p in pages {
        let set: HashSet<usize> = p.miss_indices.into_iter().map(|v| v as usize).collect();
        if !set.is_empty() {
            index_map.insert(p.physical_page, set);
        }
    }
    if index_map.is_empty() {
        return Err("All diagnostic pages had empty miss_indices".into());
    }
    let mut pages_vec: Vec<u32> = index_map.keys().copied().collect();
    pages_vec.sort_unstable();
    pages_vec.dedup();
    pages_vec.reverse();

    // Load infra via shared AppState (DI)
    let app_config = app_state.config.read().await.clone();
    let http = app_state.get_http_client().await?;
    let sync_ua = app_config.user.crawling.workers.user_agent_sync.clone();
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Discover or use snapshot for site meta
    let (total_pages, items_on_last_page, newest_html, oldest_html, oldest_page) =
        if let Some(s) = snapshot {
            // Use provided snapshot only; avoid extra network calls for edges in precise-repair mode
            let newest_html = String::new();
            let oldest_html = String::new();
            let oldest_page = s.total_pages;
            (
                s.total_pages,
                s.items_on_last_page as usize,
                newest_html,
                oldest_html,
                oldest_page,
            )
        } else {
            let newest_url = csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
            let newest_html = match http
                .fetch_response_with_options(
                    &newest_url,
                    &RequestOptions {
                        user_agent_override: sync_ua.clone(),
                        referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                        skip_robots_check: false,
                        attempt: None,
                        max_attempts: None,
                    },
                )
                .await
            {
                Ok(resp) => resp.text().await.map_err(|e| e.to_string())?,
                Err(e) => return Err(e.to_string()),
            };
            let total_pages = extractor
                .extract_total_pages(&newest_html)
                .unwrap_or(1)
                .max(1);
            let oldest_page = total_pages;
            let oldest_html = if oldest_page == 1 {
                newest_html.clone()
            } else {
                let oldest_url =
                    csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &oldest_page.to_string());
                match http
                    .fetch_response_with_options(
                        &oldest_url,
                        &RequestOptions {
                            user_agent_override: sync_ua.clone(),
                            referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                            skip_robots_check: false,
                            attempt: None,
                            max_attempts: None,
                        },
                    )
                    .await
                {
                    Ok(resp) => resp.text().await.map_err(|e| e.to_string())?,
                    Err(e) => return Err(e.to_string()),
                }
            };
            let items_on_last_page = extractor
                .extract_product_urls_from_content(&oldest_html)
                .map_err(|e| e.to_string())?
                .len();
            (
                total_pages,
                items_on_last_page,
                newest_html,
                oldest_html,
                oldest_page,
            )
        };
    let calculator = CanonicalPageIdCalculator::new(total_pages, items_on_last_page);

    // Emit start event
    let session_id = format!("sync-{}", Utc::now().format("%Y%m%d%H%M%S"));
    emit_actor_event(
        &app,
        AppEvent::SyncStarted {
            session_id: session_id.clone(),
            ranges: pages_vec.iter().map(|p| (*p, *p)).collect(),
            rate_limit: Some(app_config.user.crawling.workers.max_requests_per_second),
            timestamp: Utc::now(),
        },
    );

    let max_concurrent = app_config
        .user
        .crawling
        .workers
        .list_page_max_concurrent
        .max(1);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let pages_processed = Arc::new(AtomicU32::new(0));
    let inserted = Arc::new(AtomicU32::new(0));
    let updated = Arc::new(AtomicU32::new(0));
    let skipped = Arc::new(AtomicU32::new(0));
    let failed = Arc::new(AtomicU32::new(0));

    let app_handle = app.clone();
    let pool_arc = pool.clone();
    let http_client = http.clone();
    let extractor_global = extractor.clone();
    let calculator_global = calculator.clone();
    let dry = dry_run.unwrap_or(false);

    let diag_start = std::time::Instant::now();
    let mut handles = Vec::with_capacity(pages_vec.len());
    for physical_page in pages_vec {
        let selected = index_map.get(&physical_page).cloned().unwrap_or_default();
        if selected.is_empty() {
            continue;
        }
        let permit = semaphore.clone().acquire_owned();
        let app = app_handle.clone();
        let session_id = session_id.clone();
        let pool = pool_arc.clone();
        let http = http_client.clone();
        let extractor = extractor_global.clone();
        let calculator = calculator_global.clone();
        let newest_html_clone = newest_html.clone();
        let oldest_html_clone = oldest_html.clone();
        let sync_ua = sync_ua.clone();
        let pages_processed_c = pages_processed.clone();
        let inserted_c = inserted.clone();
        let updated_c = updated.clone();
        let skipped_c = skipped.clone();
        let failed_c = failed.clone();
        let is_dry_run = dry_run.unwrap_or(false);
    let max_list_retries = app_config.user.crawling.product_list_retry_count.max(1);
    let _max_detail_retries_cfg = app_config.user.crawling.product_detail_retry_count.max(1);

        let handle = tokio::spawn(async move {
            // Acquire concurrency slot
            let _permit = match permit.await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to acquire semaphore: {}", e);
                    return;
                }
            };

            let page_start = std::time::Instant::now();
            emit_actor_event(
                &app,
                AppEvent::SyncPageStarted {
                    session_id: session_id.clone(),
                    physical_page,
                    timestamp: Utc::now(),
                },
            );

            // Fetch + parse with retries if count mismatch or transient errors
            let expected_count = if physical_page == oldest_page {
                u32::try_from(items_on_last_page).unwrap_or(u32::MAX)
            } else {
                12u32
            };
            // Align sync retry attempts with ListCrawling settings
            let max_retries = max_list_retries; // total attempts = 1 + max_retries
            // Observability: log per-page retry config
            info!(target: "kpi.sync", "{{\"event\":\"sync_retry_config\",\"session_id\":\"{}\",\"page\":{},\"max_retries\":{}}}", session_id, physical_page, max_retries);
            let mut attempt = 0u32;
            let mut product_urls: Vec<String> = Vec::new();
            let mut last_err_msg: Option<String> = None;
            loop {
                // Choose source: first attempt can reuse cached for edges; retries always fetch fresh
                let use_cache =
                    attempt == 0 && (physical_page == oldest_page || physical_page == 1);
                let page_html = if use_cache {
                    if physical_page == oldest_page {
                        oldest_html_clone.clone()
                    } else {
                        newest_html_clone.clone()
                    }
                } else {
                    let url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
                        .replace("{}", &physical_page.to_string());
                    // Convey attempt/max to HttpClient for improved logging
                    match http
                        .fetch_response_with_options(
                            &url,
                            &RequestOptions {
                                user_agent_override: sync_ua.clone(),
                                referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                                skip_robots_check: false,
                                attempt: Some(std::cmp::max(1, attempt + 1)),
                                max_attempts: Some(std::cmp::max(1, max_retries + 1)),
                            },
                        )
                        .await
                    {
                        Ok(resp) => match resp.text().await {
                            Ok(t) => t,
                            Err(e) => {
                                last_err_msg = Some(format!("read_body_failed: {}", e));
                                // fall through to retry decision
                                String::new()
                            }
                        },
                        Err(e) => {
                            last_err_msg = Some(format!("fetch_failed: {}", e));
                            String::new()
                        }
                    }
                };

                if page_html.is_empty() {
                    // fetch/read failed
                } else {
                    match extractor.extract_product_urls_from_content(&page_html) {
                        Ok(v) => {
                            product_urls = v;
                            if u32::try_from(product_urls.len()).ok() == Some(expected_count) {
                                // success; no need to reset last_err_msg explicitly
                                // success
                                break;
                            }
                            last_err_msg = Some(format!(
                                "count_mismatch: expected {} got {}",
                                expected_count,
                                product_urls.len()
                            ));
                        }
                        Err(e) => {
                            last_err_msg = Some(format!("parse_failed: {}", e));
                        }
                    }
                }

                if attempt >= max_retries {
                    // Give up, emit warning and proceed with what we have (possibly empty/partial)
                    if let Some(msg) = &last_err_msg {
                        emit_actor_event(
                            &app,
                            &AppEvent::SyncWarning {
                                session_id: session_id.clone(),
                                code: "page_incomplete_after_retries".into(),
                                detail: format!(
                                    "page {}: {} after {} retries",
                                    physical_page, msg, attempt
                                ),
                                timestamp: Utc::now(),
                            },
                        );
                    }
                    break;
                }

                // Observability: log + emit retry attempt with last reason if any
                if let Some(msg) = &last_err_msg {
                    info!(target: "kpi.sync", "{{\"event\":\"retry_attempt\",\"session_id\":\"{}\",\"page\":{},\"attempt\":{},\"max_retries\":{},\"reason\":\"{}\"}}", session_id, physical_page, attempt + 1, max_retries, msg);
                    // appended closure & backoff
                    let shift = attempt.min(6);
                    let delay_ms = 200u64 * (1u64 << shift);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                attempt += 1;
            } // end retry loop
            pages_processed_c.fetch_add(1, Ordering::SeqCst);
            if product_urls.is_empty() && last_err_msg.is_some() {
                failed_c.fetch_add(1, Ordering::SeqCst);
            }
            let ms = u64::try_from(page_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            emit_actor_event(
                &app,
                AppEvent::SyncPageCompleted {
                    session_id: session_id.clone(),
                    physical_page,
                    inserted: 0,
                    updated: 0,
                    skipped: 0,
                    failed: if product_urls.is_empty() { 1 } else { 0 },
                    ms,
                    timestamp: Utc::now(),
                },
            );
        });
        handles.push(handle);
    } // end for pages
    for h in handles { let _ = h.await; }
    let summary = SyncSummary {
        pages_processed: pages_processed.load(Ordering::SeqCst),
        inserted: inserted.load(Ordering::SeqCst),
        updated: updated.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        duration_ms: 0,
    };
    emit_actor_event(
        &app,
        AppEvent::SyncCompleted {
            session_id: session_id.clone(),
            pages_processed: summary.pages_processed,
            inserted: summary.inserted,
            updated: summary.updated,
            skipped: summary.skipped,
            failed: summary.failed,
            duration_ms: summary.duration_ms,
            deleted: None,
            total_pages: None,
            items_on_last_page: None,
            anomalies: None,
            timestamp: Utc::now(),
        }
    );
    Ok(summary)
} // end start_diagnostic_sync