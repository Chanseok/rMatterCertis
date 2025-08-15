//! ProductDetailsActor: ListPages 단계에서 수집된 페이지/슬롯 기반으로 상세 제품 정보를 수집.
//! Phase 1: Skeleton – 실제 파싱/저장 로직은 후속 구현.
use std::sync::Arc;
use tokio::sync::Semaphore;
use chrono::Utc;
use tracing::info;
// use existing lightweight RNG (fastrand) already in Cargo.toml
use crate::new_architecture::runtime::session_registry::session_registry;

use crate::new_architecture::channels::types::AppEvent;
use crate::new_architecture::actors::types::ExecutionPlan;
use crate::new_architecture::context::AppContext;

#[derive(Debug, Clone)]
pub struct ProductDetailsActorConfig {
    pub max_concurrency: u32,
    pub per_item_timeout_ms: u64,
    pub max_retries: u32,
}

#[derive(Debug)]
pub struct ProductDetailsActor {
    session_id: String,
    config: ProductDetailsActorConfig,
}

impl ProductDetailsActor {
    pub fn new(session_id: String, config: ProductDetailsActorConfig) -> Self {
        Self { session_id, config }
    }

    /// Skeleton 실행: 지정한 개수만큼 detail task 시뮬레이션
    pub async fn run(
        &self,
        _context: Arc<AppContext>,
        plan: Arc<ExecutionPlan>,
        event_tx: tokio::sync::broadcast::Sender<AppEvent>,
    ) -> anyhow::Result<()> {
        // Fetch injected detail IDs from registry (set after ListPages phase). If absent, fallback to small simulated set.
        let detail_ids: Vec<String> = {
            let registry = session_registry();
            let g = registry.read().await;
            if let Some(entry) = g.get(&self.session_id) {
                if let Some(ids) = &entry.remaining_detail_ids { if !ids.is_empty() { ids.clone() } else { Vec::new() } } else { Vec::new() }
            } else { Vec::new() }
        };
        if detail_ids.is_empty() {
            // Fallback simulation (development mode)
            let sim_count = 3usize.min(plan.page_slots.len());
            let simulated: Vec<String> = (0..sim_count).map(|i| format!("detail_sim_{}", i)).collect();
            let registry = session_registry();
            let mut g = registry.write().await;
            if let Some(entry) = g.get_mut(&self.session_id) {
                entry.detail_tasks_total = simulated.len() as u64;
                entry.remaining_detail_ids = Some(simulated.clone());
            }
            self.run_internal(simulated, event_tx.clone()).await?;
            return Ok(());
        } else {
            // Update total if first time
            let registry = session_registry();
            let mut g = registry.write().await;
            if let Some(entry) = g.get_mut(&self.session_id) {
                if entry.detail_tasks_total == 0 { entry.detail_tasks_total = detail_ids.len() as u64; }
            }
        }
        self.run_internal(detail_ids, event_tx.clone()).await?;
        Ok(())
    }

