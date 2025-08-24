use crate::application::AppState;
use crate::crawl_engine::actors::types::{AppEvent, SyncAnomalyEntry};
use crate::domain::pagination::CanonicalPageIdCalculator;
use crate::infrastructure::{
    config::csa_iot,
    html_parser::MatterDataExtractor,
    simple_http_client::RequestOptions,
    BatchCrawlingConfig,
    BatchCrawlingEngine,
    IntegratedProductRepository,
};
use chrono::Utc;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, trace};
use scraper::Html;

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

/// Run the basic 4-stage crawling engine for an explicit set of physical page numbers
/// using the new page_filter path (avoids delegating to partial sync).
#[tauri::command(async)]
pub async fn start_basic_sync_pages(
    app: AppHandle,
    app_state: State<'_, AppState>,
    mut pages: Vec<u32>,
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
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
    let products_has_id_column: bool = match sqlx::query("PRAGMA table_info(products)")
        .fetch_all(&pool)
        .await
    {
        Ok(cols) => cols.iter().any(|r| {
            let name: String = r.try_get("name").unwrap_or_default();
            name == "id"
        }),
        Err(_) => false,
    };

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
        let oldest_url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &oldest_page.to_string());
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
        AppEvent::SyncStarted {
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
        let sync_ua_cloned = sync_ua.clone();
        let pages_processed_c = pages_processed.clone();
        let inserted_c = inserted.clone();
        let updated_c = updated.clone();
        let skipped_c = skipped.clone();
        let failed_c = failed.clone();
        let products_has_id_col = products_has_id_column;
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
                AppEvent::SyncPageStarted {
                    session_id: session_id.clone(),
                    physical_page,
                    timestamp: Utc::now(),
                },
            );

            // Fetch + parse product list with retries
            let expected_count = if physical_page == oldest_page { items_on_last_page as u32 } else { 12u32 };
            let mut attempt = 0u32;
            let mut product_urls: Vec<String> = Vec::new();
            let mut last_err_msg: Option<String> = None;
            loop {
                let use_cache = attempt == 0 && (physical_page == oldest_page || physical_page == 1);
                let page_html = if use_cache {
                    if physical_page == oldest_page { oldest_html_clone.clone() } else { newest_html_clone.clone() }
                } else {
                    let url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &physical_page.to_string());
                    match http
                        .fetch_response_with_options(
                            &url,
                            &RequestOptions {
                                user_agent_override: sync_ua_cloned.clone(),
                                referer: Some(csa_iot::PRODUCTS_BASE.to_string()),
                                skip_robots_check: false,
                                attempt: Some(std::cmp::max(1, attempt + 1)),
                                max_attempts: Some(std::cmp::max(1, max_list_retries + 1)),
                            },
                        )
                        .await
                    {
                        Ok(resp) => match resp.text().await { Ok(t) => t, Err(e) => { last_err_msg = Some(format!("read_body_failed: {}", e)); String::new() } },
                        Err(e) => { last_err_msg = Some(format!("fetch_failed: {}", e)); String::new() }
                    }
                };

                if !page_html.is_empty() {
                    match extractor.extract_product_urls_from_content(&page_html) {
                        Ok(v) => {
                            product_urls = v;
                            if product_urls.len() as u32 == expected_count { break; } else {
                                last_err_msg = Some(format!("count_mismatch: expected {} got {}", expected_count, product_urls.len()));
                            }
                        }
                        Err(e) => { last_err_msg = Some(format!("parse_failed: {}", e)); }
                    }
                }

                if attempt >= max_list_retries { break; }
                // Emit retrying event
                emit_actor_event(
                    &app,
                    AppEvent::SyncRetrying {
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
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms + (physical_page as u64 % 37))).await;
                attempt += 1;
            }

            if product_urls.len() as u32 != expected_count {
                if let Some(msg) = &last_err_msg {
                    emit_actor_event(
                        &app,
                        AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "count_mismatch".into(),
                            detail: format!("page {}: {} (got {} of {})", physical_page, msg, product_urls.len(), expected_count),
                            timestamp: Utc::now(),
                        },
                    );
                }
            }

            // Transaction per page
            let mut tx = match pool.begin().await {
                Ok(t) => t,
                Err(e) => {
                    failed_c.fetch_add(product_urls.len() as u32, Ordering::SeqCst);
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
                    emit_actor_event(&app, AppEvent::SyncUpsertProgress { session_id: session_id.clone(), physical_page, inserted: page_inserted, updated: page_updated, skipped: page_skipped, failed: page_failed, timestamp: Utc::now() });
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

                let row = match sqlx::query("SELECT page_id, index_in_page FROM products WHERE url = ? LIMIT 1")
                    .bind(url)
                    .fetch_optional(&mut *tx)
                    .await {
                        Ok(r) => r,
                        Err(e) => { page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "select_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); continue; }
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
                                    Ok(_) => { page_inserted += 1; inserted_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_inserted".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); },
                                    Err(e) => { page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "insert_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); continue; }
                                }
                        }
                        // Ensure product_details placeholder with synthetic id
                        let synthetic_id = format!("p{:04}i{:02}", calc.page_id, calc.index_in_page);
                        let _ = sqlx::query(
                            r#"INSERT INTO product_details (url, page_id, index_in_page, id)
                                    VALUES (?, ?, ?, ?)
                                    ON CONFLICT(url) DO UPDATE SET
                                        page_id = COALESCE(excluded.page_id, product_details.page_id),
                                        index_in_page = COALESCE(excluded.index_in_page, product_details.index_in_page),
                                        id = COALESCE(product_details.id, excluded.id),
                                        updated_at = CURRENT_TIMESTAMP"#,
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
                        let needs_update = match (db_pid, db_idx) { (Some(p), Some(ix)) => p as i32 != calc.page_id || ix as i32 != calc.index_in_page, _ => true };
                        if needs_update {
                            match sqlx::query("UPDATE products SET page_id = ?, index_in_page = ?, updated_at = CURRENT_TIMESTAMP WHERE url = ?")
                                .bind(calc.page_id)
                                .bind(calc.index_in_page)
                                .bind(url)
                                .execute(&mut *tx)
                                .await {
                                    Ok(_) => { page_updated += 1; updated_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_updated".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); },
                                    Err(e) => { page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "update_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); emit_actor_event(&app, AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_update_failed".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); }
                                }
                        } else { page_skipped += 1; skipped_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: None, page_number: Some(physical_page), product_ref: url.clone(), status: "product_skipped_nochange".into(), retry: None, duration_ms: None, metrics: None, timestamp: Utc::now() }); }

                        // Keep details in sync and ensure id if missing
                        let synthetic_id = format!("p{:04}i{:02}", calc.page_id, calc.index_in_page);
                        let _ = sqlx::query(
                            r#"INSERT INTO product_details (url, page_id, index_in_page, id)
                                    VALUES (?, ?, ?, ?)
                                    ON CONFLICT(url) DO UPDATE SET
                                        page_id = COALESCE(excluded.page_id, product_details.page_id),
                                        index_in_page = COALESCE(excluded.index_in_page, product_details.index_in_page),
                                        id = COALESCE(product_details.id, excluded.id),
                                        updated_at = CURRENT_TIMESTAMP"#,
                        )
                        .bind(url)
                        .bind(calc.page_id)
                        .bind(calc.index_in_page)
                        .bind(synthetic_id)
                        .execute(&mut *tx)
                        .await;

                        // If details missing, try fetch with retries
                        let details_missing = match sqlx::query_scalar::<_, i64>("SELECT 1 FROM product_details WHERE url = ? LIMIT 1")
                            .bind(url)
                            .fetch_optional(&mut *tx)
                            .await { Ok(opt) => opt.is_none(), Err(_) => false };
                        if details_missing && !is_dry_run {
                            let mut success = false;
                            for attempt in 1..=max_detail_retries_cfg {
                                let referer_url = if physical_page == 1 { csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string() } else { csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &physical_page.to_string()) };
                                match http
                                    .fetch_response_with_options(
                                        url,
                                        &RequestOptions { user_agent_override: sync_ua_cloned.clone(), referer: Some(referer_url), skip_robots_check: false, attempt: Some(attempt), max_attempts: Some(max_detail_retries_cfg) },
                                    )
                                    .await {
                                        Ok(resp) => match resp.text().await { Ok(body) => {
                                            let extracted = { let doc = Html::parse_document(&body); extractor.extract_product_detail(&doc, url.clone()) };
                                            if let Ok(mut detail) = extracted {
                                                detail.page_id = Some(calc.page_id);
                                                detail.index_in_page = Some(calc.index_in_page);
                                                if detail.id.is_none() { detail.id = Some(format!("p{:04}i{:02}", calc.page_id, calc.index_in_page)); }
                                                let program_type = Some(detail.program_type.unwrap_or_else(|| "Matter".to_string()));
                                                // clone fields for backfill
                                                let man_c = detail.manufacturer.clone();
                                                let model_c = detail.model.clone();
                                                let cert_c = detail.certificate_id.clone();
                                                if sqlx::query(
                                                    r#"INSERT INTO product_details (
                                                        url, page_id, index_in_page, id, manufacturer, model, device_type,
                                                        certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                                        specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                                        tis_trp_tested, transport_interface, primary_device_type_id, application_categories,
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
                                                        primary_device_type_id=COALESCE(excluded.primary_device_type_id, product_details.primary_device_type_id),
                                                        application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                                        description=COALESCE(excluded.description, product_details.description),
                                                        compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                                        program_type=COALESCE(excluded.program_type, product_details.program_type),
                                                        updated_at=CURRENT_TIMESTAMP
                                                "#,
                                                )
                                                .bind(&detail.url)
                                                .bind(detail.page_id)
                                                .bind(detail.index_in_page)
                                                .bind(detail.id.clone())
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
                                                .bind(detail.primary_device_type_id)
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
                                                        r#"UPDATE products SET
                                                            manufacturer = COALESCE(?, manufacturer),
                                                            model = COALESCE(?, model),
                                                            certificate_id = COALESCE(?, certificate_id),
                                                            updated_at = CURRENT_TIMESTAMP
                                                        WHERE url = ?"#,
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
                                                            r#"UPDATE products SET id = CASE WHEN id IS NULL OR id = '' THEN ? ELSE id END WHERE url = ?"#,
                                                        )
                                                        .bind(&detail.id)
                                                        .bind(&detail.url)
                                                        .execute(&mut *tx)
                                                        .await;
                                                    }
                                                    success = true;
                                                    break;
                                                }
                                            }
                                        }, Err(_) => { /* read failed */ } },
                                        Err(_) => { /* fetch failed */ }
                                    }
                                if attempt < max_detail_retries_cfg && !success {
                                    emit_actor_event(
                                        &app,
                                        AppEvent::SyncRetrying { session_id: session_id.clone(), scope: "product_detail".into(), physical_page: Some(physical_page), url: Some(url.clone()), attempt, max_attempts: max_detail_retries_cfg, reason: None, timestamp: Utc::now() },
                                    );
                                    let shift = attempt - 1; let backoff_ms = 200u64 * (1u64 << shift);
                                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms + (physical_page as u64 % 29))).await;
                                }
                            }
                            if !success { failed_c.fetch_add(1, Ordering::SeqCst); page_failed += 1; }
                        }
                    }
                }

                if (page_inserted + page_updated + page_skipped + page_failed) % 10 == 0 {
                    emit_actor_event(&app, AppEvent::SyncUpsertProgress { session_id: session_id.clone(), physical_page, inserted: page_inserted, updated: page_updated, skipped: page_skipped, failed: page_failed, timestamp: Utc::now() });
                }
            }

            // Page-level DB-only alignments and commit
            let canonical_pid = calculator.calculate(physical_page, 0).page_id;
            // Ensure details placeholders for all products on this page
            let _ = sqlx::query(
                r#"INSERT INTO product_details (url, page_id, index_in_page, id)
                    SELECT p.url, p.page_id, p.index_in_page, printf('p%04di%02d', COALESCE(p.page_id, 0), COALESCE(p.index_in_page, 0)) as id
                    FROM products p
                    WHERE p.page_id = ?
                ON CONFLICT(url) DO UPDATE SET
                    page_id = COALESCE(product_details.page_id, excluded.page_id),
                    index_in_page = COALESCE(product_details.index_in_page, excluded.index_in_page),
                    id = COALESCE(product_details.id, excluded.id),
                    updated_at = CURRENT_TIMESTAMP"#,
            )
            .bind(canonical_pid)
            .execute(&mut *tx)
            .await;

            if let Err(e) = tx.commit().await {
                page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst);
                emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "tx_commit_failed".into(), detail: format!("page {}: {}", physical_page, e), timestamp: Utc::now() });
            }

            let ms = page_start.elapsed().as_millis() as u64;
            pages_processed_c.fetch_add(1, Ordering::SeqCst);
            emit_actor_event(
                &app,
                AppEvent::SyncPageCompleted { session_id: session_id.clone(), physical_page, inserted: page_inserted, updated: page_updated, skipped: page_skipped, failed: page_failed, ms, timestamp: Utc::now() },
            );
        });
        handles.push(handle);
    }

    for h in handles { let _ = h.await; }

    let duration_ms = started.elapsed().as_millis() as u64;
    let summary = SyncSummary {
        pages_processed: pages_processed.load(Ordering::SeqCst),
        inserted: inserted.load(Ordering::SeqCst),
        updated: updated.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        duration_ms,
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
            total_pages: Some(total_pages),
            items_on_last_page: Some(items_on_last_page as u32),
            anomalies: None,
            timestamp: Utc::now(),
        },
    );

    Ok(summary)
}

