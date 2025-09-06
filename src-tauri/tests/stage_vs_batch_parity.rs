//! Parity smoke test between legacy BatchActor path and StageActor direct path.
//!
//! Ignored by default; enable with MC_RUN_PARITY=1 and run twice with/without MC_USE_STAGE_DIRECT.
//! It subscribes to AppEvent stream and prints a compact summary for manual diff.

use chrono::Utc;
use matter_certis_v2_lib::crawl_engine::actors::types::{ActorCommand, AppEvent, ExecutionPlan, PageRange, PlanInputSnapshot};
use matter_certis_v2_lib::crawl_engine::integrated_context::IntegratedContextFactory;
use matter_certis_v2_lib::crawl_engine::system_config::SystemConfig;
use matter_certis_v2_lib::crawl_engine::SessionActor;
use matter_certis_v2_lib::crawl_engine::actors::Actor;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[derive(Debug, Default, Clone)]
struct EventAgg {
    session_id: String,
    // Simple counters
    session_started: u32,
    session_completed: u32,
    session_failed: u32,
    batches_started: u32,
    batches_completed: u32,
    batches_failed: u32,
    stage_started: u32,
    stage_completed: u32,
    stage_failed: u32,
    // Totals from reports
    pages_total: u64,
    pages_success: u64,
    pages_failed: u64,
    details_success: u64,
    details_failed: u64,
    duplicates_skipped: u64,
    products_inserted: u64,
    products_updated: u64,
}

impl EventAgg {
    fn apply(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::SessionStarted { session_id, .. } => {
                self.session_id = session_id.clone();
                self.session_started += 1;
            }
            AppEvent::SessionCompleted { .. } => {
                self.session_completed += 1;
            }
            AppEvent::SessionFailed { .. } => {
                self.session_failed += 1;
            }
            AppEvent::BatchStarted { .. } => {
                self.batches_started += 1;
            }
            AppEvent::BatchCompleted { .. } => {
                self.batches_completed += 1;
            }
            AppEvent::BatchFailed { .. } => {
                self.batches_failed += 1;
            }
            AppEvent::StageStarted { .. } => {
                self.stage_started += 1;
            }
            AppEvent::StageCompleted { .. } => {
                self.stage_completed += 1;
            }
            AppEvent::StageFailed { .. } => {
                self.stage_failed += 1;
            }
            AppEvent::BatchReport { pages_total, pages_success, pages_failed, details_success, details_failed, duplicates_skipped, products_inserted, products_updated, .. } => {
                self.pages_total += *pages_total as u64;
                self.pages_success += *pages_success as u64;
                self.pages_failed += *pages_failed as u64;
                self.details_success += *details_success as u64;
                self.details_failed += *details_failed as u64;
                self.duplicates_skipped += *duplicates_skipped as u64;
                self.products_inserted += *products_inserted as u64;
                self.products_updated += *products_updated as u64;
            }
            AppEvent::CrawlReportSession { total_pages, total_success, total_failed, products_inserted, products_updated, .. } => {
                // Trust final session report as authoritative snapshot
                self.pages_total = (*total_pages) as u64;
                self.pages_success = (*total_success) as u64;
                self.pages_failed = (*total_failed) as u64;
                self.products_inserted = (*products_inserted) as u64;
                self.products_updated = (*products_updated) as u64;
            }
            _ => {}
        }
    }
}

fn tiny_plan(session_id: &str) -> ExecutionPlan {
    ExecutionPlan {
        plan_id: "parity-plan".into(),
        session_id: session_id.to_string(),
        crawling_ranges: vec![PageRange { start_page: 2, end_page: 1, estimated_products: 0, reverse_order: true }],
        batch_size: 1,
        concurrency_limit: 1,
        estimated_duration_secs: 0,
        created_at: Utc::now(),
        analysis_summary: "parity".into(),
        original_strategy: "Manual".into(),
        input_snapshot: PlanInputSnapshot {
            total_pages: 3,
            products_on_last_page: 10,
            db_max_page_id: None,
            db_max_index_in_page: None,
            db_total_products: 0,
            page_range_limit: 10,
            batch_size: 1,
            concurrency_limit: 1,
            created_at: Utc::now(),
        },
        plan_hash: "h_parity".into(),
        skip_duplicate_urls: true,
        kpi_meta: None,
        contract_version: 1,
        page_slots: vec![],
    }
}

