use crate::application::AppState;
use crate::domain::pagination::CanonicalPageIdCalculator;
use crate::infrastructure::{
    config::{ConfigManager, csa_iot},
    html_parser::MatterDataExtractor,
};
use crate::new_architecture::actors::types::{AppEvent, SyncAnomalyEntry};
use chrono::Utc;
use sqlx::Row;
use tauri::{AppHandle, Emitter, State};
use tracing::trace;
use tracing::{debug, error, info};

use serde_json::{Map, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use tokio::sync::Semaphore;

static SYNC_SEQ: AtomicU64 = AtomicU64::new(1);

fn emit_actor_event(app: &AppHandle, event: AppEvent) {
    // Keep event names consistent with actor_event_bridge.rs mapping
    let event_name = match &event {
        AppEvent::SyncStarted { .. } => "actor-sync-started",
        AppEvent::SyncPageStarted { .. } => "actor-sync-page-started",
        AppEvent::SyncUpsertProgress { .. } => "actor-sync-upsert-progress",
        AppEvent::SyncPageCompleted { .. } => "actor-sync-page-completed",
        AppEvent::SyncWarning { .. } => "actor-sync-warning",
        AppEvent::SyncCompleted { .. } => "actor-sync-completed",
        _ => return,
    };
    // Flatten enum payload to top-level fields for FE convenience (like validation does)
    if let Ok(raw) = serde_json::to_value(&event) {
        let flat = if let Value::Object(map) = raw {
            if map.len() == 1 {
                let mut out = Map::new();
                if let Some((k, v)) = map.into_iter().next() {
                    out.insert("variant".into(), Value::String(k.clone()));
                    match v {
                        Value::Object(inner) => {
                            for (ik, iv) in inner.into_iter() {
                                out.insert(ik, iv);
                            }
                        }
                        other => {
                            out.insert("value".into(), other);
                        }
                    }
                }
                Value::Object(out)
            } else {
                Value::Object(map)
            }
        } else {
            raw
        };
        let mut enriched = flat;
        if let Some(o) = enriched.as_object_mut() {
            o.insert(
                "seq".into(),
                Value::from(SYNC_SEQ.fetch_add(1, Ordering::SeqCst)),
            );
            o.insert("backend_ts".into(), Value::from(Utc::now().to_rfc3339()));
            o.insert("event_name".into(), Value::from(event_name));
        }
        if let Err(e) = app.emit(event_name, enriched) {
            error!("Failed to emit sync event {}: {}", event_name, e);
        } else {
            debug!("Emitted sync event {}", event_name);
        }
    }
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
    // sort desc by start, then merge overlaps
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

fn merge_ranges(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    // Normalize each (s,e) so s>=e, sort by s desc then merge overlaps/adjacent
    for r in ranges.iter_mut() {
        if r.0 < r.1 {
            std::mem::swap(&mut r.0, &mut r.1);
        }
    }
    ranges.sort_by(|(s1, e1), (s2, e2)| s2.cmp(s1).then(e2.cmp(e1)));
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (s, e) in ranges.into_iter() {
        if let Some((ls, le)) = merged.last_mut() {
            if *le <= s + 1 && e <= *ls {
                *le = (*le).min(e);
                *ls = (*ls).max(s);
                continue;
            }
        }
        merged.push((s, e));
    }
    merged
}

#[derive(Debug, serde::Serialize)]
pub struct SyncSummary {
    pub pages_processed: u32,
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
    pub failed: u32,
    pub duration_ms: u64,
}

/// Compute anomaly-driven buffered windows and run partial sync
#[tauri::command(async)]
pub async fn start_repair_sync(
    app: AppHandle,
    app_state: State<'_, AppState>,
    buffer: Option<u32>, // how many pages around each anomaly center
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    let buf = buffer.unwrap_or(2);

    // Load config and site meta to map page_id -> physical page
    let cfg_manager =
        ConfigManager::new().map_err(|e| format!("Config manager init failed: {e}"))?;
    let app_config = cfg_manager
        .load_config()
        .await
        .map_err(|e| format!("Config load failed: {e}"))?;
    let http = app_config.create_http_client().map_err(|e| e.to_string())?;
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    let newest_url = csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
    let newest_html = match http.fetch_response(&newest_url).await {
        Ok(resp) => resp.text().await.map_err(|e| e.to_string())?,
        Err(e) => return Err(e.to_string()),
    };
    let total_pages = extractor
        .extract_total_pages(&newest_html)
        .unwrap_or(1)
        .max(1);

    // Query anomalies (cnt != 12) from DB
    let rows = sqlx::query(
        "WITH c AS (SELECT page_id, COUNT(*) AS cnt FROM products GROUP BY page_id) SELECT page_id, cnt FROM c WHERE cnt != 12 ORDER BY page_id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        // Nothing to repair, return a no-op summary
        return Ok(SyncSummary {
            pages_processed: 0,
            inserted: 0,
            updated: 0,
            skipped: 0,
            failed: 0,
            duration_ms: 0,
        });
    }

    // Build windows around each physical page center
    let mut windows: Vec<(u32, u32)> = Vec::new();
    for r in rows.into_iter() {
        let pid: Option<i64> = r.try_get("page_id").ok();
        if let Some(page_id) = pid {
            let center = total_pages.saturating_sub(page_id as u32);
            if center == 0 {
                continue;
            }
            let start = (center + buf).min(total_pages);
            let end = center.saturating_sub(buf).max(1);
            windows.push((start, end));
        }
    }

    let merged = merge_ranges(windows);
    if merged.is_empty() {
        return Ok(SyncSummary {
            pages_processed: 0,
            inserted: 0,
            updated: 0,
            skipped: 0,
            failed: 0,
            duration_ms: 0,
        });
    }

    // Format as ranges string like "498-492,489,487-485"
    let ranges_expr = merged
        .iter()
        .map(|(s, e)| if s == e { s.to_string() } else { format!("{}-{}", s, e) })
        .collect::<Vec<_>>()
        .join(",");

    // Delegate to existing partial sync
    start_partial_sync(app, app_state, ranges_expr, dry_run).await
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

    let cfg_manager =
        ConfigManager::new().map_err(|e| format!("Config manager init failed: {e}"))?;
    let app_config = cfg_manager
        .load_config()
        .await
        .map_err(|e| format!("Config load failed: {e}"))?;
    let http = app_config.create_http_client().map_err(|e| e.to_string())?;
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Discover site meta for calculator
    let newest_url = csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
    let newest_html = match http.fetch_response(&newest_url).await {
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
        match http.fetch_response(&oldest_url).await {
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
            total_pages,
            end_newest,
            default_limit
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
            total_products,
            pages_from_count
        );
        pages_from_count
    };

    // Clamp each range to site bounds and effective span limit
    {
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
    }

    // Persist final coverage_text after clamping/defaulting
    let coverage_text = if ranges.is_empty() {
        String::new()
    } else {
        ranges
            .iter()
            .map(|(s, e)| if s == e { s.to_string() } else { format!("{}-{}", s, e) })
            .collect::<Vec<_>>()
            .join(",")
    };
    if let Err(e) = sqlx::query(
        "UPDATE sync_sessions SET coverage_text = ? WHERE session_id = ?",
    )
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

    // Include effective RPS from config in the start event for observability
    let effective_rps = app_config.user.crawling.workers.max_requests_per_second;
    emit_actor_event(
        &app,
        AppEvent::SyncStarted {
            session_id: session_id.clone(),
            ranges: ranges.clone(),
            rate_limit: Some(effective_rps),
            timestamp: Utc::now(),
        },
    );
    info!(
        "Sync started: session_id={} ranges={:?} dry_run={} rps={}",
        session_id,
        ranges,
        dry_run.unwrap_or(false),
        effective_rps
    );

    // Prepare bounded-concurrency processing for all pages across ranges
    let pages_vec: Vec<u32> = {
        let mut v = Vec::new();
        for (start_oldest, end_newest) in ranges.iter().copied() {
            info!(
                "Processing sync range: {} -> {} (span={})",
                start_oldest,
                end_newest,
                start_oldest.saturating_sub(end_newest) + 1
            );
            // Oldest -> newer (consistent with previous behavior)
            for p in (end_newest..=start_oldest).rev() {
                v.push(p);
            }
        }
        v
    };

    let max_concurrent = app_config.user.crawling.workers.list_page_max_concurrent.max(1);
    info!("Launching page workers with concurrency={} (config)", max_concurrent);
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
        let pages_processed_c = pages_processed.clone();
        let inserted_c = inserted.clone();
        let updated_c = updated.clone();
        let skipped_c = skipped.clone();
        let failed_c = failed.clone();
        let is_dry_run = dry_run.unwrap_or(false);

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
            let expected_count = if physical_page == oldest_page { items_on_last_page as u32 } else { 12u32 };
            // Align sync retry attempts with ListCrawling settings
            let max_retries = app_config.user.crawling.product_list_retry_count.max(1); // total attempts = 1 + max_retries
            // Observability: log per-page retry config
            info!(target: "kpi.sync", "{{\"event\":\"sync_retry_config\",\"session_id\":\"{}\",\"page\":{},\"max_retries\":{}}}", session_id, physical_page, max_retries);
            let mut attempt = 0u32;
            let mut product_urls: Vec<String> = Vec::new();
            let mut last_err_msg: Option<String> = None;
            loop {
                // Choose source: first attempt can reuse cached for edges; retries always fetch fresh
                let use_cache = attempt == 0 && (physical_page == oldest_page || physical_page == 1);
                let page_html = if use_cache {
                    if physical_page == oldest_page { oldest_html_clone.clone() } else { newest_html_clone.clone() }
                } else {
                    let url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &physical_page.to_string());
                    match http.fetch_response(&url).await {
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
                                detail: format!("page {}: {} after {} retries", physical_page, msg, attempt),
                                timestamp: Utc::now(),
                            },
                        );
                    }
                    break;
                }

                // Observability: log retry attempt with last reason if any
                if let Some(msg) = &last_err_msg {
                    info!(target: "kpi.sync", "{{\"event\":\"retry_attempt\",\"session_id\":\"{}\",\"page\":{},\"attempt\":{},\"max_retries\":{},\"reason\":\"{}\"}}", session_id, physical_page, attempt + 1, max_retries, msg);
                } else {
                    info!(target: "kpi.sync", "{{\"event\":\"retry_attempt\",\"session_id\":\"{}\",\"page\":{},\"attempt\":{},\"max_retries\":{}}}", session_id, physical_page, attempt + 1, max_retries);
                }

                // Backoff with jitter
                let backoff_ms = 200u64 * (1u64 << attempt);
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms + (physical_page as u64 % 50))).await;
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

            for (i, url) in product_urls.iter().enumerate() {
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
                        // Best-effort: also reflect position on product_details if exists
                        if let Err(e) = sqlx::query(
                            "UPDATE product_details SET page_id = ?, index_in_page = ?, updated_at = CURRENT_TIMESTAMP WHERE url = ?",
                        )
                        .bind(calc.page_id)
                        .bind(calc.index_in_page)
                        .bind(url)
                        .execute(&mut *tx)
                        .await
                        {
                            emit_actor_event(
                                &app,
                                AppEvent::SyncWarning {
                                    session_id: session_id.clone(),
                                    code: "details_update_failed".into(),
                                    detail: format!("{}: {}", url, e),
                                    timestamp: Utc::now(),
                                },
                            );
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
                                }
                            }
                        } else {
                            page_skipped += 1;
                            skipped_c.fetch_add(1, Ordering::SeqCst);
                        }
                        // Keep product_details in sync as well (best-effort regardless of change)
                        if let Err(e) = sqlx::query(
                            "UPDATE product_details SET page_id = ?, index_in_page = ?, updated_at = CURRENT_TIMESTAMP WHERE url = ?",
                        )
                        .bind(calc.page_id)
                        .bind(calc.index_in_page)
                        .bind(url)
                        .execute(&mut *tx)
                        .await
                        {
                            emit_actor_event(
                                &app,
                                AppEvent::SyncWarning {
                                    session_id: session_id.clone(),
                                    code: "details_update_failed".into(),
                                    detail: format!("{}: {}", url, e),
                                    timestamp: Utc::now(),
                                },
                            );
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
        if let Ok(parsed) = parse_ranges(
            &match sqlx::query_scalar::<_, String>(
                "SELECT coverage_text FROM sync_sessions WHERE session_id = ?",
            )
            .bind(&session_id)
            .fetch_one(&pool)
            .await
            {
                Ok(s) => s,
                Err(_) => String::new(),
            },
        ) {
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
            deleted: if deleted_total > 0 { Some(deleted_total) } else { None },
            total_pages: Some(total_pages),
            items_on_last_page: Some(items_on_last_page as u32),
            anomalies: if anomalies.is_empty() { None } else { Some(anomalies) },
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
