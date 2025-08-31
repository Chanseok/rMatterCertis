//! `SessionActor`: 크롤링 세션 관리 Actor
//!
//! Phase 3: Actor 구현 - 세션 레벨 제어 및 모니터링
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::Utc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::crawl_engine::actors::types::SessionSummary;

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, ActorError, CrawlingConfig};
use crate::crawl_engine::channels::types::AppEvent;
use crate::crawl_engine::context::AppContext;
use std::sync::Arc;

use crate::crawl_engine::actors::BatchActor;
use crate::crawl_engine::actors::types::BatchConfig;
use crate::crawl_engine::services::CrawlingPlanner;
use crate::domain::services::{DatabaseAnalyzer, StatusChecker};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::crawling_service_impls::{DatabaseAnalyzerImpl, StatusCheckerImpl};
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};

/// Shared dependencies used across planning and batch execution
struct SessionDeps {
    http_client: Arc<HttpClient>,
    data_extractor: Arc<MatterDataExtractor>,
    product_repo: Arc<IntegratedProductRepository>,
}

/// `SessionActor`: 크롤링 세션의 전체 생명주기 관리
///
/// 책임:
/// - 세션 시작/일시정지/재개/종료 제어
/// - 배치 Actor들의 조정 및 모니터링
/// - 세션 레벨 이벤트 발행
/// - 전체 세션 상태 추적
#[derive(Debug)]
pub struct SessionActor {
    /// Actor 고유 식별자
    actor_id: String,
    /// 현재 관리 중인 세션 ID
    session_id: Option<String>,
    /// 세션 상태
    state: SessionState,
    /// 세션 시작 시간
    start_time: Option<Instant>,
    /// 처리된 배치 수
    processed_batches: u32,
    /// 총 성공 아이템 수
    total_success_count: u32,
    /// 세션 레벨 `SiteStatus` 캐시
    site_status_cache: Option<(crate::domain::services::SiteStatus, Instant)>,
    /// 이미 외부에서 확정된 계획을 실행 중인지 여부 (재계획 방지)
    preplanned_mode: bool,
    /// 배치 완료 시 삽입된 제품 수
    products_inserted: u32,
    /// 배치 완료 시 업데이트된 제품 수
    products_updated: u32,
    /// 현재 실행 중인 `ExecutionPlan` 해시 (무결성 로그 목적)
    active_plan_hash: Option<String>,
    /// 배치/세션 실행 중 발생한 에러 메시지 누적 (요약/리포트 용)
    errors: Vec<String>,
    /// 세션 누적 duplicate skip 합계
    duplicates_skipped: u32,
    // Unified detail crawling accumulation across batches
    aggregated_product_urls: Vec<crate::domain::product_url::ProductUrl>,
    /// 단일 실행 계획 (세션 동안 불변, 재계산 금지)
    crawling_plan:
        Option<std::sync::Arc<crate::crawl_engine::services::crawling_planner::CrawlingPlan>>,
    /// 계획 버전 (향후 재계산 허용 시 증가) 현재 0 또는 1
    plan_version: u64,
    /// Completion event already emitted
    completion_emitted: bool,
}

/// 세션 상태 열거형
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Starting,
    Planned, // CrawlingPlan 확보 완료, 실행 전 상태
    Running,
    Paused { reason: String },
    Completing,
    Completed,
    Failed { error: String },
}

/// 세션 관련 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Session already running: {0}")]
    AlreadyRunning(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: SessionState,
        to: SessionState,
    },

    #[error("Context communication error: {0}")]
    ContextError(String),
}

impl SessionActor {
    /// Transition helper (kept permissive for now; hook for future guards)
    fn transition(&mut self, to: SessionState) -> Result<(), SessionError> {
        self.state = to;
        Ok(())
    }

    /// Emit a preflight DB snapshot with optional site status info
    async fn emit_preflight_db_snapshot(
        &self,
        context: &AppContext,
        product_repo: &IntegratedProductRepository,
        session_id: &str,
        site_total_pages: Option<u32>,
        site_known_last_page: Option<u32>,
        reason: &str,
    ) -> Result<(), SessionError> {
        if let Ok((cnt, minp, maxp, _last)) = product_repo.get_product_detail_stats().await {
            let evt = AppEvent::PreflightDiagnostics {
                session_id: session_id.to_string(),
                db_products: cnt,
                db_min_page: minp,
                db_max_page: maxp,
                site_total_pages,
                site_known_last_page,
                reason: Some(reason.to_string()),
                timestamp: Utc::now(),
            };
            if let Err(e) = context.emit_event(evt) {
                warn!("[DiagEmit] {} emit failed err={}", reason, e);
            } else {
                info!(
                    "[DiagEmit] {} emitted products={} page_range={:?}-{:?} site_total_pages={:?}",
                    reason, cnt, minp, maxp, site_total_pages
                );
            }
        }
        Ok(())
    }

    /// Plan the session using CrawlingPlanner with optional cached site status.
    async fn plan_session(
        &mut self,
        config: &CrawlingConfig,
        context: &AppContext,
        deps: &SessionDeps,
    ) -> Result<(
        crate::crawl_engine::services::crawling_planner::CrawlingPlan,
        crate::domain::services::SiteStatus,
    ), SessionError> {
        let status_checker: Arc<dyn StatusChecker> = Arc::new(StatusCheckerImpl::with_product_repo(
            deps.http_client.as_ref().clone(),
            deps.data_extractor.as_ref().clone(),
            AppConfig::for_development(),
            Arc::clone(&deps.product_repo),
        ));
        let db_analyzer: Arc<dyn DatabaseAnalyzer> =
            Arc::new(DatabaseAnalyzerImpl::new(Arc::clone(&deps.product_repo)));
        let planner = CrawlingPlanner::new(status_checker.clone(), db_analyzer, Arc::clone(&context.config))
            .with_repository(Arc::clone(&deps.product_repo));

        // TTL 5m cache for site status
        let ttl = Duration::from_secs(300);
        let cached = self.site_status_cache.as_ref().and_then(|(status, ts)| {
            if ts.elapsed() <= ttl { Some(status.clone()) } else { None }
        });
        if self.crawling_plan.is_some() {
            warn!(
                "[PlanInit] CrawlingPlan already exists for current session, duplicate planning suppressed"
            );
        }
        let (plan, used_site_status) = planner
            .create_crawling_plan_with_cache(config, cached)
            .await
            .map_err(|e| SessionError::InitializationFailed(format!(
                "Failed to create crawling plan: {e}"
            )))?;

        self.plan_version = 1;
        self.crawling_plan = Some(std::sync::Arc::new(plan.clone()));
        Ok((plan, used_site_status))
    }

