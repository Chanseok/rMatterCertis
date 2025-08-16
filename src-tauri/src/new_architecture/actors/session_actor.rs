//! SessionActor: 크롤링 세션 관리 Actor
//!
//! Phase 3: Actor 구현 - 세션 레벨 제어 및 모니터링
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::Utc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::new_architecture::actors::types::SessionSummary;

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, ActorError, CrawlingConfig};
use crate::new_architecture::channels::types::AppEvent;
use crate::new_architecture::context::AppContext;
use std::sync::Arc;

use crate::domain::services::{DatabaseAnalyzer, StatusChecker};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::crawling_service_impls::{DatabaseAnalyzerImpl, StatusCheckerImpl};
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};
use crate::new_architecture::actors::BatchActor;
use crate::new_architecture::actors::types::BatchConfig;
use crate::new_architecture::services::CrawlingPlanner;

/// SessionActor: 크롤링 세션의 전체 생명주기 관리
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
    /// 세션 레벨 SiteStatus 캐시
    site_status_cache: Option<(crate::domain::services::SiteStatus, Instant)>,
    /// 이미 외부에서 확정된 계획을 실행 중인지 여부 (재계획 방지)
    preplanned_mode: bool,
    /// 배치 완료 시 삽입된 제품 수
    products_inserted: u32,
    /// 배치 완료 시 업데이트된 제품 수
    products_updated: u32,
    /// 현재 실행 중인 ExecutionPlan 해시 (무결성 로그 목적)
    active_plan_hash: Option<String>,
    /// 배치/세션 실행 중 발생한 에러 메시지 누적 (요약/리포트 용)
    errors: Vec<String>,
    /// 세션 누적 duplicate skip 합계
    duplicates_skipped: u32,
    // Unified detail crawling accumulation across batches
    aggregated_product_urls: Vec<crate::domain::product_url::ProductUrl>,
    /// 단일 실행 계획 (세션 동안 불변, 재계산 금지)
    crawling_plan:
        Option<std::sync::Arc<crate::new_architecture::services::crawling_planner::CrawlingPlan>>,
    /// 계획 버전 (향후 재계산 허용 시 증가) 현재 0 또는 1
    plan_version: u64,
}

