//! SessionActor: 크롤링 세션 관리 Actor
//! 
//! Phase 3: Actor 구현 - 세션 레벨 제어 및 모니터링
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug};
use chrono::Utc;

use crate::new_architecture::actors::types::SessionSummary;

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, CrawlingConfig, ActorError};
use crate::new_architecture::channels::types::AppEvent;
use crate::new_architecture::context::AppContext;
use std::sync::Arc;

use crate::new_architecture::services::CrawlingPlanner;
use crate::domain::services::{StatusChecker, DatabaseAnalyzer};
use crate::infrastructure::crawling_service_impls::{StatusCheckerImpl, DatabaseAnalyzerImpl};
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::AppConfig;
use crate::new_architecture::actors::types::BatchConfig;
use crate::new_architecture::actors::BatchActor;

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
}

/// 세션 상태 열거형
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Idle,
    Starting,
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
    InvalidStateTransition { from: SessionState, to: SessionState },
    
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
            site_status_cache: None,
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
        
        info!("🚀 SessionActor {} starting session {}", self.actor_id, session_id);
        
        // 상태 업데이트
        self.session_id = Some(session_id.clone());
        self.state = SessionState::Starting;
        self.start_time = Some(Instant::now());
        self.processed_batches = 0;
        self.total_success_count = 0;
        
        // 세션 시작 이벤트 발행
        let start_event = AppEvent::SessionStarted {
            session_id: session_id.clone(),
            config: config.clone(),
            timestamp: Utc::now(),
        };
        
        context.emit_event(start_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        
        // 실제 크롤링 실행 로직 시작
        info!("📊 SessionActor {} analyzing crawling range: {} -> {}", 
              self.actor_id, config.end_page, config.start_page);
        
        // 🔗 CrawlingPlanner로 실행 계획 수립 → 배치별 페이지 집합 생성(SSOT)
        info!("🧠 SessionActor {} creating CrawlingPlanner and planning batches", self.actor_id);

        // 서비스 구성
        let http_client = Arc::new(HttpClient::create_from_global_config()
            .map_err(|e| SessionError::InitializationFailed(format!("Failed to create HttpClient: {}", e)))?);
        let data_extractor = Arc::new(MatterDataExtractor::new()
            .map_err(|e| SessionError::InitializationFailed(format!("Failed to create MatterDataExtractor: {}", e)))?);

        // DB 연결 재사용 경로가 없다면 간단히 새로 생성
        let database_url = crate::infrastructure::database_paths::get_main_database_url();
        let db_pool = sqlx::SqlitePool::connect(&database_url).await
            .map_err(|e| SessionError::InitializationFailed(format!("Failed to connect to database: {}", e)))?;
        let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));

        // 플래너 생성에 필요한 서비스들
    let status_checker: Arc<dyn StatusChecker> = Arc::new(StatusCheckerImpl::with_product_repo(
            (*http_client).clone(),
            (*data_extractor).clone(),
            AppConfig::for_development(),
            Arc::clone(&product_repo),
    ));
    let db_analyzer: Arc<dyn DatabaseAnalyzer> = Arc::new(DatabaseAnalyzerImpl::new(Arc::clone(&product_repo)));

        let planner = CrawlingPlanner::new(
            status_checker.clone(),
            db_analyzer,
            Arc::clone(&context.config),
        );

        // TTL 5분 캐시 사용해 계획 생성
        let ttl = Duration::from_secs(300);
        let cached = self.site_status_cache.as_ref().and_then(|(status, ts)| {
            if ts.elapsed() <= ttl { Some(status.clone()) } else { None }
        });
        let (plan, used_site_status) = planner.create_crawling_plan_with_cache(&config, cached).await
            .map_err(|e| SessionError::InitializationFailed(format!("Failed to create crawling plan: {}", e)))?;
        info!("📋 Crawling plan created: {} phases", plan.phases.len());
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
        context.emit_event(planning_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // 캐시 갱신 및 사용 로그
        self.site_status_cache = Some((used_site_status.clone(), Instant::now()));
        let site_status = used_site_status;
        info!("🌐 SiteStatus: total_pages={}, products_on_last_page={}", site_status.total_pages, site_status.products_on_last_page);

        // ListPageCrawling phases만 추출 → 각 phase 페이지들을 순차 처리
        let mut batch_idx = 0u32;
        for phase in plan.phases.iter().filter(|p| matches!(p.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling)) {
            batch_idx += 1;
            let pages = phase.pages.clone();
            if pages.is_empty() { continue; }
            let batch_id = format!("{}-batch-{}", session_id, batch_idx);
            info!("🏃 SessionActor {} running batch {} with {} pages: {:?}", self.actor_id, batch_id, pages.len(), pages);

            // 배치 시작 Progress 이벤트 (세션 관점)
            let progress_event = AppEvent::Progress {
                session_id: session_id.clone(),
                current_step: 1,
                total_steps: plan.phases.len() as u32,
                message: format!("Starting batch {} with {} pages", batch_id, pages.len()),
                percentage: ((batch_idx - 1) as f64 / plan.phases.len() as f64) * 100.0,
                timestamp: Utc::now(),
            };
            context.emit_event(progress_event).await
                .map_err(|e| SessionError::ContextError(e.to_string()))?;

            if let Err(e) = self.run_batch_with_services(
                    &batch_id,
                    &pages,
                    context,
                    &http_client,
                    &data_extractor,
                    &product_repo,
                    &site_status,
                ).await {
                error!("❌ Batch {} failed: {}", batch_id, e);
                // 세션 실패 이벤트 발행
                let fail_event = AppEvent::SessionFailed {
                    session_id: session_id.clone(),
                    error: format!("Batch {} failed: {}", batch_id, e),
                    final_failure: false,
                    timestamp: Utc::now(),
                };
                context.emit_event(fail_event).await
                    .map_err(|er| SessionError::ContextError(er.to_string()))?;
                // 일단 다음 배치로 계속 진행 (요구 시 중단 정책으로 변경 가능)
                continue;
            }

            self.processed_batches += 1;
            self.total_success_count += pages.len() as u32;
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
            context.emit_event(progress_event).await
                .map_err(|e| SessionError::ContextError(e.to_string()))?;
        }
        
        // 상태를 Running으로 전환 후 Complete로 이동
        self.state = SessionState::Running;
        
        info!("🎯 SessionActor {} completing session: {} batches, {} pages total", 
              self.actor_id, self.processed_batches, self.total_success_count);
        
        // 세션 완료 처리
        self.state = SessionState::Completed;
        
        // 완료 이벤트 발행
        let completion_event = AppEvent::SessionCompleted {
            session_id: session_id.clone(),
            summary: SessionSummary {
                session_id: session_id.clone(),
                total_duration_ms: self.start_time.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0),
                total_pages_processed: self.total_success_count,
                total_products_processed: 0, // TODO: 실제 제품 수 계산
                success_rate: 1.0, // TODO: 실제 성공률 계산
                avg_page_processing_time: self.start_time.map(|t| t.elapsed().as_millis() as u64 / std::cmp::max(self.total_success_count as u64, 1)).unwrap_or(0),
                error_summary: Vec::new(), // TODO: 실제 에러 수집
                processed_batches: self.processed_batches,
                total_success_count: self.total_success_count,
                final_state: "completed".to_string(),
                timestamp: Utc::now(),
            },
            timestamp: Utc::now(),
        };
        
        context.emit_event(completion_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;

        // === 추가: 세션 리포트 이벤트 발행 ===
        let duration_ms = self.start_time.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
        // 총 실패 페이지/리트라이 수는 배치 리포트 합산이 이상적이지만, 최소한 현재 수치로 요약 제공
        let crawl_report = AppEvent::CrawlReportSession {
            session_id: session_id.clone(),
            batches_processed: self.processed_batches,
            total_pages: self.total_success_count,
            total_success: self.total_success_count,
            total_failed: 0, // TODO: 배치 결과 수집 시 합산
            total_retries: 0, // TODO: 배치 리포트 기반 합산
            duration_ms,
            timestamp: Utc::now(),
        };
        context.emit_event(crawl_report).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        
        info!("✅ Session {} completed successfully", session_id);
        Ok(())
    }

    /// 실서비스가 주입된 BatchActor를 생성해 주어진 페이지들을 처리
    async fn run_batch_with_services(
        &self,
        batch_id: &str,
        pages: &[u32],
        context: &AppContext,
        http_client: &Arc<HttpClient>,
        data_extractor: &Arc<MatterDataExtractor>,
        product_repo: &Arc<IntegratedProductRepository>,
        site_status: &crate::domain::services::SiteStatus,
    ) -> Result<(), SessionError> {
        use crate::new_architecture::actors::traits::Actor;

    // AppConfig 로드(개발 기본값)
    let app_config = AppConfig::for_development();
    let config_concurrency = app_config.user.crawling.workers.list_page_max_concurrent as u32;

        let mut batch_actor = BatchActor::new_with_services(
            batch_id.to_string(),
            batch_id.to_string(),
            Arc::clone(http_client),
            Arc::clone(data_extractor),
            Arc::clone(product_repo),
            app_config.clone(),
        );

        let (tx, rx) = mpsc::channel::<super::types::ActorCommand>(100);

        // BatchActor 실행 태스크 시작
    let actor_context = context.clone();
        let actor_task = tokio::spawn(async move {
            let _ = batch_actor.run(actor_context, rx).await;
        });

        // 배치 설정 및 명령 전송
    // Use config-driven concurrency
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
        tx.send(cmd).await.map_err(|e| SessionError::ContextError(format!("Failed to send ProcessBatch: {}", e)))?;

        // 종료 명령
        tx.send(super::types::ActorCommand::Shutdown).await.map_err(|e| SessionError::ContextError(format!("Failed to send Shutdown: {}", e)))?;

        // 완료 대기
        actor_task.await.map_err(|e| SessionError::ContextError(format!("BatchActor join error: {}", e)))?;
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
                to: SessionState::Paused { reason: reason.clone() },
            });
        }
        
        warn!("⏸️ SessionActor {} pausing session {}: {}", self.actor_id, session_id, reason);
        
        // 상태 업데이트
        self.state = SessionState::Paused { reason: reason.clone() };
        
        // 일시정지 이벤트 발행
        let pause_event = AppEvent::SessionPaused {
            session_id,
            reason,
            timestamp: Utc::now(),
        };
        
        context.emit_event(pause_event).await
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
        
        info!("▶️ SessionActor {} resuming session {}", self.actor_id, session_id);
        
        // 상태 업데이트
        self.state = SessionState::Running;
        
        // 재개 이벤트 발행
        let resume_event = AppEvent::SessionResumed {
            session_id,
            timestamp: Utc::now(),
        };
        
        context.emit_event(resume_event).await
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
        
        error!("❌ SessionActor {} cancelling session {}: {}", self.actor_id, session_id, reason);
        
        // 상태 업데이트
        self.state = SessionState::Failed { error: reason.clone() };
        
        // 취소 이벤트 발행
        let cancel_event = AppEvent::SessionFailed {
            session_id,
            error: reason,
            final_failure: true,
            timestamp: Utc::now(),
        };
        
        context.emit_event(cancel_event).await
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
                "Expected {}, got {}", current_id, session_id
            ))),
            None => Err(SessionError::SessionNotFound("No active session".to_string())),
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
            let duration = self.start_time
                .map(|start| start.elapsed())
                .unwrap_or(Duration::ZERO);
            
            SessionSummary {
                session_id: session_id.clone(),
                total_duration_ms: duration.as_millis() as u64,
                total_pages_processed: 0, // TODO: 실제 처리된 페이지 수 계산
                total_products_processed: 0, // TODO: 실제 처리된 상품 수 계산
                success_rate: if self.processed_batches > 0 { 
                    self.total_success_count as f64 / self.processed_batches as f64 
                } else { 
                    0.0 
                },
                avg_page_processing_time: if self.processed_batches > 0 { 
                    duration.as_millis() as u64 / self.processed_batches as u64 
                } else { 
                    0 
                },
                error_summary: vec![], // TODO: 실제 에러 요약 구현
                processed_batches: self.processed_batches,
                total_success_count: self.total_success_count,
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
    }    async fn run(
        &mut self,
        mut context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🎬 SessionActor {} starting execution loop", self.actor_id);
        
        loop {
            tokio::select! {
                // 명령 처리
                command = command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            debug!("📨 SessionActor {} received command: {:?}", self.actor_id, cmd);
                            
                            match cmd {
                                ActorCommand::StartCrawling { session_id, config } => {
                                    if let Err(e) = self.handle_start_crawling(session_id, config, &context).await {
                                        error!("Failed to start crawling: {}", e);
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
            
            let _ = context.emit_event(completion_event).await;
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
            active_tasks: if matches!(self.state, SessionState::Running) { 1 } else { 0 },
            commands_processed: 0, // TODO: 실제 처리된 명령 수 계산
            errors_count: 0, // TODO: 실제 에러 수 계산
            avg_command_processing_time_ms: 0.0, // TODO: 실제 평균 처리 시간 계산
            metadata: serde_json::json!({
                "session_id": self.session_id,
                "state": format!("{:?}", self.state),
                "processed_batches": self.processed_batches,
                "total_success_count": self.total_success_count
            }).to_string(),
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
