//! Session registry & failure policy management (extracted from actor_system_commands)
use std::collections::HashMap;
use std::sync::Arc;
use chrono::{Utc, DateTime};
use once_cell::sync::OnceCell;
use tokio::sync::{RwLock, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus { Running, Paused, Completed, Failed, ShuttingDown }

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub status: SessionStatus,
    pub pause_tx: watch::Sender<bool>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_pages_planned: u64,
    pub processed_pages: u64,
    pub total_batches_planned: u64,
    pub completed_batches: u64,
    pub batch_size: u32,
    pub concurrency_limit: u32,
    pub last_error: Option<String>,
    pub error_count: u32,
    pub resume_token: Option<String>,
    pub remaining_page_slots: Option<Vec<u32>>,
    pub plan_hash: Option<String>,
    pub removal_deadline: Option<DateTime<Utc>>,
    pub failed_emitted: bool,
    pub retries_per_page: HashMap<u32,u32>,
    pub failed_pages: Vec<u32>,
    pub retrying_pages: Vec<u32>,
    pub product_list_max_retries: u32,
    pub error_type_stats: HashMap<String,(u32, DateTime<Utc>, DateTime<Utc>)>,
    // Product details metrics
    pub detail_tasks_total: u64,
    pub detail_tasks_completed: u64,
    pub detail_tasks_failed: u64,
    // Detail task retry / remaining state (v2)
    pub detail_retry_counts: HashMap<String, u32>,
    pub detail_retries_total: u64,
    pub detail_retry_histogram: HashMap<u32, u32>,
    pub remaining_detail_ids: Option<Vec<String>>, // for resume token v2
    pub detail_failed_ids: Vec<String>,
    pub page_failure_threshold: u32,
    pub detail_failure_threshold: u32,
    pub detail_downshifted: bool,
    // Downshift metadata (first occurrence)
    pub detail_downshift_timestamp: Option<DateTime<Utc>>,
    pub detail_downshift_old_limit: Option<u32>,
    pub detail_downshift_new_limit: Option<u32>,
    pub detail_downshift_trigger: Option<String>,
}

static SESSION_REGISTRY: OnceCell<Arc<RwLock<HashMap<String, SessionEntry>>>> = OnceCell::new();
pub fn session_registry() -> Arc<RwLock<HashMap<String, SessionEntry>>> {
    SESSION_REGISTRY.get_or_init(|| Arc::new(RwLock::new(HashMap::new()))).clone()
}

#[derive(Debug, Clone)]
pub struct FailurePolicy { pub failure_threshold: u32, pub removal_grace_secs: i64 }
static GLOBAL_FAILURE_POLICY: OnceCell<RwLock<FailurePolicy>> = OnceCell::new();

pub fn failure_policy() -> FailurePolicy {
    GLOBAL_FAILURE_POLICY
    .get_or_init(|| RwLock::new(FailurePolicy { failure_threshold: 50, removal_grace_secs: 30 }))
    .try_read()
    .map(|g| g.clone())
    .unwrap_or(FailurePolicy { failure_threshold: 50, removal_grace_secs: 30 })
}
pub fn failure_threshold() -> u32 { failure_policy().failure_threshold }
pub fn removal_grace_secs() -> i64 { failure_policy().removal_grace_secs }

pub fn update_global_failure_policy_from_config(cfg: &crate::infrastructure::config::AppConfig) {
    if let Some(lock) = GLOBAL_FAILURE_POLICY.get() {
        if let Ok(mut guard) = lock.try_write() {
            let env_fail = std::env::var("APP_FAILURE_THRESHOLD").ok().and_then(|v| v.parse::<u32>().ok());
            let env_grace = std::env::var("APP_REMOVAL_GRACE_SECS").ok().and_then(|v| v.parse::<i64>().ok());
            let th = env_fail.unwrap_or_else(|| cfg.advanced.failure_policy.failure_threshold);
            let grace = env_grace.unwrap_or_else(|| cfg.advanced.failure_policy.removal_grace_secs);
            guard.failure_threshold = th.max(1);
            guard.removal_grace_secs = grace.max(5);
        }
    }
}
