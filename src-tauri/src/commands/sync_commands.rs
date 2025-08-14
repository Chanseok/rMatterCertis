use chrono::Utc;
use tracing::trace;
use tauri::{AppHandle, State, Emitter};
use tracing::{error, info, debug};
use crate::application::AppState;
use crate::new_architecture::actors::types::AppEvent;
use crate::infrastructure::{config::{ConfigManager, csa_iot}, html_parser::MatterDataExtractor};
use crate::domain::pagination::CanonicalPageIdCalculator;
use sqlx::Row;

use std::sync::atomic::{AtomicU64, Ordering};
use serde_json::{Value, Map};

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
                            for (ik, iv) in inner.into_iter() { out.insert(ik, iv); }
                        }
                        other => { out.insert("value".into(), other); }
                    }
                }
                Value::Object(out)
            } else { Value::Object(map) }
        } else { raw };
        let mut enriched = flat;
        if let Some(o) = enriched.as_object_mut() {
            o.insert("seq".into(), Value::from(SYNC_SEQ.fetch_add(1, Ordering::SeqCst)));
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

fn parse_ranges(expr: &str) -> Result<Vec<(u32,u32)>, String> {
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
    let mut out: Vec<(u32,u32)> = Vec::new();
    for token in norm_all.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let sep = if token.contains('~') { '~' } else { '-' };
        if let Some((a,b)) = token.split_once(sep) {
            let mut s: u32 = a.parse().map_err(|_| format!("Invalid number: {}", a))?;
            let mut e: u32 = b.parse().map_err(|_| format!("Invalid number: {}", b))?;
            if e > s { std::mem::swap(&mut s, &mut e); } // ensure s>=e
            out.push((s,e));
        } else {
            let v: u32 = token.parse().map_err(|_| format!("Invalid number: {}", token))?;
            out.push((v,v));
        }
    }
    // sort desc by start, then merge overlaps
    out.sort_by(|(s1,e1),(s2,e2)| s2.cmp(s1).then(e2.cmp(e1)));
    let mut merged: Vec<(u32,u32)> = Vec::new();
    for (s,e) in out.into_iter() {
        if let Some((ls,le)) = merged.last_mut() {
            if *le <= s+1 && e <= *ls { // overlapping or adjacent and ordered
                *le = (*le).min(e);
                *ls = (*ls).max(s);
                continue;
            }
        }
        merged.push((s,e));
    }
    Ok(merged)
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

#[tauri::command(async)]
pub async fn start_partial_sync(
    app: AppHandle,
    app_state: State<'_, AppState>,
    ranges: String,           // e.g., "498-492,489,487-485"
    dry_run: Option<bool>,
) -> Result<SyncSummary, String> {
    let session_id = format!("sync-{}", Utc::now().format("%Y%m%d%H%M%S"));
    let started = std::time::Instant::now();
    info!("start_partial_sync args: ranges=\"{}\" dry_run={:?}", ranges, dry_run);
    let mut ranges = parse_ranges(&ranges)?;
    if ranges.is_empty() { return Err("No valid ranges provided".into()); }

    let cfg_manager = ConfigManager::new().map_err(|e| format!("Config manager init failed: {e}"))?;
    let app_config = cfg_manager.load_config().await.map_err(|e| format!("Config load failed: {e}"))?;
    let http = app_config.create_http_client().map_err(|e| e.to_string())?;
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = app_state.get_database_pool().await.map_err(|e| format!("DB pool unavailable: {e}"))?;

    // Discover site meta for calculator
    let newest_url = csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
    let newest_html = match http.fetch_response(&newest_url).await { Ok(resp) => resp.text().await.map_err(|e| e.to_string())?, Err(e) => return Err(e.to_string()) };
    let total_pages = extractor.extract_total_pages(&newest_html).unwrap_or(1).max(1);
    let oldest_page = total_pages;
    let oldest_html = if oldest_page == 1 { newest_html.clone() } else {
        let oldest_url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &oldest_page.to_string());
        match http.fetch_response(&oldest_url).await { Ok(resp) => resp.text().await.map_err(|e| e.to_string())?, Err(e) => return Err(e.to_string()) }
    };
    let items_on_last_page = extractor.extract_product_urls_from_content(&oldest_html).map_err(|e| e.to_string())?.len();
    let calculator = CanonicalPageIdCalculator::new(total_pages, items_on_last_page);

    info!("Sync site meta: total_pages={} items_on_last_page={}", total_pages, items_on_last_page);

    // Clamp each range to site bounds and optional validation_page_limit
    if !ranges.is_empty() {
        let limit_opt = app_config.user.crawling.validation_page_limit.filter(|v| *v > 0);
        for r in ranges.iter_mut() {
            let (mut s, mut e) = *r;
            let before = (s, e);
            if s > total_pages { s = total_pages; }
            if e > total_pages { e = total_pages; }
            if s < e { std::mem::swap(&mut s, &mut e); }
            if let Some(limit) = limit_opt {
                let span = s.saturating_sub(e) + 1;
                if span > limit {
                    let new_e = s.saturating_sub(limit - 1);
                    info!("Clamping sync span from {} to {} by config validation_page_limit={}, range {}->{}, new {}->{}", span, limit, limit, before.0, before.1, s, new_e);
                    e = new_e.max(1);
                }
            }
            if (s,e) != before { info!("Sync range adjusted: {}->{} => {}->{} (site bounds/limit)", before.0, before.1, s, e); }
            *r = (s,e);
        }
    }

    emit_actor_event(&app, AppEvent::SyncStarted { session_id: session_id.clone(), ranges: ranges.clone(), rate_limit: None, timestamp: Utc::now() });
    info!("Sync started: session_id={} ranges={:?} dry_run={}", session_id, ranges, dry_run.unwrap_or(false));

    let mut pages_processed = 0u32;
    let mut inserted = 0u32;
    let mut updated = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for (start_oldest, end_newest) in ranges.into_iter() {
        info!("Processing sync range: {} -> {} (span={})", start_oldest, end_newest, start_oldest.saturating_sub(end_newest)+1);
        for physical_page in (end_newest..=start_oldest).rev() { // oldest->newer
            emit_actor_event(&app, AppEvent::SyncPageStarted { session_id: session_id.clone(), physical_page, timestamp: Utc::now() });
            let page_html = if physical_page == oldest_page { oldest_html.clone() } else if physical_page == 1 { newest_html.clone() } else {
                let url = csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &physical_page.to_string());
                match http.fetch_response(&url).await { Ok(resp) => resp.text().await.map_err(|e| e.to_string())?, Err(e) => { failed += 1; emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "fetch_failed".into(), detail: format!("page {}: {}", physical_page, e), timestamp: Utc::now() }); continue; } }
            };
            let product_urls = match extractor.extract_product_urls_from_content(&page_html) { Ok(v) => v, Err(e) => { failed += 1; emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "parse_failed".into(), detail: format!("page {}: {}", physical_page, e), timestamp: Utc::now() }); continue; } };

            let mut page_inserted = 0u32; let mut page_updated = 0u32; let mut page_skipped = 0u32; let mut page_failed = 0u32; // page_failed aggregated into `failed`
            let page_start = std::time::Instant::now();
            // Batch all DB operations for this page inside a single transaction to reduce fsync/commit overhead
            let mut tx = match pool.begin().await { Ok(t) => t, Err(e) => { failed += product_urls.len() as u32; emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "tx_begin_failed".into(), detail: format!("page {}: {}", physical_page, e), timestamp: Utc::now() }); continue; } };

            for (i, url) in product_urls.iter().enumerate() {
                let calc = calculator.calculate(physical_page, i);
                if dry_run.unwrap_or(false) {
                    page_skipped += 1; // dry-run counts as skipped
                    emit_actor_event(&app, AppEvent::SyncUpsertProgress { session_id: session_id.clone(), physical_page, inserted: page_inserted, updated: page_updated, skipped: page_skipped, failed: page_failed, timestamp: Utc::now() });
                    continue;
                }
                // Try get existing
                let row = sqlx::query("SELECT page_id, index_in_page FROM products WHERE url = ? LIMIT 1")
                    .bind(url)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                match row {
                    None => {
                        let res = sqlx::query("INSERT INTO products (url, page_id, index_in_page) VALUES (?, ?, ?)")
                            .bind(url)
                            .bind(calc.page_id)
                            .bind(calc.index_in_page)
                            .execute(&mut *tx).await;
                        match res {
                            Ok(_) => { page_inserted += 1; inserted += 1; },
                            Err(e) => { page_failed += 1; failed += 1; emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "insert_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); }
                        }
                    }
                    Some(r) => {
                        let db_pid: Option<i64> = r.get("page_id");
                        let db_idx: Option<i64> = r.get("index_in_page");
                        let needs_update = match (db_pid, db_idx) {
                            (Some(p), Some(ix)) => p as i32 != calc.page_id || ix as i32 != calc.index_in_page,
                            _ => true,
                        };
                        if needs_update {
                            let res = sqlx::query("UPDATE products SET page_id = ?, index_in_page = ? WHERE url = ?")
                                .bind(calc.page_id)
                                .bind(calc.index_in_page)
                                .bind(url)
                                .execute(&mut *tx).await;
                            match res {
                                Ok(_) => { page_updated += 1; updated += 1; },
                                Err(e) => { page_failed += 1; failed += 1; emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "update_failed".into(), detail: format!("{}: {}", url, e), timestamp: Utc::now() }); }
                            }
                        } else {
                            page_skipped += 1; skipped += 1;
                        }
                    }
                }
                if (page_inserted + page_updated + page_skipped + page_failed) % 10 == 0 {
                    emit_actor_event(&app, AppEvent::SyncUpsertProgress { session_id: session_id.clone(), physical_page, inserted: page_inserted, updated: page_updated, skipped: page_skipped, failed: page_failed, timestamp: Utc::now() });
                }
            }

            // Commit transaction for this page
            if let Err(e) = tx.commit().await { page_failed += 1; failed += 1; emit_actor_event(&app, AppEvent::SyncWarning { session_id: session_id.clone(), code: "tx_commit_failed".into(), detail: format!("page {}: {}", physical_page, e), timestamp: Utc::now() }); }

            // Fold per-page failure metric into session-level accumulator explicitly (warning avoidance & clarity)
            if page_failed > 0 { trace!("page {} had {} failed product upserts", physical_page, page_failed); }

            let ms = page_start.elapsed().as_millis() as u64;
            pages_processed += 1;
        emit_actor_event(&app, AppEvent::SyncPageCompleted { session_id: session_id.clone(), physical_page, inserted: page_inserted, updated: page_updated, skipped: page_skipped, failed: page_failed, ms, timestamp: Utc::now() });
        debug!("Sync page completed: p{} ins={} upd={} skip={} fail={} ({}ms)", physical_page, page_inserted, page_updated, page_skipped, page_failed, ms);
        }
    }

    let duration_ms = started.elapsed().as_millis() as u64;
    emit_actor_event(&app, AppEvent::SyncCompleted { session_id: session_id.clone(), pages_processed, inserted, updated, skipped, failed, duration_ms, timestamp: Utc::now() });
    info!("Sync completed: session_id={} pages={} ins={} upd={} skip={} fail={} duration_ms={}", session_id, pages_processed, inserted, updated, skipped, failed, duration_ms);
    Ok(SyncSummary { pages_processed, inserted, updated, skipped, failed, duration_ms })
}
