//! Test utilities for building common DI bundles
//! Only compiled for tests

#![allow(clippy::module_name_repetitions)]

use std::sync::Arc;

use crate::crawl_engine::actors::stage_actor::StageDeps;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};

/// Build a StageDeps bundle backed by an in-memory SQLite repository.
/// Returns the deps and the repo for optional direct assertions.
#[cfg(test)]
pub async fn make_stage_deps_in_memory(
    app_config: Option<AppConfig>,
) -> (StageDeps, Arc<IntegratedProductRepository>) {
    let app_cfg = app_config.unwrap_or_default();
    let http_client: Arc<HttpClient> =
        Arc::new(app_cfg.create_http_client().expect("http client"));
    let extractor: Arc<MatterDataExtractor> =
        Arc::new(MatterDataExtractor::new().expect("extractor"));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("memory pool");
    let repo = Arc::new(IntegratedProductRepository::new(pool));
    let deps = StageDeps {
        http_client: Arc::clone(&http_client),
        data_extractor: Arc::clone(&extractor),
        product_repo: Arc::clone(&repo),
        app_config: app_cfg,
        duplicate_policy:
            crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
    };
    (deps, repo)
}

/// Convenience builder: package already-constructed deps into StageDeps.
#[cfg(test)]
pub fn make_stage_deps_from_parts(
    http_client: Arc<HttpClient>,
    extractor: Arc<MatterDataExtractor>,
    repo: Arc<IntegratedProductRepository>,
    app_config: AppConfig,
    duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy,
) -> StageDeps {
    StageDeps {
        http_client,
        data_extractor: extractor,
        product_repo: repo,
        app_config,
        duplicate_policy,
    }
}