    /// Emit a simple "plan ready" progress event
    fn emit_plan_ready(
        &self,
        context: &AppContext,
        session_id: &str,
        plan: &crate::crawl_engine::services::crawling_planner::CrawlingPlan,
    ) -> Result<(), SessionError> {
        let evt = AppEvent::Progress {
            session_id: session_id.to_string(),
            current_step: 0,
            total_steps: plan.phases.len() as u32,
            message: format!(
                "Crawling plan ready: {} phases, list-batches={}",
                plan.phases.len(),
                plan.phases
                    .iter()
                    .filter(|p| matches!(
                        p.phase_type,
                        crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
                    ))
                    .count()
            ),
            percentage: 0.0,
            timestamp: Utc::now(),
        };
        context
            .emit_event(evt)
            .map(|_| ())
            .map_err(|e| SessionError::ContextError(e.to_string()))
    }

    /// Run all list-page batches in the plan sequentially, honoring cancellation.
    async fn run_list_batches(
        &mut self,
        context: &AppContext,
        session_id: &str,
        plan: &crate::crawl_engine::services::crawling_planner::CrawlingPlan,
    deps: &SessionDeps,
        site_status: &crate::domain::services::SiteStatus,
    ) -> Result<usize, SessionError> {
        let planned_list_batches: Vec<_> = plan
            .phases
            .iter()
            .filter(|p| {
                matches!(
                    p.phase_type,
                    crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
                )
            })
            .collect();
        if planned_list_batches.is_empty() {
            warn!(
                "⚠️ No ListPageCrawling phases planned (requested start/end maybe collapsed)."
            );
        } else {
            let mut agg: Vec<u32> = planned_list_batches
                .iter()
                .flat_map(|p| p.pages.clone())
                .collect();
            agg.sort_unstable();
            agg.dedup();
            info!(
                "🧪 Planned list page batches={} unique_pages_total={} pages={:?}",
                planned_list_batches.len(),
                agg.len(),
                agg
            );
        }
        let planned_batches_count = planned_list_batches.len();
        let mut batch_idx = 0u32;
        for phase in &planned_list_batches {
            if context.is_cancelled() {
                info!("⏹️ Cancellation detected before starting next batch; exiting run loop");
                break;
            }
            batch_idx += 1;
            let pages = phase.pages.clone();
            if pages.is_empty() { continue; }
            let batch_id = format!("{session_id}-batch-{batch_idx}");
            info!(
                "🏃 SessionActor {} running batch {} (batch_index={}/{}) with {} pages: {:?}",
                self.actor_id,
                batch_id,
                batch_idx,
                planned_batches_count,
                pages.len(),
                pages
            );
            let start_evt = AppEvent::Progress {
                session_id: session_id.to_string(),
                current_step: 1,
                total_steps: plan.phases.len() as u32,
                message: format!("Starting batch {} with {} pages", batch_id, pages.len()),
                percentage: (f64::from(batch_idx - 1) / plan.phases.len() as f64) * 100.0,
                timestamp: Utc::now(),
            };
            context
                .emit_event(start_evt)
                .map_err(|e| SessionError::ContextError(e.to_string()))?;

            if let Err(e) = self
                .run_batch_with_services(
                    &batch_id,
                    &pages,
                    context,
                    deps,
                    site_status,
                    None,
                    None,
                )
                .await
            {
                error!("❌ Batch {} failed: {}", batch_id, e);
                self.errors.push(format!("batch {batch_id}: {e}"));
                let fail_event = AppEvent::SessionFailed { session_id: session_id.to_string(), error: format!("Batch {batch_id} failed: {e}"), final_failure: false, timestamp: Utc::now() };
                context
                    .emit_event(fail_event)
                    .map_err(|er| SessionError::ContextError(er.to_string()))?;
                continue;
            }

            self.processed_batches += 1;
            self.total_success_count += pages.len() as u32;
            info!("✅ Completed batch {} ({} pages)", batch_id, pages.len());
            let done_evt = AppEvent::Progress {
                session_id: session_id.to_string(),
                current_step: 2,
                total_steps: plan.phases.len() as u32,
                message: format!("Completed batch {} ({} pages)", batch_id, pages.len()),
                percentage: (f64::from(batch_idx) / plan.phases.len() as f64) * 100.0,
                timestamp: Utc::now(),
            };
            context
                .emit_event(done_evt)
                .map_err(|e| SessionError::ContextError(e.to_string()))?;
        }
        Ok(planned_batches_count)
    }

    /// Run batches for a preplanned ExecutionPlan (ranges map 1:1 to batches)
    async fn run_preplanned_batches(
        &mut self,
        context: &AppContext,
        session_id: &str,
        plan: &crate::crawl_engine::actors::types::ExecutionPlan,
        deps: &SessionDeps,
        site_status: &crate::domain::services::SiteStatus,
    ) -> Result<usize, SessionError> {
        let planned_batches = plan.crawling_ranges.len();
        for (idx, range) in plan.crawling_ranges.iter().enumerate() {
            // Build physical pages respecting reverse_order flag
            let pages: Vec<u32> = if range.reverse_order {
                (range.end_page..=range.start_page).rev().collect()
            } else {
                (range.start_page..=range.end_page).collect()
            };
            if pages.is_empty() { continue; }
            let batch_id = format!("{}-pre-{}", session_id, idx + 1);
            if let Err(e) = self
                .run_batch_with_services(
                    &batch_id,
                    &pages,
                    context,
                    deps,
                    site_status,
                    Some(plan.skip_duplicate_urls),
                    Some(plan.plan_id.clone()),
                )
                .await
            {
                error!("Batch {} failed: {}", batch_id, e);
                self.errors.push(format!("batch {batch_id}: {e}"));
                let fail_event = AppEvent::SessionFailed {
                    session_id: session_id.to_string(),
                    error: format!("Batch {batch_id} failed: {e}"),
                    final_failure: false,
                    timestamp: Utc::now(),
                };
                if let Err(er) = context.emit_event(fail_event) {
                    error!("emit batch fail event error: {}", er);
                }
            }
            self.processed_batches = self.processed_batches.saturating_add(1);
            self.total_success_count = self
                .total_success_count
                .saturating_add(pages.len() as u32);
        }
        Ok(planned_batches)
    }