/// Run partial sync in sequential batches of contiguous pages.
/// Temporarily simplified to delegate to start_partial_sync without batching logic.
#[tauri::command(async)]
pub async fn start_batched_sync(
    app: AppHandle,
    app_state: State<'_, AppState>,
    ranges: String,
    _batch_size_override: Option<u32>,
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    // If no explicit ranges, keep existing policy by delegating directly (default span inside partial_sync)
    if ranges.trim().is_empty() {
        return start_partial_sync(app, app_state, ranges, dry_run).await;
    }

    // Resolve batch size: override > config > sane default
    let app_cfg = app_state.config.read().await.clone();
    let cfg_batch = app_cfg.user.batch.batch_size.max(1);
    let batch_size = _batch_size_override.unwrap_or(cfg_batch).max(1);

    // Expand ranges into distinct physical pages (desc), then chunk
    let mut pages: Vec<u32> = Vec::new();
    for (s, e) in parse_ranges(&ranges)? {
        // physical pages are descending: s (older) .. e (newer)
        for p in (e..=s).rev() {
            pages.push(p);
        }
    }
    pages.sort_unstable();
    pages.dedup();
    pages.reverse();

    if pages.is_empty() {
        return Err("No pages to sync after parsing ranges".into());
    }

    // Prepare aggregate summary
    let started = std::time::Instant::now();
    let mut agg = SyncSummary {
        pages_processed: 0,
        inserted: 0,
        updated: 0,
        skipped: 0,
        failed: 0,
        duration_ms: 0,
    };

    // Run batches sequentially to reduce contention and simplify observability
    let mut idx = 0usize;
    while idx < pages.len() {
        let end = (idx + batch_size as usize).min(pages.len());
        let batch_expr = pages[idx..end]
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let res = start_partial_sync(app.clone(), app_state.clone(), batch_expr, dry_run).await?;
        agg.pages_processed = agg.pages_processed.saturating_add(res.pages_processed);
        agg.inserted = agg.inserted.saturating_add(res.inserted);
        agg.updated = agg.updated.saturating_add(res.updated);
        agg.skipped = agg.skipped.saturating_add(res.skipped);
        agg.failed = agg.failed.saturating_add(res.failed);
        idx = end;
    }

    agg.duration_ms = started.elapsed().as_millis() as u64;
    Ok(agg)
}

/// Compute anomaly-driven buffered windows and run partial sync.
/// Temporarily disabled; returns an error for now.
#[tauri::command(async)]
pub async fn start_repair_sync(
    app: AppHandle,
    app_state: State<'_, AppState>,
    buffer: Option<u32>,
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    // 1) Discover site meta (same approach as partial sync)
    let app_config = app_state.config.read().await.clone();
    let http = app_state.get_http_client().await?;
    let sync_ua = app_config.user.crawling.workers.user_agent_sync.clone();
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Detect schema compatibility for this DB: does products have an 'id' column?
    let products_has_id_column: bool = match sqlx::query("PRAGMA table_info(products)")
        .fetch_all(&pool)
        .await
    {
        Ok(cols) => cols.iter().any(|r| {
            let name: String = r.try_get("name").unwrap_or_default();
            name == "id"
        }),
        Err(_) => false,
    };
    // (deduped id-column detection)

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

    // 2) Collect anomaly centers from DB (groups with count != 12)
    let mut centers: Vec<u32> = Vec::new();
    if let Ok(rows) = sqlx::query(
        "WITH c AS (SELECT page_id, COUNT(*) AS cnt FROM products GROUP BY page_id) SELECT page_id, cnt FROM c WHERE cnt != 12 ORDER BY page_id",
    )
    .fetch_all(&pool)
    .await
    {
        for r in rows {
            let pid: Option<i64> = r.try_get("page_id").ok();
            if let Some(page_id) = pid {
                // current physical page number = total_pages - page_id
                let physical = total_pages.saturating_sub(page_id as u32);
                if physical >= 1 && physical <= total_pages {
                    centers.push(physical);
                }
            }
        }
    }

    if centers.is_empty() {
        return Ok(SyncSummary {
            pages_processed: 0,
            inserted: 0,
            updated: 0,
            skipped: 0,
            failed: 0,
            duration_ms: 0,
        });
    }

    // 3) Build buffered windows around centers and merge
    let b = buffer.unwrap_or(2);
    let mut windows: Vec<(u32, u32)> = Vec::new();
    for p in centers {
        let start_oldest = (p + b).min(total_pages);
        let end_newest = p.saturating_sub(b).max(1);
        let mut s = start_oldest;
        let mut e = end_newest;
        if s < e {
            std::mem::swap(&mut s, &mut e);
        }
        windows.push((s, e));
    }
    // Merge overlaps: sort desc by start, then coalesce
    windows.sort_by(|(s1, e1), (s2, e2)| s2.cmp(s1).then(e2.cmp(e1)));
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (s, e) in windows.into_iter() {
        if let Some((ls, le)) = merged.last_mut() {
            if *le <= s + 1 && e <= *ls {
                *le = (*le).min(e);
                *ls = (*ls).max(s);
                continue;
            }
        }
        merged.push((s, e));
    }

    // 4) Delegate to partial sync with merged ranges
    let expr = merged
        .iter()
        .map(|(s, e)| if s == e { s.to_string() } else { format!("{}-{}", s, e) })
        .collect::<Vec<_>>()
        .join(",");
    start_partial_sync(app, app_state, expr, dry_run).await
}

/// Diagnostic input: specific pages and slot indices to repair
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

fn parse_ranges(expr: &str) -> Result<Vec<(u32, u32)>, String> {
    // "498-492,489,487-485" or with tildes/Unicode -> vec![(498,492),(489,489),(487,485)]
    let norm_all = expr
        .replace(char::is_whitespace, "")
        .replace('–', "-")
        .replace('—', "-")
        .replace('−', "-")
        .replace('﹣', "-")
        .replace('－', "-")
        .replace('〜', "~")
        .replace('～', "~");
    let mut out: Vec<(u32, u32)> = Vec::new();
    for token in norm_all
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let sep = if token.contains('~') { '~' } else { '-' };
        if let Some((a, b)) = token.split_once(sep) {
            let mut s: u32 = a.parse().map_err(|_| format!("Invalid number: {}", a))?;
            let mut e: u32 = b.parse().map_err(|_| format!("Invalid number: {}", b))?;
            if e > s {
                std::mem::swap(&mut s, &mut e);
            } // ensure s>=e
            out.push((s, e));
        } else {
            let v: u32 = token
                .parse()
                .map_err(|_| format!("Invalid number: {}", token))?;
            out.push((v, v));
        }
    }
    // sort desc by start, then merge overlaps/adjacent
    out.sort_by(|(s1, e1), (s2, e2)| s2.cmp(s1).then(e2.cmp(e1)));
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (s, e) in out.into_iter() {
        if let Some((ls, le)) = merged.last_mut() {
            if *le <= s + 1 && e <= *ls {
                // overlapping or adjacent and ordered
                *le = (*le).min(e);
                *ls = (*ls).max(s);
                continue;
            }
        }
        merged.push((s, e));
    }
    Ok(merged)
}

