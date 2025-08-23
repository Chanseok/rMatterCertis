use crate::crawl_engine::actors::types::AppEvent;
use crate::domain::pagination::CanonicalPageIdCalculator;
use crate::infrastructure::{
    config::csa_iot, html_parser::MatterDataExtractor, simple_http_client::RequestOptions,
}; // uses ConfigManager (no AppConfigManager)
use chrono::Utc;
use serde_json::{Map, Value};
use sqlx::Row;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, State};
use tracing::{debug, error, info, warn};

static VALIDATION_SEQ: AtomicU64 = AtomicU64::new(1);

// Expected max products per page (site pattern)
const SITE_PRODUCTS_PER_PAGE: usize = 12;

#[derive(Debug)]
struct DetectedAnomaly {
    code: String,
    detail: String,
}

fn detect_page_anomalies(
    physical_page: u32,
    product_urls: &[String],
    is_oldest_page: bool,
    items_on_last_page: usize,
) -> Vec<DetectedAnomaly> {
    let mut anomalies = Vec::new();
    // 1. duplicate_index (duplicate URL occurrences)
    use std::collections::HashMap;
    let mut counts = HashMap::new();
    for u in product_urls.iter() {
        *counts.entry(u).or_insert(0usize) += 1;
    }
    for (u, c) in counts.iter() {
        if *c > 1 {
            anomalies.push(DetectedAnomaly {
                code: "duplicate_index".into(),
                detail: format!("physical_page={} url={} count={}", physical_page, u, c),
            });
        }
    }
    // 2. sparse_page (non-oldest page with < full items)
    if !is_oldest_page && product_urls.len() < SITE_PRODUCTS_PER_PAGE {
        anomalies.push(DetectedAnomaly {
            code: "sparse_page".into(),
            detail: format!(
                "physical_page={} found={} expected={}",
                physical_page,
                product_urls.len(),
                SITE_PRODUCTS_PER_PAGE
            ),
        });
    }
    // 3. oldest page size mismatch (oldest page size > expected or zero)
    if is_oldest_page {
        if product_urls.len() > items_on_last_page {
            anomalies.push(DetectedAnomaly {
                code: "oldest_page_overflow".into(),
                detail: format!(
                    "physical_page={} items={} expected_last_page_items={}",
                    physical_page,
                    product_urls.len(),
                    items_on_last_page
                ),
            });
        }
        if product_urls.is_empty() {
            anomalies.push(DetectedAnomaly {
                code: "oldest_page_empty".into(),
                detail: format!("physical_page={} had zero products", physical_page),
            });
        }
    }
    // 4. out_of_range (sanity: any absurdly long URL or missing required pattern)
    for u in product_urls.iter() {
        if !u.contains("csa_product") {
            anomalies.push(DetectedAnomaly {
                code: "unexpected_url_pattern".into(),
                detail: format!(
                    "physical_page={} url={} missing csa_product pattern",
                    physical_page, u
                ),
            });
        }
    }
    anomalies
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ValidationSummary {
    pub pages_scanned: u32,
    pub products_checked: u64,
    pub divergences: u32,
    pub anomalies: u32,
    pub duration_ms: u64,
    // Detailed additions
    pub divergence_samples: Vec<DivergenceSample>,
    pub per_page: Vec<PerPageStat>,
    pub highest_divergence_physical_page: Option<u32>, // numerically largest page id with divergence (i.e., oldest among scanned)
    pub lowest_divergence_physical_page: Option<u32>, // numerically smallest page id with divergence (i.e., newest among scanned)
    pub gap_ranges: Vec<GapRange>,
    pub cross_page_duplicate_urls: u32,
    // Diagnostics/confirmation
    pub pages_attempted: u32,
    pub total_pages_site: u32,
    pub items_on_last_page: u32,
    pub resolved_start_oldest: u32,
    pub resolved_end_newest: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DivergenceSample {
    pub url: String,
    pub physical_page: u32,
    pub kind: String,
    pub expected_page_id: i32,
    pub expected_index_in_page: i32,
    pub db_page_id: Option<i32>,
    pub db_index_in_page: Option<i32>,
    pub detail: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PerPageStat {
    pub physical_page: u32,
    pub products_found: u32,
    pub divergences: u32,
    pub anomalies: u32,
    pub mismatch_shift_pattern: Option<i32>, // if all coord_mismatch on page share same (db_index - expected_index)
    pub mismatch_missing: u32,
    pub mismatch_coord: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GapRange {
    pub start_offset: u64,
    pub end_offset: u64,
    pub size: u64,
}

/// Emit an AppEvent directly to the frontend (lightweight bridge clone)
pub(crate) fn emit_actor_event(app: &AppHandle, event: AppEvent) {
    // Map variant -> event name (keep in sync with actor_event_bridge.rs)
    let event_name = match &event {
        // Validation event stream
        AppEvent::ValidationStarted { .. } => "actor-validation-started",
        AppEvent::ValidationPageScanned { .. } => "actor-validation-page-scanned",
        AppEvent::ValidationDivergenceFound { .. } => "actor-validation-divergence",
        AppEvent::ValidationAnomaly { .. } => "actor-validation-anomaly",
        AppEvent::ValidationCompleted { .. } => "actor-validation-completed",
        // Sync event stream
        AppEvent::SyncStarted { .. } => "actor-sync-started",
        AppEvent::SyncPageStarted { .. } => "actor-sync-page-started",
        AppEvent::SyncUpsertProgress { .. } => "actor-sync-upsert-progress",
        AppEvent::SyncPageCompleted { .. } => "actor-sync-page-completed",
        AppEvent::SyncWarning { .. } => "actor-sync-warning",
    AppEvent::SyncRetrying { .. } => "actor-sync-retrying",
        AppEvent::SyncCompleted { .. } => "actor-sync-completed",
        // Product lifecycle forwarding
        AppEvent::ProductLifecycle { .. } => "actor-product-lifecycle",
        _ => return,
    };
    // Serialize & flatten
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
                Value::from(VALIDATION_SEQ.fetch_add(1, Ordering::SeqCst)),
            );
            o.insert("backend_ts".into(), Value::from(Utc::now().to_rfc3339()));
            o.insert("event_name".into(), Value::from(event_name));
        }
        if let Err(e) = app.emit(event_name, enriched) {
            error!("Failed to emit validation event {}: {}", event_name, e);
        } else {
            debug!("Emitted validation event {}", event_name);
        }
    }
}

/// Real oldest-forward validation pass.
/// Steps:
/// 1. Fetch newest page (page=1) -> determine total_pages
/// 2. Fetch oldest page (total_pages) -> determine items_on_last_page
/// 3. Iterate physical pages from oldest (total_pages) toward newer, limited by scan_pages
/// 4. For each product, compute expected (page_id, index_in_page) using canonical calculator
/// 5. Cross-check DB (url, page_id, index_in_page) -> record divergences
/// 6. Emit AppEvent stream reflecting progress & findings
#[tauri::command(async)]
pub async fn start_validation(
    app: AppHandle,
    app_state: State<'_, crate::application::AppState>,
    scan_pages: Option<u32>,
    start_physical_page: Option<u32>, // oldest (larger number)
    end_physical_page: Option<u32>,   // newest (smaller number)
    // Optional fallback: a human-friendly expression like "498-489,487~485,480"
    // If provided and explicit numeric args are missing, we'll parse the first range.
    ranges_expr: Option<String>,
) -> Result<ValidationSummary, String> {
    // Preserve user-provided scan_pages separately; dynamic default may override if None
    let user_scan_pages = scan_pages.filter(|v| *v > 0);
    info!(
        "start_validation args: scan_pages={:?}, start_physical_page={:?}, end_physical_page={:?}, ranges_expr={:?}",
        user_scan_pages, start_physical_page, end_physical_page, ranges_expr
    );
    // Treat presence of either start or end as intent to use a custom explicit range.
    // Semantics:
    //   start_physical_page (oldest, numerically largest) defaults to total_pages (resolved after fetch) but we only know total_pages later.
    //   end_physical_page (newest, numerically smallest) defaults to 1.
    // We'll temporarily store the raw user inputs and later, after discovering total_pages, normalize.
    let user_range_requested = start_physical_page.is_some() || end_physical_page.is_some();
    // Defer final computation of custom range until after total_pages known; we keep the raw inputs here.
    // We'll represent this by Option<(Option<u32>, Option<u32>)> holding raw values.
    let pending_custom_range: Option<(Option<u32>, Option<u32>)> = if user_range_requested {
        info!(
            "User provided explicit range inputs detected (raw): start={:?}, end={:?}",
            start_physical_page, end_physical_page
        );
        Some((start_physical_page, end_physical_page))
    } else {
        // If numeric args missing, try parsing from ranges_expr as a fallback for robustness
        if let Some(expr_raw) = ranges_expr
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            // Parse first valid token from comma-separated list. Accept "a-b", "a~b", or single number "n".
            let tokens: Vec<String> = expr_raw
                .split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(|t| {
                    let s = t.replace(char::is_whitespace, "");
                    // Normalize unicode dashes to '-'
                    let s = s
                        .replace('–', "-")
                        .replace('—', "-")
                        .replace('−', "-")
                        .replace('﹣', "-")
                        .replace('－', "-");
                    // Normalize unicode tildes to '~'
                    let s = s.replace('〜', "~").replace('～', "~");
                    s
                })
                .collect();
            debug!("ranges_expr tokens(normalized)={:?}", tokens);
            let mut parsed: Option<(u32, u32)> = None;
            for norm in tokens {
                let (sep_dash, sep_tilde) = (norm.contains('-'), norm.contains('~'));
                if sep_dash || sep_tilde {
                    let parts: Vec<&str> = norm.split(if sep_tilde { '~' } else { '-' }).collect();
                    if parts.len() == 2 {
                        if let (Ok(a), Ok(b)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            let start = a.max(b);
                            let end = a.min(b);
                            parsed = Some((start, end));
                            break;
                        }
                    }
                } else if let Ok(n) = norm.parse::<u32>() {
                    // single page token
                    parsed = Some((n, n));
                    break;
                }
            }
            if let Some((s, e)) = parsed {
                info!(
                    "Parsed explicit range from ranges_expr fallback: {} -> {}",
                    s, e
                );
                Some((Some(s), Some(e)))
            } else {
                info!(
                    "No explicit range provided (ranges_expr parse failed or empty); will compute dynamic default window after site discovery"
                );
                None
            }
        } else {
            info!(
                "No explicit range provided; will compute dynamic default window after site discovery"
            );
            None
        }
    };
    let session_id = format!("validation-{}", Utc::now().format("%Y%m%d%H%M%S"));
    let started = std::time::Instant::now();
    // Use shared AppConfig and HttpClient from AppState (DI)
    let app_config = app_state.config.read().await.clone();
    let http = app_state.get_http_client().await?;
    let sync_ua = app_config.user.crawling.workers.user_agent_sync.clone();
    info!("Validation HTTP client initialized with rate limit (worker config applied)");
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    info!(
        "Starting validation pass: session_id={} (pending scan_pages determination) oldest-forward",
        session_id
    );

    // 1. Fetch newest page (page=1)
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
        Ok(resp) => match resp.text().await {
            Ok(t) => t,
            Err(e) => return Err(format!("Read newest page text error: {e}")),
        },
        Err(e) => return Err(format!("Failed to fetch newest page: {e}")),
    };
    let total_pages = match extractor.extract_total_pages(&newest_html) {
        Ok(p) if p > 0 => p,
        _ => 1,
    };

    // 2. Fetch oldest page to determine items_on_last_page
    let oldest_physical_page = total_pages;
    let oldest_url = if total_pages == 1 {
        newest_url.clone()
    } else {
        csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &oldest_physical_page.to_string())
    };
    let oldest_html = if oldest_physical_page == 1 {
        newest_html.clone()
    } else {
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
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(e) => return Err(format!("Read oldest page text error: {e}")),
            },
            Err(e) => {
                return Err(format!(
                    "Failed to fetch oldest page {}: {e}",
                    oldest_physical_page
                ));
            }
        }
    };
    let oldest_urls = extractor
        .extract_product_urls_from_content(&oldest_html)
        .map_err(|e| e.to_string())?;
    let items_on_last_page = oldest_urls.len(); // may be full 12 or partial
    let calculator = CanonicalPageIdCalculator::new(total_pages, items_on_last_page);

    // Acquire shared database pool early for dynamic default calculation
    let pool = app_state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Determine scan_pages_used & physical window
    let (scan_pages_used, physical_range_start_oldest, physical_range_end_newest) = if let Some((
        raw_start_opt,
        raw_end_opt,
    )) =
        pending_custom_range
    {
        // Resolve defaults now that we know total_pages
        let mut start_oldest = raw_start_opt.unwrap_or(total_pages);
        let mut end_newest = raw_end_opt.unwrap_or(1);
        // Clamp to valid bounds
        if start_oldest == 0 {
            start_oldest = total_pages;
        }
        if start_oldest > total_pages {
            warn!(
                "start_physical_page {} > total_pages {}, clamping to total_pages",
                start_oldest, total_pages
            );
            start_oldest = total_pages;
        }
        if end_newest == 0 {
            end_newest = 1;
        }
        if end_newest > start_oldest {
            warn!(
                "Provided range inverted (end_newest {} > start_oldest {}), swapping",
                end_newest, start_oldest
            );
            std::mem::swap(&mut start_oldest, &mut end_newest);
        }
        let mut span = start_oldest.saturating_sub(end_newest) + 1;
        // Apply optional validation_page_limit if configured
        if let Some(limit) = app_config
            .user
            .crawling
            .validation_page_limit
            .filter(|v| *v > 0)
        {
            if span > limit {
                let new_end = start_oldest.saturating_sub(limit - 1);
                info!(
                    "Clamping validation span from {} to {} by config validation_page_limit={}, new range {} -> {}",
                    span, limit, limit, start_oldest, new_end
                );
                span = limit;
                end_newest = new_end.max(1);
            }
        }
        info!(
            "Using explicit custom validation range start_oldest={} end_newest={} span={}",
            start_oldest, end_newest, span
        );
        (span, start_oldest, end_newest)
    } else {
        // Dynamic default if user didn't specify scan_pages:
        let mut pages_to_scan = if let Some(user) = user_scan_pages {
            user.max(1)
        } else {
            // Query DB for product count & max page_id
            let total_products: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM products")
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("DB count failed: {e}"))?
                .try_get::<i64, _>("cnt")
                .unwrap_or(0);
            let max_page_id: Option<i64> = sqlx::query(
                "SELECT MAX(page_id) as max_pid FROM products WHERE page_id IS NOT NULL",
            )
            .fetch_one(&pool)
            .await
            .ok()
            .and_then(|row| row.try_get::<Option<i64>, _>("max_pid").ok())
            .flatten();
            if total_products <= 360 {
                // Use max_page_id span (max_page_id inclusive means +1 pages), fallback to count-derived
                let pages_from_max = max_page_id.map(|v| (v as u32) + 1).unwrap_or(1);
                let pages_from_count = (((total_products as u32) + 11) / 12).max(1);
                pages_from_max.max(pages_from_count)
            } else {
                30u32
            }
        };
        // Apply optional validation_page_limit if configured
        if let Some(limit) = app_config
            .user
            .crawling
            .validation_page_limit
            .filter(|v| *v > 0)
        {
            if pages_to_scan > limit {
                info!(
                    "Clamping dynamic validation pages_to_scan from {} to {} by config validation_page_limit",
                    pages_to_scan, limit
                );
                pages_to_scan = limit;
            }
        }
        let pages_to_scan = pages_to_scan.min(total_pages.max(1));
        let start_oldest = total_pages; // always the oldest physical page number
        let end_newest = if pages_to_scan >= total_pages {
            1
        } else {
            total_pages - pages_to_scan + 1
        };
        info!(
            "Using dynamic default validation window start_oldest={} end_newest={} scan_pages={} (total_pages={})",
            start_oldest, end_newest, pages_to_scan, total_pages
        );
        (pages_to_scan, start_oldest, end_newest)
    };

    emit_actor_event(
        &app,
        AppEvent::ValidationStarted {
            session_id: session_id.clone(),
            scan_pages: scan_pages_used,
            total_pages_site: Some(total_pages),
            timestamp: Utc::now(),
        },
    );

    info!(
        "Validation window resolved: session_id={} total_pages={} start_oldest={} end_newest={} scan_pages_used={}",
        session_id,
        total_pages,
        physical_range_start_oldest,
        physical_range_end_newest,
        scan_pages_used
    );
    let mut pages_scanned = 0u32;
    let mut pages_attempted = 0u32;
    let mut products_checked = 0u64;
    let mut divergences = 0u32;
    let mut anomalies = 0u32; // now tracking
    let mut divergence_samples: Vec<DivergenceSample> = Vec::new();
    let mut per_page_stats: Vec<PerPageStat> = Vec::new();
    let mut highest_divergence_physical_page: Option<u32> = None;
    let mut lowest_divergence_physical_page: Option<u32> = None;
    let mut gap_ranges: Vec<GapRange> = Vec::new();
    let mut last_end_offset: Option<u64> = None; // for gap detection across pages
    use std::collections::HashSet;
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut cross_page_duplicate_urls: u32 = 0;

    for physical_page in (physical_range_end_newest..=physical_range_start_oldest).rev() {
        // oldest -> newer (descending numbers)
        pages_attempted += 1;
        let html_str = if physical_page == oldest_physical_page {
            oldest_html.clone()
        } else if physical_page == 1 {
            newest_html.clone()
        } else {
            let url =
                csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &physical_page.to_string());
            match http
                .fetch_response_with_options(
                    &url,
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
                Ok(resp) => match resp.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        warn!("Read page {} text error: {}", physical_page, e);
                        emit_actor_event(
                            &app,
                            AppEvent::ValidationAnomaly {
                                session_id: session_id.clone(),
                                code: "page_read_failed".into(),
                                detail: format!("physical_page={} error={}", physical_page, e),
                                timestamp: Utc::now(),
                            },
                        );
                        anomalies += 1;
                        continue;
                    }
                },
                Err(e) => {
                    warn!("Skip page {} fetch error: {}", physical_page, e);
                    emit_actor_event(
                        &app,
                        AppEvent::ValidationAnomaly {
                            session_id: session_id.clone(),
                            code: "page_fetch_failed".into(),
                            detail: format!("physical_page={} error={}", physical_page, e),
                            timestamp: Utc::now(),
                        },
                    );
                    anomalies += 1;
                    continue;
                }
            }
        };
        let product_urls = match extractor.extract_product_urls_from_content(&html_str) {
            Ok(u) => u,
            Err(e) => {
                warn!("Parse failure page {}: {}", physical_page, e);
                emit_actor_event(
                    &app,
                    AppEvent::ValidationAnomaly {
                        session_id: session_id.clone(),
                        code: "page_parse_failed".into(),
                        detail: format!("physical_page={} error={}", physical_page, e),
                        timestamp: Utc::now(),
                    },
                );
                anomalies += 1;
                continue;
            }
        };
        // Anomaly detection for this page
        let page_anomalies = detect_page_anomalies(
            physical_page,
            &product_urls,
            physical_page == oldest_physical_page,
            items_on_last_page,
        );
        let mut page_anomaly_count = 0u32;
        for a in page_anomalies.into_iter() {
            anomalies += 1;
            page_anomaly_count += 1;
            emit_actor_event(
                &app,
                AppEvent::ValidationAnomaly {
                    session_id: session_id.clone(),
                    code: a.code,
                    detail: a.detail,
                    timestamp: Utc::now(),
                },
            );
        }
        // We'll process physical listing order (newest->oldest within page). Index mapping accounts for this.
        let mut min_offset: Option<u64> = None;
        let mut max_offset: Option<u64> = None;
        let mut page_divergences = 0u32;
        // For pattern detection
        let mut shift_values: Vec<i32> = Vec::new();
        let mut mismatch_missing = 0u32;
        let mut mismatch_coord = 0u32;
        for (i, url) in product_urls.iter().enumerate() {
            let calc_res = calculator.calculate(physical_page, i); // i: newest-first within physical page
            let expected_offset = (calc_res.page_id as u64) * 12 + (calc_res.index_in_page as u64);
            if min_offset.map(|m| expected_offset < m).unwrap_or(true) {
                min_offset = Some(expected_offset);
            }
            if max_offset.map(|m| expected_offset > m).unwrap_or(true) {
                max_offset = Some(expected_offset);
            }
            // Cross-page duplicate detection (ignore duplicates within same page already handled by anomaly)
            if !seen_urls.insert(url.clone()) {
                cross_page_duplicate_urls += 1;
                emit_actor_event(
                    &app,
                    AppEvent::ValidationAnomaly {
                        session_id: session_id.clone(),
                        code: "cross_page_duplicate".into(),
                        detail: format!(
                            "url={} physical_page={} duplicate_across_pages",
                            url, physical_page
                        ),
                        timestamp: Utc::now(),
                    },
                );
                anomalies += 1; // count as anomaly
            }
            // DB lookup
            let row =
                sqlx::query("SELECT page_id, index_in_page FROM products WHERE url = ? LIMIT 1")
                    .bind(url)
                    .fetch_optional(&pool)
                    .await
                    .map_err(|e| format!("DB query failed: {e}"))?;
            match row {
                None => {
                    divergences += 1;
                    page_divergences += 1;
                    mismatch_missing += 1;
                    emit_actor_event(
                        &app,
                        AppEvent::ValidationDivergenceFound {
                            session_id: session_id.clone(),
                            physical_page,
                            kind: "missing".to_string(),
                            detail: format!(
                                "Missing url {} (expected page_id={}, index_in_page={})",
                                url, calc_res.page_id, calc_res.index_in_page
                            ),
                            expected_offset,
                            timestamp: Utc::now(),
                        },
                    );
                    if divergence_samples.len() < 200 {
                        divergence_samples.push(DivergenceSample {
                            url: url.clone(),
                            physical_page,
                            kind: "missing".into(),
                            expected_page_id: calc_res.page_id,
                            expected_index_in_page: calc_res.index_in_page,
                            db_page_id: None,
                            db_index_in_page: None,
                            detail: "missing in DB".into(),
                        });
                    }
                    highest_divergence_physical_page = Some(
                        highest_divergence_physical_page
                            .map(|h| h.max(physical_page))
                            .unwrap_or(physical_page),
                    );
                    lowest_divergence_physical_page = Some(
                        lowest_divergence_physical_page
                            .map(|l| l.min(physical_page))
                            .unwrap_or(physical_page),
                    );
                }
                Some(r) => {
                    let db_pid: Option<i64> = r.get("page_id");
                    let db_idx: Option<i64> = r.get("index_in_page");
                    match (db_pid, db_idx) {
                        (Some(p), Some(idx))
                            if p as i32 == calc_res.page_id
                                && idx as i32 == calc_res.index_in_page =>
                        {
                            products_checked += 1;
                        }
                        (p, idx) => {
                            divergences += 1;
                            page_divergences += 1;
                            mismatch_coord += 1;
                            if let (Some(_), Some(iv)) = (p, idx) {
                                shift_values.push(iv as i32 - calc_res.index_in_page);
                            }
                            emit_actor_event(
                                &app,
                                AppEvent::ValidationDivergenceFound {
                                    session_id: session_id.clone(),
                                    physical_page,
                                    kind: "coord_mismatch".to_string(),
                                    detail: format!(
                                        "URL {} db=({:?},{:?}) expected=({}, {})",
                                        url, p, idx, calc_res.page_id, calc_res.index_in_page
                                    ),
                                    expected_offset,
                                    timestamp: Utc::now(),
                                },
                            );
                            if divergence_samples.len() < 200 {
                                divergence_samples.push(DivergenceSample {
                                    url: url.clone(),
                                    physical_page,
                                    kind: "coord_mismatch".into(),
                                    expected_page_id: calc_res.page_id,
                                    expected_index_in_page: calc_res.index_in_page,
                                    db_page_id: p.map(|v| v as i32),
                                    db_index_in_page: idx.map(|v| v as i32),
                                    detail: format!(
                                        "db=({:?},{:?}) expected=({}, {})",
                                        p, idx, calc_res.page_id, calc_res.index_in_page
                                    ),
                                });
                            }
                            highest_divergence_physical_page = Some(
                                highest_divergence_physical_page
                                    .map(|h| h.max(physical_page))
                                    .unwrap_or(physical_page),
                            );
                            lowest_divergence_physical_page = Some(
                                lowest_divergence_physical_page
                                    .map(|l| l.min(physical_page))
                                    .unwrap_or(physical_page),
                            );
                        }
                    }
                }
            }
        }
        emit_actor_event(
            &app,
            AppEvent::ValidationPageScanned {
                session_id: session_id.clone(),
                physical_page,
                products_found: product_urls.len() as u32,
                assigned_start_offset: min_offset.unwrap_or(0),
                assigned_end_offset: max_offset.unwrap_or(0),
                timestamp: Utc::now(),
            },
        );
        // Pattern detection: if all shift_values identical and non-empty
        let mismatch_shift_pattern =
            if !shift_values.is_empty() && shift_values.iter().all(|v| *v == shift_values[0]) {
                Some(shift_values[0])
            } else {
                None
            };
        per_page_stats.push(PerPageStat {
            physical_page,
            products_found: product_urls.len() as u32,
            divergences: page_divergences,
            anomalies: page_anomaly_count,
            mismatch_shift_pattern,
            mismatch_missing,
            mismatch_coord,
        });
        // Gap detection across pages (using assigned offsets ascending with newer pages)
        if let (Some(start_o), Some(prev_end)) = (min_offset, last_end_offset) {
            if start_o > prev_end + 1 {
                // gap
                let gap = GapRange {
                    start_offset: prev_end + 1,
                    end_offset: start_o - 1,
                    size: start_o - prev_end - 1,
                };
                emit_actor_event(
                    &app,
                    AppEvent::ValidationDivergenceFound {
                        session_id: session_id.clone(),
                        physical_page,
                        kind: "gap".into(),
                        detail: format!(
                            "gap offsets {}..{} size={} before physical_page {}",
                            gap.start_offset, gap.end_offset, gap.size, physical_page
                        ),
                        expected_offset: gap.start_offset,
                        timestamp: Utc::now(),
                    },
                );
                gap_ranges.push(gap);
            }
        }
        if let Some(end_o) = max_offset {
            last_end_offset = Some(end_o);
        }
        pages_scanned += 1;
    }

    let duration_ms = started.elapsed().as_millis() as u64;
    let summary = ValidationSummary {
        pages_scanned,
        products_checked,
        divergences,
        anomalies,
        duration_ms,
        divergence_samples,
        per_page: per_page_stats,
        highest_divergence_physical_page,
        lowest_divergence_physical_page,
        gap_ranges,
        cross_page_duplicate_urls,
        pages_attempted,
        total_pages_site: total_pages,
        items_on_last_page: items_on_last_page as u32,
        resolved_start_oldest: physical_range_start_oldest,
        resolved_end_newest: physical_range_end_newest,
    };
    emit_actor_event(
        &app,
        AppEvent::ValidationCompleted {
            session_id,
            pages_scanned,
            products_checked,
            divergences,
            anomalies,
            duration_ms,
            timestamp: Utc::now(),
        },
    );
    info!(
        "Validation completed: pages_scanned={} products_checked={} divergences={} duration_ms={} total_pages={} items_on_last={}",
        pages_scanned, products_checked, divergences, duration_ms, total_pages, items_on_last_page
    );
    Ok(summary)
}
