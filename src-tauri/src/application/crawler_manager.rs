//! 통합 크롤러 매니저 - PHASE2_CRAWLING_ENHANCEMENT_PLAN 구현
//! 
//! 이 모듈은 .local/crawling_explanation.md에 정의된 CrawlerManager 역할을 수행하며,
//! 3개의 크롤링 엔진을 통합 관리하고 재시도 메커니즘을 제공합니다.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

use crate::domain::session_manager::SessionManager;
use crate::domain::events::{CrawlingProgress, CrawlingStage, CrawlingStatus};
use crate::application::EventEmitter;
use crate::infrastructure::{
    BatchCrawlingEngine, 
    ServiceBasedBatchCrawlingEngine, 
    AdvancedBatchCrawlingEngine
};

/// 배치 프로세서 트레이트 - 3개 엔진을 통합하는 인터페이스
#[async_trait::async_trait]
pub trait BatchProcessor: Send + Sync {
    async fn execute_task(&self, config: CrawlingConfig) -> Result<TaskResult>;
    async fn get_progress(&self) -> CrawlingProgress;
    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}

/// 크롤링 설정 통합 구조체
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrawlingConfig {
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency: u32,
    pub delay_ms: u64,
    pub batch_size: u32,
    pub retry_max: u32,
    pub timeout_ms: u64,
    pub engine_type: CrawlingEngineType,
}

/// 크롤링 엔진 타입
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CrawlingEngineType {
    Basic,      // BatchCrawlingEngine
    Service,    // ServiceBasedBatchCrawlingEngine  
    Advanced,   // AdvancedBatchCrawlingEngine
}

/// 작업 결과
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub session_id: String,
    pub items_processed: u32,
    pub items_success: u32,
    pub items_failed: u32,
    pub duration: Duration,
    pub final_status: CrawlingStatus,
}

/// 재시도 관리자
pub struct RetryManager {
    max_retries: u32,
    retry_queue: Arc<Mutex<std::collections::VecDeque<RetryItem>>>,
    failure_classifier: Arc<dyn FailureClassifier>,
}

/// 재시도 아이템
#[derive(Debug, Clone)]
pub struct RetryItem {
    pub item_id: String,
    pub stage: CrawlingStage,
    pub attempt_count: u32,
    pub last_error: String,
    pub next_retry_time: DateTime<Utc>,
    pub exponential_backoff: Duration,
}

/// 실패 분류기 트레이트
#[async_trait::async_trait]
pub trait FailureClassifier: Send + Sync {
    async fn classify_error(&self, error: &str, stage: CrawlingStage) -> ErrorClassification;
    async fn calculate_backoff(&self, attempt_count: u32) -> Duration;
}

/// 에러 분류
#[derive(Debug, Clone)]
pub enum ErrorClassification {
    Recoverable { retry_after: Duration },
    NonRecoverable { reason: String },
    RateLimited { retry_after: Duration },
    NetworkError { retry_after: Duration },
}

/// 성능 모니터
pub struct PerformanceMonitor {
    session_metrics: Arc<RwLock<HashMap<String, SessionMetrics>>>,
    global_metrics: Arc<RwLock<GlobalMetrics>>,
}

#[derive(Debug, Clone)]
pub struct SessionMetrics {
    pub start_time: Instant,
    pub items_processed: u32,
    pub average_response_time: Duration,
    pub error_rate: f64,
    pub current_concurrency: u32,
}

#[derive(Debug, Clone)]
pub struct GlobalMetrics {
    pub total_sessions: u32,
    pub active_sessions: u32,
    pub average_session_duration: Duration,
    pub total_items_processed: u64,
    pub overall_success_rate: f64,
}

/// 통합 크롤러 매니저
#[derive(Clone)]
pub struct CrawlerManager {
    // 핵심 컴포넌트
    session_manager: Arc<SessionManager>,
    retry_manager: Arc<RetryManager>,
    performance_monitor: Arc<PerformanceMonitor>,
    event_emitter: Arc<RwLock<Option<EventEmitter>>>,
    