#[tauri::command(async)]
pub async fn start_partial_sync(
    app: AppHandle,
    app_state: State<'_, AppState>,
    ranges: String, // e.g., "498-492,489,487-485"
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    let session_id = format!("sync-{}", Utc::now().format("%Y%m%d%H%M%S"));
    let started = std::time::Instant::now();
    info!(
        "start_partial_sync args: ranges=\"{}\" dry_run={:?}",
        ranges, dry_run
    );
    let mut ranges = parse_ranges(&ranges)?;

    // Emit a preflight start event immediately so the UI reacts without waiting for network or DB
    emit_actor_event(
        &app,
        AppEvent::SyncStarted {
            session_id: session_id.clone(),
            ranges: ranges.clone(),
            rate_limit: None,
            timestamp: Utc::now(),
        },
    );
    info!("Sync preflight: session_id={} ranges={:?} dry_run={}", session_id, ranges, dry_run.unwrap_or(false));

    // Use shared AppConfig and HttpClient from AppState (DI)
    let app_config = app_state.config.read().await.clone();
    let http = app_state.get_http_client().await?;
    let sync_ua = app_config.user.crawling.workers.user_agent_sync.clone();
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // (rate_limit will be included in subsequent events if needed)

    // start_partial_sync: Detect if products table has an 'id' column (legacy/production schema)
    let products_has_id_column: bool = match sqlx::query("PRAGMA table_info(products)")
        .fetch_all(&pool)
        .await
    {
        Ok(cols) => cols.iter().any(|r| {
            let name: String = r.try_get("name").unwrap_or_default();
            name == "id"
        }),
        Err(_) => false,
    };

    // Discover site meta for calculator
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

    info!(
        "Sync site meta: total_pages={} items_on_last_page={}",
        total_pages, items_on_last_page
    );

    // Determine effective page span limit based on conditional policy
    // - If no explicit ranges provided: default span limit = 50 pages
    // - If explicit ranges provided: span limit = floor(local DB product count / 12)
    let limit: u32 = if ranges.is_empty() {
        let default_limit = 50u32;
        let end_newest = total_pages.saturating_sub(default_limit - 1).max(1);
        ranges = vec![(total_pages, end_newest)];
        info!(
            "Using default sync range (no explicit ranges): {} -> {} (span={})",
            total_pages, end_newest, default_limit
        );
        default_limit
    } else {
        let total_products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);
        let pages_from_count = ((total_products as u32) / 12).max(1);
        info!(
            "Using conditional sync span limit from DB: products={} => pages={} (floor(/12))",
            total_products, pages_from_count
        );
        pages_from_count
    };

    // Clamp each range to site bounds and effective span limit
    {
    let original = ranges.clone();
        for r in ranges.iter_mut() {
            let (mut s, mut e) = *r;
            let before = (s, e);
            if s > total_pages {
                s = total_pages;
            }
            if e > total_pages {
                e = total_pages;
            }
            if s < e {
                std::mem::swap(&mut s, &mut e);
            }
            let span = s.saturating_sub(e) + 1;
            if span > limit {
                let new_e = s.saturating_sub(limit - 1);
                info!(
                    "Clamping sync span from {} to {} by effective policy limit={}, range {}->{}, new {}->{}",
                    span, limit, limit, before.0, before.1, s, new_e
                );
                e = new_e.max(1);
            }
            if (s, e) != before {
                info!(
                    "Sync range adjusted: {}->{} => {}->{} (site bounds/limit)",
                    before.0, before.1, s, e
                );
            }
            *r = (s, e);
        }
        if ranges != original {
            info!("Resolved sync ranges after clamping: {:?}", ranges);
        } else {
            info!("Resolved sync ranges: {:?}", ranges);
        }
    }

    // Persist final coverage_text after clamping/defaulting
    let coverage_text = if ranges.is_empty() {
        String::new()
    } else {
        ranges
            .iter()
            .map(|(s, e)| {
                if s == e {
                    s.to_string()
                } else {
                    format!("{}-{}", s, e)
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    if let Err(e) = sqlx::query("UPDATE sync_sessions SET coverage_text = ? WHERE session_id = ?")
        .bind(&coverage_text)
        .bind(&session_id)
        .execute(&pool)
        .await
    {
        error!("Failed to update sync session coverage: {}", e);
    }

    // Record session start in DB (idempotent upsert by primary key)
    if let Err(e) = sqlx::query(
        "INSERT INTO sync_sessions(session_id, status, coverage_text, started_at) VALUES(?, 'running', ?, CURRENT_TIMESTAMP)
         ON CONFLICT(session_id) DO UPDATE SET status='running', coverage_text=excluded.coverage_text, started_at=excluded.started_at, finished_at=NULL",
    )
    .bind(&session_id)
    .bind(match ranges.as_slice() {
        rs if rs.is_empty() => String::new(),
        rs => rs
            .iter()
            .map(|(s, e)| if s == e { s.to_string() } else { format!("{}-{}", s, e) })
            .collect::<Vec<_>>()
            .join(","),
    })
    .execute(&pool)
    .await
    {
        error!("Failed to record sync session start: {}", e);
    }

    // Note: detailed "Sync started" log was moved to preflight above

    // Prepare bounded-concurrency processing for all pages across ranges
    // 경계 보정 확장: 각 지정 범위마다 양방향 이웃 페이지를 함께 포함해 캐논िकल 페이지 경계 누락을 방지
    // - 오래된 쪽 이웃: start_oldest + 1 (더 오래된 물리 페이지)
    // - 최신 쪽 이웃: end_newest - 1 (더 최신 물리 페이지)
    // 예) 단일 페이지 377 지정 시 실행 집합: {378, 376, 377} (범위 포함 시 중복은 HashSet으로 제거)
    let pages_vec: Vec<u32> = {
        use std::collections::HashSet;
        let mut ordered: Vec<u32> = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();

        for (start_oldest, end_newest) in ranges.iter().copied() {
            let span = start_oldest.saturating_sub(end_newest) + 1;
            info!(
                "Processing sync range: {} -> {} (span={})",
                start_oldest, end_newest, span
            );

            // 1) 경계 확장(오래된 쪽): start_oldest 바로 다음(더 오래된) 페이지 포함
            if start_oldest < total_pages {
                let extra_older = start_oldest + 1;
                if !seen.contains(&extra_older) {
                    ordered.push(extra_older);
                    seen.insert(extra_older);
                }
            }

            // 2) 경계 확장(최신 쪽): end_newest 바로 이전(더 최신) 페이지 포함
            if end_newest > 1 {
                let extra_newer = end_newest - 1;
                if !seen.contains(&extra_newer) {
                    ordered.push(extra_newer);
                    seen.insert(extra_newer);
                }
            }

            // 3) 원래 범위 Oldest -> Newer 순으로 추가 (start_oldest, start_oldest-1, ..., end_newest)
            for p in (end_newest..=start_oldest).rev() {
                if !seen.contains(&p) {
                    ordered.push(p);
                    seen.insert(p);
                }
            }
        }
        ordered
    };

    let max_concurrent = app_config
        .user
        .crawling
        .workers
        .list_page_max_concurrent
        .max(1);
    info!(
        "Launching page workers with concurrency={} (config)",
        max_concurrent
    );
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

    // Cache retry configs to avoid moving config into tasks
    let list_retry_count: u32 = app_config.user.crawling.product_list_retry_count.max(1);
    let detail_retry_count: u32 = app_config.user.crawling.product_detail_retry_count.max(1);

    let mut handles = Vec::with_capacity(pages_vec.len());
    for physical_page in pages_vec {
        let permit = semaphore.clone().acquire_owned();
        let app = app_handle.clone();
        let session_id = session_id.clone();
        let pool = pool_arc.clone();
        let http = http_client.clone();
        let extractor = extractor_global.clone();
        let calculator = calculator_global.clone();
        let newest_html_clone = newest_html.clone();
        let oldest_html_clone = oldest_html.clone();
        // Clone per-iteration data to avoid moving across spawned tasks
        let sync_ua_cloned = sync_ua.clone();
        let pages_processed_c = pages_processed.clone();
        let inserted_c = inserted.clone();
        let updated_c = updated.clone();
        let skipped_c = skipped.clone();
        let failed_c = failed.clone();
    let is_dry_run = dry_run.unwrap_or(false);
        let max_list_retries = list_retry_count;
        let max_detail_retries_cfg = detail_retry_count;

    let has_id_col = products_has_id_column; // copy into task
    let handle = tokio::spawn(async move {
            // Acquire concurrency slot
            let _permit = match permit.await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to acquire semaphore: {}", e);
                    return;
                }
            };

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
                items_on_last_page as u32
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
                                user_agent_override: sync_ua_cloned.clone(),
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
                            if product_urls.len() as u32 == expected_count {
                                // success; no need to reset last_err_msg explicitly
                                // success
                                break;
                            } else {
                                last_err_msg = Some(format!(
                                    "count_mismatch: expected {} got {}",
                                    expected_count,
                                    product_urls.len()
                                ));
                            }
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
                            AppEvent::SyncWarning {
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
                    emit_actor_event(
                        &app,
                        AppEvent::SyncRetrying {
                            session_id: session_id.clone(),
                            scope: "list_page".into(),
                            physical_page: Some(physical_page),
                            url: None,
                            attempt: attempt + 1,
                            max_attempts: max_retries,
                            reason: Some(msg.clone()),
                            timestamp: Utc::now(),
                        },
                    );
                } else {
                    info!(target: "kpi.sync", "{{\"event\":\"retry_attempt\",\"session_id\":\"{}\",\"page\":{},\"attempt\":{},\"max_retries\":{}}}", session_id, physical_page, attempt + 1, max_retries);
                    emit_actor_event(
                        &app,
                        AppEvent::SyncRetrying {
                            session_id: session_id.clone(),
                            scope: "list_page".into(),
                            physical_page: Some(physical_page),
                            url: None,
                            attempt: attempt + 1,
                            max_attempts: max_retries,
                            reason: None,
                            timestamp: Utc::now(),
                        },
                    );
                }

                // Backoff with jitter
                let backoff_ms = 200u64 * (1u64 << attempt);
                tokio::time::sleep(std::time::Duration::from_millis(
                    backoff_ms + (physical_page as u64 % 50),
                ))
                .await;
                attempt += 1;
            }

            // Log mismatch if persists
            if product_urls.len() as u32 != expected_count {
                emit_actor_event(
                    &app,
                    AppEvent::SyncWarning {
                        session_id: session_id.clone(),
                        code: "count_mismatch".into(),
                        detail: format!(
                            "page {}: expected {} items, extracted {} (after retries)",
                            physical_page,
                            expected_count,
                            product_urls.len()
                        ),
                        timestamp: Utc::now(),
                    },
                );
            }

            let mut page_inserted = 0u32;
            let mut page_updated = 0u32;
            let mut page_skipped = 0u32;
            let mut page_failed = 0u32; // aggregated into failed_c
            let page_start = std::time::Instant::now();

            // Begin a transaction for this page
            let mut tx = match pool.begin().await {
                Ok(t) => t,
                Err(e) => {
                    failed_c.fetch_add(product_urls.len() as u32, Ordering::SeqCst);
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

            // 0) 우선순위 재정렬: products에는 존재하지만 product_details에 미존재한 URL을 먼저 처리
            let mut missing_first: Vec<usize> = Vec::new();
            let mut remaining: Vec<usize> = Vec::new();
            for (i, url) in product_urls.iter().enumerate() {
                let has_product = match sqlx::query_scalar::<_, i64>(
                    "SELECT 1 FROM products WHERE url = ? LIMIT 1",
                )
                .bind(url)
                .fetch_optional(&mut *tx)
                .await
                {
                    Ok(opt) => opt.is_some(),
                    Err(_) => false,
                };
                if has_product {
                    let has_details = match sqlx::query_scalar::<_, i64>(
                        "SELECT 1 FROM product_details WHERE url = ? LIMIT 1",
                    )
                    .bind(url)
                    .fetch_optional(&mut *tx)
                    .await
                    {
                        Ok(opt) => opt.is_some(),
                        Err(_) => false,
                    };
                    if has_details {
                        remaining.push(i);
                    } else {
                        missing_first.push(i);
                    }
                } else {
                    remaining.push(i);
                }
            }

            for idx in missing_first.into_iter().chain(remaining.into_iter()) {
                let i = idx;
                let url = &product_urls[i];
                let calc = calculator.calculate(physical_page, i);
                if is_dry_run {
                    page_skipped += 1; // dry-run counts as skipped
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
                    continue;
                }

                // Record observed set for this session (per URL)
                if let Err(e) = sqlx::query(
                    "INSERT INTO sync_observed(session_id, url, page_id, index_in_page) VALUES(?, ?, ?, ?) \
                     ON CONFLICT(session_id, url) DO UPDATE SET page_id=excluded.page_id, index_in_page=excluded.index_in_page",
                )
                .bind(&session_id)
                .bind(url)
                .bind(calc.page_id)
                .bind(calc.index_in_page)
                .execute(&mut *tx)
                .await
                {
                    emit_actor_event(
                        &app,
                        AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "observed_record_failed".into(),
                            detail: format!("{}: {}", url, e),
                            timestamp: Utc::now(),
                        },
                    );
                }

                // Try get existing
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
                            AppEvent::SyncWarning {
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
                        // Log attempt to insert a missing product
                        info!(target: "kpi.sync", "{}",
                            format!(
                                r#"{{"event":"product_upsert","action":"insert_attempt","page":{},"page_id":{},"index":{},"url":"{}"}}"#,
                                physical_page, calc.page_id, calc.index_in_page, url
                            )
                        );
                        // Guard: only insert when coordinates are valid (>0)
                        if calc.page_id > 0 && calc.index_in_page > 0 {
                            let res = sqlx::query(
                                "INSERT INTO products (url, page_id, index_in_page) VALUES (?, ?, ?)",
                            )
                            .bind(url)
                            .bind(calc.page_id)
                            .bind(calc.index_in_page)
                            .execute(&mut *tx)
                            .await;
                            match res {
                                Ok(_) => {
                                    page_inserted += 1;
                                    inserted_c.fetch_add(1, Ordering::SeqCst);
                                    // Success logs + FE lifecycle
                                    info!(target: "kpi.sync", "{}",
                                        format!(
                                            r#"{{"event":"product_upsert","action":"inserted","page":{},"page_id":{},"index":{},"url":"{}"}}"#,
                                            physical_page, calc.page_id, calc.index_in_page, url
                                        )
                                    );
                                    emit_actor_event(
                                        &app,
                                        AppEvent::ProductLifecycle {
                                            session_id: session_id.clone(),
                                            batch_id: None,
                                            page_number: Some(physical_page),
                                            product_ref: url.clone(),
                                            status: "product_inserted".into(),
                                            retry: None,
                                            duration_ms: None,
                                            metrics: None,
                                            timestamp: Utc::now(),
                                        },
                                    );
                                }
                                Err(e) => {
                                    emit_actor_event(
                                        &app,
                                        AppEvent::SyncWarning {
                                            session_id: session_id.clone(),
                                            code: "insert_failed".into(),
                                            detail: format!("{}: {}", url, e),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    continue;
                                }
                            }
                        } else {
                            // skip invalid coordinates
                            emit_actor_event(
                                &app,
                                AppEvent::SyncWarning {
                                    session_id: session_id.clone(),
                                    code: "invalid_coordinates".into(),
                                    detail: format!("skip url={} pid={} idx={}", url, calc.page_id, calc.index_in_page),
                                    timestamp: Utc::now(),
                                },
                            );
                            continue;
                        }

                                                // Ensure product_details has a placeholder row with synthetic id from the start (do not overwrite existing id)
                                                let synthetic_id = format!("p{:04}i{:02}", calc.page_id, calc.index_in_page);
                                                let _ = sqlx::query(
                                                        r#"INSERT INTO product_details (url, page_id, index_in_page, id)
                                                                VALUES (?, ?, ?, ?)
                                                                ON CONFLICT(url) DO UPDATE SET
                                                                    page_id = COALESCE(excluded.page_id, product_details.page_id),
                                                                    index_in_page = COALESCE(excluded.index_in_page, product_details.index_in_page),
                                                                    id = COALESCE(product_details.id, excluded.id),
                                                                    updated_at = CURRENT_TIMESTAMP"#,
                                                )
                                                .bind(url)
                                                .bind(calc.page_id)
                                                .bind(calc.index_in_page)
                                                .bind(synthetic_id)
                                                .execute(&mut *tx)
                                                .await;
                    },
                    Some(r) => {
                        let db_pid: Option<i64> = r.get("page_id");
                        let db_idx: Option<i64> = r.get("index_in_page");
                        let needs_update = match (db_pid, db_idx) {
                            (Some(p), Some(ix)) => {
                                p as i32 != calc.page_id || ix as i32 != calc.index_in_page
                            }
                            _ => true,
                        };
                        if needs_update {
                            info!(target: "kpi.sync", "{}",
                                format!(
                                    r#"{{"event":"product_upsert","action":"update_attempt","page":{},"page_id":{},"index":{},"url":"{}"}}"#,
                                    physical_page, calc.page_id, calc.index_in_page, url
                                )
                            );
                            let res = sqlx::query(
                                "UPDATE products SET page_id = ?, index_in_page = ?, updated_at = CURRENT_TIMESTAMP WHERE url = ?",
                            )
                            .bind(calc.page_id)
                            .bind(calc.index_in_page)
                            .bind(url)
                            .execute(&mut *tx)
                            .await;
                            match res {
                                Ok(_) => {
                                    page_updated += 1;
                                    updated_c.fetch_add(1, Ordering::SeqCst);
                                    info!(target: "kpi.sync", "{}",
                                        format!(
                                            r#"{{"event":"product_upsert","action":"updated","page":{},"page_id":{},"index":{},"url":"{}"}}"#,
                                            physical_page, calc.page_id, calc.index_in_page, url
                                        )
                                    );
                                    emit_actor_event(
                                        &app,
                                        AppEvent::ProductLifecycle {
                                            session_id: session_id.clone(),
                                            batch_id: None,
                                            page_number: Some(physical_page),
                                            product_ref: url.clone(),
                                            status: "product_updated".into(),
                                            retry: None,
                                            duration_ms: None,
                                            metrics: None,
                                            timestamp: Utc::now(),
                                        },
                                    );
                                }
                                Err(e) => {
                                    page_failed += 1;
                                    failed_c.fetch_add(1, Ordering::SeqCst);
                                    emit_actor_event(
                                        &app,
                                        AppEvent::SyncWarning {
                                            session_id: session_id.clone(),
                                            code: "update_failed".into(),
                                            detail: format!("{}: {}", url, e),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    info!(target: "kpi.sync", "{}",
                                        format!(
                                            r#"{{"event":"product_upsert","action":"update_failed","page":{},"page_id":{},"index":{},"url":"{}","error":"{}"}}"#,
                                            physical_page, calc.page_id, calc.index_in_page, url, e
                                        )
                                    );
                                    emit_actor_event(
                                        &app,
                                        AppEvent::ProductLifecycle {
                                            session_id: session_id.clone(),
                                            batch_id: None,
                                            page_number: Some(physical_page),
                                            product_ref: url.clone(),
                                            status: "product_update_failed".into(),
                                            retry: None,
                                            duration_ms: None,
                                            metrics: None,
                                            timestamp: Utc::now(),
                                        },
                                    );
                                }
                            }
                        } else {
                            page_skipped += 1;
                            skipped_c.fetch_add(1, Ordering::SeqCst);
                            info!(target: "kpi.sync", "{}",
                                format!(
                                    r#"{{"event":"product_upsert","action":"skipped_nochange","page":{},"page_id":{},"index":{},"url":"{}"}}"#,
                                    physical_page, calc.page_id, calc.index_in_page, url
                                )
                            );
                            emit_actor_event(
                                &app,
                                AppEvent::ProductLifecycle {
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
                                                // Keep product_details in sync as well and ensure id is set if missing
                                                let synthetic_id = format!("p{:04}i{:02}", calc.page_id, calc.index_in_page);
                                                match sqlx::query(
                                                        r#"INSERT INTO product_details (url, page_id, index_in_page, id)
                                                                VALUES (?, ?, ?, ?)
                                                                ON CONFLICT(url) DO UPDATE SET
                                                                    page_id = COALESCE(excluded.page_id, product_details.page_id),
                                                                    index_in_page = COALESCE(excluded.index_in_page, product_details.index_in_page),
                                                                    id = COALESCE(product_details.id, excluded.id),
                                                                    updated_at = CURRENT_TIMESTAMP"#,
                                                )
                                                .bind(url)
                                                .bind(calc.page_id)
                                                .bind(calc.index_in_page)
                                                .bind(synthetic_id)
                                                .execute(&mut *tx)
                                                .await
                                                {
                            Ok(res) => {
                                info!(target: "kpi.sync", "{}",
                                    format!(
                                        r#"{{"event":"details_position_sync","action":"updated","affected":{},"page":{},"page_id":{},"index":{},"url":"{}"}}"#,
                                        res.rows_affected(), physical_page, calc.page_id, calc.index_in_page, url
                                    )
                                );
                            }
                            Err(e) => {
                                emit_actor_event(
                                    &app,
                                    AppEvent::SyncWarning {
                                        session_id: session_id.clone(),
                                        code: "details_update_failed".into(),
                                        detail: format!("{}: {}", url, e),
                                        timestamp: Utc::now(),
                                    },
                                );
                                info!(target: "kpi.sync", "{}",
                                    format!(
                                        r#"{{"event":"details_position_sync","action":"update_failed","page":{},"page_id":{},"index":{},"url":"{}","error":"{}"}}"#,
                                        physical_page, calc.page_id, calc.index_in_page, url, e
                                    )
                                );
                            }
                        }
                        // 추가: 기존 제품이지만 product_details에 미존재한 경우에 한해 상세 수집/업서트 수행 (우선순위 처리됨)
                        let details_missing = match sqlx::query_scalar::<_, i64>(
                            "SELECT 1 FROM product_details WHERE url = ? LIMIT 1",
                        )
                        .bind(url)
                        .fetch_optional(&mut *tx)
                        .await
                        {
                            Ok(opt) => opt.is_none(),
                            Err(_) => false,
                        };
                        if details_missing {
                            let max_detail_retries = max_detail_retries_cfg;
                            let mut success = false;
                            for attempt in 1..=max_detail_retries {
                                let referer_url = if physical_page == 1 {
                                    csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string()
                                } else {
                                    csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
                                        .replace("{}", &physical_page.to_string())
                                };
                                match http
                                    .fetch_response_with_options(
                                        url,
                                        &RequestOptions {
                                            user_agent_override: sync_ua_cloned.clone(),
                                            referer: Some(referer_url),
                                            skip_robots_check: false,
                                            attempt: Some(attempt),
                                            max_attempts: Some(max_detail_retries),
                                        },
                                    )
                                    .await
                                {
                                    Ok(resp) => match resp.text().await {
                                        Ok(body) => {
                                            let extracted = {
                                                let doc = Html::parse_document(&body);
                                                extractor.extract_product_detail(&doc, url.clone())
                                            };
                                            match extracted {
                                                Ok(mut detail) => {
                                                    detail.page_id = Some(calc.page_id);
                                                    detail.index_in_page = Some(calc.index_in_page);
                                                    if detail.id.is_none() {
                                                        detail.id = Some(format!(
                                                            "p{:04}i{:02}",
                                                            calc.page_id, calc.index_in_page
                                                        ));
                                                    }
                                                    let program_type =
                                                        Some(detail.program_type.unwrap_or_else(
                                                            || "Matter".to_string(),
                                                        ));
                                                    // Clone fields we need later for products backfill to avoid move
                                                    let man_clone = detail.manufacturer.clone();
                                                    let model_clone = detail.model.clone();
                                                    let cert_clone = detail.certificate_id.clone();
                                                    if let Err(e) = sqlx::query(
                                                        r#"INSERT INTO product_details (
                                                            url, page_id, index_in_page, id, manufacturer, model, device_type,
                                                            certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                                            specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                                            tis_trp_tested, transport_interface, primary_device_type_id, application_categories,
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
                                                            primary_device_type_id=COALESCE(excluded.primary_device_type_id, product_details.primary_device_type_id),
                                                            application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                                            description=COALESCE(excluded.description, product_details.description),
                                                            compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                                            program_type=COALESCE(excluded.program_type, product_details.program_type),
                                                            updated_at=CURRENT_TIMESTAMP
                                                        "#,
                                                    )
                                                    .bind(&detail.url)
                                                    .bind(detail.page_id)
                                                    .bind(detail.index_in_page)
                                                    .bind(detail.id)
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
                                                    .bind(detail.primary_device_type_id)
                                                    .bind(detail.application_categories)
                                                    .bind(detail.description)
                                                    .bind(detail.compliance_document_url)
                                                    .bind(program_type)
                                                    .execute(&mut *tx)
                                                    .await
                                                    {
                                                        emit_actor_event(
                                                            &app,
                                                            AppEvent::SyncWarning {
                                                                session_id: session_id.clone(),
                                                                code: "details_insert_failed".into(),
                                                                detail: format!("{}: {}", url, e),
                                                                timestamp: Utc::now(),
                                                            },
                                                        );
                                                        info!(target: "kpi.sync", "{}",
                                                            format!(
                                                                r#"{{"event":"details_upsert","action":"insert_failed","page":{},"page_id":{},"index":{},"url":"{}","attempt":{},"max":{},"error":"{}"}}"#,
                                                                physical_page, calc.page_id, calc.index_in_page, url, attempt, max_detail_retries, e
                                                            )
                                                        );
                                                    } else if let Ok(res) = sqlx::query(
                                                        r#"SELECT changes() as affected"#,
                                                    )
                                                    .fetch_one(&mut *tx)
                                                    .await
                                                    {
                                                        let affected: i64 = res.get::<i64, _>("affected");
                                                        emit_actor_event(
                                                            &app,
                                                            AppEvent::ProductLifecycle {
                                                                session_id: session_id.clone(),
                                                                batch_id: None,
                                                                page_number: Some(physical_page),
                                                                product_ref: url.clone(),
                                                                status: if affected > 0 { "details_persisted".into() } else { "details_skipped_exists".into() },
                                                                retry: Some(attempt - 1),
                                                                duration_ms: None,
                                                                metrics: None,
                                                                timestamp: Utc::now(),
                                                            },
                                                        );
                                                        info!(target: "kpi.sync", "{}",
                                                            format!(
                                                                r#"{{"event":"details_upsert","action":"{}","page":{},"page_id":{},"index":{},"url":"{}","attempt":{},"max":{}}}"#,
                                                                if affected > 0 { "inserted" } else { "skipped_exists" },
                                                                physical_page, calc.page_id, calc.index_in_page, url, attempt, max_detail_retries
                                                            )
                                                        );
                                                        // 성공적으로 상세를 확보했으므로 products의 코어 필드도 채움(누락만 채움)
                                                        let _ = sqlx::query(
                                                            r#"UPDATE products SET
                                                                manufacturer = COALESCE(?, manufacturer),
                                                                model = COALESCE(?, model),
                                                                certificate_id = COALESCE(?, certificate_id),
                                                                updated_at = CURRENT_TIMESTAMP
                                                            WHERE url = ?"#,
                                                        )
                                                        .bind(&man_clone)
                                                        .bind(&model_clone)
                                                        .bind(&cert_clone)
                                                        .bind(&detail.url)
                                                        .execute(&mut *tx)
                                                        .await;
                                                        success = true;
                                                        break;
                                                    }
                                                }
                                                Err(e) => {
                                                    emit_actor_event(
                                                        &app,
                                                        AppEvent::SyncWarning {
                                                            session_id: session_id.clone(),
                                                            code: "details_extract_failed".into(),
                                                            detail: format!("{}: {}", url, e),
                                                            timestamp: Utc::now(),
                                                        },
                                                    );
                                                    info!(target: "kpi.sync", "{}",
                                                        format!(
                                                            r#"{{"event":"details_upsert","action":"extract_failed","page":{},"page_id":{},"index":{},"url":"{}","attempt":{},"max":{},"error":"{}"}}"#,
                                                            physical_page, calc.page_id, calc.index_in_page, url, attempt, max_detail_retries, e
                                                        )
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            emit_actor_event(
                                                &app,
                                                AppEvent::SyncWarning {
                                                    session_id: session_id.clone(),
                                                    code: "details_read_failed".into(),
                                                    detail: format!("{}: {}", url, e),
                                                    timestamp: Utc::now(),
                                                },
                                            );
                                            info!(target: "kpi.sync", "{}",
                                                format!(
                                                    r#"{{"event":"details_upsert","action":"read_failed","page":{},"page_id":{},"index":{},"url":"{}","attempt":{},"max":{},"error":"{}"}}"#,
                                                    physical_page, calc.page_id, calc.index_in_page, url, attempt, max_detail_retries, e
                                                )
                                            );
                                        }
                                    },
                                    Err(e) => {
                                        emit_actor_event(
                                            &app,
                                            AppEvent::SyncWarning {
                                                session_id: session_id.clone(),
                                                code: "details_fetch_failed".into(),
                                                detail: format!("{}: {}", url, e),
                                                timestamp: Utc::now(),
                                            },
                                        );
                                        info!(target: "kpi.sync", "{}",
                                            format!(
                                                r#"{{"event":"details_upsert","action":"fetch_failed","page":{},"page_id":{},"index":{},"url":"{}","attempt":{},"max":{},"error":"{}"}}"#,
                                                physical_page, calc.page_id, calc.index_in_page, url, attempt, max_detail_retries, e
                                            )
                                        );
                                    }
                                }
                                if attempt < max_detail_retries && !success {
                                    // Emit detail retrying
                                    emit_actor_event(
                                        &app,
                                        AppEvent::SyncRetrying {
                                            session_id: session_id.clone(),
                                            scope: "product_detail".into(),
                                            physical_page: Some(physical_page),
                                            url: Some(url.clone()),
                                            attempt,
                                            max_attempts: max_detail_retries,
                                            reason: None,
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    let shift = attempt - 1;
                                    let backoff_ms = 200u64 * (1u64 << shift);
                                    info!(target: "kpi.sync", "{}",
                                        format!(
                                            r#"{{"event":"details_retry_attempt","page":{},"page_id":{},"index":{},"url":"{}","next_delay_ms":{},"attempt":{},"max":{}}}"#,
                                            physical_page, calc.page_id, calc.index_in_page, url, backoff_ms, attempt, max_detail_retries
                                        )
                                    );
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        backoff_ms + (physical_page as u64 % 23),
                                    ))
                                    .await;
                                }
                            }
                            if !success {
                                info!(target: "kpi.sync", "{}",
                                    format!(
                                        r#"{{"event":"details_retry_exhausted","page":{},"page_id":{},"index":{},"url":"{}","max":{}}}"#,
                                        physical_page, calc.page_id, calc.index_in_page, url, max_detail_retries
                                    )
                                );
                            }
                        } else {
                            // details가 이미 있는 경우: products의 코어 필드 결손치 보정(누락만 채움)
                            if let Ok(Some(r)) = sqlx::query(
                                "SELECT manufacturer, model, certificate_id FROM product_details WHERE url = ? LIMIT 1",
                            )
                            .bind(url)
                            .fetch_optional(&mut *tx)
                            .await
                            {
                                let man: Option<String> = r.get("manufacturer");
                                let model: Option<String> = r.get("model");
                                let cert: Option<String> = r.get("certificate_id");
                                let _ = sqlx::query(
                                    r#"UPDATE products SET
                                        manufacturer = COALESCE(?, manufacturer),
                                        model = COALESCE(?, model),
                                        certificate_id = COALESCE(?, certificate_id),
                                        updated_at = CURRENT_TIMESTAMP
                                    WHERE url = ?"#,
                                )
                                .bind(man)
                                .bind(model)
                                .bind(cert)
                                .bind(url)
                                .execute(&mut *tx)
                                .await;
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

        // Commit transaction for this page
            // Page-scoped DB-only placeholder/backfill to ensure invariants even if listing fetch failed or was partial
            // 1) Ensure product_details placeholders (with synthetic id) exist for all products on this canonical page
        let canonical_pid = calculator.calculate(physical_page, 0).page_id;
        let mut aff_placeholder: u64 = 0;
        let mut aff_prod_backfill: u64 = 0;
        let mut aff_id_backfill: u64 = 0;
        match sqlx::query(
                r#"INSERT INTO product_details (url, page_id, index_in_page, id)
                    SELECT p.url, p.page_id, p.index_in_page, printf('p%04di%02d', COALESCE(p.page_id, 0), COALESCE(p.index_in_page, 0)) as id
                    FROM products p
                    WHERE p.page_id = ?
                ON CONFLICT(url) DO UPDATE SET
                    page_id = COALESCE(product_details.page_id, excluded.page_id),
                    index_in_page = COALESCE(product_details.index_in_page, excluded.index_in_page),
                    id = COALESCE(product_details.id, excluded.id),
                    updated_at = CURRENT_TIMESTAMP"#,
            )
            .bind(canonical_pid)
            .execute(&mut *tx)
            .await
            {
                Ok(res) => {
            let aff = res.rows_affected();
            aff_placeholder = aff;
                    debug!(
                        "DB-only placeholder ensured: page={} (pid={}) affected={}",
                        physical_page, canonical_pid, aff
                    );
                }
                Err(e) => {
                    emit_actor_event(
                        &app,
                        AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "db_only_placeholder_failed".into(),
                            detail: format!("page {} (pid {}): {}", physical_page, canonical_pid, e),
                            timestamp: Utc::now(),
                        },
                    );
                }
            }

            // 2) Backfill products' core fields from existing details within this page (fills only NULLs)
        match sqlx::query(
                r#"UPDATE products AS p SET
                        manufacturer = COALESCE(p.manufacturer, (SELECT d.manufacturer FROM product_details d WHERE d.url = p.url)),
                        model        = COALESCE(p.model,        (SELECT d.model        FROM product_details d WHERE d.url = p.url)),
                        certificate_id = COALESCE(p.certificate_id, (SELECT d.certificate_id FROM product_details d WHERE d.url = p.url)),
                        updated_at = CURRENT_TIMESTAMP
                    WHERE p.page_id = ?"#,
            )
            .bind(canonical_pid)
            .execute(&mut *tx)
            .await
            {
                Ok(res) => {
            let aff = res.rows_affected();
            aff_prod_backfill = aff;
                    debug!(
                        "DB-only backfill applied: page={} (pid={}) affected={}",
                        physical_page, canonical_pid, aff
                    );
                }
                Err(e) => {
                    emit_actor_event(
                        &app,
                        AppEvent::SyncWarning {
                            session_id: session_id.clone(),
                            code: "db_only_backfill_failed".into(),
                            detail: format!("page {} (pid {}): {}", physical_page, canonical_pid, e),
                            timestamp: Utc::now(),
                        },
                    );
                }
            }

            // 3) Optional: Backfill products.id from product_details.id if the column exists (legacy/production schema)
            if has_id_col {
                match sqlx::query(
                    r#"UPDATE products AS p SET
                            id = CASE WHEN p.id IS NULL OR p.id = ''
                                      THEN (SELECT d.id FROM product_details d WHERE d.url = p.url)
                                      ELSE p.id END,
                            updated_at = CURRENT_TIMESTAMP
                        WHERE p.page_id = ?"#,
                )
                .bind(canonical_pid)
                .execute(&mut *tx)
                .await
                {
                    Ok(res) => {
                        aff_id_backfill = res.rows_affected();
                        debug!(
                            "DB-only products.id backfill: page={} (pid={}) affected={}",
                            physical_page,
                            canonical_pid,
                            res.rows_affected()
                        );
                    }
                    Err(e) => {
                        emit_actor_event(
                            &app,
                            AppEvent::SyncWarning {
                                session_id: session_id.clone(),
                                code: "db_only_products_id_backfill_failed".into(),
                                detail: format!("page {} (pid {}): {}", physical_page, canonical_pid, e),
                                timestamp: Utc::now(),
                            },
                        );
                    }
                }
            }

            if let Err(e) = tx.commit().await {
                page_failed += 1;
                failed_c.fetch_add(1, Ordering::SeqCst);
                emit_actor_event(
                    &app,
                    AppEvent::SyncWarning {
                        session_id: session_id.clone(),
                        code: "tx_commit_failed".into(),
                        detail: format!("page {}: {}", physical_page, e),
                        timestamp: Utc::now(),
                    },
                );
            }

            // Emit a compact metric snapshot for per-page DB-only backfills
            info!(target: "kpi.sync", "{}",
                format!(
                    r#"{{"event":"db_only_backfill_metrics","session_id":"{}","page":{},"pid":{},"placeholders":{},"product_core_backfilled":{},"products_id_backfilled":{}}}"#,
                    session_id, physical_page, canonical_pid, aff_placeholder, aff_prod_backfill, aff_id_backfill
                )
            );
            // Also emit as SyncWarning for UI consumption
            emit_actor_event(
                &app,
                AppEvent::SyncWarning {
                    session_id: session_id.clone(),
                    code: "db_only_backfill_metrics".into(),
                    detail: format!(
                        r#"{{"page":{},"pid":{},"placeholders":{},"product_core_backfilled":{},"products_id_backfilled":{}}}"#,
                        physical_page, canonical_pid, aff_placeholder, aff_prod_backfill, aff_id_backfill
                    ),
                    timestamp: Utc::now(),
                },
            );

            // In-range retry: attempt details for URLs on this page with NULL certificate_id (bounded within this page)
            // Runs outside the main per-page transaction
            if !is_dry_run {
                let to_retry: Vec<(String, Option<i64>)> = match sqlx::query_as(
                    r#"SELECT url, index_in_page FROM products WHERE page_id = ? AND certificate_id IS NULL ORDER BY index_in_page ASC"#,
                )
                .bind(canonical_pid)
                .fetch_all(&pool)
                .await
                {
                    Ok(rows) => rows,
                    Err(e) => {
                        emit_actor_event(
                            &app,
                            AppEvent::SyncWarning {
                                session_id: session_id.clone(),
                                code: "in_range_retry_query_failed".into(),
                                detail: format!("page {} (pid {}): {}", physical_page, canonical_pid, e),
                                timestamp: Utc::now(),
                            },
                        );
                        Vec::new()
                    }
                };

                if !to_retry.is_empty() {
                    info!(target: "kpi.sync", "{}",
                        format!(
                            r#"{{"event":"in_range_retry_start","page":{},"pid":{},"count":{}}}"#,
                            physical_page, canonical_pid, to_retry.len()
                        )
                    );
                }

                for (url, idx_opt) in to_retry.into_iter() {
                    let max_detail_retries = max_detail_retries_cfg;
                    let mut success = false;
                    for attempt in 1..=max_detail_retries {
                        let referer_url = if physical_page == 1 {
                            csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string()
                        } else {
                            csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &physical_page.to_string())
                        };
                        match http
                            .fetch_response_with_options(
                                &url,
                                &RequestOptions {
                                    user_agent_override: sync_ua_cloned.clone(),
                                    referer: Some(referer_url),
                                    skip_robots_check: false,
                                    attempt: Some(attempt),
                                    max_attempts: Some(max_detail_retries),
                                },
                            )
                            .await
                        {
                            Ok(resp) => match resp.text().await {
                                Ok(body) => {
                                    let extracted = {
                                        let doc = Html::parse_document(&body);
                                        extractor.extract_product_detail(&doc, url.clone())
                                    };
                                    match extracted {
                                        Ok(mut detail) => {
                                            // Clone fields needed after the INSERT (since INSERT will move them)
                                            let man_clone_bf = detail.manufacturer.clone();
                                            let model_clone_bf = detail.model.clone();
                                            let cert_clone_bf = detail.certificate_id.clone();
                                            let id_clone_bf = detail.id.clone();
                                            // Inject coordinates and synthetic id if missing
                                            detail.page_id = Some(canonical_pid);
                                            detail.index_in_page = detail.index_in_page.or(idx_opt.map(|v| v as i32));
                                            if detail.id.is_none() {
                                                if let (Some(pid), Some(ix)) = (detail.page_id, detail.index_in_page) {
                                                    detail.id = Some(format!("p{:04}i{:02}", pid, ix));
                                                }
                                            }
                                            let program_type = Some(detail.program_type.unwrap_or_else(|| "Matter".to_string()));

                                            // Persist details and backfill
                                            let mut tx2 = match pool.begin().await { Ok(t) => t, Err(_) => { break; } };
                                            let _ = sqlx::query(
                                                r#"INSERT INTO product_details (
                                                    url, page_id, index_in_page, id, manufacturer, model, device_type,
                                                    certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                                    specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                                    tis_trp_tested, transport_interface, primary_device_type_id, application_categories,
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
                                                    id=COALESCE(product_details.id, excluded.id),
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
                                                    primary_device_type_id=COALESCE(excluded.primary_device_type_id, product_details.primary_device_type_id),
                                                    application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                                    description=COALESCE(excluded.description, product_details.description),
                                                    compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                                    program_type=COALESCE(excluded.program_type, product_details.program_type),
                                                    updated_at=CURRENT_TIMESTAMP
                                            "#,
                                            )
                                            .bind(&detail.url)
                                            .bind(detail.page_id)
                                            .bind(detail.index_in_page)
                                            .bind(detail.id)
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
                                            .bind(detail.primary_device_type_id)
                                            .bind(detail.application_categories)
                                            .bind(detail.description)
                                            .bind(detail.compliance_document_url)
                                            .bind(program_type)
                                            .execute(&mut *tx2)
                                            .await;

                                            // Backfill core fields on products
                                            let _ = sqlx::query(
                                                r#"UPDATE products SET
                                                        manufacturer = COALESCE(?, manufacturer),
                                                        model = COALESCE(?, model),
                                                        certificate_id = COALESCE(?, certificate_id),
                                                        updated_at = CURRENT_TIMESTAMP
                                                    WHERE url = ?"#,
                                            )
                                            .bind(&man_clone_bf)
                                            .bind(&model_clone_bf)
                                            .bind(&cert_clone_bf)
                                            .bind(&detail.url)
                                            .execute(&mut *tx2)
                                            .await;

                                            // Optionally backfill products.id if column exists (fill only when NULL)
                                            if has_id_col {
                                                let _ = sqlx::query(
                                                    r#"UPDATE products SET id = CASE WHEN id IS NULL OR id = '' THEN ? ELSE id END WHERE url = ?"#,
                                                )
                                                .bind(&id_clone_bf)
                                                .bind(&detail.url)
                                                .execute(&mut *tx2)
                                                .await;
                                            }

                                            if tx2.commit().await.is_ok() {
                                                success = true;
                                                break;
                                            }
                                        }
                                        Err(_) => {
                                            // parse failed; will retry
                                        }
                                    }
                                }
                                Err(_) => { /* read failed; will retry */ }
                            },
                            Err(_) => { /* fetch failed; will retry */ }
                        }
                        if attempt < max_detail_retries && !success {
                            emit_actor_event(
                                &app,
                                AppEvent::SyncRetrying {
                                    session_id: session_id.clone(),
                                    scope: "product_detail".into(),
                                    physical_page: Some(physical_page),
                                    url: Some(url.clone()),
                                    attempt,
                                    max_attempts: max_detail_retries,
                                    reason: None,
                                    timestamp: Utc::now(),
                                },
                            );
                            let shift = attempt - 1;
                            let backoff_ms = 200u64 * (1u64 << shift);
                            tokio::time::sleep(std::time::Duration::from_millis(
                                backoff_ms + (physical_page as u64 % 17),
                            ))
                            .await;
                        }
                    }
                    if !success {
                        // keep failure counts for visibility
                        failed_c.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }

            if page_failed > 0 {
                trace!(
                    "page {} had {} failed product upserts",
                    physical_page, page_failed
                );
            }

            let ms = page_start.elapsed().as_millis() as u64;
            pages_processed_c.fetch_add(1, Ordering::SeqCst);
            emit_actor_event(
                &app,
                AppEvent::SyncPageCompleted {
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
            r#"UPDATE products AS p
               SET id = CASE WHEN p.id IS NULL OR p.id = ''
                              THEN (SELECT d.id FROM product_details d WHERE d.url = p.url)
                              ELSE p.id END,
                   updated_at = CURRENT_TIMESTAMP
               WHERE p.id IS NULL OR p.id = ''"#,
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
                    AppEvent::SyncWarning {
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
                    AppEvent::SyncWarning {
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

    let duration_ms = started.elapsed().as_millis() as u64;

    // Phase-2: bounded sweep for pages covered in this session
    // Only if not a dry_run and some pages were processed
    let mut deleted_total: u32 = 0;
    if !dry_run.unwrap_or(false) && pages_processed > 0 {
        // Merge and normalize ranges again for safety
        let mut sweep_ranges: Vec<(u32, u32)> = Vec::new();
        if let Ok(parsed) = parse_ranges(&match sqlx::query_scalar::<_, String>(
            "SELECT coverage_text FROM sync_sessions WHERE session_id = ?",
        )
        .bind(&session_id)
        .fetch_one(&pool)
        .await
        {
            Ok(s) => s,
            Err(_) => String::new(),
        }) {
            sweep_ranges = parsed;
        }

        // Sweep only within ranges, but additionally limit to page_ids actually observed in this session,
        // and delete rows whose URL wasn't observed (URL-only match).
        for (start_oldest, end_newest) in sweep_ranges.into_iter() {
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
                    let affected = res.rows_affected() as u32;
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
                let current_page_number = total_pages.saturating_sub(page_id as u32);
                anomalies.push(SyncAnomalyEntry {
                    page_id: page_id as i32,
                    count,
                    current_page_number,
                });
            }
        }
    }
    emit_actor_event(
        &app,
        AppEvent::SyncCompleted {
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
            items_on_last_page: Some(items_on_last_page as u32),
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
    // Build expression like "498,497,489" (each as a singleton)
    let expr = pages
        .into_iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    start_partial_sync(app, app_state, expr, dry_run).await
}

/// Run a diagnostic-driven sync for specific pages and slot indices.
/// Only the specified indices on each page will be processed (precise repair).
#[tauri::command(async)]
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

        let handle = tokio::spawn(async move {
            let _permit = match permit.await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to acquire semaphore: {}", e);
                    return;
                }
            };

            emit_actor_event(
                &app,
                AppEvent::SyncPageStarted {
                    session_id: session_id.clone(),
                    physical_page,
                    timestamp: Utc::now(),
                },
            );

            // Fetch page HTML (same retry logic as partial sync but simplified)
            // expected item count for observability (not used for gating here)
            // let expected_count = if physical_page == oldest_page { items_on_last_page as u32 } else { 12u32 };
            let max_retries = app_config.user.crawling.product_list_retry_count.max(1);
            let mut attempt = 0u32;
            let mut product_urls: Vec<String> = Vec::new();
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
                                attempt: Some(attempt + 1),
                                max_attempts: Some(max_retries + 1),
                            },
                        )
                        .await
                    {
                        Ok(resp) => resp.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    }
                };
                if !page_html.is_empty() {
                    if let Ok(v) = extractor.extract_product_urls_from_content(&page_html) {
                        product_urls = v;
                    }
                }
                if !product_urls.is_empty() || attempt >= max_retries {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200 * (1u64 << attempt))).await;
                attempt += 1;
            }

            // Begin transaction
            let mut tx = match pool.begin().await {
                Ok(t) => t,
                Err(e) => {
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
            for i in 0..product_urls.len() {
                if !selected.contains(&i) {
                    continue;
                }
                let url = match product_urls.get(i) {
                    Some(u) => u.clone(),
                    None => {
                        page_failed += 1;
                        failed_c.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                };
                let calc = calculator.calculate(physical_page, i);
                if dry {
                    page_skipped += 1;
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
                    continue;
                }

                // Upsert product row (same as partial)
                let row = sqlx::query(
                    "SELECT page_id, index_in_page FROM products WHERE url = ? LIMIT 1",
                )
                .bind(&url)
                .fetch_optional(&mut *tx)
                .await;
                let row = match row {
                    Ok(r) => r,
                    Err(e) => {
                        page_failed += 1;
                        failed_c.fetch_add(1, Ordering::SeqCst);
                        emit_actor_event(
                            &app,
                            AppEvent::SyncWarning {
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
                        let res = sqlx::query(
                            "INSERT INTO products (url, page_id, index_in_page) VALUES (?, ?, ?)",
                        )
                        .bind(&url)
                        .bind(calc.page_id)
                        .bind(calc.index_in_page)
                        .execute(&mut *tx)
                        .await;
                        match res {
                            Ok(_) => {
                                page_inserted += 1;
                                inserted_c.fetch_add(1, Ordering::SeqCst);
                            }
                            Err(e) => {
                                page_failed += 1;
                                failed_c.fetch_add(1, Ordering::SeqCst);
                                emit_actor_event(
                                    &app,
                                    AppEvent::SyncWarning {
                                        session_id: session_id.clone(),
                                        code: "insert_failed".into(),
                                        detail: format!("{}: {}", url, e),
                                        timestamp: Utc::now(),
                                    },
                                );
                            }
                        }
                    }
                    Some(r) => {
                        let db_pid: Option<i64> = r.get("page_id");
                        let db_idx: Option<i64> = r.get("index_in_page");
                        let needs_update = match (db_pid, db_idx) {
                            (Some(p), Some(ix)) => {
                                p as i32 != calc.page_id || ix as i32 != calc.index_in_page
                            }
                            _ => true,
                        };
                        if needs_update {
                            match sqlx::query("UPDATE products SET page_id = ?, index_in_page = ?, updated_at = CURRENT_TIMESTAMP WHERE url = ?").bind(calc.page_id).bind(calc.index_in_page).bind(&url).execute(&mut *tx).await { Ok(_) => { page_updated += 1; updated_c.fetch_add(1, Ordering::SeqCst); }, Err(e) => { page_failed += 1; failed_c.fetch_add(1, Ordering::SeqCst); emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "update_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); } }
                        } else {
                            page_skipped += 1;
                            skipped_c.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }

                // Upsert or repair product_details if missing or incomplete
                let mut details_is_complete = false;
                if let Ok(Some(r)) = sqlx::query("SELECT manufacturer, model, device_type, certificate_id FROM product_details WHERE url = ? LIMIT 1").bind(&url).fetch_optional(&mut *tx).await {
                    let man: Option<String> = r.get("manufacturer"); let model: Option<String> = r.get("model"); let dtype: Option<String> = r.get("device_type"); let cert: Option<String> = r.get("certificate_id");
                    details_is_complete = man.is_some() && model.is_some() && dtype.is_some() && cert.is_some();
                }
                if !details_is_complete {
                    let max_detail_retries =
                        app_config.user.crawling.product_detail_retry_count.max(1);
                    let mut success = false;
                    for attempt in 1..=max_detail_retries {
                        // Fetch detail page
                        // Compute appropriate referer based on physical page
                        let referer = if physical_page == 1 {
                            csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string()
                        } else {
                            csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
                                .replace("{}", &physical_page.to_string())
                        };
                        let fetched = http
                            .fetch_response_with_options(
                                &url,
                                &RequestOptions {
                                    user_agent_override: sync_ua.clone(),
                                    referer: Some(referer),
                                    skip_robots_check: false,
                                    attempt: Some(attempt),
                                    max_attempts: Some(max_detail_retries),
                                },
                            )
                            .await;
                        if let Ok(resp) = fetched {
                            if let Ok(body) = resp.text().await {
                                // Extract in a limited scope to avoid carrying non-Send Html across await
                                let extracted = {
                                    let doc = Html::parse_document(&body);
                                    extractor.extract_product_detail(&doc, url.clone())
                                };
                                match extracted {
                                    Ok(mut detail) => {
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
                                        // Upsert (fill missing fields)
                                        let upsert_res = sqlx::query(
                                            r#"INSERT INTO product_details (
                                                url, page_id, index_in_page, id, manufacturer, model, device_type,
                                                certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                                specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                                tis_trp_tested, transport_interface, primary_device_type_id, application_categories,
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
                                                primary_device_type_id=COALESCE(excluded.primary_device_type_id, product_details.primary_device_type_id),
                                                application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                                description=COALESCE(excluded.description, product_details.description),
                                                compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                                program_type=COALESCE(excluded.program_type, product_details.program_type),
                                                updated_at=CURRENT_TIMESTAMP
                                        "#,
                                        )
                                        .bind(&detail.url)
                                        .bind(detail.page_id)
                                        .bind(detail.index_in_page)
                                        .bind(detail.id)
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
                                        .bind(detail.primary_device_type_id)
                                        .bind(detail.application_categories)
                                        .bind(detail.description)
                                        .bind(detail.compliance_document_url)
                                        .bind(program_type)
                                        .execute(&mut *tx)
                                        .await;
                                        if upsert_res.is_ok() {
                                            success = true;
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        // parse failed; will retry
                                    }
                                }
                            }
                        }
                        if !success && attempt < max_detail_retries {
                            let shift = attempt - 1;
                            let base = 1u64.checked_shl(shift).unwrap_or(u64::MAX / 200);
                            let delay: u64 = 200u64.saturating_mul(base);
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        }
                    }
                    if !success {
                        page_failed += 1;
                        failed_c.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }

            if let Err(e) = tx.commit().await {
                failed_c.fetch_add(1, Ordering::SeqCst);
                emit_actor_event(
                    &app,
                    AppEvent::SyncWarning {
                        session_id: session_id.clone(),
                        code: "tx_commit_failed".into(),
                        detail: format!("page {}: {}", physical_page, e),
                        timestamp: Utc::now(),
                    },
                );
            }

            pages_processed_c.fetch_add(1, Ordering::SeqCst);
            emit_actor_event(
                &app,
                AppEvent::SyncPageCompleted {
                    session_id: session_id.clone(),
                    physical_page,
                    inserted: page_inserted,
                    updated: page_updated,
                    skipped: page_skipped,
                    failed: page_failed,
                    ms: 0,
                    timestamp: Utc::now(),
                },
            );
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }
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
            session_id: format!("sync-{}", Utc::now().format("%Y%m%d%H%M%S")),
            pages_processed: summary.pages_processed,
            inserted: summary.inserted,
            updated: summary.updated,
            skipped: summary.skipped,
            failed: summary.failed,
            duration_ms: summary.duration_ms,
            deleted: None,
            total_pages: Some(total_pages),
            items_on_last_page: Some(items_on_last_page as u32),
            anomalies: None,
            timestamp: Utc::now(),
        },
    );
    Ok(summary)
}

/// Retry fetching product details for products with NULL certificate_id.
/// Optionally limit the number of URLs processed. Uses simple referer and reuses extractor logic.
#[tauri::command(async)]
pub async fn retry_failed_details(
    _app: AppHandle,
    app_state: State<'_, AppState>,
    limit: Option<u32>,
    dry_run: Option<bool>,
) -> Result<serde_json::Value, String> {
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;
    let http = app_state.get_http_client().await?;
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let app_config = app_state.config.read().await.clone();
    let sync_ua = app_config.user.crawling.workers.user_agent_sync.clone();

    let lim = limit.unwrap_or(200).max(1) as i64;
    let urls: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
        r#"SELECT url, page_id, index_in_page FROM products WHERE certificate_id IS NULL ORDER BY page_id ASC, index_in_page ASC LIMIT ?"#,
    )
    .bind(lim)
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("query failed: {e}"))?;

    let max_concurrent = app_config.user.crawling.workers.product_detail_max_concurrent.max(1);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let attempted = Arc::new(AtomicU32::new(0));
    let succeeded = Arc::new(AtomicU32::new(0));
    let failed = Arc::new(AtomicU32::new(0));
    let dry = dry_run.unwrap_or(false);

    let mut handles = Vec::with_capacity(urls.len());
    for (url, page_id_opt, index_opt) in urls.into_iter() {
        let permit = semaphore.clone().acquire_owned();
        let http_c = http.clone();
        let extractor_c = extractor.clone();
        let sync_ua_c = sync_ua.clone();
        let pool_c = pool.clone();
        let attempted_c = attempted.clone();
        let succeeded_c = succeeded.clone();
        let failed_c = failed.clone();
        let handle = tokio::spawn(async move {
            let _p = match permit.await { Ok(p) => p, Err(_) => return };
            attempted_c.fetch_add(1, Ordering::SeqCst);
            if dry { return; }
            // Basic referer: CSA base page (sufficient for detail fetch)
            let referer = csa_iot::PRODUCTS_BASE.to_string();
            match http_c
                .fetch_response_with_options(
                    &url,
                    &RequestOptions {
                        user_agent_override: sync_ua_c.clone(),
                        referer: Some(referer),
                        skip_robots_check: false,
                        attempt: None,
                        max_attempts: None,
                    },
                )
                .await
            {
                Ok(resp) => match resp.text().await {
                    Ok(body) => {
                        let extracted = {
                            let doc = Html::parse_document(&body);
                            extractor_c.extract_product_detail(&doc, url.clone())
                        };
                        if let Ok(detail0) = extracted {
                            let mut detail = detail0;
                            // Prefer existing coordinates if present
                            detail.page_id = detail.page_id.or(page_id_opt.map(|v| v as i32));
                            detail.index_in_page = detail.index_in_page.or(index_opt.map(|v| v as i32));
                            if detail.id.is_none() {
                                if let (Some(pid), Some(ix)) = (detail.page_id, detail.index_in_page) {
                                    detail.id = Some(format!("p{:04}i{:02}", pid, ix));
                                }
                            }
                            // Clone fields for later backfill
                            let man_clone = detail.manufacturer.clone();
                            let model_clone = detail.model.clone();
                            let cert_clone = detail.certificate_id.clone();
                            let program_type = Some(
                                detail
                                    .program_type
                                    .clone()
                                    .unwrap_or_else(|| "Matter".to_string()),
                            );
                            let mut tx = match pool_c.begin().await { Ok(t) => t, Err(_) => { failed_c.fetch_add(1, Ordering::SeqCst); return; } };
                            let _ = sqlx::query(
                                r#"INSERT INTO product_details (
                                    url, page_id, index_in_page, id, manufacturer, model, device_type,
                                    certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                    specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                    tis_trp_tested, transport_interface, primary_device_type_id, application_categories,
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
                                    id=COALESCE(product_details.id, excluded.id),
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
                                    primary_device_type_id=COALESCE(excluded.primary_device_type_id, product_details.primary_device_type_id),
                                    application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                    description=COALESCE(excluded.description, product_details.description),
                                    compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                    program_type=COALESCE(excluded.program_type, product_details.program_type),
                                    updated_at=CURRENT_TIMESTAMP
                            "#,
                            )
                            .bind(&detail.url)
                            .bind(detail.page_id)
                            .bind(detail.index_in_page)
                            .bind(detail.id)
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
                            .bind(detail.primary_device_type_id)
                            .bind(detail.application_categories)
                            .bind(detail.description)
                            .bind(detail.compliance_document_url)
                            .bind(program_type)
                            .execute(&mut *tx)
                            .await;

                            // Backfill core products fields
                            let _ = sqlx::query(
                                r#"UPDATE products SET
                                    manufacturer = COALESCE(?, manufacturer),
                                    model = COALESCE(?, model),
                                    certificate_id = COALESCE(?, certificate_id),
                                    updated_at = CURRENT_TIMESTAMP
                                WHERE url = ?"#,
                            )
                            .bind(&man_clone)
                            .bind(&model_clone)
                            .bind(&cert_clone)
                            .bind(&detail.url)
                            .execute(&mut *tx)
                            .await;

                            if tx.commit().await.is_ok() {
                                succeeded_c.fetch_add(1, Ordering::SeqCst);
                            } else {
                                failed_c.fetch_add(1, Ordering::SeqCst);
                            }
                        } else {
                            failed_c.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(_) => { failed_c.fetch_add(1, Ordering::SeqCst); },
                },
                Err(_) => { failed_c.fetch_add(1, Ordering::SeqCst); },
            }
        });
        handles.push(handle);
    }
    for h in handles { let _ = h.await; }
    Ok(serde_json::json!({
        "attempted": attempted.load(Ordering::SeqCst),
        "succeeded": succeeded.load(Ordering::SeqCst),
        "failed": failed.load(Ordering::SeqCst),
    }))
}