    /// Compute a follow-up plan after session completion and emit NextPlanReady
    async fn compute_and_emit_next_plan_ready(
        &self,
        context: &AppContext,
        config: &CrawlingConfig,
        session_id: &str,
    ) -> Result<(), SessionError> {
        let http_client = Arc::new(HttpClient::create_from_global_config().map_err(|e| {
            SessionError::InitializationFailed(format!("Failed to create HttpClient: {e}"))
        })?);
        let data_extractor = Arc::new(MatterDataExtractor::new().map_err(|e| {
            SessionError::InitializationFailed(format!(
                "Failed to create MatterDataExtractor: {e}"
            ))
        })?);
        let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
            .await
            .map_err(|e| {
                SessionError::InitializationFailed(format!(
                    "Failed to obtain database pool: {e}"
                ))
            })?;
        let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));

        let status_checker: Arc<dyn StatusChecker> = Arc::new(StatusCheckerImpl::with_product_repo(
            http_client.as_ref().clone(),
            data_extractor.as_ref().clone(),
            AppConfig::for_development(),
            Arc::clone(&product_repo),
        ));
        let db_analyzer: Arc<dyn DatabaseAnalyzer> =
            Arc::new(DatabaseAnalyzerImpl::new(Arc::clone(&product_repo)));
        let planner = CrawlingPlanner::new(
            status_checker.clone(),
            db_analyzer,
            Arc::clone(&context.config),
        )
        .with_repository(Arc::clone(&product_repo));

        // 기본 전략: 기존 config 기반 (현재 그대로 복제)
        let next_config = CrawlingConfig {
            site_url: config.site_url.clone(),
            start_page: config.start_page,
            end_page: config.end_page,
            concurrency_limit: config.concurrency_limit,
            batch_size: config.batch_size,
            request_delay_ms: config.request_delay_ms,
            timeout_secs: config.timeout_secs,
            max_retries: config.max_retries,
            strategy: config.strategy.clone(),
        };
        let (next_plan, _used_site_status) = planner
            .create_crawling_plan_with_cache(&next_config, None)
            .await
            .map_err(|e| {
                SessionError::InitializationFailed(format!(
                    "Failed to create next crawling plan: {e}"
                ))
            })?;

        let next_event = AppEvent::NextPlanReady {
            session_id: session_id.to_string(),
            plan: next_plan,
            timestamp: Utc::now(),
        };
        context
            .emit_event(next_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        Ok(())
    }

    /// Emit completion events and KPI logs using aggregated session summary
    fn emit_session_completion(
        &mut self,
        context: &AppContext,
        session_id: &str,
        planned_batches_count: usize,
    ) -> Result<(), SessionError> {
        // 완료 이벤트 발행 (집계된 요약 사용)
        let aggregated_summary = self.create_session_summary().unwrap_or(SessionSummary {
            session_id: session_id.to_string(),
            total_duration_ms: self
                .start_time
                .map_or(0, |t| t.elapsed().as_millis() as u64),
            total_pages_processed: self.total_success_count,
            total_products_processed: self.products_inserted + self.products_updated,
            success_rate: if self.total_success_count > 0 { 1.0 } else { 0.0 },
            avg_page_processing_time: if self.total_success_count > 0 {
                self.start_time
                    .map_or(0, |t| t.elapsed().as_millis() as u64 / u64::from(self.total_success_count))
            } else {
                0
            },
            error_summary: Vec::new(),
            total_retry_events: 0,
            max_retries_single_page: 0,
            pages_retried: 0,
            retry_histogram: Vec::new(),
            processed_batches: self.processed_batches,
            total_success_count: self.total_success_count,
            duplicates_skipped: self.duplicates_skipped,
            planned_list_batches: planned_batches_count as u32,
            executed_list_batches: self.processed_batches,
            failed_pages_count: 0,
            failed_page_ids: Vec::new(),
            products_inserted: self.products_inserted,
            products_updated: self.products_updated,
            final_state: "completed".to_string(),
            timestamp: Utc::now(),
        });

        let completion_event = AppEvent::SessionCompleted {
            session_id: session_id.to_string(),
            summary: aggregated_summary.clone(),
            timestamp: Utc::now(),
        };

        context
            .emit_event(completion_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // === 추가: 세션 리포트 이벤트 발행 ===
        let duration_ms = self
            .start_time
            .map_or(0, |t| t.elapsed().as_millis() as u64);
        let crawl_report = AppEvent::CrawlReportSession {
            session_id: session_id.to_string(),
            batches_processed: self.processed_batches,
            total_pages: self.total_success_count,
            total_success: self.total_success_count,
            total_failed: 0,
            total_retries: 0,
            duration_ms,
            products_inserted: self.products_inserted,
            products_updated: self.products_updated,
            timestamp: Utc::now(),
        };
        context
            .emit_event(crawl_report)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // === KPI: 최종 세션 요약 로그 (DB 저장 내역 포함) ===
        let plan_hash_json = match &self.active_plan_hash {
            Some(h) => format!("\"{h}\""),
            None => "null".to_string(),
        };
        let s = &aggregated_summary;
        info!(target: "kpi.session",
            "{{\"event\":\"session_final_summary\",\"session_id\":\"{}\",\"final_state\":\"{}\",\"duration_ms\":{},\"processed_batches\":{},\"total_pages_processed\":{},\"total_success_count\":{},\"failed_pages_count\":{},\"total_retry_events\":{},\"products_inserted\":{},\"products_updated\":{},\"duplicates_skipped\":{},\"plan_hash\":{},\"ts\":\"{}\"}}",
            s.session_id,
            s.final_state,
            s.total_duration_ms,
            s.processed_batches,
            s.total_pages_processed,
            s.total_success_count,
            s.failed_pages_count,
            s.total_retry_events,
            s.products_inserted,
            s.products_updated,
            s.duplicates_skipped,
            plan_hash_json,
            chrono::Utc::now()
        );

        // Mirror a concise human-readable summary to the main backend log (back_front.log)
        let plan_hash_disp = match &self.active_plan_hash {
            Some(h) if !h.is_empty() => h.as_str(),
            _ => "-",
        };
        info!(
            "📊 Session Final Summary | session_id={} state={} duration_ms={} batches={} pages_processed={} success={} failed={} retries={} inserted={} updated={} duplicates={} plan_hash={} ts={}",
            s.session_id,
            s.final_state,
            s.total_duration_ms,
            s.processed_batches,
            s.total_pages_processed,
            s.total_success_count,
            s.failed_pages_count,
            s.total_retry_events,
            s.products_inserted,
            s.products_updated,
            s.duplicates_skipped,
            plan_hash_disp,
            chrono::Utc::now()
        );

    info!("✅ Session {} completed successfully", session_id);
    // Mark completion to prevent duplicate emission later
    self.completion_emitted = true;
    Ok(())
    }

    /// 새로운 `SessionActor` 인스턴스 생성
    ///
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    ///
    /// # Returns
    /// * `Self` - 새로운 `SessionActor` 인스턴스
    #[must_use] pub const fn new(actor_id: String) -> Self {
        Self {
            actor_id,
            session_id: None,
            state: SessionState::Idle,
            start_time: None,
            processed_batches: 0,
            total_success_count: 0,
            products_inserted: 0,
            products_updated: 0,
            site_status_cache: None,
            preplanned_mode: false,
            active_plan_hash: None,
            errors: Vec::new(),
            duplicates_skipped: 0,
            aggregated_product_urls: Vec::new(),
            crawling_plan: None,
            plan_version: 0,
            completion_emitted: false,
        }
    }

    /// 세션 시작 처리
    ///
    /// # Arguments
    /// * `session_id` - 시작할 세션 ID
    /// * `config` - 크롤링 설정
    /// * `context` - Actor 컨텍스트
    ///
    /// # Returns
    /// * `Result<(), SessionError>` - 성공 시 (), 실패 시 에러
    async fn handle_start_crawling(
        &mut self,
        session_id: String,
        config: CrawlingConfig,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 상태 검증
        if !matches!(self.state, SessionState::Idle) {
            return Err(SessionError::AlreadyRunning(session_id));
        }

        info!(
            "🚀 SessionActor {} starting session {}",
            self.actor_id, session_id
        );

        // 상태 업데이트
        self.session_id = Some(session_id.clone());
    self.transition(SessionState::Starting)?;
        self.start_time = Some(Instant::now());

        // 세션 시작 이벤트 발행
        let start_event = AppEvent::SessionStarted {
            session_id: session_id.clone(),
            config: config.clone(),
            timestamp: Utc::now(),
        };

        context
            .emit_event(start_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // 실제 크롤링 실행 로직 시작
        info!(
            "📊 SessionActor {} analyzing crawling range: {} -> {}",
            self.actor_id, config.end_page, config.start_page
        );

        // 🔗 CrawlingPlanner 단일 호출 (SSOT)
        info!(
            "🧠 [PlanInit] SessionActor {} creating CrawlingPlanner (single invocation)",
            self.actor_id
        );

        // 서비스 구성
    let http_client = Arc::new(HttpClient::create_from_global_config().map_err(|e| {
            SessionError::InitializationFailed(format!("Failed to create HttpClient: {e}"))
        })?);
        let data_extractor = Arc::new(MatterDataExtractor::new().map_err(|e| {
            SessionError::InitializationFailed(format!(
                "Failed to create MatterDataExtractor: {e}"
            ))
        })?);

        // DB 풀 재사용 우선 (글로벌 풀), 필요 시 안전하게 초기화
        let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
            .await
            .map_err(|e| {
                SessionError::InitializationFailed(format!("Failed to obtain database pool: {e}"))
            })?;
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));
    let deps = SessionDeps { http_client, data_extractor, product_repo };

        info!("🎬 [SessionRun] Enter run() for session_id={}", session_id);
        // Preflight DB stats emit (best-effort)
        self
            .emit_preflight_db_snapshot(
                context,
                deps.product_repo.as_ref(),
                &session_id,
                None,
                None,
                "initial_db_scan",
            )
            .await?;

        // 계획 생성 (헬퍼 사용)
        let (plan, used_site_status) = self
            .plan_session(&config, context, &deps)
            .await?;
        let list_pages: usize = plan
            .phases
            .iter()
            .filter(|p| {
                matches!(
                    p.phase_type,
                    crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
                )
            })
            .map(|p| p.pages.len())
            .sum();
        let detail_pages: usize = plan.phases.iter()
            .filter(|p| matches!(p.phase_type, crate::crawl_engine::services::crawling_planner::PhaseType::ProductDetailCrawling))
            .map(|p| p.pages.len()).sum();
        info!(
            "PLAN plan_version={} phases={} opt_strategy={:?} list_pages={} detail_pages={} created_at={}",
            self.plan_version,
            plan.phases.len(),
            plan.optimization_strategy,
            list_pages,
            detail_pages,
            plan.created_at
        );
    self.transition(SessionState::Planned)?;
        debug!(
            "[PlanInit] CrawlingPlan stored (Arc) for session_id={}",
            session_id
        );
        // (이후 실행 단계에서 Running 전환)
        info!(
            "📋 Crawling plan created: {} phases (state=Planned)",
            plan.phases.len()
        );
    // 플래너 완료 Progress 이벤트 발행 (헬퍼 사용)
    self.emit_plan_ready(context, &session_id, &plan)?;

        // 캐시 갱신 및 사용 로그
        self.site_status_cache = Some((used_site_status.clone(), Instant::now()));
        let site_status = used_site_status;
        info!(
            "🌐 SiteStatus: total_pages={}, products_on_last_page={}",
            site_status.total_pages, site_status.products_on_last_page
        );

        // Update DB stats with site info after site status known
        self
            .emit_preflight_db_snapshot(
                context,
                deps.product_repo.as_ref(),
                &session_id,
                Some(site_status.total_pages),
                Some(site_status.total_pages),
                "post_site_status",
            )
            .await?;

        // 배치 실행 (헬퍼 사용)
        let planned_batches_count = self
            .run_list_batches(
                context,
                &session_id,
                &plan,
                &deps,
                &site_status,
            )
            .await?;

        // Unified detail crawling (if enabled) BEFORE marking completion
        let unified_flag = std::env::var("MC_UNIFIED_DETAIL")
            .map(|v| {
                let t = v.trim();
                !(t.eq("0") || t.eq_ignore_ascii_case("false"))
            })
            .unwrap_or(false);
        if unified_flag {
            let unique_urls: usize = {
                use std::collections::HashSet;
                let mut set = HashSet::new();
                for u in &self.aggregated_product_urls {
                    set.insert(u.url.clone());
                }
                set.len()
            };
            info!(
                "🧩 Unified detail crawling placeholder: aggregated_urls_total={} unique_urls={} (execution not yet implemented)",
                self.aggregated_product_urls.len(),
                unique_urls
            );
            // TODO: Implement execution of unified detail crawling pipeline using StageActor once aggregation wiring is complete.
        }

        // 상태를 Running으로 전환 후 Complete로 이동
    self.transition(SessionState::Running)?;
        if self.processed_batches as usize == planned_batches_count {
            info!(
                "📊 All planned list batches executed planned={} executed={}",
                planned_batches_count, self.processed_batches
            );
        } else {
            warn!(
                "⚠️ List batch execution mismatch planned={} executed={}",
                planned_batches_count, self.processed_batches
            );
        }
        info!(
            "🎯 SessionActor {} completing session: {} batches, {} pages total",
            self.actor_id, self.processed_batches, self.total_success_count
        );
    self.transition(SessionState::Completed)?;

    // Emit completion events and KPI logs (helper)
    self.emit_session_completion(context, &session_id, planned_batches_count)?;

        // === 세션 종료 후: 다음 계획 자동 수립 및 이벤트 발행 ===
        self
            .compute_and_emit_next_plan_ready(context, &config, &session_id)
            .await?;

        Ok(())
    }

    // (Removed misplaced unified detail crawling block)

    /// 실서비스가 주입된 `BatchActor를` 생성해 주어진 페이지들을 처리
    async fn run_batch_with_services(
        &mut self,
        batch_id: &str,
        pages: &[u32],
        context: &AppContext,
        deps: &SessionDeps,
        site_status: &crate::domain::services::SiteStatus,
        skip_duplicate_urls: Option<bool>,
        plan_id: Option<String>,
    ) -> Result<(), SessionError> {
        use crate::crawl_engine::actors::traits::Actor;
        let app_config = AppConfig::for_development();
        let config_concurrency = app_config.user.crawling.workers.list_page_max_concurrent as u32;
        let shared_metrics = Arc::new(std::sync::Mutex::new((0u32, 0u32)));
        let mut batch_actor = BatchActor::new_with_services(
            batch_id.to_string(),
            batch_id.to_string(),
            Arc::clone(&deps.http_client),
            Arc::clone(&deps.data_extractor),
            Arc::clone(&deps.product_repo),
            app_config.clone(),
        );
        // Apply explicit setting when provided (e.g., preplanned ExecutionPlan)
        if let Some(flag) = skip_duplicate_urls {
            batch_actor.set_skip_duplicate_urls(flag);
            info!(
                "[DedupCfg] Applied skip_duplicate_urls={} to BatchActor (batch_id={})",
                flag, batch_id
            );
        }
        batch_actor.shared_metrics = Some(shared_metrics.clone());
        let (tx, rx) = mpsc::channel::<super::types::ActorCommand>(100);
        let actor_context = match plan_id {
            Some(pid) => context.with_plan(pid),
            None => context.clone(),
        };
        let actor_task = tokio::spawn(async move {
            let _ = batch_actor.run(actor_context, rx).await;
        });
        let batch_config = BatchConfig {
            batch_size: pages.len() as u32,
            concurrency_limit: config_concurrency,
            batch_delay_ms: 1000,
            retry_on_failure: true,
            start_page: pages.first().copied(),
            end_page: pages.last().copied(),
        };
    let cmd = super::types::ActorCommand::ProcessBatch {
            batch_id: batch_id.to_string(),
            pages: pages.to_vec(),
            config: batch_config,
            batch_size: pages.len() as u32,
            concurrency_limit: config_concurrency,
            total_pages: site_status.total_pages,
            products_on_last_page: site_status.products_on_last_page,
        };
        tx.send(cmd).await.map_err(|e| {
            SessionError::ContextError(format!("Failed to send ProcessBatch: {e}"))
        })?;
        tx.send(super::types::ActorCommand::Shutdown)
            .await
            .map_err(|e| SessionError::ContextError(format!("Failed to send Shutdown: {e}")))?;
        actor_task
            .await
            .map_err(|e| SessionError::ContextError(format!("BatchActor join error: {e}")))?;
        if let Ok(g) = shared_metrics.lock() {
            self.products_inserted = self.products_inserted.saturating_add(g.0);
            self.products_updated = self.products_updated.saturating_add(g.1);
        }
        Ok(())
    }

    /// 세션 일시정지 처리
    ///
    /// # Arguments
    /// * `session_id` - 일시정지할 세션 ID
    /// * `reason` - 일시정지 이유
    /// * `context` - Actor 컨텍스트
    fn handle_pause_session(
        &mut self,
        session_id: String,
        reason: String,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 세션 검증
        self.validate_session(&session_id)?;

        if !matches!(self.state, SessionState::Running) {
            return Err(SessionError::InvalidStateTransition {
                from: self.state.clone(),
                to: SessionState::Paused {
                    reason,
                },
            });
        }

        warn!(
            "⏸️ SessionActor {} pausing session {}: {}",
            self.actor_id, session_id, reason
        );

        // 상태 업데이트
        self.state = SessionState::Paused {
            reason: reason.clone(),
        };

        // 일시정지 이벤트 발행
        let pause_event = AppEvent::SessionPaused {
            session_id,
            reason,
            timestamp: Utc::now(),
        };
        // (metrics already aggregated post batch run)

        context
            .emit_event(pause_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        Ok(())
    }

    /// 세션 재개 처리
    ///
    /// # Arguments
    /// * `session_id` - 재개할 세션 ID
    /// * `context` - Actor 컨텍스트
    fn handle_resume_session(
        &mut self,
        session_id: String,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 세션 검증
        self.validate_session(&session_id)?;

        if !matches!(self.state, SessionState::Paused { .. }) {
            return Err(SessionError::InvalidStateTransition {
                from: self.state.clone(),
                to: SessionState::Running,
            });
        }

        info!(
            "▶️ SessionActor {} resuming session {}",
            self.actor_id, session_id
        );

        // 상태 업데이트
        self.state = SessionState::Running;

        // 재개 이벤트 발행
        let resume_event = AppEvent::SessionResumed {
            session_id,
            timestamp: Utc::now(),
        };

        context
            .emit_event(resume_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        Ok(())
    }

    /// 세션 취소 처리
    ///
    /// # Arguments
    /// * `session_id` - 취소할 세션 ID
    /// * `reason` - 취소 이유
    /// * `context` - Actor 컨텍스트
    fn handle_cancel_session(
        &mut self,
        session_id: String,
        reason: String,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 세션 검증
        self.validate_session(&session_id)?;

        error!(
            "❌ SessionActor {} cancelling session {}: {}",
            self.actor_id, session_id, reason
        );

        // 상태 업데이트
        self.state = SessionState::Failed {
            error: reason.clone(),
        };

        // 취소 이벤트 발행
        let cancel_event = AppEvent::SessionFailed {
            session_id,
            error: reason,
            final_failure: true,
            timestamp: Utc::now(),
        };

        context
            .emit_event(cancel_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // 세션 정리
        self.cleanup_session();

        Ok(())
    }

    /// 세션 ID 검증
    ///
    /// # Arguments
    /// * `session_id` - 검증할 세션 ID
    fn validate_session(&self, session_id: &str) -> Result<(), SessionError> {
        match &self.session_id {
            Some(current_id) if current_id == session_id => Ok(()),
            Some(current_id) => Err(SessionError::SessionNotFound(format!(
                "Expected {current_id}, got {session_id}"
            ))),
            None => Err(SessionError::SessionNotFound(
                "No active session".to_string(),
            )),
        }
    }

    /// 세션 정리
    fn cleanup_session(&mut self) {
        self.session_id = None;
        self.state = SessionState::Idle;
        self.start_time = None;
        self.processed_batches = 0;
        self.total_success_count = 0;
    }

    /// 현재 세션 요약 생성
    ///
    /// # Returns
    /// * `Option<SessionSummary>` - 세션이 활성화된 경우 요약, 그렇지 않으면 None
    fn create_session_summary(&self) -> Option<SessionSummary> {
        self.session_id.as_ref().map(|session_id| {
            let duration = self
                .start_time
                .map_or(Duration::ZERO, |start| start.elapsed());

            // 에러 문자열을 ErrorSummary 집계로 변환
            use std::collections::BTreeMap;
            let mut map: BTreeMap<
                String,
                (
                    u32,
                    chrono::DateTime<chrono::Utc>,
                    chrono::DateTime<chrono::Utc>,
                ),
            > = BTreeMap::new();
            for e in &self.errors {
                let now = chrono::Utc::now();
                map.entry(e.clone())
                    .and_modify(|entry| {
                        entry.0 += 1;
                        entry.2 = now;
                    })
                    .or_insert((1, now, now));
            }
            let aggregated: Vec<crate::crawl_engine::actors::types::ErrorSummary> = map
                .into_iter()
                .map(
                    |(k, (count, first, last))| crate::crawl_engine::actors::types::ErrorSummary {
                        error_type: k,
                        count,
                        first_occurrence: first,
                        last_occurrence: last,
                    },
                )
                .collect();

            SessionSummary {
                session_id: session_id.clone(),
                total_duration_ms: duration.as_millis() as u64,
                total_pages_processed: self.total_success_count, // 페이지 성공 누적
                total_products_processed: self.products_inserted + self.products_updated,
                success_rate: if self.total_success_count > 0 {
                    1.0
                } else {
                    0.0
                },
                avg_page_processing_time: if self.total_success_count > 0 {
                    duration.as_millis() as u64 / u64::from(self.total_success_count)
                } else {
                    0
                },
                error_summary: aggregated,
                processed_batches: self.processed_batches,
                total_success_count: self.total_success_count,
                duplicates_skipped: self.duplicates_skipped,
                planned_list_batches: self.processed_batches,
                executed_list_batches: self.processed_batches,
                failed_pages_count: 0,
                failed_page_ids: Vec::new(),
                total_retry_events: 0,
                max_retries_single_page: 0,
                pages_retried: 0,
                retry_histogram: Vec::new(),
                products_inserted: self.products_inserted,
                products_updated: self.products_updated,
                final_state: format!("{:?}", self.state),
                timestamp: Utc::now(),
            }
        })
    }
}

#[async_trait::async_trait]
impl Actor for SessionActor {
    type Command = ActorCommand;
    type Error = ActorError;

    fn actor_id(&self) -> &str {
        &self.actor_id
    }

    fn actor_type(&self) -> ActorType {
        ActorType::Session
    }
    async fn run(
        &mut self,
        mut context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🎬 SessionActor {} starting execution loop", self.actor_id);
        // 새 이벤트 구독자 생성 (BatchReport 실시간 집계 목적)
        let mut event_rx = context.subscribe_events();

        loop {
            tokio::select! {
                // 명령 처리
                command = command_rx.recv() => {
                    if let Some(cmd) = command {
                        debug!("📨 SessionActor {} received command: {:?}", self.actor_id, cmd);

                        match cmd {
                            ActorCommand::StartCrawling { session_id, config } => {
                                if self.preplanned_mode || self.active_plan_hash.is_some() {
                                    warn!("🛑 Ignoring StartCrawling – preplanned or active plan hash already set (SessionActor {})", self.actor_id);
                                } else if !matches!(self.state, SessionState::Idle) {
                                    warn!("🛑 Ignoring StartCrawling – state not Idle: {:?}", self.state);
                                } else if let Err(e) = self.handle_start_crawling(session_id, config, &context).await {
                                    error!("Failed to start crawling: {}", e);
                                }
                            }
                            ActorCommand::ExecutePrePlanned { session_id, plan } => {
                                if self.preplanned_mode {
                                    if let Some(active_hash) = &self.active_plan_hash {
                                        if *active_hash == plan.plan_hash {
                                            warn!("⚠️ SessionActor {} duplicate ExecutePrePlanned (same hash) ignored", self.actor_id);
                                        } else {
                                            warn!("⚠️ SessionActor {} already executing a different plan hash (active={}, new={})", self.actor_id, active_hash, plan.plan_hash);
                                        }
                                    } else {
                                        warn!("⚠️ SessionActor {} preplanned_mode without active hash (unexpected) — ignoring", self.actor_id);
                                    }
                                } else if !matches!(self.state, SessionState::Idle) {
                                    error!("Cannot execute preplanned plan – session not idle");
                                } else {
                                    self.preplanned_mode = true;
                                    self.active_plan_hash = Some(plan.plan_hash.clone());
                                    info!("🔐 SessionActor {} executing pre-planned ExecutionPlan (hash={})", self.actor_id, plan.plan_hash);
                                    // Initialize session registry entry and emit initial SessionStarted
                                    {
                                        use crate::crawl_engine::runtime::session_registry::{session_registry, SessionEntry, SessionStatus, failure_threshold};
                                        use chrono::Utc;
                                        use tokio::sync::watch;
                                        let total_pages_planned: u64 = plan
                                            .crawling_ranges
                                            .iter()
                                            .map(|r| if r.start_page >= r.end_page { (r.start_page - r.end_page + 1) as u64 } else { (r.end_page - r.start_page + 1) as u64 })
                                            .sum();
                                        let batch_unit = plan.batch_size.max(1) as usize;
                                        let total_batches_planned: u64 = plan
                                            .crawling_ranges
                                            .iter()
                                            .map(|r| {
                                                let pages = if r.start_page >= r.end_page { (r.start_page - r.end_page + 1) as usize } else { (r.end_page - r.start_page + 1) as usize };
                                                (pages.div_ceil(batch_unit)) as u64
                                            })
                                            .sum();
                                        let mut remaining_pages: Vec<u32> = Vec::new();
                                        for r in &plan.crawling_ranges {
                                            if r.start_page <= r.end_page {
                                                remaining_pages.extend(r.start_page..=r.end_page);
                                            } else {
                                                remaining_pages.extend((r.end_page..=r.start_page).rev());
                                            }
                                        }
                                        let (pause_tx, _pause_rx) = watch::channel(false);
                                        {
                                            let reg = session_registry();
                                            let mut g = reg.write().await;
                                            g.insert(
                                                session_id.clone(),
                                                SessionEntry {
                                                    status: SessionStatus::Running,
                                                    pause_tx,
                                                    started_at: Utc::now(),
                                                    completed_at: None,
                                                    total_pages_planned,
                                                    processed_pages: 0,
                                                    total_batches_planned,
                                                    completed_batches: 0,
                                                    batch_size: plan.batch_size,
                                                    concurrency_limit: plan.concurrency_limit,
                                                    last_error: None,
                                                    error_count: 0,
                                                    resume_token: None,
                                                    remaining_page_slots: Some(remaining_pages),
                                                    plan_hash: Some(plan.plan_hash.clone()),
                                                    removal_deadline: None,
                                                    failed_emitted: false,
                                                    retries_per_page: std::collections::HashMap::new(),
                                                    failed_pages: Vec::new(),
                                                    retrying_pages: Vec::new(),
                                                    product_list_max_retries: 0,
                                                    error_type_stats: std::collections::HashMap::new(),
                                                    detail_tasks_total: 0,
                                                    detail_tasks_completed: 0,
                                                    detail_tasks_failed: 0,
                                                    detail_retry_counts: std::collections::HashMap::new(),
                                                    detail_retries_total: 0,
                                                    detail_retry_histogram: std::collections::HashMap::new(),
                                                    remaining_detail_ids: None,
                                                    detail_failed_ids: Vec::new(),
                                                    page_failure_threshold: failure_threshold(),
                                                    detail_failure_threshold:  plan.concurrency_limit, // placeholder until config available
                                                    detail_downshifted: false,
                                                    detail_downshift_timestamp: None,
                                                    detail_downshift_old_limit: None,
                                                    detail_downshift_new_limit: None,
                                                    detail_downshift_trigger: None,
                                                },
                                            );
                                        }
                                        // Emit SessionStarted with minimal config derived from plan
                                        let start_cfg = CrawlingConfig {
                                            site_url: "preplanned".into(),
                                            start_page: plan.crawling_ranges.first().map_or(1, |r| r.start_page),
                                            end_page: plan.crawling_ranges.last().map_or(1, |r| r.end_page),
                                            concurrency_limit: plan.concurrency_limit,
                                            batch_size: plan.batch_size,
                                            request_delay_ms: 0,
                                            timeout_secs: 300,
                                            max_retries: 3,
                                            strategy: crate::crawl_engine::actors::types::CrawlingStrategy::NewestFirst,
                                        };
                                        if let Err(e) = context.emit_event(AppEvent::SessionStarted { session_id: session_id.clone(), config: start_cfg, timestamp: Utc::now() }) {
                                            error!("Failed to emit SessionStarted: {}", e);
                                        }
                                    }
                                    // 서비스 준비 (실패 시 중단)
                                    match crate::infrastructure::database_connection::get_or_init_global_pool().await {
                                        Ok(db_pool) => {
                                            let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));
                                            let http_client = match HttpClient::create_from_global_config() {
                                                Ok(c) => Arc::new(c),
                                                Err(e) => {
                                                    error!("HTTP client init failed: {}", e);
                                                    let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("HTTP client init failed: {e}"), final_failure: true, timestamp: Utc::now() };
                                                    if let Err(er) = context.emit_event(fail_event) { error!("emit fail event error: {}", er); }
                                                    self.state = SessionState::Failed { error: "http_client_init".into() };
                                                    continue;
                                                }
                                            };
                                            let data_extractor = match MatterDataExtractor::new() {
                                                Ok(d) => Arc::new(d),
                                                Err(e) => {
                                                    error!("Extractor init failed: {}", e);
                                                    let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("Extractor init failed: {e}"), final_failure: true, timestamp: Utc::now() };
                                                    if let Err(er) = context.emit_event(fail_event) { error!("emit fail event error: {}", er); }
                                                    self.state = SessionState::Failed { error: "extractor_init".into() };
                                                    continue;
                                                }
                                            };
                                            let deps = SessionDeps { http_client, data_extractor, product_repo };
                                            self.session_id = Some(session_id.clone());
                                            let _ = self.transition(SessionState::Running);
                                            self.start_time = Some(Instant::now());
                                            // Note: SessionStarted was already emitted above during registry initialization with plan-derived config.
                                            let site_status = plan.input_snapshot_to_site_status();
                                            let planned_batches = match self
                                                .run_preplanned_batches(
                                                    &context,
                                                    &session_id,
                                                    &plan,
                                                    &deps,
                                                    &site_status,
                                                )
                                                .await
                                            {
                                                Ok(n) => n,
                                                Err(e) => {
                                                    error!("Preplanned run error: {}", e);
                                                    0
                                                }
                                            };
                                            let _ = self.transition(SessionState::Completed);
                                            // Emit completion via helper
                                            if let Err(e) = self.emit_session_completion(&context, &session_id, planned_batches) {
                                                error!("emit_session_completion failed: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            error!("DB pool init failed: {}", e);
                                            let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("DB pool init failed: {e}"), final_failure: true, timestamp: Utc::now() };
                                            if let Err(er) = context.emit_event(fail_event) { error!("emit fail event error: {}", er); }
                                            self.state = SessionState::Failed { error: "db_pool_init".into() };
                                        }
                                    }
                                }
                            }

                            ActorCommand::PauseSession { session_id, reason } => {
                                if let Err(e) = self.handle_pause_session(session_id, reason, &context) {
                                    error!("Failed to pause session: {}", e);
                                }
                            }

                            ActorCommand::ResumeSession { session_id } => {
                                if let Err(e) = self.handle_resume_session(session_id, &context) {
                                    error!("Failed to resume session: {}", e);
                                }
                            }

                            ActorCommand::CancelSession { session_id, reason } => {
                                if let Err(e) = self.handle_cancel_session(session_id, reason, &context) {
                                    error!("Failed to cancel session: {}", e);
                                }
                            }

                            ActorCommand::Shutdown => {
                                info!("🛑 SessionActor {} received shutdown command", self.actor_id);
                                break;
                            }

                            _ => {
                                debug!("SessionActor {} ignoring non-session command", self.actor_id);
                            }
                        }
                    } else {
                        warn!("📪 SessionActor {} command channel closed", self.actor_id);
                        break;
                    }
                }

                // 이벤트 스트림 수신 (BatchReport -> duplicates_skipped 누적)
                Ok(evt) = event_rx.recv() => {
                    if let AppEvent::BatchReport { duplicates_skipped, .. } = evt {
                        if duplicates_skipped > 0 {
                            let before = self.duplicates_skipped;
                            self.duplicates_skipped = self.duplicates_skipped.saturating_add(duplicates_skipped);
                            debug!("🧮 SessionActor {} accumulated duplicates_skipped: +{} ({} -> {})", self.actor_id, duplicates_skipped, before, self.duplicates_skipped);
                        }
                    } else { /* ignore other events */ }
                }

                // 취소 신호 확인
                _ = context.cancellation_token.changed() => {
                    if *context.cancellation_token.borrow() {
                        warn!("🚫 SessionActor {} received cancellation signal", self.actor_id);
                        break;
                    }
                }
            }
        }

        // 정리 작업 (중복 방지)
        if !self.completion_emitted {
            if let Some(summary) = self.create_session_summary() {
                let completion_event = AppEvent::SessionCompleted {
                    session_id: summary.session_id.clone(),
                    summary,
                    timestamp: Utc::now(),
                };

                let _ = context.emit_event(completion_event);
                self.completion_emitted = true;
            }
        }

        info!("🏁 SessionActor {} execution loop ended", self.actor_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<ActorHealth, Self::Error> {
        let status = match &self.state {
            SessionState::Idle | SessionState::Running => ActorStatus::Healthy,
            SessionState::Paused { reason } => ActorStatus::Degraded {
                reason: format!("Paused: {reason}"),
                since: Utc::now(),
            },
            SessionState::Failed { error } => ActorStatus::Unhealthy {
                error: error.clone(),
                since: Utc::now(),
            },
            _ => ActorStatus::Degraded {
                reason: format!("In transition state: {:?}", self.state),
                since: Utc::now(),
            },
        };

        Ok(ActorHealth {
            actor_id: self.actor_id.clone(),
            actor_type: ActorType::Session,
            status,
            last_activity: Utc::now(),
            memory_usage_mb: 0, // TODO: 실제 메모리 사용량 계산
            active_tasks: u32::from(matches!(self.state, SessionState::Running)),
            commands_processed: 0, // TODO: 실제 처리된 명령 수 계산
            errors_count: 0,       // TODO: 실제 에러 수 계산
            avg_command_processing_time_ms: 0.0, // TODO: 실제 평균 처리 시간 계산
            metadata: serde_json::json!({
                "session_id": self.session_id,
                "state": format!("{:?}", self.state),
                "processed_batches": self.processed_batches,
                "total_success_count": self.total_success_count
            })
            .to_string(),
        })
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        info!("🔌 SessionActor {} shutting down", self.actor_id);

        // 활성 세션이 있다면 정리
        if self.session_id.is_some() {
            warn!("Cleaning up active session during shutdown");
            self.cleanup_session();
        }

        Ok(())
    }
}