    // 크롤링 엔진들
    basic_engine: Arc<BatchCrawlingEngine>,
    service_engine: Arc<ServiceBasedBatchCrawlingEngine>,
    advanced_engine: Arc<AdvancedBatchCrawlingEngine>,
    
    // 활성 세션들
    active_processors: Arc<RwLock<HashMap<String, Arc<dyn BatchProcessor>>>>,
}

impl CrawlerManager {
    /// 새 크롤러 매니저 생성
    pub fn new(
        session_manager: Arc<SessionManager>,
        basic_engine: Arc<BatchCrawlingEngine>,
        service_engine: Arc<ServiceBasedBatchCrawlingEngine>,
        advanced_engine: Arc<AdvancedBatchCrawlingEngine>,
    ) -> Self {
        let retry_manager = Arc::new(RetryManager::new(3)); // 기본 3회 재시도
        let performance_monitor = Arc::new(PerformanceMonitor::new());
        
        Self {
            session_manager,
            retry_manager,
            performance_monitor,
            event_emitter: Arc::new(RwLock::new(None)),
            basic_engine,
            service_engine,
            advanced_engine,
            active_processors: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 이벤트 에미터 설정
    pub async fn set_event_emitter(&self, emitter: EventEmitter) {
        let mut event_emitter = self.event_emitter.write().await;
        *event_emitter = Some(emitter);
    }
    
    /// 배치 크롤링 시작 - .local/crawling_explanation.md의 CrawlerManager::startBatchCrawling 구현
    pub async fn start_batch_crawling(&self, config: CrawlingConfig) -> Result<String> {
        info!("🚀 Starting batch crawling with config: {:?}", config);
        
        // 1. 세션 생성
        let session_id = self.session_manager.create_session().await;
        info!("📝 Created crawling session: {}", session_id);
        
        // 2. 엔진 타입에 따른 배치 프로세서 선택
        let processor = self.create_batch_processor(&config).await?;
        
        // 3. 활성 프로세서에 등록
        {
            let mut active = self.active_processors.write().await;
            active.insert(session_id.clone(), processor.clone());
        }
        
        // 4. 성능 모니터링 시작
        self.performance_monitor.start_session_tracking(&session_id).await;
        
        // 5. 백그라운드에서 크롤링 실행
        let manager_clone = Arc::new(self.clone());
        let session_id_clone = session_id.clone();
        let config_clone = config.clone();
        
        tokio::spawn(async move {
            match manager_clone.execute_batch_crawling(session_id_clone.clone(), config_clone).await {
                Ok(result) => {
                    info!("✅ Batch crawling completed successfully: {:?}", result);
                    manager_clone.handle_batch_success(&session_id_clone, result).await;
                }
                Err(error) => {
                    error!("❌ Batch crawling failed: {}", error);
                    manager_clone.handle_batch_failure(&session_id_clone, error).await;
                }
            }
        });
        
        Ok(session_id)
    }
    
    /// 배치 크롤링 중지
    pub async fn stop_batch_crawling(&self, session_id: &str) -> Result<()> {
        info!("🛑 Stopping batch crawling for session: {}", session_id);
        
        // 1. 활성 프로세서에서 찾기
        let processor = {
            let active = self.active_processors.read().await;
            active.get(session_id).cloned()
        };
        
        if let Some(processor) = processor {
            // 2. 프로세서 중지
            processor.stop().await?;
            
            // 3. 활성 프로세서에서 제거
            {
                let mut active = self.active_processors.write().await;
                active.remove(session_id);
            }
            
            // 4. 성능 모니터링 중지
            self.performance_monitor.stop_session_tracking(session_id).await;
            
            info!("✅ Batch crawling stopped for session: {}", session_id);
        } else {
            warn!("⚠️ Session not found or already stopped: {}", session_id);
        }
        
        Ok(())
    }
    
    /// 배치 크롤링 일시정지
    pub async fn pause_batch_crawling(&self, session_id: &str) -> Result<()> {
        info!("⏸️ Pausing batch crawling for session: {}", session_id);
        
        let processor = {
            let active = self.active_processors.read().await;
            active.get(session_id).cloned()
        };
        
        if let Some(processor) = processor {
            processor.pause().await?;
            info!("✅ Batch crawling paused for session: {}", session_id);
        } else {
            return Err(anyhow!("Session not found: {}", session_id));
        }
        
        Ok(())
    }
    
    /// 배치 크롤링 재개
    pub async fn resume_batch_crawling(&self, session_id: &str) -> Result<()> {
        info!("▶️ Resuming batch crawling for session: {}", session_id);
        
        let processor = {
            let active = self.active_processors.read().await;
            active.get(session_id).cloned()
        };
        
        if let Some(processor) = processor {
            processor.resume().await?;
            info!("✅ Batch crawling resumed for session: {}", session_id);
        } else {
            return Err(anyhow!("Session not found: {}", session_id));
        }
        
        Ok(())
    }
    
    /// 배치 크롤링 진행 상황 조회
    pub async fn get_batch_progress(&self, session_id: &str) -> Result<CrawlingProgress> {
        let processor = {
            let active = self.active_processors.read().await;
            active.get(session_id).cloned()
        };
        
        if let Some(processor) = processor {
            Ok(processor.get_progress().await)
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }
    
    /// 배치 프로세서 생성
    async fn create_batch_processor(&self, config: &CrawlingConfig) -> Result<Arc<dyn BatchProcessor>> {
        match config.engine_type {
            CrawlingEngineType::Basic => {
                Ok(Arc::new(BasicBatchProcessor::new(self.basic_engine.clone())))
            }
            CrawlingEngineType::Service => {
                Ok(Arc::new(ServiceBatchProcessor::new(self.service_engine.clone())))
            }
            CrawlingEngineType::Advanced => {
                Ok(Arc::new(AdvancedBatchProcessor::new(self.advanced_engine.clone())))
            }
        }
    }
    
    /// 실제 배치 크롤링 실행
    async fn execute_batch_crawling(&self, session_id: String, config: CrawlingConfig) -> Result<TaskResult> {
        let start_time = Instant::now();
        
        // 배치 프로세서 가져오기
        let active_processors = self.active_processors.read().await;
        let processor = active_processors.get(&session_id)
            .ok_or_else(|| anyhow!("Processor not found for session: {}", session_id))?;
        
        // 크롤링 실행
        let result = processor.execute_task(config.clone()).await?;
        
        Ok(TaskResult {
            session_id,
            items_processed: result.items_processed,
            items_success: result.items_success,
            items_failed: result.items_failed,
            duration: start_time.elapsed(),
            final_status: CrawlingStatus::Completed,
        })
    }
    
    /// 배치 성공 처리
    async fn handle_batch_success(&self, session_id: &str, result: TaskResult) {
        info!("🎉 Batch crawling success for session {}: {:?}", session_id, result);
        
        // 세션 상태 업데이트
        self.session_manager.update_session_status(session_id, CrawlingStatus::Completed).await;
        
        // 성능 지표 업데이트
        self.performance_monitor.record_success(session_id, &result).await;
        
        // 이벤트 발송
        self.emit_batch_complete(session_id, result).await;
    }
    
    /// 배치 실패 처리
    async fn handle_batch_failure(&self, session_id: &str, error: anyhow::Error) {
        warn!("💥 Batch crawling failed for session {}: {}", session_id, error);
        
        // 재시도 가능한지 확인
        if self.should_retry(&error).await {
            info!("🔄 Scheduling retry for session: {}", session_id);
            self.schedule_retry(session_id, error.to_string()).await;
        } else {
            // 최종 실패 처리
            self.session_manager.update_session_status(session_id, CrawlingStatus::Error).await;
            self.emit_batch_failed(session_id, error.to_string()).await;
        }
    }
    
    /// 재시도 여부 결정
    async fn should_retry(&self, _error: &anyhow::Error) -> bool {
        // TODO: 에러 타입에 따른 재시도 로직 구현
        false
    }
    
    /// 재시도 예약
    async fn schedule_retry(&self, session_id: &str, error: String) {
        // TODO: RetryManager를 통한 재시도 스케줄링
    }
    
    /// 상태 변경 이벤트 발송
    async fn emit_status_change(&self, session_id: &str, status: CrawlingStatus) {
        if let Some(emitter) = self.event_emitter.read().await.as_ref() {
            let progress = CrawlingProgress {
                current: 0,
                total: 100,
                percentage: 0.0,
                current_stage: CrawlingStage::Idle,
                status,
                new_items: 0,
                updated_items: 0,
                errors: 0,
                timestamp: Utc::now().to_rfc3339(),
                current_step: format!("Session {} status changed", session_id),
                message: "".to_string(),
                elapsed_time: 0,
            };
            
            if let Err(e) = emitter.emit_progress(progress).await {
                error!("Failed to emit status change: {}", e);
            }
        }
    }
    
    /// 배치 완료 이벤트 발송
    async fn emit_batch_complete(&self, session_id: &str, result: TaskResult) {
        if let Some(emitter) = self.event_emitter.read().await.as_ref() {
            let progress = CrawlingProgress {
                current: result.items_processed,
                total: result.items_processed,
                percentage: 100.0,
                current_stage: CrawlingStage::Completed,
                status: CrawlingStatus::Completed,
                new_items: result.items_success,
                updated_items: 0,
                errors: result.items_failed,
                timestamp: Utc::now().to_rfc3339(),
                current_step: format!("Batch crawling completed for session {}", session_id),
                message: format!("Processed {} items in {:?}", result.items_processed, result.duration),
                elapsed_time: result.duration.as_secs(),
            };
            
            if let Err(e) = emitter.emit_progress(progress).await {
                error!("Failed to emit batch complete: {}", e);
            }
        }
    }
    
    /// 배치 실패 이벤트 발송
    async fn emit_batch_failed(&self, session_id: &str, error: String) {
        if let Some(emitter) = self.event_emitter.read().await.as_ref() {
            let progress = CrawlingProgress {
                current: 0,
                total: 0,
                percentage: 0.0,
                current_stage: CrawlingStage::Error,
                status: CrawlingStatus::Error,
                new_items: 0,
                updated_items: 0,
                errors: 1,
                timestamp: Utc::now().to_rfc3339(),
                current_step: format!("Batch crawling failed for session {}", session_id),
                message: error,
                elapsed_time: 0,
            };
            
            if let Err(e) = emitter.emit_progress(progress).await {
                error!("Failed to emit batch failed: {}", e);
            }
        }
    }
}

// Clone 구현 (Arc 기반이므로 안전)
impl Clone for CrawlerManager {
    fn clone(&self) -> Self {
        Self {
            session_manager: self.session_manager.clone(),
            retry_manager: self.retry_manager.clone(),
            performance_monitor: self.performance_monitor.clone(),
            event_emitter: self.event_emitter.clone(),
            basic_engine: self.basic_engine.clone(),
            service_engine: self.service_engine.clone(),
            advanced_engine: self.advanced_engine.clone(),
            active_processors: self.active_processors.clone(),
        }
    }
}

// ============================================================================
// BatchProcessor 구현체들
// ============================================================================

impl BasicBatchProcessor {
    pub fn new(engine: Arc<BatchCrawlingEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl BatchProcessor for BasicBatchProcessor {
    async fn execute_task(&self, config: CrawlingConfig) -> Result<TaskResult> {
        info!("🔧 Executing task with BasicBatchProcessor");
        
        // BatchCrawlingEngine의 설정 타입으로 변환
        let batch_config = crate::infrastructure::crawling_engine::BatchCrawlingConfig {
            start_page: config.start_page,
            end_page: config.end_page,
            concurrency: config.concurrency,
            delay_ms: config.delay_ms,
            batch_size: config.batch_size,
            retry_max: config.retry_max,
            timeout_ms: config.timeout_ms,
        };
        
        // 실제 크롤링 실행 (TODO: 실제 메서드 호출로 교체)
        let start_time = Instant::now();
        
        // 임시 결과 반환 (실제 구현에서는 engine을 사용)
        Ok(TaskResult {
            session_id: "basic_session".to_string(),
            items_processed: config.end_page - config.start_page + 1,
            items_success: config.end_page - config.start_page + 1,
            items_failed: 0,
            duration: start_time.elapsed(),
            final_status: CrawlingStatus::Completed,
        })
    }
    
    async fn get_progress(&self) -> CrawlingProgress {
        CrawlingProgress {
            current: 0,
            total: 100,
            percentage: 0.0,
            current_stage: CrawlingStage::Processing,
            status: CrawlingStatus::Running,
            new_items: 0,
            updated_items: 0,
            errors: 0,
            timestamp: Utc::now().to_rfc3339(),
            current_step: "BasicBatchProcessor running".to_string(),
            message: "".to_string(),
            elapsed_time: 0,
        }
    }
    
    async fn pause(&self) -> Result<()> {
        info!("⏸️ BasicBatchProcessor paused");
        Ok(())
    }
    
    async fn resume(&self) -> Result<()> {
        info!("▶️ BasicBatchProcessor resumed");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("⏹️ BasicBatchProcessor stopped");
        Ok(())
    }
}

impl ServiceBatchProcessor {
    pub fn new(engine: Arc<ServiceBasedBatchCrawlingEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl BatchProcessor for ServiceBatchProcessor {
    async fn execute_task(&self, config: CrawlingConfig) -> Result<TaskResult> {
        info!("🔧 Executing task with ServiceBatchProcessor");
        
        let start_time = Instant::now();
        
        // 임시 결과 반환 (실제 구현에서는 engine을 사용)
        Ok(TaskResult {
            session_id: "service_session".to_string(),
            items_processed: config.end_page - config.start_page + 1,
            items_success: config.end_page - config.start_page + 1,
            items_failed: 0,
            duration: start_time.elapsed(),
            final_status: CrawlingStatus::Completed,
        })
    }
    
    async fn get_progress(&self) -> CrawlingProgress {
        CrawlingProgress {
            current: 0,
            total: 100,
            percentage: 0.0,
            current_stage: CrawlingStage::Processing,
            status: CrawlingStatus::Running,
            new_items: 0,
            updated_items: 0,
            errors: 0,
            timestamp: Utc::now().to_rfc3339(),
            current_step: "ServiceBatchProcessor running".to_string(),
            message: "".to_string(),
            elapsed_time: 0,
        }
    }
    
    async fn pause(&self) -> Result<()> {
        info!("⏸️ ServiceBatchProcessor paused");
        Ok(())
    }
    
    async fn resume(&self) -> Result<()> {
        info!("▶️ ServiceBatchProcessor resumed");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("⏹️ ServiceBatchProcessor stopped");
        Ok(())
    }
}

impl AdvancedBatchProcessor {
    pub fn new(engine: Arc<AdvancedBatchCrawlingEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl BatchProcessor for AdvancedBatchProcessor {
    async fn execute_task(&self, config: CrawlingConfig) -> Result<TaskResult> {
        info!("🔧 Executing task with AdvancedBatchProcessor");
        
        let start_time = Instant::now();
        
        // 임시 결과 반환 (실제 구현에서는 engine을 사용)
        Ok(TaskResult {
            session_id: "advanced_session".to_string(),
            items_processed: config.end_page - config.start_page + 1,
            items_success: config.end_page - config.start_page + 1,
            items_failed: 0,
            duration: start_time.elapsed(),
            final_status: CrawlingStatus::Completed,
        })
    }
    
    async fn get_progress(&self) -> CrawlingProgress {
        CrawlingProgress {
            current: 0,
            total: 100,
            percentage: 0.0,
            current_stage: CrawlingStage::Processing,
            status: CrawlingStatus::Running,
            new_items: 0,
            updated_items: 0,
            errors: 0,
            timestamp: Utc::now().to_rfc3339(),
            current_step: "AdvancedBatchProcessor running".to_string(),
            message: "".to_string(),
            elapsed_time: 0,
        }
    }
    
    async fn pause(&self) -> Result<()> {
        info!("⏸️ AdvancedBatchProcessor paused");
        Ok(())
    }
    
    async fn resume(&self) -> Result<()> {
        info!("▶️ AdvancedBatchProcessor resumed");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("⏹️ AdvancedBatchProcessor stopped");
        Ok(())
    }
}

// 재시도 관리자 구현
impl RetryManager {
    /// 새로운 재시도 관리자 생성
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            retry_queue: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            failure_classifier: Arc::new(DefaultFailureClassifier::new()),
        }
    }
    
    /// 실패한 항목을 재시도 큐에 추가
    pub async fn add_retry_item(&self, item_id: String, stage: CrawlingStage, error: String) -> Result<()> {
        let mut queue = self.retry_queue.lock().await;
        
        // 기존 항목이 있는지 확인
        let existing_item = queue.iter_mut().find(|item| item.item_id == item_id);
        
        if let Some(item) = existing_item {
            // 기존 항목 업데이트
            item.attempt_count += 1;
            item.last_error = error.clone();
            item.exponential_backoff = self.failure_classifier.calculate_backoff(item.attempt_count).await;
            item.next_retry_time = Utc::now() + chrono::Duration::from_std(item.exponential_backoff)?;
            
            if item.attempt_count >= self.max_retries {
                warn!("❌ Item {} exceeded max retries ({}), removing from queue", item_id, self.max_retries);
                queue.retain(|i| i.item_id != item_id);
                return Ok(());
            }
        } else {
            // 새 항목 추가
            let classification = self.failure_classifier.classify_error(&error, stage).await;
            
            match classification {
                ErrorClassification::NonRecoverable { reason } => {
                    warn!("❌ Non-recoverable error for {}: {}", item_id, reason);
                    return Ok(());
                }
                ErrorClassification::Recoverable { retry_after } |
                ErrorClassification::RateLimited { retry_after } |
                ErrorClassification::NetworkError { retry_after } => {
                    let retry_item = RetryItem {
                        item_id,
                        stage,
                        attempt_count: 1,
                        last_error: error,
                        next_retry_time: Utc::now() + chrono::Duration::from_std(retry_after)?,
                        exponential_backoff: retry_after,
                    };
                    
                    queue.push_back(retry_item);
                    info!("🔄 Added item to retry queue, total items: {}", queue.len());
                }
            }
        }
        
        Ok(())
    }
    
    /// 재시도 가능한 항목들을 가져옴
    pub async fn get_ready_items(&self) -> Vec<RetryItem> {
        let mut queue = self.retry_queue.lock().await;
        let now = Utc::now();
        
        let ready_items: Vec<RetryItem> = queue
            .iter()
            .filter(|item| item.next_retry_time <= now)
            .cloned()
            .collect();
        
        // 준비된 항목들을 큐에서 제거
        queue.retain(|item| item.next_retry_time > now);
        
        if !ready_items.is_empty() {
            info!("🔄 Retrieved {} items ready for retry", ready_items.len());
        }
        
        ready_items
    }
    
    /// 재시도 큐 상태 조회
    pub async fn get_queue_status(&self) -> (usize, usize) {
        let queue = self.retry_queue.lock().await;
        let total_items = queue.len();
        let ready_items = queue
            .iter()
            .filter(|item| item.next_retry_time <= Utc::now())
            .count();
        
        (total_items, ready_items)
    }
}

/// 기본 실패 분류기 구현
pub struct DefaultFailureClassifier;

impl DefaultFailureClassifier {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FailureClassifier for DefaultFailureClassifier {
    async fn classify_error(&self, error: &str, stage: CrawlingStage) -> ErrorClassification {
        let error_lower = error.to_lowercase();
        
        // HTTP 상태 코드 분석
        if error_lower.contains("429") || error_lower.contains("rate limit") {
            return ErrorClassification::RateLimited { 
                retry_after: Duration::from_secs(60) // 1분 대기
            };
        }
        
        if error_lower.contains("timeout") || error_lower.contains("connection") {
            return ErrorClassification::NetworkError { 
                retry_after: Duration::from_secs(30) // 30초 대기
            };
        }
        
        if error_lower.contains("500") || error_lower.contains("502") || error_lower.contains("503") {
            return ErrorClassification::Recoverable { 
                retry_after: Duration::from_secs(45) // 45초 대기
            };
        }
        
        // 404, 400 등은 재시도해도 의미 없음
        if error_lower.contains("404") || error_lower.contains("400") || error_lower.contains("403") {
            return ErrorClassification::NonRecoverable { 
                reason: "Client error - retry will not help".to_string()
            };
        }
        
        // 파싱 단계별 에러 분류
        match stage {
            CrawlingStage::ParseItemDetails => {
                if error_lower.contains("parse") || error_lower.contains("format") {
                    ErrorClassification::NonRecoverable { 
                        reason: "Parsing error - data format issue".to_string()
                    }
                } else {
                    ErrorClassification::Recoverable { 
                        retry_after: Duration::from_secs(20)
                    }
                }
            }
            CrawlingStage::DatabaseSave => {
                ErrorClassification::Recoverable { 
                    retry_after: Duration::from_secs(10) // DB 에러는 빠르게 재시도
                }
            }
            _ => {
                ErrorClassification::Recoverable { 
                    retry_after: Duration::from_secs(30)
                }
            }
        }
    }
    
    async fn calculate_backoff(&self, attempt_count: u32) -> Duration {
        // 지수 백오프: 2^attempt_count 초, 최대 5분
        let base_seconds = 2_u64.pow(attempt_count.min(8)); // 최대 256초
        let max_seconds = 300; // 5분
        
        Duration::from_secs(base_seconds.min(max_seconds))
    }
}

impl PerformanceMonitor {
    /// 새로운 성능 모니터 생성
    pub fn new() -> Self {
        Self {
            session_metrics: Arc::new(RwLock::new(HashMap::new())),
            global_metrics: Arc::new(RwLock::new(GlobalMetrics {
                total_sessions: 0,
                active_sessions: 0,
                average_session_duration: Duration::from_secs(0),
                total_items_processed: 0,
                overall_success_rate: 0.0,
            })),
        }
    }
    
    /// 세션 추적 시작
    pub async fn start_session_tracking(&self, session_id: &str) {
        let mut sessions = self.session_metrics.write().await;
        let mut global = self.global_metrics.write().await;
        
        sessions.insert(session_id.to_string(), SessionMetrics {
            start_time: Instant::now(),
            items_processed: 0,
            average_response_time: Duration::from_secs(0),
            error_rate: 0.0,
            current_concurrency: 1,
        });
        
        global.total_sessions += 1;
        global.active_sessions += 1;
        
        info!("📊 Started performance tracking for session: {}", session_id);
    }
    
    /// 세션 추적 종료
    pub async fn end_session_tracking(&self, session_id: &str) {
        let mut sessions = self.session_metrics.write().await;
        let mut global = self.global_metrics.write().await;
        
        if sessions.remove(session_id).is_some() {
            global.active_sessions = global.active_sessions.saturating_sub(1);
            info!("📊 Ended performance tracking for session: {}", session_id);
        }
    }
    
    /// 세션 메트릭 업데이트
    pub async fn update_session_metrics(&self, session_id: &str, items_processed: u32, response_time: Duration, error_rate: f64) {
        let mut sessions = self.session_metrics.write().await;
        
        if let Some(metrics) = sessions.get_mut(session_id) {
            metrics.items_processed = items_processed;
            metrics.average_response_time = response_time;
            metrics.error_rate = error_rate;
        }
    }
    
    /// 글로벌 메트릭 조회
    pub async fn get_global_metrics(&self) -> GlobalMetrics {
        self.global_metrics.read().await.clone()
    }
    
    /// 세션 메트릭 조회
    pub async fn get_session_metrics(&self, session_id: &str) -> Option<SessionMetrics> {
        let sessions = self.session_metrics.read().await;
        sessions.get(session_id).cloned()
    }
}
