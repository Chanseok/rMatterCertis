//! BatchActor sanity runner to verify concurrency caps and StatusCheck de-dup when hints are present
//!
//! This binary bootstraps real services, creates an AppContext, and executes a single
//! BatchActor run with pages and SiteStatus hints provided. It sets MC_SKIP_DB_SAVE=1
//! to avoid database writes and uses a conservative page count if site status fails.

use std::sync::Arc;
use tracing::{info, warn};

use matter_certis_v2_lib::infrastructure::{
    HttpClient, MatterDataExtractor, IntegratedProductRepository,
};
use matter_certis_v2_lib::infrastructure::config::AppConfig;
use matter_certis_v2_lib::infrastructure::database_connection::DatabaseConnection;
use matter_certis_v2_lib::infrastructure::database_paths;
use matter_certis_v2_lib::new_architecture::actors::{BatchActor, types as actor_types, Actor};
use matter_certis_v2_lib::new_architecture::context::AppContext;
use matter_certis_v2_lib::new_architecture::integrated_context::IntegratedContextFactory;
use matter_certis_v2_lib::new_architecture::system_config::SystemConfig;
use matter_certis_v2_lib::domain::services::StatusChecker;
use matter_certis_v2_lib::infrastructure::crawling_service_impls::StatusCheckerImpl;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Use infrastructure logging so concise_startup and ENV overrides apply
    let _ = matter_certis_v2_lib::infrastructure::logging::init_logging();
    info!("concise_startup_env={:?}", std::env::var("MC_CONCISE_STARTUP").ok());
    // Expect MC_SKIP_DB_SAVE to be set by the environment to avoid DB writes
    info!("MC_SKIP_DB_SAVE={:?}", std::env::var("MC_SKIP_DB_SAVE").ok());

    info!("🚀 BatchActor sanity runner starting");

    // Build AppContext
    let system_config = Arc::new(SystemConfig::default());
    let factory = IntegratedContextFactory::new(Arc::clone(&system_config));
    let (session_ctx, channels) = factory
        .create_session_context("sanity-session".to_string())
        .expect("Failed to create IntegratedContext");

    // Spawn a simple event listener to drain events (optional)
    let mut event_rx = channels.event_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(evt) = event_rx.recv().await {
            tracing::debug!("[event] {:?}", evt);
        }
    });

    // Real services
    // Label the client so rate-limit provenance appears in logs
    let http_client = Arc::new(HttpClient::create_from_global_config()?.with_context_label("BatchActor:sanity"));
    let data_extractor = Arc::new(MatterDataExtractor::new()?);
    // Initialize database paths and schema (prevents global manager panic and ensures tables)
    database_paths::initialize_database_paths().await?;
    let database_url = database_paths::get_main_database_url();
    tracing::info!("🗄️ Using database: {}", database_url);
    let db = DatabaseConnection::new(&database_url).await?;
    db.migrate().await?;
    let db_pool = db.pool().clone();
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));

    // Site status (for hints); degrade gracefully if site unreachable
    let app_config = AppConfig::for_development();
    let status_checker: Arc<dyn StatusChecker> = Arc::new(StatusCheckerImpl::with_product_repo(
        (*http_client).clone(),
        (*data_extractor).clone(),
        app_config.clone(),
        Arc::clone(&product_repo),
    ));

    let (total_pages, products_on_last_page) = match status_checker.check_site_status().await {
        Ok(s) => {
            info!("🌐 SiteStatus: total_pages={}, products_on_last_page={}", s.total_pages, s.products_on_last_page);
            (s.total_pages, s.products_on_last_page)
        }
        Err(e) => {
            warn!("⚠️  Failed to fetch SiteStatus: {}. Using conservative defaults.", e);
            (50u32, 8u32)
        }
    };

    // Optionally force a specific page via env to simulate failures (e.g., MC_FAIL_PAGE=9999)
    let pages: Vec<u32> = if let Ok(forced_page_str) = std::env::var("MC_FAIL_PAGE") {
        if let Ok(forced_page) = forced_page_str.parse::<u32>() {
            info!("⚠️ Forcing sanity run to request page {} (may simulate failures)", forced_page);
            vec![forced_page]
        } else {
            warn!("Invalid MC_FAIL_PAGE value '{}', falling back to default pages", forced_page_str);
            if total_pages >= 2 { vec![total_pages, total_pages - 1] } else { vec![total_pages] }
        }
    } else {
        // Plan a tiny newest-first page slice: last 2 pages
        if total_pages >= 2 { vec![total_pages, total_pages - 1] } else { vec![total_pages] }
    };
    info!("📄 Target pages: {:?}", pages);

    // Wire BatchActor with real services
    let mut batch_actor = BatchActor::new_with_services(
        "sanity-batch-actor".to_string(),
        "sanity-batch".to_string(),
        Arc::clone(&http_client),
        Arc::clone(&data_extractor),
        Arc::clone(&product_repo),
        app_config,
    );

    // Run the actor
    let (tx, rx) = tokio::sync::mpsc::channel::<actor_types::ActorCommand>(100);
    let ctx: AppContext = session_ctx.clone();
    let actor_task = tokio::spawn(async move {
        let _ = batch_actor.run(ctx, rx).await;
    });

    // Send ProcessBatch with hints and capped concurrency
    let concurrency_limit = 5u32;
    let batch_config = actor_types::BatchConfig {
        batch_size: pages.len() as u32,
        concurrency_limit,
        batch_delay_ms: 250,
        retry_on_failure: true,
        start_page: pages.first().copied(),
        end_page: pages.last().copied(),
    };
    tx.send(actor_types::ActorCommand::ProcessBatch {
        batch_id: "sanity-batch".to_string(),
        pages: pages.clone(),
        config: batch_config,
        batch_size: pages.len() as u32,
        concurrency_limit,
        total_pages,
        products_on_last_page,
    }).await.expect("Failed to send ProcessBatch");

    // Shutdown after the run
    tx.send(actor_types::ActorCommand::Shutdown).await.expect("Failed to send Shutdown");

    actor_task.await.expect("Actor join failed");
    info!("🏁 Sanity run finished");
    Ok(())
}

// Note: using infrastructure logging; local init removed
