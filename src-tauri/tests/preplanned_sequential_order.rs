//! Integration test: preplanned `ExecutionPlan` runs batches sequentially in order.
//!
//! Note: This test uses the real pipeline (HTTP/DB). It is marked ignored by default
//! because it requires network access and a local DB setup. Unignore to run locally.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use matter_certis_v2_lib::crawl_engine::SessionActor;
use matter_certis_v2_lib::crawl_engine::actors::Actor; // bring trait for run()
use matter_certis_v2_lib::crawl_engine::actors::types::{
    ActorCommand, AppEvent, ExecutionPlan, PageRange, PlanInputSnapshot,
};
use matter_certis_v2_lib::crawl_engine::integrated_context::IntegratedContextFactory;
use matter_certis_v2_lib::crawl_engine::system_config::SystemConfig;
use tokio::sync::mpsc;

#[tokio::test]
#[ignore = "set MC_RUN_PREPLANNED_IT=1 to enable; requires network and DB"]
#[allow(clippy::too_many_lines)]
async fn preplanned_batches_run_sequentially_in_order() {
    // Toggle guard
    if std::env::var("MC_RUN_PREPLANNED_IT").ok().as_deref() != Some("1") {
        eprintln!("MC_RUN_PREPLANNED_IT not set; skipping test body");
        return;
    }
    // Arrange: tiny two-range plan, reverse order newest->oldest
    let session_id = format!("actor_session_{}", chrono::Utc::now().timestamp());
    let plan = ExecutionPlan {
        plan_id: "plan_seq".into(),
        session_id: session_id.clone(),
        crawling_ranges: vec![
            PageRange {
                start_page: 10,
                end_page: 9,
                estimated_products: 24,
                reverse_order: true,
            },
            PageRange {
                start_page: 8,
                end_page: 8,
                estimated_products: 12,
                reverse_order: true,
            },
        ],
        batch_size: 2,
        concurrency_limit: 2,
        estimated_duration_secs: 0,
        created_at: Utc::now(),
        analysis_summary: "seq test".into(),
        original_strategy: "Manual".into(),
        input_snapshot: PlanInputSnapshot {
            total_pages: 20,
            products_on_last_page: 12,
            db_max_page_id: None,
            db_max_index_in_page: None,
            db_total_products: 0,
            page_range_limit: 100,
            batch_size: 2,
            concurrency_limit: 2,
            created_at: Utc::now(),
        },
        plan_hash: "h_seq".into(),
        skip_duplicate_urls: true,
        kpi_meta: None,
        contract_version: 1,
        page_slots: vec![],
        list_only: false,
    };

    // Build context and channels
    let config = Arc::new(SystemConfig::default());
    let factory = IntegratedContextFactory::new(config);
    let (context, channels) = factory
        .create_session_context(session_id.clone())
        .expect("context");
    let mut event_rx = channels.event_tx.subscribe();
    // Dedicated command channel for SessionActor (uses actors::types::ActorCommand)
    let (cmd_tx, cmd_rx) = mpsc::channel::<ActorCommand>(100);

    // Spawn SessionActor run loop
    let mut session = SessionActor::new(session_id.clone());
    let handle = tokio::spawn(async move { session.run(context, cmd_rx).await });

    // Trigger preplanned execution
    // Use the context's control_tx via helper
    let cmd = ActorCommand::ExecutePrePlanned {
        session_id: session_id.clone(),
        plan,
    };
    cmd_tx.send(cmd).await.expect("send ExecutePrePlanned");

    // Assert: observe BatchStarted/Completed for ...-pre-1 then ...-pre-2 in order
    let mut saw_started_1 = false;
    let mut saw_completed_1 = false;
    let mut saw_started_2 = false;
    let mut saw_completed_2 = false;
    let mut saw_plan_id = false;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        if tokio::time::Instant::now() > deadline {
            break;
        }
        if let Ok(Ok(ev)) = tokio::time::timeout(Duration::from_secs(5), event_rx.recv()).await {
            match ev {
                AppEvent::BatchStarted {
                    batch_id, plan_id, ..
                } => {
                    if plan_id.is_some() {
                        saw_plan_id = true;
                    }
                    if batch_id.ends_with("-pre-1") {
                        // first batch must start before the second
                        assert!(!saw_started_2, "Batch 2 started before Batch 1");
                        saw_started_1 = true;
                    } else if batch_id.ends_with("-pre-2") {
                        saw_started_2 = true;
                        assert!(saw_completed_1, "Batch 2 started before Batch 1 completed");
                    }
                }
                AppEvent::BatchCompleted {
                    batch_id, plan_id, ..
                } => {
                    if plan_id.is_some() {
                        saw_plan_id = true;
                    }
                    if batch_id.ends_with("-pre-1") {
                        saw_completed_1 = true;
                    } else if batch_id.ends_with("-pre-2") {
                        saw_completed_2 = true;
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    let _ = handle.await;
    assert!(
        saw_started_1 && saw_completed_1 && saw_started_2 && saw_completed_2,
        "Expected sequential preplanned batches to start and complete in order"
    );
    assert!(
        saw_plan_id,
        "Expected plan_id to be present on batch events"
    );
}