    async fn run_internal(&self, detail_ids: Vec<String>, event_tx: tokio::sync::broadcast::Sender<AppEvent>) -> anyhow::Result<()> {
        let sem = Arc::new(Semaphore::new(self.config.max_concurrency as usize));
        let mut handles = Vec::new();
        for did in detail_ids.clone() {
            let permit = sem.clone().acquire_owned().await?;
            let session_id = self.session_id.clone();
            let tx = event_tx.clone();
            let cfg = self.config.clone();
            let sem_clone_for_downshift = sem.clone();
            handles.push(tokio::spawn(async move {
                let started_at = std::time::Instant::now();
                let _ = tx.send(AppEvent::DetailTaskStarted {
                    session_id: session_id.clone(),
                    detail_id: did.clone(),
                    page: None,
                    batch_id: None,
                    range_idx: None,
                    batch_index: None,
                    scope: Some("session".to_string()),
                    timestamp: Utc::now(),
                });
                let mut attempt: u32 = 0;
                loop {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    let roll = fastrand::f32();
                    let (fail, err_type) = if roll < 0.12 { (true, if roll < 0.04 { "NetworkError" } else if roll < 0.08 { "TimeoutError" } else { "ParsingError" }) } else { (false, "") };
                    if !fail {
                        let dur_ms = started_at.elapsed().as_millis() as u64;
                        let _ = tx.send(AppEvent::DetailTaskCompleted {
                            session_id: session_id.clone(),
                            detail_id: did.clone(),
                            page: None,
                            duration_ms: dur_ms,
                            batch_id: None,
                            range_idx: None,
                            batch_index: None,
                            scope: Some("session".to_string()),
                            timestamp: Utc::now(),
                        });
                        let registry = session_registry();
                        let mut g = registry.write().await;
                        if let Some(e) = g.get_mut(&session_id) {
                            e.detail_tasks_completed += 1;
                            if let Some(ref mut rem) = e.remaining_detail_ids { rem.retain(|r| r != &did); }
                        }
                        break;
                    } else {
                        let mut final_failure = false;
                        if attempt > cfg.max_retries { final_failure = true; }
                        let registry = session_registry();
                        let mut g = registry.write().await;
                        if let Some(e) = g.get_mut(&session_id) {
                            let c = e.detail_retry_counts.entry(did.clone()).or_insert(0); *c += 1;
                            e.detail_retries_total += 1;
                            e.detail_retry_histogram.entry(*c).and_modify(|v| *v += 1).or_insert(1);
                            if !e.detail_downshifted {
                                let processed = (e.detail_tasks_completed + e.detail_tasks_failed).max(1) as f64;
                                let fail_rate = e.detail_tasks_failed as f64 / processed;
                                if fail_rate > 0.30 {
                                    let old = cfg.max_concurrency;
                                    e.detail_downshifted = true;
                                    let new_limit = (old / 2).max(1);
                                    if e.detail_downshift_timestamp.is_none() {
                                        e.detail_downshift_timestamp = Some(Utc::now());
                                        e.detail_downshift_old_limit = Some(old);
                                        e.detail_downshift_new_limit = Some(new_limit);
                                        e.detail_downshift_trigger = Some(format!("fail_rate>{:.2}", fail_rate));
                                    }
                                    // Downshift: reduce available permits by half (best effort)
                                    let current = sem_clone_for_downshift.available_permits();
                                    if (new_limit as usize) < current {
                                        let to_consume = current - new_limit as usize;
                                        for _ in 0..to_consume { let _ = sem_clone_for_downshift.try_acquire(); }
                                    }
                                    let _ = tx.send(AppEvent::DetailConcurrencyDownshifted { session_id: session_id.clone(), old_limit: old, new_limit, trigger: format!("fail_rate>{:.2}", fail_rate), timestamp: Utc::now() });
                                }
                            }
                        }
                        if final_failure {
                            let err = format!("simulated_detail_error:{}", err_type);
                            let _ = tx.send(AppEvent::DetailTaskFailed {
                                session_id: session_id.clone(),
                                detail_id: did.clone(),
                                page: None,
                                error: err.clone(),
                                final_failure: true,
                                batch_id: None,
                                range_idx: None,
                                batch_index: None,
                                scope: Some("session".to_string()),
                                timestamp: Utc::now(),
                            });
                            let registry = session_registry();
                            let mut g = registry.write().await;
                            if let Some(e) = g.get_mut(&session_id) {
                                e.detail_tasks_failed += 1;
                                e.detail_failed_ids.push(did.clone());
                                if let Some(ref mut rem) = e.remaining_detail_ids { rem.retain(|r| r != &did); }
                                if e.detail_tasks_failed as u32 >= e.detail_failure_threshold && !e.failed_emitted {
                                    e.failed_emitted = true;
                                    e.status = crate::new_architecture::runtime::session_registry::SessionStatus::Failed;
                                    e.completed_at = Some(Utc::now());
                                    e.removal_deadline = Some(Utc::now() + chrono::Duration::seconds(crate::new_architecture::runtime::session_registry::removal_grace_secs()));
                                }
                            }
                            break;
                        } else {
                            let base = match err_type { "NetworkError" => 90, "TimeoutError" => 140, "ParsingError" => 60, _ => 40 } as u64;
                            let jitter = fastrand::u64(0..25);
                            tokio::time::sleep(std::time::Duration::from_millis(base * attempt as u64 + jitter)).await;
                            continue;
                        }
                    }
                }
                drop(permit);
            }));
        }
        for h in handles { let _ = h.await; }
        info!("[ProductDetailsActor] detail collection phase done (total={}, completed, failed captured in registry)", detail_ids.len());
        let sem = Arc::new(Semaphore::new(self.config.max_concurrency as usize));
        let mut handles = Vec::new();
        for did in detail_ids.clone() {
            let permit = sem.clone().acquire_owned().await?;
            let session_id = self.session_id.clone();
            let tx = event_tx.clone();
            let cfg = self.config.clone();
            handles.push(tokio::spawn(async move {
                let started_at = std::time::Instant::now();
                let _ = tx.send(AppEvent::DetailTaskStarted {
                    session_id: session_id.clone(),
                    detail_id: did.clone(),
                    page: None,
                    batch_id: None,
                    range_idx: None,
                    batch_index: None,
                    scope: Some("session".to_string()),
                    timestamp: Utc::now(),
                });
                let mut attempt: u32 = 0;
                loop {
                    attempt += 1;
                    // Simulated work placeholder
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    // random failure classification
                    let roll = fastrand::f32();
                    let (fail, err_type) = if roll < 0.12 { (true, if roll < 0.04 { "NetworkError" } else if roll < 0.08 { "TimeoutError" } else { "ParsingError" }) } else { (false, "") };
                    if !fail {
                        let dur_ms = started_at.elapsed().as_millis() as u64;
                        let _ = tx.send(AppEvent::DetailTaskCompleted {
                            session_id: session_id.clone(),
                            detail_id: did.clone(),
                            page: None,
                            duration_ms: dur_ms,
                            batch_id: None,
                            range_idx: None,
                            batch_index: None,
                            scope: Some("session".to_string()),
                            timestamp: Utc::now(),
                        });
                        let registry = session_registry();
                        let mut g = registry.write().await;
                        if let Some(e) = g.get_mut(&session_id) {
                            e.detail_tasks_completed += 1;
                            if let Some(ref mut rem) = e.remaining_detail_ids { rem.retain(|r| r != &did); }
                        }
                        break;
                    } else {
                        let mut final_failure = false;
                        if attempt > cfg.max_retries { final_failure = true; }
                        // update retry stats
                        let registry = session_registry();
                        let mut g = registry.write().await;
                        if let Some(e) = g.get_mut(&session_id) {
                            let c = e.detail_retry_counts.entry(did.clone()).or_insert(0); *c += 1;
                            e.detail_retries_total += 1;
                            e.detail_retry_histogram.entry(*c).and_modify(|v| *v += 1).or_insert(1);
                            // dynamic downshift flag (only marks; actual concurrency adjustment not yet applied globally)
                            if !e.detail_downshifted {
                                let processed = (e.detail_tasks_completed + e.detail_tasks_failed).max(1) as f64;
                                let fail_rate = e.detail_tasks_failed as f64 / processed;
                                if fail_rate > 0.30 { 
                                    e.detail_downshifted = true; 
                                    if e.detail_downshift_timestamp.is_none() {
                                        e.detail_downshift_timestamp = Some(Utc::now());
                                        e.detail_downshift_old_limit = Some(cfg.max_concurrency);
                                        e.detail_downshift_new_limit = Some((cfg.max_concurrency/2).max(1));
                                        e.detail_downshift_trigger = Some(format!("fail_rate>{:.2}", fail_rate));
                                    }
                                }
                            }
                        }
                        if final_failure {
                            let err = format!("simulated_detail_error:{}", err_type);
                            let _ = tx.send(AppEvent::DetailTaskFailed {
                                session_id: session_id.clone(),
                                detail_id: did.clone(),
                                page: None,
                                error: err.clone(),
                                final_failure: true,
                                batch_id: None,
                                range_idx: None,
                                batch_index: None,
                                scope: Some("session".to_string()),
                                timestamp: Utc::now(),
                            });
                            let registry = session_registry();
                            let mut g = registry.write().await;
                            if let Some(e) = g.get_mut(&session_id) {
                                e.detail_tasks_failed += 1;
                                e.detail_failed_ids.push(did.clone());
                                if let Some(ref mut rem) = e.remaining_detail_ids { rem.retain(|r| r != &did); }
                                if e.detail_tasks_failed as u32 >= e.detail_failure_threshold && !e.failed_emitted {
                                    e.failed_emitted = true;
                                    e.status = crate::new_architecture::runtime::session_registry::SessionStatus::Failed;
                                    e.completed_at = Some(Utc::now());
                                    e.removal_deadline = Some(Utc::now() + chrono::Duration::seconds(crate::new_architecture::runtime::session_registry::removal_grace_secs()));
                                }
                            }
                            break;
                        } else {
                            // error-type based backoff (simple linear * attempt + jitter)
                            let base = match err_type { "NetworkError" => 90, "TimeoutError" => 140, "ParsingError" => 60, _ => 40 } as u64;
                            let jitter = fastrand::u64(0..25);
                            tokio::time::sleep(std::time::Duration::from_millis(base * attempt as u64 + jitter)).await;
                            continue;
                        }
                    }
                }
                drop(permit);
            }));
        }
        for h in handles { let _ = h.await; }
        Ok(())
    }
}