/// 세션 상태 열거형
#[derive(Debug, Clone, PartialEq)]
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
    /// 새로운 SessionActor 인스턴스 생성
    ///
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    ///
    /// # Returns
    /// * `Self` - 새로운 SessionActor 인스턴스
    pub fn new(actor_id: String) -> Self {
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
        self.state = SessionState::Starting;
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
            SessionError::InitializationFailed(format!("Failed to create HttpClient: {}", e))
        })?);
        let data_extractor = Arc::new(MatterDataExtractor::new().map_err(|e| {
            SessionError::InitializationFailed(format!(
                "Failed to create MatterDataExtractor: {}",
                e
            ))
        })?);

        // DB 연결 재사용 경로가 없다면 간단히 새로 생성
        let database_url = crate::infrastructure::database_paths::get_main_database_url();
        let db_pool = sqlx::SqlitePool::connect(&database_url)
            .await
            .map_err(|e| {
                SessionError::InitializationFailed(format!("Failed to connect to database: {}", e))
            })?;
        let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));

        info!("🎬 [SessionRun] Enter run() for session_id={}", session_id);
        // Preflight DB stats emit (best-effort)
        if let Ok((cnt, minp, maxp, _last)) = product_repo.get_product_detail_stats().await {
            let pre_event = AppEvent::PreflightDiagnostics {
                session_id: session_id.clone(),
                db_products: cnt,
                db_min_page: minp,
                db_max_page: maxp,
                site_total_pages: None, // filled after site status if needed
                site_known_last_page: None,
                reason: Some("initial_db_scan".into()),
                timestamp: Utc::now(),
            };
            match context.emit_event(pre_event) {
                Ok(_) => info!(
                    "[DiagEmit] PreflightDiagnostics initial_db_scan emitted products={} page_range={:?}-{:?}",
                    cnt, minp, maxp
                ),
                Err(e) => warn!("[DiagEmit] initial_db_scan emit failed err={}", e),
            }
        }

        // 플래너 생성에 필요한 서비스들
        let status_checker: Arc<dyn StatusChecker> =
            Arc::new(StatusCheckerImpl::with_product_repo(
                (*http_client).clone(),
                (*data_extractor).clone(),
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

        // TTL 5분 캐시 사용해 계획 생성
        let ttl = Duration::from_secs(300);
        let cached = self.site_status_cache.as_ref().and_then(|(status, ts)| {
            if ts.elapsed() <= ttl {
                Some(status.clone())
            } else {
                None
            }
        });
        // NOTE: Strategy currently default (NewestFirst) unless caller overrides
        if self.crawling_plan.is_some() {
            warn!(
                "[PlanInit] CrawlingPlan already exists for session_id={}, duplicate planning suppressed",
                session_id
            );
        }
        let (plan, used_site_status) = planner
            .create_crawling_plan_with_cache(&config, cached)
            .await
            .map_err(|e| {
                SessionError::InitializationFailed(format!("Failed to create crawling plan: {}", e))
            })?;

        self.plan_version = 1;
        let list_pages: usize = plan.phases.iter()
            .filter(|p| matches!(p.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling))
            .map(|p| p.pages.len()).sum();
        let detail_pages: usize = plan.phases.iter()
            .filter(|p| matches!(p.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ProductDetailCrawling))
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
        self.crawling_plan = Some(std::sync::Arc::new(plan.clone()));
        self.state = SessionState::Planned;
        debug!(
            "[PlanInit] CrawlingPlan stored (Arc) for session_id={}",
            session_id
        );
        // (이후 실행 단계에서 Running 전환)
        info!(
            "📋 Crawling plan created: {} phases (state=Planned)",
            plan.phases.len()
        );
        // 플래너 완료 Progress 이벤트 발행 (플래너 단계 관측용)
        let planning_event = AppEvent::Progress {
            session_id: session_id.clone(),
            current_step: 0,
            total_steps: plan.phases.len() as u32,
            message: format!(
                "Crawling plan ready: {} phases, list-batches={}",
                plan.phases.len(),
                plan.phases.iter().filter(|p| matches!(p.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling)).count()
            ),
            percentage: 0.0,
            timestamp: Utc::now(),
        };
        context
            .emit_event(planning_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // 캐시 갱신 및 사용 로그
        self.site_status_cache = Some((used_site_status.clone(), Instant::now()));
        let site_status = used_site_status;
        info!(
            "🌐 SiteStatus: total_pages={}, products_on_last_page={}",
            site_status.total_pages, site_status.products_on_last_page
        );

        // Update DB stats with site info after site status known
        if let Ok((cnt, minp, maxp, _last)) = product_repo.get_product_detail_stats().await {
            let pre_event2 = AppEvent::PreflightDiagnostics {
                session_id: session_id.clone(),
                db_products: cnt,
                db_min_page: minp,
                db_max_page: maxp,
                site_total_pages: Some(site_status.total_pages),
                site_known_last_page: Some(site_status.total_pages),
                reason: Some("post_site_status".into()),
                timestamp: Utc::now(),
            };
            match context.emit_event(pre_event2) {
                Ok(_) => info!(
                    "[DiagEmit] PreflightDiagnostics post_site_status emitted products={} site_total_pages={} page_range={:?}-{:?}",
                    cnt, site_status.total_pages, minp, maxp
                ),
                Err(e) => warn!("[DiagEmit] post_site_status emit failed err={}", e),
            }
        }

        // ListPageCrawling phases만 추출 → 각 phase 페이지들을 순차 처리
        let planned_list_batches: Vec<_> = plan.phases.iter().filter(|p| matches!(p.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling)).collect();
        if planned_list_batches.is_empty() {
            warn!(
                "⚠️ No ListPageCrawling phases planned (requested start/end maybe collapsed). start_page={} end_page={}",
                config.start_page, config.end_page
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
        let mut executed_batches_pages: Vec<Vec<u32>> = Vec::new();
        let mut batch_idx = 0u32;
        for phase in planned_list_batches.iter() {
            batch_idx += 1;
            let pages = phase.pages.clone();
            if pages.is_empty() {
                continue;
            }
            let batch_id = format!("{}-batch-{}", session_id, batch_idx);
            info!(
                "🏃 SessionActor {} running batch {} (batch_index={}/{}) with {} pages: {:?}",
                self.actor_id,
                batch_id,
                batch_idx,
                planned_batches_count,
                pages.len(),
                pages
            );

            // 배치 시작 Progress 이벤트 (세션 관점)
            let progress_event = AppEvent::Progress {
                session_id: session_id.clone(),
                current_step: 1,
                total_steps: plan.phases.len() as u32,
                message: format!("Starting batch {} with {} pages", batch_id, pages.len()),
                percentage: ((batch_idx - 1) as f64 / plan.phases.len() as f64) * 100.0,
                timestamp: Utc::now(),
            };
            context
                .emit_event(progress_event)
                .map_err(|e| SessionError::ContextError(e.to_string()))?;

            if let Err(e) = self
                .run_batch_with_services(
                    &batch_id,
                    &pages,
                    context,
                    &http_client,
                    &data_extractor,
                    &product_repo,
                    &site_status,
                )
                .await
            {
                error!("❌ Batch {} failed: {}", batch_id, e);
                self.errors.push(format!("batch {}: {}", batch_id, e));
                // 세션 실패 이벤트 발행
                let fail_event = AppEvent::SessionFailed {
                    session_id: session_id.clone(),
                    error: format!("Batch {} failed: {}", batch_id, e),
                    final_failure: false,
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(fail_event)
                    .map_err(|er| SessionError::ContextError(er.to_string()))?;
                // 일단 다음 배치로 계속 진행 (요구 시 중단 정책으로 변경 가능)
                continue;
            }

            self.processed_batches += 1;
            executed_batches_pages.push(pages.clone());
            self.total_success_count += pages.len() as u32;
            // TODO: Collect batch-level deferred URLs via a channel/event once BatchActor emits them.
            info!("✅ Completed batch {} ({} pages)", batch_id, pages.len());

            // 배치 완료 Progress 이벤트
            let progress_event = AppEvent::Progress {
                session_id: session_id.clone(),
                current_step: 2,
                total_steps: plan.phases.len() as u32,
                message: format!("Completed batch {} ({} pages)", batch_id, pages.len()),
                percentage: (batch_idx as f64 / plan.phases.len() as f64) * 100.0,
                timestamp: Utc::now(),
            };
            context
                .emit_event(progress_event)
                .map_err(|e| SessionError::ContextError(e.to_string()))?;
        }

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
        self.state = SessionState::Running;
        if self.processed_batches as usize != planned_batches_count {
            warn!(
                "⚠️ List batch execution mismatch planned={} executed={}",
                planned_batches_count, self.processed_batches
            );
        } else {
            info!(
                "📊 All planned list batches executed planned={} executed={}",
                planned_batches_count, self.processed_batches
            );
        }
        info!(
            "🎯 SessionActor {} completing session: {} batches, {} pages total",
            self.actor_id, self.processed_batches, self.total_success_count
        );
        self.state = SessionState::Completed;

        // 완료 이벤트 발행
        let completion_event = AppEvent::SessionCompleted {
            session_id: session_id.clone(),
            summary: SessionSummary {
                session_id: session_id.clone(),
                total_duration_ms: self
                    .start_time
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0),
                total_pages_processed: self.total_success_count,
                total_products_processed: 0, // TODO: 실제 제품 수 계산
                success_rate: 1.0,           // TODO: 실제 성공률 계산
                avg_page_processing_time: self
                    .start_time
                    .map(|t| {
                        t.elapsed().as_millis() as u64
                            / std::cmp::max(self.total_success_count as u64, 1)
                    })
                    .unwrap_or(0),
                error_summary: Vec::new(), // TODO: 실제 에러 수집
                processed_batches: self.processed_batches,
                total_success_count: self.total_success_count,
                duplicates_skipped: self.duplicates_skipped,
                planned_list_batches: planned_batches_count as u32,
                executed_list_batches: self.processed_batches,
                failed_pages_count: 0,
                failed_page_ids: Vec::new(),
                total_retry_events: 0,
                max_retries_single_page: 0,
                pages_retried: 0,
                retry_histogram: Vec::new(),
                products_inserted: 0,
                products_updated: 0,
                final_state: "completed".to_string(),
                timestamp: Utc::now(),
            },
            timestamp: Utc::now(),
        };

        context
            .emit_event(completion_event)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // === 추가: 세션 리포트 이벤트 발행 ===
        let duration_ms = self
            .start_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        // 총 실패 페이지/리트라이 수는 배치 리포트 합산이 이상적이지만, 최소한 현재 수치로 요약 제공
        let crawl_report = AppEvent::CrawlReportSession {
            session_id: session_id.clone(),
            batches_processed: self.processed_batches,
            total_pages: self.total_success_count,
            total_success: self.total_success_count,
            total_failed: 0,  // TODO: 배치 결과 수집 시 합산
            total_retries: 0, // TODO: 배치 리포트 기반 합산
            duration_ms,
            products_inserted: 0,
            products_updated: 0,
            timestamp: Utc::now(),
        };
        context
            .emit_event(crawl_report)
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        info!("✅ Session {} completed successfully", session_id);
        Ok(())
    }

    // (Removed misplaced unified detail crawling block)

    /// 실서비스가 주입된 BatchActor를 생성해 주어진 페이지들을 처리
    async fn run_batch_with_services(
        &mut self,
        batch_id: &str,
        pages: &[u32],
        context: &AppContext,
        http_client: &Arc<HttpClient>,
        data_extractor: &Arc<MatterDataExtractor>,
        product_repo: &Arc<IntegratedProductRepository>,
        site_status: &crate::domain::services::SiteStatus,
    ) -> Result<(), SessionError> {
        use crate::new_architecture::actors::traits::Actor;
        let app_config = AppConfig::for_development();
        let config_concurrency = app_config.user.crawling.workers.list_page_max_concurrent as u32;
        let shared_metrics = Arc::new(std::sync::Mutex::new((0u32, 0u32)));
        let mut batch_actor = BatchActor::new_with_services(
            batch_id.to_string(),
            batch_id.to_string(),
            Arc::clone(http_client),
            Arc::clone(data_extractor),
            Arc::clone(product_repo),
            app_config.clone(),
        );
        batch_actor.shared_metrics = Some(shared_metrics.clone());
        let (tx, rx) = mpsc::channel::<super::types::ActorCommand>(100);
        let actor_context = context.clone();
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
            SessionError::ContextError(format!("Failed to send ProcessBatch: {}", e))
        })?;
        tx.send(super::types::ActorCommand::Shutdown)
            .await
            .map_err(|e| SessionError::ContextError(format!("Failed to send Shutdown: {}", e)))?;
        actor_task
            .await
            .map_err(|e| SessionError::ContextError(format!("BatchActor join error: {}", e)))?;
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
    async fn handle_pause_session(
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
                    reason: reason.clone(),
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
    async fn handle_resume_session(
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
    async fn handle_cancel_session(
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
                "Expected {}, got {}",
                current_id, session_id
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
                .map(|start| start.elapsed())
                .unwrap_or(Duration::ZERO);

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
            let aggregated: Vec<crate::new_architecture::actors::types::ErrorSummary> = map
                .into_iter()
                .map(|(k, (count, first, last))| {
                    crate::new_architecture::actors::types::ErrorSummary {
                        error_type: k,
                        count,
                        first_occurrence: first,
                        last_occurrence: last,
                    }
                })
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
                    duration.as_millis() as u64 / self.total_success_count as u64
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
                    match command {
                        Some(cmd) => {
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
                                            if *active_hash != plan.plan_hash {
                                                warn!("⚠️ SessionActor {} already executing a different plan hash (active={}, new={})", self.actor_id, active_hash, plan.plan_hash);
                                            } else {
                                                warn!("⚠️ SessionActor {} duplicate ExecutePrePlanned (same hash) ignored", self.actor_id);
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
                                        // 서비스 준비 (실패 시 중단)
                                        let db_url = crate::infrastructure::database_paths::get_main_database_url();
                                        match sqlx::SqlitePool::connect(&db_url).await {
                                            Ok(db_pool) => {
                                                let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));
                                                let http_client = match HttpClient::create_from_global_config() {
                                                    Ok(c) => Arc::new(c),
                                                    Err(e) => {
                                                        error!("HTTP client init failed: {}", e);
                                                        let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("HTTP client init failed: {}", e), final_failure: true, timestamp: Utc::now() };
                                                        if let Err(er) = context.emit_event(fail_event) { error!("emit fail event error: {}", er); }
                                                        self.state = SessionState::Failed { error: "http_client_init".into() };
                                                        continue;
                                                    }
                                                };
                                                let data_extractor = match MatterDataExtractor::new() {
                                                    Ok(d) => Arc::new(d),
                                                    Err(e) => {
                                                        error!("Extractor init failed: {}", e);
                                                        let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("Extractor init failed: {}", e), final_failure: true, timestamp: Utc::now() };
                                                        if let Err(er) = context.emit_event(fail_event) { error!("emit fail event error: {}", er); }
                                                        self.state = SessionState::Failed { error: "extractor_init".into() };
                                                        continue;
                                                    }
                                                };
                                                self.session_id = Some(session_id.clone());
                                                self.state = SessionState::Running;
                                                self.start_time = Some(Instant::now());
                                                let start_event = AppEvent::SessionStarted { session_id: session_id.clone(), config: CrawlingConfig { site_url: "preplanned".into(), start_page: 1, end_page: 1, concurrency_limit: plan.concurrency_limit, batch_size: plan.batch_size, request_delay_ms: 0, timeout_secs: 300, max_retries: 3, strategy: crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst }, timestamp: Utc::now() };
                                                if let Err(e) = context.emit_event(start_event) { error!("Failed to emit start event: {}", e); }
                                                let site_status = plan.input_snapshot_to_site_status();
                                                for (idx, range) in plan.crawling_ranges.iter().enumerate() {
                                                    let pages: Vec<u32> = if range.reverse_order { (range.start_page..=range.end_page).rev().collect() } else { (range.start_page..=range.end_page).collect() };
                                                    let batch_id = format!("{}-pre-{}", session_id, idx+1);
                                                    if let Err(e) = self.run_batch_with_services(&batch_id, &pages, &context, &http_client, &data_extractor, &product_repo, &site_status).await {
                                                        error!("Batch {} failed: {}", batch_id, e);
                                                        self.errors.push(format!("batch {}: {}", batch_id, e));
                                                        let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("Batch {} failed: {}", batch_id, e), final_failure: false, timestamp: Utc::now() };
                                                        if let Err(er) = context.emit_event(fail_event) { error!("emit batch fail event error: {}", er); }
                                                    }
                                                    self.processed_batches += 1; self.total_success_count += pages.len() as u32;
                                                    // BatchReport 이벤트에서 누적 중복 스킵을 수신할 수 없으므로 여기서는 BatchActor 내부 누적이 반영된 값 없. 향후 이벤트 브릿지에서 BatchReport 수신 시 합산.
                                                }
                                                let duration_ms = self.start_time.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
                                                self.state = SessionState::Completed;
                                                // 에러 집계 (동일 로직 재사용)
                                                use std::collections::BTreeMap;
                                                let mut map: BTreeMap<String, (u32, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = BTreeMap::new();
                                                for e in &self.errors { let now = chrono::Utc::now(); map.entry(e.clone()).and_modify(|entry| { entry.0 += 1; entry.2 = now; }).or_insert((1, now, now)); }
                                                let aggregated: Vec<crate::new_architecture::actors::types::ErrorSummary> = map.into_iter().map(|(k,(count, first, last))| crate::new_architecture::actors::types::ErrorSummary { error_type: k, count, first_occurrence: first, last_occurrence: last }).collect();
                                                let summary = SessionSummary { session_id: session_id.clone(), total_duration_ms: duration_ms, total_pages_processed: self.total_success_count, total_products_processed: 0, success_rate: 1.0, avg_page_processing_time: if self.total_success_count>0 { duration_ms / self.total_success_count as u64 } else {0}, error_summary: aggregated, processed_batches: self.processed_batches, total_success_count: self.total_success_count, duplicates_skipped: self.duplicates_skipped, planned_list_batches: self.processed_batches, executed_list_batches: self.processed_batches, failed_pages_count: 0, failed_page_ids: Vec::new(), total_retry_events: 0, max_retries_single_page: 0, pages_retried: 0, retry_histogram: Vec::new(), products_inserted: 0, products_updated: 0, final_state: "completed".into(), timestamp: Utc::now() };
                                                if let Err(e) = context.emit_event(AppEvent::SessionCompleted { session_id: session_id.clone(), summary: summary.clone(), timestamp: Utc::now() }) { error!("emit completion event failed: {}", e); }
                                                if let Err(e) = context.emit_event(AppEvent::CrawlReportSession { session_id: session_id.clone(), batches_processed: self.processed_batches, total_pages: self.total_success_count, total_success: self.total_success_count, total_failed: 0, total_retries: 0, duration_ms, products_inserted: 0, products_updated: 0, timestamp: Utc::now() }) { error!("emit crawl report failed: {}", e); }
                                            }
                                            Err(e) => {
                                                error!("DB pool init failed: {}", e);
                                                let fail_event = AppEvent::SessionFailed { session_id: session_id.clone(), error: format!("DB pool init failed: {}", e), final_failure: true, timestamp: Utc::now() };
                                                if let Err(er) = context.emit_event(fail_event) { error!("emit fail event error: {}", er); }
                                                self.state = SessionState::Failed { error: "db_pool_init".into() };
                                            }
                                        }
                                    }
                                }

                                ActorCommand::PauseSession { session_id, reason } => {
                                    if let Err(e) = self.handle_pause_session(session_id, reason, &context).await {
                                        error!("Failed to pause session: {}", e);
                                    }
                                }

                                ActorCommand::ResumeSession { session_id } => {
                                    if let Err(e) = self.handle_resume_session(session_id, &context).await {
                                        error!("Failed to resume session: {}", e);
                                    }
                                }

                                ActorCommand::CancelSession { session_id, reason } => {
                                    if let Err(e) = self.handle_cancel_session(session_id, reason, &context).await {
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
                        }
                        None => {
                            warn!("📪 SessionActor {} command channel closed", self.actor_id);
                            break;
                        }
                    }
                }

                // 이벤트 스트림 수신 (BatchReport -> duplicates_skipped 누적)
                Ok(evt) = event_rx.recv() => {
                    match evt {
                        AppEvent::BatchReport { duplicates_skipped, .. } => {
                            if duplicates_skipped > 0 {
                                let before = self.duplicates_skipped;
                                self.duplicates_skipped = self.duplicates_skipped.saturating_add(duplicates_skipped);
                                debug!("🧮 SessionActor {} accumulated duplicates_skipped: +{} ({} -> {})", self.actor_id, duplicates_skipped, before, self.duplicates_skipped);
                            }
                        }
                        _ => { /* ignore other events */ }
                    }
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

        // 정리 작업
        if let Some(summary) = self.create_session_summary() {
            let completion_event = AppEvent::SessionCompleted {
                session_id: summary.session_id.clone(),
                summary,
                timestamp: Utc::now(),
            };

            let _ = context.emit_event(completion_event);
        }

        info!("🏁 SessionActor {} execution loop ended", self.actor_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<ActorHealth, Self::Error> {
        let status = match &self.state {
            SessionState::Idle | SessionState::Running => ActorStatus::Healthy,
            SessionState::Paused { reason } => ActorStatus::Degraded {
                reason: format!("Paused: {}", reason),
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
            active_tasks: if matches!(self.state, SessionState::Running) {
                1
            } else {
                0
            },
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