async fn run_once_collect() -> Option<EventAgg> {
    let sid = format!("parity_{}", Utc::now().timestamp());
    let plan = tiny_plan(&sid);
    let config = std::sync::Arc::new(SystemConfig::default());
    let factory = IntegratedContextFactory::new(config);
    let (context, channels) = factory.create_session_context(sid.clone()).ok()?;
    let (tx, rx) = mpsc::channel::<ActorCommand>(100);
    let mut actor = SessionActor::new(sid.clone());
    let mut event_rx = channels.event_tx.subscribe();
    let handle = tokio::spawn(async move { actor.run(context, rx).await });

    // Note: We avoid setting env vars here to comply with forbid(unsafe_code) builds.
    // Runtime path selection is validated in dedicated integration runs; this is a smoke scaffold only.

    tx.send(ActorCommand::ExecutePrePlanned { session_id: sid.clone(), plan }).await.ok()?;

    let mut agg = EventAgg { session_id: sid.clone(), ..Default::default() };
    // Listen up to 60s for a SessionCompleted pertaining to our session
    let _ = timeout(Duration::from_secs(60), async {
        loop {
            match event_rx.recv().await {
                Ok(ev) => {
                    match &ev {
                        AppEvent::SessionCompleted { session_id, .. } if session_id == &sid => {
                            agg.apply(&ev);
                            break;
                        }
                        AppEvent::SessionFailed { session_id, .. } if session_id == &sid => {
                            agg.apply(&ev);
                            break;
                        }
                        AppEvent::SessionStarted { session_id, .. } if session_id == &sid => {
                            agg.apply(&ev);
                        }
                        _ => {
                            // Apply only if event appears to belong to our session id when present
                            match &ev {
                                AppEvent::BatchReport { session_id, .. }
                                | AppEvent::CrawlReportSession { session_id, .. }
                                | AppEvent::Progress { session_id, .. }
                                | AppEvent::StageStarted { session_id, .. }
                                | AppEvent::StageCompleted { session_id, .. }
                                | AppEvent::StageFailed { session_id, .. }
                                | AppEvent::BatchStarted { session_id, .. }
                                | AppEvent::BatchCompleted { session_id, .. }
                                | AppEvent::BatchFailed { session_id, .. } => {
                                    if session_id == &sid { agg.apply(&ev); }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }).await;

    // Allow actor task to wind down if still running
    let _ = handle.await.ok()?;

    Some(agg)
}

#[tokio::test]
#[ignore = "set MC_RUN_PARITY=1 to enable; requires network/DB and emits real events"]
async fn parity_stage_vs_batch_smoke() {
    if std::env::var("MC_RUN_PARITY").ok().as_deref() != Some("1") { eprintln!("MC_RUN_PARITY not set; skipping"); return; }

    // Single run collects a compact summary depending on current env/config.
    // Run this test twice externally to compare: once with MC_USE_STAGE_DIRECT=1, once without.
    let res = run_once_collect().await;
    if let Some(agg) = res {
        // Print a concise line for side-by-side diff in CI logs
        println!(
            "PARITY_SUMMARY session={} started={} completed={} failed={} batches(s/c/f)={}/{}/{} pages(t/s/f)={}/{}/{} details(s/f)={}/{} dup={} ins={} upd={}",
            agg.session_id,
            agg.session_started, agg.session_completed, agg.session_failed,
            agg.batches_started, agg.batches_completed, agg.batches_failed,
            agg.pages_total, agg.pages_success, agg.pages_failed,
            agg.details_success, agg.details_failed,
            agg.duplicates_skipped, agg.products_inserted, agg.products_updated,
        );
    } else {
        eprintln!("PARITY_SUMMARY unavailable (run did not produce events)");
    }
}
