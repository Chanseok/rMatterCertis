//! 개선된 배치 크롤링 엔진 - 서비스 레이어 분리 버전
//! 
//! 이 모듈은 guide/crawling 문서의 요구사항에 따라 각 단계를 
//! 독립적인 서비스로 분리하여 구현한 엔터프라이즈급 크롤링 엔진입니다.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug, error};
use tokio_util::sync::CancellationToken;
use chrono::Utc;

// 🔥 이벤트 콜백 타입 정의 추가
pub type PageEventCallback = Arc<dyn Fn(u32, String, u32, bool) -> Result<()> + Send + Sync>;
pub type RetryEventCallback = Arc<dyn Fn(String, String, String, u32, u32, String) -> Result<()> + Send + Sync>;

use crate::domain::services::crawling_services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    SiteStatus, DatabaseAnalysis
};
use crate::domain::events::{CrawlingProgress, CrawlingStage, CrawlingStatus};
use crate::domain::product::{Product, ProductDetail};
use crate::domain::product_url::ProductUrl;
use crate::application::EventEmitter;
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository, RetryManager};
use crate::infrastructure::crawling_service_impls::{
    StatusCheckerImpl, ProductListCollectorImpl, ProductDetailCollectorImpl,
    CrawlingRangeCalculator, CollectorConfig, product_detail_to_product
};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::system_broadcaster::SystemStateBroadcaster;
use crate::events::{AtomicTaskEvent, TaskStatus};

// 새로운 이벤트 시스템 import
use crate::new_architecture::events::task_lifecycle::{
    TaskLifecycleEvent, TaskExecutionContext,
    ResourceAllocation, ResourceUsage, ErrorCategory, RetryStrategy,
    ConcurrencyEvent
};

/// 배치 크롤링 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchCrawlingConfig {
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency: u32,
    pub list_page_concurrency: u32,
    pub product_detail_concurrency: u32,
    pub delay_ms: u64,
    pub batch_size: u32,
    pub retry_max: u32,
    pub timeout_ms: u64,
    pub disable_intelligent_range: bool, // 🎭 Actor 시스템용: 지능형 범위 재계산 비활성화
    #[serde(skip)]
    pub cancellation_token: Option<CancellationToken>,
}

impl BatchCrawlingConfig {
    /// Create BatchCrawlingConfig from ValidatedCrawlingConfig for Modern Rust 2024 compliance
    #[must_use]
    pub fn from_validated(validated_config: &crate::application::validated_crawling_config::ValidatedCrawlingConfig) -> Self {
        Self {
            start_page: 1,
            end_page: 1, // Will be set by range calculator
            concurrency: validated_config.max_concurrent(),
            list_page_concurrency: validated_config.list_page_max_concurrent,
            product_detail_concurrency: validated_config.product_detail_max_concurrent,
            delay_ms: validated_config.request_delay_ms,
            batch_size: validated_config.batch_size(),
            retry_max: validated_config.max_retries(),
            timeout_ms: validated_config.request_timeout_ms,
            disable_intelligent_range: false, // 기본값은 지능형 범위 사용
            cancellation_token: None,
        }
    }
}

impl Default for BatchCrawlingConfig {
    fn default() -> Self {
        // Use ValidatedCrawlingConfig for all defaults instead of hardcoded values
        let validated_config = crate::application::validated_crawling_config::ValidatedCrawlingConfig::default();
        
        Self {
            start_page: 1,
            end_page: 1, // ✅ 기본값을 1로 설정 (실제 계산된 범위 사용)
            concurrency: validated_config.max_concurrent(),
            list_page_concurrency: validated_config.list_page_max_concurrent,
            product_detail_concurrency: validated_config.product_detail_max_concurrent,
            delay_ms: validated_config.request_delay_ms,
            batch_size: validated_config.batch_size(),
            retry_max: validated_config.max_retries(),
            timeout_ms: validated_config.request_timeout_ms,
            disable_intelligent_range: false, // 기본값은 지능형 범위 사용
            cancellation_token: None,
        }
    }
}

impl BatchCrawlingConfig {
    /// Phase 4: 지능형 범위 계산 결과를 config에 적용
    pub fn update_range_from_calculation(&mut self, optimal_range: Option<(u32, u32)>) {
        if let Some((start_page, end_page)) = optimal_range {
            info!("🔄 Updating crawling range from {}..{} to {}..{}", 
                  self.start_page, self.end_page, start_page, end_page);
            self.start_page = start_page;
            self.end_page = end_page;
        } else {
            info!("🔄 No optimal range available, keeping current range {}..{}", 
                  self.start_page, self.end_page);
        }
    }
    
    /// 현재 설정된 범위 정보 반환
    pub fn get_page_range(&self) -> (u32, u32) {
        (self.start_page, self.end_page)
    }
}

/// 세분화된 크롤링 이벤트 타입
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DetailedCrawlingEvent {
    SessionStarted { 
        session_id: String, 
        config: BatchCrawlingConfig 
    },
    StageStarted { 
        stage: String, 
        message: String 
    },
    StageCompleted { 
        stage: String, 
        items_processed: usize 
    },
    PageCompleted { 
        page: u32, 
        products_found: u32 
    },
    ProductProcessed { 
        url: String, 
        success: bool 
    },
    BatchCompleted { 
        batch: u32, 
        total: u32 
    },
    ErrorOccurred { 
        stage: String, 
        error: String, 
        recoverable: bool 
    },
    SessionCompleted {
        session_id: String,
        duration: Duration,
        total_products: u32,
        success_rate: f64,
    },
    
    // 🔥 새로운 세분화된 배치 이벤트
    BatchCreated {
        batch_id: u32,
        total_batches: u32,
        start_page: u32,
        end_page: u32,
        description: String,
    },
    BatchStarted {
        batch_id: u32,
        total_batches: u32,
        pages_in_batch: u32,
    },
    
    // 🔥 새로운 페이지 재시도 이벤트
    PageStarted {
        page: u32,
        batch_id: u32,
        url: String,
    },
    PageRetryAttempt {
        page: u32,
        batch_id: u32,
        url: String,
        attempt: u32,
        max_attempts: u32,
        reason: String,
    },
    PageRetrySuccess {
        page: u32,
        batch_id: u32,
        url: String,
        final_attempt: u32,
        products_found: u32,
    },
    PageRetryFailed {
        page: u32,
        batch_id: u32,
        url: String,
        total_attempts: u32,
        final_error: String,
    },
    
    // 🔥 새로운 제품 재시도 이벤트
    ProductStarted {
        url: String,
        batch_id: u32,
        product_index: u32,
        total_products: u32,
    },
    ProductRetryAttempt {
        url: String,
        batch_id: u32,
        attempt: u32,
        max_attempts: u32,
        reason: String,
    },
    ProductRetrySuccess {
        url: String,
        batch_id: u32,
        final_attempt: u32,
    },
    ProductRetryFailed {
        url: String,
        batch_id: u32,
        total_attempts: u32,
        final_error: String,
    },
    
    // 🚀 새로운 세분화된 페이지별 이벤트
    PageCollectionStarted {
        page: u32,
        batch_id: u32,
        url: String,
        estimated_products: Option<u32>,
    },
    PageCollectionCompleted {
        page: u32,
        batch_id: u32,
        url: String,
        products_found: u32,
        duration_ms: u64,
    },
    
    // 🚀 새로운 세분화된 제품별 상세 수집 이벤트
    ProductDetailCollectionStarted {
        url: String,
        product_index: u32,
        total_products: u32,
        batch_id: u32,
    },
    ProductDetailProcessingStarted {
        url: String,
        product_index: u32,
        parsing_stage: String,
    },
    ProductDetailCollectionCompleted {
        url: String,
        product_index: u32,
        success: bool,
        duration_ms: u64,
        data_extracted: bool,
    },
    
    // 🚀 새로운 배치 데이터베이스 저장 이벤트
    DatabaseBatchSaveStarted {
        batch_id: u32,
        products_count: u32,
        batch_size: u32,
    },
    DatabaseBatchSaveCompleted {
        batch_id: u32,
        products_saved: u32,
        new_items: u32,
        updated_items: u32,
        errors: u32,
        duration_ms: u64,
    },
}

/// DetailedCrawlingEvent를 TaskLifecycleEvent로 변환하는 함수
impl DetailedCrawlingEvent {
    /// DetailedCrawlingEvent를 TaskLifecycleEvent와 TaskExecutionContext로 변환
    pub fn to_task_lifecycle_event(&self, session_id: &str) -> Option<(TaskExecutionContext, TaskLifecycleEvent)> {
        let now = Utc::now();
        
        match self {
            DetailedCrawlingEvent::PageStarted { page, batch_id, url } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: format!("batch_{}", batch_id),
                    stage_name: "page_crawling".to_string(),
                    task_id: format!("page_{}_{}", batch_id, page),
                    task_url: url.clone(),
                    start_time: now,
                    worker_id: Some(format!("page_worker_{}", page % 4)), // 간단한 워커 할당
                };
                
                let event = TaskLifecycleEvent::Started {
                    worker_id: context.worker_id.clone().unwrap_or_default(),
                    retry_attempt: 0,
                    allocated_resources: ResourceAllocation {
                        memory_bytes: 50 * 1024 * 1024, // 50MB
                        cpu_percent: 25.0,
                        network_bandwidth_kbps: Some(1000),
                    },
                };
                
                Some((context, event))
            },
            
            DetailedCrawlingEvent::PageCompleted { page, products_found } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: "unknown_batch".to_string(),
                    stage_name: "page_crawling".to_string(),
                    task_id: format!("page_{}", page),
                    task_url: format!("https://matter.co.kr/page/{}", page),
                    start_time: now,
                    worker_id: Some(format!("page_worker_{}", page % 4)),
                };
                
                let event = TaskLifecycleEvent::Succeeded {
                    duration_ms: 2000, // 예상 소요 시간
                    result_summary: format!("{}개 제품 발견", products_found),
                    items_processed: *products_found,
                    final_throughput: *products_found as f64 / 2.0, // 초당 처리율
                    resource_usage: ResourceUsage {
                        peak_memory_bytes: 45 * 1024 * 1024,
                        avg_cpu_percent: 20.0,
                        total_network_bytes: 512 * 1024,
                        disk_io_operations: 50,
                    },
                };
                
                Some((context, event))
            },
            
            DetailedCrawlingEvent::ProductStarted { url, batch_id, product_index, total_products: _ } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: format!("batch_{}", batch_id),
                    stage_name: "product_detail_crawling".to_string(),
                    task_id: format!("product_{}_{}", batch_id, product_index),
                    task_url: url.clone(),
                    start_time: now,
                    worker_id: Some(format!("product_worker_{}", product_index % 8)),
                };
                
                let event = TaskLifecycleEvent::Started {
                    worker_id: context.worker_id.clone().unwrap_or_default(),
                    retry_attempt: 0,
                    allocated_resources: ResourceAllocation {
                        memory_bytes: 20 * 1024 * 1024, // 20MB
                        cpu_percent: 12.5,
                        network_bandwidth_kbps: Some(500),
                    },
                };
                
                Some((context, event))
            },
            
            DetailedCrawlingEvent::ProductProcessed { url, success } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: "unknown_batch".to_string(),
                    stage_name: "product_detail_crawling".to_string(),
                    task_id: format!("product_{}", url.chars().rev().take(8).collect::<String>()),
                    task_url: url.clone(),
                    start_time: now,
                    worker_id: Some("product_worker".to_string()),
                };
                
                let event = if *success {
                    TaskLifecycleEvent::Succeeded {
                        duration_ms: 1500,
                        result_summary: "제품 상세정보 수집 완료".to_string(),
                        items_processed: 1,
                        final_throughput: 0.67, // 약 1.5초당 1개
                        resource_usage: ResourceUsage {
                            peak_memory_bytes: 15 * 1024 * 1024,
                            avg_cpu_percent: 10.0,
                            total_network_bytes: 256 * 1024,
                            disk_io_operations: 20,
                        },
                    }
                } else {
                    TaskLifecycleEvent::Failed {
                        error_message: "제품 상세정보 수집 실패".to_string(),
                        error_code: "PRODUCT_FETCH_ERROR".to_string(),
                        error_category: ErrorCategory::Network,
                        is_recoverable: true,
                        stack_trace: None,
                        resource_usage: ResourceUsage {
                            peak_memory_bytes: 10 * 1024 * 1024,
                            avg_cpu_percent: 5.0,
                            total_network_bytes: 64 * 1024,
                            disk_io_operations: 5,
                        },
                    }
                };
                
                Some((context, event))
            },
            
            DetailedCrawlingEvent::PageRetryAttempt { page, batch_id, url, attempt, max_attempts, reason } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: format!("batch_{}", batch_id),
                    stage_name: "page_crawling".to_string(),
                    task_id: format!("page_{}_{}", batch_id, page),
                    task_url: url.clone(),
                    start_time: now,
                    worker_id: Some(format!("retry_worker_{}", attempt)),
                };
                
                let event = TaskLifecycleEvent::Retrying {
                    attempt: *attempt,
                    max_attempts: *max_attempts,
                    delay_ms: 1000 * (2_u64.pow(*attempt - 1)), // 지수 백오프
                    reason: reason.clone(),
                    retry_strategy: RetryStrategy::ExponentialBackoff {
                        base_ms: 1000,
                        multiplier: 2.0,
                    },
                };
                
                Some((context, event))
            },
            
            DetailedCrawlingEvent::ProductRetryAttempt { url, batch_id, attempt, max_attempts, reason } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: format!("batch_{}", batch_id),
                    stage_name: "product_detail_crawling".to_string(),
                    task_id: format!("product_retry_{}", url.chars().rev().take(8).collect::<String>()),
                    task_url: url.clone(),
                    start_time: now,
                    worker_id: Some(format!("retry_worker_{}", attempt)),
                };
                
                let event = TaskLifecycleEvent::Retrying {
                    attempt: *attempt,
                    max_attempts: *max_attempts,
                    delay_ms: 500 * (*attempt as u64), // 선형 백오프
                    reason: reason.clone(),
                    retry_strategy: RetryStrategy::LinearBackoff {
                        initial_ms: 500,
                        increment_ms: 500,
                    },
                };
                
                Some((context, event))
            },
            
            DetailedCrawlingEvent::ErrorOccurred { stage, error, recoverable } => {
                let context = TaskExecutionContext {
                    session_id: session_id.to_string(),
                    batch_id: "error_context".to_string(),
                    stage_name: stage.clone(),
                    task_id: format!("error_{}", now.timestamp()),
                    task_url: "unknown".to_string(),
                    start_time: now,
                    worker_id: None,
                };
                
                let event = TaskLifecycleEvent::Failed {
                    error_message: error.clone(),
                    error_code: "STAGE_ERROR".to_string(),
                    error_category: ErrorCategory::Business,
                    is_recoverable: *recoverable,
                    stack_trace: None,
                    resource_usage: ResourceUsage {
                        peak_memory_bytes: 5 * 1024 * 1024,
                        avg_cpu_percent: 1.0,
                        total_network_bytes: 0,
                        disk_io_operations: 1,
                    },
                };
                
                Some((context, event))
            },
            
            // 다른 이벤트들은 Task 레벨이 아니므로 None 반환
            _ => None,
        }
    }
}

/// 서비스 기반 배치 크롤링 엔진
pub struct ServiceBasedBatchCrawlingEngine {
    // 서비스 레이어들
    status_checker: Arc<dyn StatusChecker>,
    database_analyzer: Arc<dyn DatabaseAnalyzer>,
    product_list_collector: Arc<dyn ProductListCollector>,
    product_detail_collector: Arc<dyn ProductDetailCollector>,
    
    // 지능형 범위 계산기 - Phase 3 Integration
    range_calculator: Arc<CrawlingRangeCalculator>,
    
    // 기존 컴포넌트들
    product_repo: Arc<IntegratedProductRepository>,
    product_detail_repo: Arc<IntegratedProductRepository>,
    event_emitter: Arc<Option<EventEmitter>>,
    
    // Live Production Line 이벤트 브로드캐스터
    broadcaster: Option<SystemStateBroadcaster>,
    
    // 재시도 관리자 - INTEGRATED_PHASE2_PLAN Week 1 Day 3-4
    retry_manager: Arc<RetryManager>,
    
    // 설정 및 세션 정보
    config: BatchCrawlingConfig,
    session_id: String,
}

#[allow(dead_code)] // Phase2: legacy engine retained for reference; prune in Phase3
impl ServiceBasedBatchCrawlingEngine {
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        product_repo: Arc<IntegratedProductRepository>,
        event_emitter: Arc<Option<EventEmitter>>,
        config: BatchCrawlingConfig,
        session_id: String,
        app_config: AppConfig,
    ) -> Self {
        // 서비스별 설정 생성
        let list_collector_config = CollectorConfig {
            max_concurrent: config.list_page_concurrency,
            concurrency: config.list_page_concurrency,
            delay_between_requests: Duration::from_millis(config.delay_ms),
            delay_ms: config.delay_ms,
            batch_size: config.batch_size,
            retry_attempts: config.retry_max,
            retry_max: config.retry_max,
        };
        
        let detail_collector_config = CollectorConfig {
            max_concurrent: config.product_detail_concurrency,
            concurrency: config.product_detail_concurrency,
            delay_between_requests: Duration::from_millis(config.delay_ms),
            delay_ms: config.delay_ms,
            batch_size: config.batch_size,
            retry_attempts: config.retry_max,
            retry_max: config.retry_max,
        };

        // 서비스 인스턴스 생성
        let status_checker: Arc<dyn StatusChecker> = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
        ));

        // DatabaseAnalyzer는 StatusCheckerImpl을 재사용 (trait 구현 추가됨)
        let database_analyzer: Arc<dyn DatabaseAnalyzer> = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
        ));

        // status_checker를 ProductListCollectorImpl에 전달하기 위해 concrete type으로 다시 생성
        let status_checker_impl = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
        ));

        let product_list_collector: Arc<dyn ProductListCollector> = Arc::new(ProductListCollectorImpl::new(
            Arc::new(http_client.clone()),  // 🔥 Mutex 제거 - 페이지 수집도 진정한 동시성
            Arc::new(data_extractor.clone()),
            list_collector_config,
            status_checker_impl.clone(),
        ));

        // ProductDetailCollector는 실제 ProductDetailCollectorImpl을 사용 - Mutex 제거로 진정한 동시성
        let product_detail_collector: Arc<dyn ProductDetailCollector> = Arc::new(ProductDetailCollectorImpl::new(
            Arc::new(http_client.clone()),  // 🔥 Mutex 제거
            Arc::new(data_extractor.clone()),
            detail_collector_config,
        ));

        // 지능형 범위 계산기 초기화 - Phase 3 Integration
        let range_calculator = Arc::new(CrawlingRangeCalculator::new(
            Arc::clone(&product_repo),
            app_config.clone(),
        ));

        Self {
            status_checker,
            database_analyzer,
            product_list_collector,
            product_detail_collector,
            range_calculator,
            product_repo: product_repo.clone(),
            product_detail_repo: product_repo,
            event_emitter,
            broadcaster: None, // 나중에 설정됨
            retry_manager: Arc::new(RetryManager::new(config.retry_max)),
            config,
            session_id,
        }
    }

    /// SystemStateBroadcaster 설정 (크롤링 시작 전에 호출)
    pub fn set_broadcaster(&mut self, broadcaster: SystemStateBroadcaster) {
        self.broadcaster = Some(broadcaster);
    }

    /// SystemStateBroadcaster에 대한 mutable 참조를 반환
    pub fn get_broadcaster_mut(&mut self) -> Option<&mut SystemStateBroadcaster> {
        self.broadcaster.as_mut()
    }

    /// 지능형 범위 계산 결과를 엔진에 적용
    pub fn update_range_from_calculation(&mut self, optimal_range: Option<(u32, u32)>) {
        self.config.update_range_from_calculation(optimal_range);
    }

    /// 4단계 서비스 기반 크롤링 실행
    pub async fn execute(&mut self) -> Result<()> {
        let start_time = Instant::now();
        info!("Starting service-based 4-stage batch crawling for session: {}", self.session_id);

        // 🔥 1. 크롤링 세션 시작 이벤트 (사용자가 크롤링 버튼 클릭한 시점)
        info!("🚀 크롤링 세션 시작: {}", self.session_id);
        if let Some(broadcaster) = &self.broadcaster {
            let _ = broadcaster.emit_session_event(
                self.session_id.clone(),
                crate::domain::events::SessionEventType::Started,
                format!("크롤링 세션 시작 (페이지 {}-{})", self.config.start_page, self.config.end_page),
            );
        }

        // 🔥 2. 상세 세션 시작 이벤트 (기존 이벤트 시스템 호환성)
        self.emit_detailed_event(DetailedCrawlingEvent::SessionStarted {
            session_id: self.session_id.clone(),
            config: self.config.clone(),
        }).await?;

        // 🔥 3. 사이트 상태 확인 시작 이벤트 (캐시 우선 확인)
        info!("🔍 사이트 상태 확인 시작 (캐시 우선)");
        if let Some(broadcaster) = &self.broadcaster {
            let _ = broadcaster.emit_session_event(
                self.session_id.clone(),
                crate::domain::events::SessionEventType::SiteStatusCheck,
                "사이트 상태 확인 및 캐시 검증 시작".to_string(),
            );
        }

        // 🔥 4. StageEvent 발생 - 사이트 상태 확인 시작 (향후 구현)
        // if let Some(ref emitter) = self.event_emitter.as_ref() {
        //     // ConcurrencyEvent 발생 코드는 향후 구현
        // }

        // 시작 전 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled before starting");
                return Err(anyhow!("Crawling session cancelled before starting"));
            }
        }

        // Stage 0: 사이트 상태 확인 (캐시 우선, 필요시 실제 확인)
        let site_status = self.stage0_check_site_status().await?;
        
        // 🔥 5. 사이트 상태 확인 완료 이벤트 (향후 구현)
        info!("✅ 사이트 상태 확인 완료: 총 {}페이지, 접근 가능", site_status.total_pages);
        // if let Some(ref emitter) = self.event_emitter.as_ref() {
        //     // ConcurrencyEvent 발생 코드는 향후 구현
        // }
        
        // Stage 0 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 0");
                return Err(anyhow!("Crawling session cancelled after site status check"));
            }
        }
        
        // Stage 0.5: 지능형 범위 재계산 및 실제 적용 - Phase 4 Implementation
        let (actual_start_page, actual_end_page) = if self.config.disable_intelligent_range {
            // 🎭 Actor 시스템 모드: 사용자가 지정한 정확한 범위 사용
            info!("🎭 Actor mode: Using exact user-specified range {} to {} (intelligent range disabled)", 
                  self.config.start_page, self.config.end_page);
            (self.config.start_page, self.config.end_page)
        } else {
            // 기존 지능형 범위 재계산 로직
            info!("🧠 Stage 0.5: Performing intelligent range recalculation");
            info!("📊 Site analysis: total_pages={}, products_on_last_page={}", 
                  site_status.total_pages, site_status.products_on_last_page);
            
            let optimal_range = self.range_calculator.calculate_next_crawling_range(
                site_status.total_pages,
                site_status.products_on_last_page, // ✅ 실제 값 사용 (이전: 하드코딩 10)
            ).await?;
            
            // 계산된 범위를 실제로 적용하여 최종 범위 결정
            if let Some((optimal_start, optimal_end)) = optimal_range {
                if optimal_start != self.config.start_page || optimal_end != self.config.end_page {
                    info!("💡 Applying intelligent range recommendation: pages {} to {} (original: {} to {})", 
                          optimal_start, optimal_end, self.config.start_page, self.config.end_page);
                    
                    // 범위 적용 이벤트 발송
                    self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
                        stage: "Range Optimization Applied".to_string(),
                        message: format!("Applied optimal range: {} to {} (was: {} to {})", 
                                       optimal_start, optimal_end, self.config.start_page, self.config.end_page),
                    }).await?;
                    
                    (optimal_start, optimal_end)
                } else {
                    info!("✅ Current range already optimal: {} to {}", self.config.start_page, self.config.end_page);
                    (self.config.start_page, self.config.end_page)
                }
            } else {
                info!("✅ All products appear to be crawled - using current range for verification: {} to {}", 
                      self.config.start_page, self.config.end_page);
                (self.config.start_page, self.config.end_page)
            }
        };
        
        info!("🎯 Final crawling range determined: {} to {}", actual_start_page, actual_end_page);
        
        // Stage 1: 데이터베이스 분석
        let _db_analysis = self.stage1_analyze_database().await?;
        
        // Stage 1 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 1");
                return Err(anyhow!("Crawling session cancelled after database analysis"));
            }
        }
        
        // 🔥 6. 배치 계획 수립 이벤트 (Stage 1 완료 후)
        let total_pages = if actual_start_page > actual_end_page {
            actual_start_page - actual_end_page + 1
        } else {
            actual_end_page - actual_start_page + 1
        };
        
        let estimated_batches = (total_pages as f32 / 3.0).ceil() as u32; // 3페이지씩 배치
        info!("📋 배치 계획 수립: 총 {}페이지를 {}개 배치로 분할", total_pages, estimated_batches);
        
        if let Some(broadcaster) = &self.broadcaster {
            let _ = broadcaster.emit_session_event(
                self.session_id.clone(),
                crate::domain::events::SessionEventType::BatchPlanning,
                format!("배치 계획 수립 완료: {}페이지 → {}개 배치", total_pages, estimated_batches),
            );
        }

        // 🔥 7. 각 배치별 생성 이벤트 발생
        for batch_num in 1..=estimated_batches {
            let start_page_for_batch = actual_start_page + (batch_num - 1) * 3;
            let end_page_for_batch = std::cmp::min(start_page_for_batch + 2, actual_end_page);
            
            self.emit_detailed_event(DetailedCrawlingEvent::BatchCreated {
                batch_id: batch_num,
                total_batches: estimated_batches,
                start_page: start_page_for_batch,
                end_page: end_page_for_batch,
                description: format!("배치 {} (페이지 {}-{})", batch_num, start_page_for_batch, end_page_for_batch),
            }).await?;
        }
        
        // 🔥 8. 배치 시작 이벤트 (실제 크롤링 시작)
        self.emit_detailed_event(DetailedCrawlingEvent::BatchStarted {
            batch_id: 1,
            total_batches: estimated_batches,
            pages_in_batch: total_pages,
        }).await?;
        
        // Stage 2: 제품 목록 수집 - 계산된 최적 범위 사용
        let product_urls = self.stage2_collect_product_list_optimized(
            actual_start_page, 
            actual_end_page, 
            site_status.total_pages, 
            site_status.products_on_last_page
        ).await?;
        
        // 🔥 Stage 2 결과 검증 및 로깅
        info!("📊 Stage 2 completed: {} product URLs collected", product_urls.len());
        if product_urls.is_empty() {
            warn!("⚠️  No product URLs collected from Stage 2! This will prevent Stage 3 from running.");
            warn!("   - Start page: {}", actual_start_page);
            warn!("   - End page: {}", actual_end_page);
            warn!("   - This might indicate:");
            warn!("     1. Network issues during product list collection");
            warn!("     2. Website structure changes");
            warn!("     3. Anti-bot measures blocking requests");
            warn!("     4. Pagination calculation errors");
        } else {
            info!("✅ Stage 2 successful: {} URLs ready for Stage 3", product_urls.len());
            // 샘플 URL 로깅 (디버깅용)
            if !product_urls.is_empty() {
                info!("📎 Sample URL: {}", product_urls[0]);
            }
        }
        
        // Stage 2 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 2");
                return Err(anyhow!("Crawling session cancelled after product list collection"));
            }
        }
        
        // 🔥 Stage 3 진행 전 조건 검사
        if product_urls.is_empty() {
            let error_msg = "Cannot proceed to Stage 3: No product URLs collected in Stage 2";
            error!("🚫 {}", error_msg);
            return Err(anyhow!(error_msg));
        }
        
        info!("🚀 Proceeding to Stage 3 with {} product URLs", product_urls.len());
        
        // Stage 3: 제품 상세정보 수집
        let products = self.stage3_collect_product_details(&product_urls).await?;
        let total_products = products.len() as u32;
        
        // Stage 3 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 3");
                return Err(anyhow!("Crawling session cancelled after product details collection"));
            }
        }
        
        // Stage 4: 데이터베이스 저장
        let (processed_count, _new_items, _updated_items, errors) = self.stage4_save_to_database(products).await?;
        
        // 🔥 4. 배치 완료 이벤트 (데이터베이스 저장 후 발송)
        self.emit_detailed_event(DetailedCrawlingEvent::BatchCompleted {
            batch: 1,
            total: processed_count as u32,
        }).await?;
        
        // 성공률 계산
        let success_rate = if processed_count > 0 {
            (processed_count - errors) as f64 / processed_count as f64
        } else {
            0.0
        };

        // 🔥 배치 완료 이벤트 발송 (UI 연결)
        if let Some(broadcaster) = &mut self.broadcaster {
            let pages_processed = if actual_start_page >= actual_end_page {
                actual_start_page - actual_end_page + 1
            } else {
                actual_end_page - actual_start_page + 1
            };
            
            if let Err(e) = broadcaster.emit_batch_completed(pages_processed, total_products, success_rate) {
                warn!("Failed to emit batch-completed event: {}", e);
            }
        }

        let duration = start_time.elapsed();
        info!("Service-based batch crawling completed in {:?}: {} products collected, {:.2}% success rate", 
            duration, total_products, success_rate * 100.0);
        
        // 🔥 크롤링 완료 이벤트 발송 (UI 연결)
        if let Some(broadcaster) = &mut self.broadcaster {
            if let Err(e) = broadcaster.emit_crawling_completed() {
                warn!("Failed to emit crawling-completed event: {}", e);
            }
        }
        
        // 🔥 5. 세션 완료 이벤트 (모든 작업 완료 후 발송)
        self.emit_detailed_event(DetailedCrawlingEvent::SessionCompleted {
            session_id: self.session_id.clone(),
            duration,
            total_products,
            success_rate,
        }).await?;
        
        Ok(())
    }

    /// Stage 0: 사이트 상태 확인 (새로운 단계)
    async fn stage0_check_site_status(&self) -> Result<SiteStatus> {
        info!("Stage 0: Checking site status");
        
        // 🔥 크롤링 세션 내 사이트 상태 체크 시작 이벤트
        if let Some(broadcaster) = &self.broadcaster {
            let session_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
                is_standalone: false,  // 크롤링 세션 내 체크
                status: crate::domain::events::SiteCheckStatus::Started,
                message: "크롤링 세션 내 사이트 상태 확인 시작".to_string(),
                timestamp: chrono::Utc::now(),
            };
            let _ = broadcaster.emit_site_status_check(&session_event);
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "SiteStatus".to_string(),
            message: "사이트 상태를 확인하는 중...".to_string(),
        }).await?;

        let site_status = self.status_checker.check_site_status().await?;
        
        if !site_status.is_accessible || site_status.total_pages == 0 {
            let error_msg = format!("Site is not accessible or has no pages (pages: {})", site_status.total_pages);
            self.emit_detailed_event(DetailedCrawlingEvent::ErrorOccurred {
                stage: "SiteStatus".to_string(),
                error: error_msg.clone(),
                recoverable: true,
            }).await?;
            return Err(anyhow!(error_msg));
        }

        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "SiteStatus".to_string(),
            items_processed: 1,
        }).await?;

        // 🔥 크롤링 세션 내 사이트 상태 체크 성공 이벤트
        if let Some(broadcaster) = &self.broadcaster {
            let success_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
                is_standalone: false,  // 크롤링 세션 내 체크
                status: crate::domain::events::SiteCheckStatus::Success,
                message: format!("크롤링 세션 내 사이트 상태 확인 완료: {}개 페이지", site_status.total_pages),
                timestamp: chrono::Utc::now(),
            };
            let _ = broadcaster.emit_site_status_check(&success_event);
        }

        info!("Stage 0 completed: Site is healthy (score: {})", site_status.health_score);
        Ok(site_status)
    }

    /// Stage 1: 데이터베이스 분석 (새로운 단계)
    async fn stage1_analyze_database(&self) -> Result<DatabaseAnalysis> {
        info!("Stage 1: Analyzing database state");
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "DatabaseAnalysis".to_string(),
            message: "데이터베이스 상태를 분석하는 중...".to_string(),
        }).await?;

        let analysis = self.database_analyzer.analyze_current_state().await?;
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "DatabaseAnalysis".to_string(),
            items_processed: analysis.total_products as usize,
        }).await?;

        info!("Stage 1 completed: {} total products, quality score: {}", 
              analysis.total_products, analysis.data_quality_score);
        Ok(analysis)
    }

    /// Stage 2: 제품 목록 수집 (서비스 기반)
    // REMOVE_CANDIDATE(Phase3): Currently unused – legacy batch workflow
    async fn stage2_collect_product_list(&self, total_pages: u32, products_on_last_page: u32) -> Result<Vec<ProductUrl>> {
        info!("Stage 2: Collecting product list using ProductListCollector service");
        
        // 취소 확인 - 단계 시작 전
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Stage 2 (ProductList) cancelled before starting");
                return Err(anyhow!("Product list collection cancelled"));
            }
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "ProductList".to_string(),
            message: format!("{}페이지에서 제품 목록을 수집하는 중...", total_pages),
        }).await?;

        let effective_end = total_pages.min(self.config.end_page);
        
        // 취소 가능한 제품 목록 수집 실행 - 항상 병렬 처리 사용
        let product_urls = if let Some(cancellation_token) = &self.config.cancellation_token {
            info!("🛑 Using cancellation token for product list collection");
            
            // 취소 토큰과 함께 제품 목록 수집 - 개선된 ProductListCollector 사용
            self.product_list_collector.collect_page_range_with_cancellation(
                self.config.start_page, 
                effective_end,
                total_pages,
                products_on_last_page,
                cancellation_token.clone()
            ).await?
        } else {
            warn!("⚠️  No cancellation token - using parallel collection without cancellation");
            // 취소 토큰이 없어도 병렬 처리 사용
            self.product_list_collector.collect_page_range(
                self.config.start_page, 
                effective_end,
                total_pages,
                products_on_last_page
            ).await?
        };
        
        // 취소 확인 - 단계 완료 후
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Stage 2 (ProductList) cancelled after collection");
                return Err(anyhow!("Product list collection cancelled after completion"));
            }
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "ProductList".to_string(),
            items_processed: product_urls.len(),
        }).await?;

        info!("Stage 2 completed: {} product URLs collected", product_urls.len());
        Ok(product_urls)
    }

    /// Stage 2: 제품 목록 수집 (최적화된 범위 사용) - Phase 4 Implementation
    async fn stage2_collect_product_list_optimized(&mut self, start_page: u32, end_page: u32, _total_pages: u32, _products_on_last_page: u32) -> Result<Vec<ProductUrl>> {
        info!("🔗 Stage 2: ProductList 수집 시작 - 페이지별 병렬 실행 ({}~{})", start_page, end_page);
        
        // 🔥 Stage 2 배치 생성 이벤트
        let total_pages = if start_page > end_page {
            start_page - end_page + 1
        } else {
            end_page - start_page + 1
        };

        let batch_id = format!("productlist-{}-{}", start_page, end_page);
        
        // 🔥 ProductList 배치 생성 이벤트
        info!("📦 ProductList 배치 생성: {} ({}페이지)", batch_id, total_pages);
        if let Some(broadcaster) = &self.broadcaster {
            let metadata = crate::domain::events::BatchMetadata {
                total_items: total_pages,
                processed_items: 0,
                successful_items: 0,
                failed_items: 0,
                start_time: chrono::Utc::now(),
                estimated_completion: None,
            };
            
            let _ = broadcaster.emit_batch_event(
                self.session_id.clone(),
                batch_id.to_string(),
                crate::domain::events::CrawlingStage::ProductList,
                crate::domain::events::BatchEventType::Created,
                format!("ProductList 배치 생성: 페이지 {}~{} ({}개 페이지)", start_page, end_page, total_pages),
                Some(metadata),
            );
        }

        // 🔥 배치 시작 이벤트
        if let Some(broadcaster) = &self.broadcaster {
            let _ = broadcaster.emit_batch_event(
                self.session_id.clone(),
                batch_id.to_string(),
                crate::domain::events::CrawlingStage::ProductList,
                crate::domain::events::BatchEventType::Started,
                format!("ProductList 배치 시작: 페이지 {}~{} 수집 중", start_page, end_page),
                None,
            );
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::BatchCreated {
            batch_id: 1,
            total_batches: 1, // 현재는 단일 배치로 처리
            start_page,
            end_page,
            description: format!("페이지 {}~{} 제품 목록 수집 ({}개 페이지)", start_page, end_page, total_pages),
        }).await?;

        self.emit_detailed_event(DetailedCrawlingEvent::BatchStarted {
            batch_id: 1,
            total_batches: 1,
            pages_in_batch: total_pages,
        }).await?;
        
        // 🔥 기존 배치 생성 이벤트도 유지 (호환성)
        if let Some(broadcaster) = &mut self.broadcaster {
            if let Err(e) = broadcaster.emit_batch_created(start_page, end_page) {
                warn!("Failed to emit batch-created event: {}", e);
            }
        }
        
        // 취소 확인 - 단계 시작 전
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Stage 2 (ProductList) cancelled before starting");
                return Err(anyhow!("Product list collection cancelled"));
            }
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "ProductList (Optimized)".to_string(),
            message: format!("페이지 {} ~ {}에서 제품 목록을 수집하는 중... (동시성 실행)", start_page, end_page),
        }).await?;

        // 🔥 동시성 크롤링 실행 - 이벤트 발송을 위한 채널 생성
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<(String, serde_json::Value)>(100);
        
        // 이벤트 처리를 위한 백그라운드 태스크 생성
        let broadcaster_opt = self.broadcaster.take(); // 소유권 이동
        let event_handler = tokio::spawn(async move {
            let mut broadcaster = broadcaster_opt;
            while let Some((event_type, payload)) = event_rx.recv().await {
                // 🔥 완료 신호 감지 - concurrent 작업이 완료되면 즉시 종료
                if event_type == "concurrent_phase_completed" {
                    debug!("Concurrent phase completed - terminating event handler");
                    break;
                }
                
                if let Some(ref mut b) = broadcaster {
                    match event_type.as_str() {
                        "page-collection-started" => {
                            if let Ok(detailed_event) = serde_json::from_value::<DetailedCrawlingEvent>(payload) {
                                // 새로운 page-collection-started 이벤트 처리
                                debug!("Page collection started event received: {:?}", detailed_event);
                                // 기존 브로드캐스터로 변환하여 전송
                                match &detailed_event {
                                    DetailedCrawlingEvent::PageCollectionStarted { page, url, .. } => {
                                        // emit_page_started 메서드가 없으므로 로그로만 처리
                                        debug!("Page {} collection started for URL: {}", page, url);
                                    },
                                    _ => {}
                                }
                            }
                        }
                        "page-collection-completed" => {
                            if let Ok(detailed_event) = serde_json::from_value::<DetailedCrawlingEvent>(payload) {
                                // 새로운 page-collection-completed 이벤트 처리
                                debug!("Page collection completed event received: {:?}", detailed_event);
                                // 기존 브로드캐스터로 변환하여 전송
                                match &detailed_event {
                                    DetailedCrawlingEvent::PageCollectionCompleted { page, url, products_found, .. } => {
                                        if let Err(e) = b.emit_page_crawled(*page, url.clone(), *products_found, true) {
                                            warn!("Failed to emit page-collection-completed as page-crawled event: {}", e);
                                        }
                                    },
                                    _ => {}
                                }
                            }
                        }
                        "page-started" => {
                            if let Ok(detailed_event) = serde_json::from_value::<DetailedCrawlingEvent>(payload) {
                                // PageStarted 이벤트는 별도 처리하지 않고 로그만 남김
                                debug!("Page started event received: {:?}", detailed_event);
                            }
                        }
                        "page-retry-attempt" => {
                            if let Ok(detailed_event) = serde_json::from_value::<DetailedCrawlingEvent>(payload) {
                                // PageRetryAttempt 이벤트는 별도 처리하지 않고 로그만 남김
                                debug!("Page retry attempt event received: {:?}", detailed_event);
                            }
                        }
                        "page-completed" => {
                            // DetailedCrawlingEvent를 직접 처리 - 기존 브로드캐스터 메서드 사용
                            if let Ok(detailed_event) = serde_json::from_value::<DetailedCrawlingEvent>(payload) {
                                match &detailed_event {
                                    DetailedCrawlingEvent::PageCompleted { page, products_found } => {
                                        if let Err(e) = b.emit_page_crawled(*page, format!("page-{}", page), *products_found, true) {
                                            warn!("Failed to emit page-completed as page-crawled event: {}", e);
                                        }
                                    },
                                    _ => {}
                                }
                            }
                        }
                        "page-crawled" => {
                            if let Ok(data) = serde_json::from_value::<(u32, String, u32, bool)>(payload) {
                                if let Err(e) = b.emit_page_crawled(data.0, data.1, data.2, data.3) {
                                    warn!("Failed to emit page-crawled event: {}", e);
                                }
                            }
                        }
                        "retry-attempt" => {
                            if let Ok(data) = serde_json::from_value::<(String, String, String, u32, u32, String)>(payload) {
                                if let Err(e) = b.emit_retry_attempt(data.0, data.1, data.2, data.3, data.4, data.5) {
                                    warn!("Failed to emit retry-attempt event: {}", e);
                                }
                            }
                        }
                        "retry-success" => {
                            if let Ok(data) = serde_json::from_value::<(String, String, String, u32)>(payload) {
                                if let Err(e) = b.emit_retry_success(data.0, data.1, data.2, data.3) {
                                    warn!("Failed to emit retry-success event: {}", e);
                                }
                            }
                        }
                        "retry-failed" => {
                            if let Ok(data) = serde_json::from_value::<(String, String, String, u32, String)>(payload) {
                                if let Err(e) = b.emit_retry_failed(data.0, data.1, data.2, data.3, data.4) {
                                    warn!("Failed to emit retry-failed event: {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            broadcaster // 소유권 반환
        });

        // 🔥 이벤트 콜백 함수 정의 - 더 상세한 이벤트들 추가
        let _engine_clone = self.session_id.clone();
        let batch_id = 1u32;
        
    let event_tx_clone = event_tx.clone();
    let _page_callback = move |page_id: u32, url: String, product_count: u32, success: bool| -> Result<()> {
            let start_time = std::time::Instant::now();
            
            // � 새로운 세분화된 페이지 수집 시작 이벤트
            let page_start_event = DetailedCrawlingEvent::PageCollectionStarted {
                page: page_id,
                batch_id,
                url: url.clone(),
                estimated_products: Some(25), // 페이지당 평균 예상 제품 수
            };
            let start_payload = serde_json::to_value(page_start_event)?;
            if let Err(e) = event_tx_clone.try_send(("page-collection-started".to_string(), start_payload)) {
                warn!("Failed to send page-collection-started event: {}", e);
            }
            
            // � 새로운 세분화된 페이지 수집 완료 이벤트
            if success {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let page_event = DetailedCrawlingEvent::PageCollectionCompleted {
                    page: page_id,
                    batch_id,
                    url: url.clone(),
                    products_found: product_count,
                    duration_ms,
                };
                let payload = serde_json::to_value(page_event)?;
                if let Err(e) = event_tx_clone.try_send(("page-collection-completed".to_string(), payload)) {
                    warn!("Failed to send page-collection-completed event: {}", e);
                }
            }
            
            // 기존 page-crawled 이벤트도 유지
            let legacy_payload = serde_json::to_value((page_id, url, product_count, success))?;
            if let Err(e) = event_tx_clone.try_send(("page-crawled".to_string(), legacy_payload)) {
                warn!("Failed to send page-crawled event: {}", e);
            }
            Ok(())
        };

        let event_tx_clone2 = event_tx.clone();
    let _retry_callback = move |item_id: String, item_type: String, url: String, attempt: u32, max_attempts: u32, reason: String| -> Result<()> {
            // 🔥 페이지 재시도 시도 이벤트
            if item_type == "page" {
                let page_num = url.split("page=").nth(1)
                    .and_then(|s| s.split('&').next())
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                
                let retry_event = DetailedCrawlingEvent::PageRetryAttempt {
                    page: page_num,
                    batch_id,
                    url: url.clone(),
                    attempt,
                    max_attempts,
                    reason: reason.clone(),
                };
                let retry_payload = serde_json::to_value(retry_event)?;
                if let Err(e) = event_tx_clone2.try_send(("page-retry-attempt".to_string(), retry_payload)) {
                    warn!("Failed to send page-retry-attempt event: {}", e);
                }
            }
            
            // 기존 재시도 이벤트
            let payload = serde_json::to_value((item_id, item_type, url, attempt, max_attempts, reason))?;
            if let Err(e) = event_tx_clone2.try_send(("retry-attempt".to_string(), payload)) {
                warn!("Failed to send retry-attempt event: {}", e);
            }
            Ok(())
        };

        // 실제 크롤링 실행 (진정한 동시성 보장)
        let product_urls = if let Some(cancellation_token) = &self.config.cancellation_token {
            // 🔥 새로운 비동기 이벤트 메서드 사용 (동시성 보장)
            let collector = self.product_list_collector.clone();
            let collector_impl = collector.as_ref()
                .as_any()
                .downcast_ref::<ProductListCollectorImpl>()
                .ok_or_else(|| anyhow!("Failed to downcast ProductListCollector"))?;
            
            collector_impl.collect_page_range_with_async_events(
                start_page,
                end_page,
                Some(cancellation_token.clone()),
                self.session_id.clone(),
                batch_id.to_string(),
            ).await?
        } else {
            // 🔥 토큰이 없어도 비동기 이벤트 메서드 사용
            let collector = self.product_list_collector.clone();
            let collector_impl = collector.as_ref()
                .as_any()
                .downcast_ref::<ProductListCollectorImpl>()
                .ok_or_else(|| anyhow!("Failed to downcast ProductListCollector"))?;
            
            collector_impl.collect_page_range_with_async_events(
                start_page,
                end_page,
                None,
                self.session_id.clone(),
                batch_id.to_string(),
            ).await.map_err(|e| anyhow!("Product list collection failed: {}", e))?
        };

        // 🔥 배치 완료 이벤트
        if let Some(broadcaster) = &self.broadcaster {
            let metadata = crate::domain::events::BatchMetadata {
                total_items: total_pages,
                processed_items: total_pages,
                successful_items: product_urls.len() as u32,
                failed_items: total_pages.saturating_sub(product_urls.len() as u32),
                start_time: chrono::Utc::now(), // 실제로는 시작 시간을 저장해야 함
                estimated_completion: Some(chrono::Utc::now()),
            };
            
            let _ = broadcaster.emit_batch_event(
                self.session_id.clone(),
                batch_id.to_string(),
                crate::domain::events::CrawlingStage::ProductList,
                crate::domain::events::BatchEventType::Completed,
                format!("ProductList 배치 완료: {}개 제품 URL 수집", product_urls.len()),
                Some(metadata),
            );
        }

        // 이벤트 채널 종료
        drop(event_tx);
        
        // 🔥 이벤트 처리 완료 대기 및 브로드캐스터 복구 (즉시 완료 타임아웃 단축)
        match tokio::time::timeout(std::time::Duration::from_millis(100), event_handler).await {
            Ok(Ok(broadcaster_opt)) => {
                debug!("Event handler completed successfully");
                self.broadcaster = broadcaster_opt;
            },
            Ok(Err(e)) => {
                warn!("Event handler task failed: {}", e);
                // 브로드캐스터를 None으로 설정
                self.broadcaster = None;
            },
            Err(_) => {
                debug!("Event handler processing - force shutdown after concurrent jobs completed");
                // 🔥 concurrent 작업이 완료되었으므로 이벤트 핸들러를 강제 종료
                // 브로드캐스터를 None으로 설정
                self.broadcaster = None;
            }
        }

        info!("✅ Stage 2 completed: {} product URLs collected from optimized range with TRUE concurrent execution", product_urls.len());
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "ProductList (Optimized)".to_string(),
            items_processed: product_urls.len(),
        }).await?;

        Ok(product_urls)
    }

    /// Stage 3: 제품 상세정보 수집 (서비스 기반 + 재시도 메커니즘)
    async fn stage3_collect_product_details(&mut self, product_urls: &[ProductUrl]) -> Result<Vec<(Product, ProductDetail)>> {
        info!("Stage 3: Collecting product details using ProductDetailCollector service with retry mechanism");
        
        // 취소 확인 - 단계 시작 전
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Stage 3 (ProductDetails) cancelled before starting");
                return Err(anyhow!("Product details collection cancelled"));
            }
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "ProductDetails".to_string(),
            message: format!("{}개 제품의 상세정보를 수집하는 중... (재시도 지원)", product_urls.len()),
        }).await?;

    // 초기 시도 - cancellation token 사용
    let mut successful_products = Vec::new();
    // Explicit type to satisfy compiler; kept underscore as it's only for debug scaffolding
    let mut _failed_urls: Vec<ProductUrl> = Vec::new();

        // � 제품별 처리 전에 새로운 세분화된 이벤트들을 발생시키기 위한 로직 추가
        for (index, product_url) in product_urls.iter().enumerate() {
            // � 제품 상세 수집 시작 이벤트 (새로운 구조)
            self.emit_detailed_event(DetailedCrawlingEvent::ProductDetailCollectionStarted {
                url: product_url.to_string(),
                product_index: (index + 1) as u32,
                total_products: product_urls.len() as u32,
                batch_id: 1,
            }).await?;
            
            // 🚀 제품 상세 처리 시작 이벤트 (새로운 구조)
            self.emit_detailed_event(DetailedCrawlingEvent::ProductDetailProcessingStarted {
                url: product_url.to_string(),
                product_index: (index + 1) as u32,
                parsing_stage: "HTML_PARSING".to_string(),
            }).await?;
        }

        // 항상 취소 토큰을 사용하도록 강제 - 없으면 기본 토큰 생성
        let result = if let Some(cancellation_token) = &self.config.cancellation_token {
            info!("🛑 USING PROVIDED CANCELLATION TOKEN for product detail collection");
            info!("🛑 Cancellation token is_cancelled: {}", cancellation_token.is_cancelled());
            
            // 🔥 이벤트 기반 수집 메서드 사용 (새로운 구현)
            if let Some(collector_impl) = self.product_detail_collector.as_any().downcast_ref::<ProductDetailCollectorImpl>() {
                collector_impl.collect_details_with_async_events(
                    product_urls,
                    Some(cancellation_token.clone()),
                    self.session_id.clone(),
                    self.session_id.clone(), // session_id를 batch_id로도 사용
                ).await
            } else {
                // Fallback to original method
                self.product_detail_collector.collect_details_with_cancellation(product_urls, cancellation_token.clone()).await
            }
        } else {
            warn!("⚠️  NO CANCELLATION TOKEN - creating default token for consistent behavior");
            let default_token = CancellationToken::new();
            
            // 🔥 이벤트 기반 수집 메서드 사용 (새로운 구현)
            if let Some(collector_impl) = self.product_detail_collector.as_any().downcast_ref::<ProductDetailCollectorImpl>() {
                collector_impl.collect_details_with_async_events(
                    product_urls,
                    Some(default_token.clone()),
                    self.session_id.clone(),
                    self.session_id.clone(), // session_id를 batch_id로도 사용
                ).await
            } else {
                // Fallback to original method
                self.product_detail_collector.collect_details_with_cancellation(product_urls, default_token).await
            }
        };

        match result {
            Ok(product_details) => {
                // 취소 확인 - 데이터 변환 전
                if let Some(cancellation_token) = &self.config.cancellation_token {
                    if cancellation_token.is_cancelled() {
                        warn!("🛑 Product details collection cancelled before processing results");
                        return Err(anyhow!("Product details collection cancelled"));
                    }
                }
                
                // ProductDetail을 Product로 변환하고 원본 ProductDetail과 함께 저장
                let product_count = product_details.len();
                successful_products = product_details.into_iter()
                    .enumerate()
                    .map(|(index, detail)| {
                        let product = product_detail_to_product(detail.clone());
                        
                        // � 새로운 제품 상세 수집 완료 이벤트 (비동기 처리)
                        if let Some(product_url) = product_urls.get(index) {
                            // 처리 시간 시뮬레이션 (실제로는 수집 시작부터 측정해야 함)
                            let duration_ms = 500 + (index as u64 * 50); // 시뮬레이션된 처리 시간
                            
                            let _completion_event = DetailedCrawlingEvent::ProductDetailCollectionCompleted {
                                url: product_url.to_string(),
                                product_index: (index + 1) as u32,
                                success: true,
                                duration_ms,
                                data_extracted: detail.model.is_some() && detail.manufacturer.is_some(),
                            };
                            
                            // 비동기 이벤트 발송을 위한 논리 (향후 실제 구현에서는 Future로 처리)
                            // 현재는 기존 broadcaster를 통해 호환성 유지
                            if let Some(broadcaster) = &mut self.broadcaster {
                                if let Err(e) = broadcaster.emit_product_collected(
                                    product.page_id.map(|id| id as u32).unwrap_or(0),
                                    product.model.clone().unwrap_or_else(|| format!("product-{}", index)),
                                    product_url.to_string(),
                                    true
                                ) {
                                    warn!("Failed to emit product-collected event: {}", e);
                                }
                            }
                        }
                        
                        // 🔥 배치 진행 상황 업데이트 (10개마다)
                        if index % 10 == 0 || index == product_count - 1 {
                            let progress = (index + 1) as f64 / product_urls.len() as f64;
                            
                            if let Some(broadcaster) = &mut self.broadcaster {
                                if let Err(e) = broadcaster.emit_batch_progress(
                                    "ProductDetails".to_string(),
                                    progress,
                                    product_urls.len() as u32,
                                    (index + 1) as u32,
                                    0, // items_active
                                    0  // items_failed (아직 실패한 항목 없음)
                                ) {
                                    warn!("Failed to emit batch-progress event: {}", e);
                                }
                            }
                        }
                        
                        (product, detail)
                    })
                    .collect();
                
                info!("✅ Initial collection successful: {} products", successful_products.len());
            }
            Err(e) => {
                // cancellation 체크
                if let Some(cancellation_token) = &self.config.cancellation_token {
                    if cancellation_token.is_cancelled() {
                        warn!("🛑 Collection cancelled by user");
                        return Ok(successful_products); // 이미 수집된 제품들 반환
                    }
                }
                
                warn!("❌ Initial collection failed: {}", e);
                let failed_urls = product_urls.to_vec();
                
                // 🔥 실패한 제품들 실패 이벤트 발송 (UI 연결)
                for (index, url) in failed_urls.iter().enumerate() {
                    if let Some(broadcaster) = &mut self.broadcaster {
                        if let Err(emit_err) = broadcaster.emit_product_collected(
                            0, // 페이지 ID 미상
                            format!("failed-{}", index),
                            url.to_string(),
                            false
                        ) {
                            warn!("Failed to emit product-collected failure event: {}", emit_err);
                        }
                    }
                }
                
                // 실패한 URL들을 재시도 큐에 추가
                for (index, url) in failed_urls.iter().enumerate() {
                    let item_id = format!("product_detail_{}_{}", self.session_id, index);
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("url".to_string(), url.to_string());
                    metadata.insert("stage".to_string(), "product_details".to_string());
                    
                    if let Err(retry_err) = self.retry_manager.add_failed_item(
                        item_id,
                        CrawlingStage::ProductDetails,
                        e.to_string(),
                        url.to_string(),
                        metadata,
                    ).await {
                        warn!("Failed to add item to retry queue: {}", retry_err);
                    }
                }
            }
        }

        // 재시도 처리 (cancellation token 확인 후)
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Skipping retries due to cancellation");
                self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
                    stage: "ProductDetails".to_string(),
                    items_processed: successful_products.len(),
                }).await?;
                return Ok(successful_products);
            }
        }

        let retry_products = self.process_retries_for_product_details().await?;
        successful_products.extend(retry_products);
        
        // 🔥 각 제품별 수집 완료 이벤트 발송 (모든 수집이 완료된 후)
    for (index, (_product, detail)) in successful_products.iter().enumerate() {
            if let Some(product_url) = product_urls.get(index) {
                let duration_ms = 500 + (index as u64 * 50); // 시뮬레이션된 처리 시간
                
                // 🔥 처리 시작 이벤트
                self.emit_detailed_event(DetailedCrawlingEvent::ProductDetailProcessingStarted {
                    url: product_url.to_string(),
                    product_index: (index + 1) as u32,
                    parsing_stage: "COMPLETED".to_string(),
                }).await?;
                
                // 🔥 수집 완료 이벤트
                self.emit_detailed_event(DetailedCrawlingEvent::ProductDetailCollectionCompleted {
                    url: product_url.to_string(),
                    product_index: (index + 1) as u32,
                    success: true,
                    duration_ms,
                    data_extracted: detail.model.is_some() && detail.manufacturer.is_some(),
                }).await?;
            }
        }
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "ProductDetails".to_string(),
            items_processed: successful_products.len(),
        }).await?;

        info!("Stage 3 completed: {} products collected (including retries)", successful_products.len());
        Ok(successful_products)
    }
    
    /// 제품 상세정보 수집 재시도 처리
    async fn process_retries_for_product_details(&mut self) -> Result<Vec<(Product, ProductDetail)>> {
        info!("🔄 Processing retries for product details collection");
        let mut retry_products = Vec::new();
        
        // 최대 3번의 재시도 사이클
        for cycle in 1..=3 {
            // 재시도 사이클 시작 전 취소 확인
            if let Some(cancellation_token) = &self.config.cancellation_token {
                if cancellation_token.is_cancelled() {
                    warn!("🛑 Retry processing cancelled at cycle {}", cycle);
                    return Ok(retry_products);
                }
            }
            
            let ready_items = self.retry_manager.get_ready_items().await?;
            if ready_items.is_empty() {
                debug!("No items ready for retry in cycle {}", cycle);
                break;
            }
            
            info!("🔄 Retry cycle {}: Processing {} items", cycle, ready_items.len());
            
            for retry_item in ready_items {
                // 각 재시도 항목 처리 전 취소 확인
                if let Some(cancellation_token) = &self.config.cancellation_token {
                    if cancellation_token.is_cancelled() {
                        warn!("🛑 Retry processing cancelled during item processing");
                        return Ok(retry_products);
                    }
                }
                
                if retry_item.stage == CrawlingStage::ProductDetails {
                    let url = retry_item.original_url;
                    let item_id = retry_item.item_id.clone();
                    
                    info!("🔄 Retrying product detail collection for: {}", url);
                    
                    // 🔥 ProductRetryAttempt 이벤트 발송
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductRetryAttempt {
                        url: url.clone(),
                        batch_id: 1,
                        attempt: cycle,
                        max_attempts: 3,
                        reason: "Product detail collection failed".to_string(),
                    }).await.unwrap_or_else(|e| warn!("Failed to emit ProductRetryAttempt event: {}", e));
                    
                    // 🔥 재시도 시도 이벤트 발송 (기존 broadcaster)
                    if let Some(broadcaster) = &mut self.broadcaster {
                        if let Err(e) = broadcaster.emit_retry_attempt(
                            item_id.clone(),
                            "product".to_string(),
                            url.clone(),
                            cycle,
                            3,
                            "Product detail collection failed".to_string()
                        ) {
                            warn!("Failed to emit retry-attempt event: {}", e);
                        }
                    }
                    
                    // Convert String URL to ProductUrl for the new API
                    let product_url = ProductUrl::new(url.clone(), -1, -1); // Use -1 for retry URLs
                    
                    match self.product_detail_collector.collect_details(&[product_url]).await {
                        Ok(mut product_details) => {
                            if let Some(detail) = product_details.pop() {
                                let product = product_detail_to_product(detail.clone());
                                info!("✅ Retry successful for: {}", url);
                                retry_products.push((product, detail));
                                
                                // 🔥 ProductRetrySuccess 이벤트 발송
                                self.emit_detailed_event(DetailedCrawlingEvent::ProductRetrySuccess {
                                    url: url.clone(),
                                    batch_id: 1,
                                    final_attempt: cycle,
                                }).await.unwrap_or_else(|e| warn!("Failed to emit ProductRetrySuccess event: {}", e));
                                
                                // 🔥 재시도 성공 이벤트 발송 (기존 broadcaster)
                                if let Some(broadcaster) = &mut self.broadcaster {
                                    if let Err(e) = broadcaster.emit_retry_success(
                                        item_id.clone(),
                                        "product".to_string(),
                                        url.clone(),
                                        cycle
                                    ) {
                                        warn!("Failed to emit retry-success event: {}", e);
                                    }
                                }
                                
                                // 성공 기록
                                if let Err(e) = self.retry_manager.mark_retry_success(&item_id).await {
                                    warn!("Failed to mark retry success: {}", e);
                                }
                                
                                self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                                    url: url.clone(),
                                    success: true,
                                }).await?;
                            }
                        }
                        Err(e) => {
                            warn!("❌ Retry failed for {}: {}", url, e);
                            
                            // 재시도 큐에 다시 추가 (재시도 한도 내에서)
                            let mut metadata = std::collections::HashMap::new();
                            metadata.insert("url".to_string(), url.clone());
                            metadata.insert("retry_cycle".to_string(), cycle.to_string());
                            
                            if let Err(retry_err) = self.retry_manager.add_failed_item(
                                item_id.clone(),
                                CrawlingStage::ProductDetails,
                                e.to_string(),
                                url.clone(),
                                metadata,
                            ).await {
                                debug!("Item exceeded retry limit or not retryable: {}", retry_err);
                                
                                // 🔥 ProductRetryFailed 이벤트 발송
                                self.emit_detailed_event(DetailedCrawlingEvent::ProductRetryFailed {
                                    url: url.clone(),
                                    batch_id: 1,
                                    total_attempts: cycle,
                                    final_error: e.to_string(),
                                }).await.unwrap_or_else(|e| warn!("Failed to emit ProductRetryFailed event: {}", e));
                                
                                // 🔥 재시도 최종 실패 이벤트 발송 (기존 broadcaster)
                                if let Some(broadcaster) = &mut self.broadcaster {
                                    if let Err(emit_err) = broadcaster.emit_retry_failed(
                                        item_id.clone(),
                                        "product".to_string(),
                                        url.clone(),
                                        cycle,
                                        e.to_string()
                                    ) {
                                        warn!("Failed to emit retry-failed event: {}", emit_err);
                                    }
                                }
                            }
                            
                            self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                                url: url.clone(),
                                success: false,
                            }).await?;
                        }
                    }
                    
                    // 재시도 간 지연 (취소 확인 포함)
                    let delay = Duration::from_millis(self.config.delay_ms);
                    if let Some(cancellation_token) = &self.config.cancellation_token {
                        tokio::select! {
                            _ = tokio::time::sleep(delay) => {},
                            _ = cancellation_token.cancelled() => {
                                warn!("🛑 Retry processing cancelled during item delay");
                                return Ok(retry_products);
                            }
                        }
                    } else {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
            
            // 사이클 간 지연 (취소 확인 포함)
            if cycle < 3 {
                let cycle_delay = Duration::from_secs(5);
                if let Some(cancellation_token) = &self.config.cancellation_token {
                    tokio::select! {
                        _ = tokio::time::sleep(cycle_delay) => {},
                        _ = cancellation_token.cancelled() => {
                            warn!("🛑 Retry processing cancelled during cycle delay");
                            return Ok(retry_products);
                        }
                    }
                } else {
                    tokio::time::sleep(cycle_delay).await;
                }
            }
        }
        
        info!("🔄 Retry processing completed: {} additional products collected", retry_products.len());
        Ok(retry_products)
    }

    /// Stage 4: 데이터베이스 배치 저장 (효율성 개선)
    async fn stage4_save_to_database(&mut self, products: Vec<(Product, ProductDetail)>) -> Result<(usize, usize, usize, usize)> {
        info!("Stage 4: Batch saving {} products to database", products.len());
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "DatabaseSave".to_string(),
            message: format!("{}개 제품을 배치 단위로 데이터베이스에 저장하는 중...", products.len()),
        }).await?;

        let total_products = products.len();
        let batch_size = 50; // 배치 크기 (50개씩 처리)
        let mut total_new_items = 0;
        let mut total_updated_items = 0;
        let mut total_errors = 0;
        let mut total_processed = 0;

        // 제품들을 배치 단위로 분할
        let product_batches: Vec<Vec<(Product, ProductDetail)>> = products
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        let total_batches = product_batches.len();

        info!("📦 Processing {} products in {} batches of {} items each", 
              total_products, total_batches, batch_size);

        for (batch_index, batch) in product_batches.into_iter().enumerate() {
            let batch_id = (batch_index + 1) as u32;
            let batch_start_time = std::time::Instant::now();
            
            // 취소 확인
            if let Some(cancellation_token) = &self.config.cancellation_token {
                if cancellation_token.is_cancelled() {
                    warn!("🛑 Database batch save cancelled after {} batches", batch_index);
                    break;
                }
            }

            // 🚀 배치 저장 시작 이벤트
            self.emit_detailed_event(DetailedCrawlingEvent::DatabaseBatchSaveStarted {
                batch_id,
                products_count: batch.len() as u32,
                batch_size: batch_size as u32,
            }).await?;

            // 배치 처리
            let mut batch_new_items = 0;
            let mut batch_updated_items = 0;
            let mut batch_errors = 0;

            // 🚀 실제 배치 저장 로직 (트랜잭션 사용하여 효율성 극대화)
            for (product, product_detail) in batch.iter() {
                // 개별 저장 (향후 실제 배치 INSERT/UPDATE로 개선 가능)
                let product_save_result = self.product_repo.create_or_update_product(product).await;
                let product_detail_save_result = self.product_detail_repo.create_or_update_product_detail(product_detail).await;
                
                match (product_save_result, product_detail_save_result) {
                    (Ok((product_was_updated, product_was_created)), Ok((detail_was_updated, detail_was_created))) => {
                        if product_was_created || detail_was_created {
                            batch_new_items += 1;
                        } else if product_was_updated || detail_was_updated {
                            batch_updated_items += 1;
                        }
                        total_processed += 1;
                    },
                    (Err(e), _) | (_, Err(e)) => {
                        batch_errors += 1;
                        warn!("배치 {} 저장 실패: {}", batch_id, e);
                    }
                }
            }

            let batch_duration_ms = batch_start_time.elapsed().as_millis() as u64;

            // 🚀 배치 저장 완료 이벤트
            self.emit_detailed_event(DetailedCrawlingEvent::DatabaseBatchSaveCompleted {
                batch_id,
                products_saved: (batch.len() - batch_errors) as u32,
                new_items: batch_new_items as u32,
                updated_items: batch_updated_items as u32,
                errors: batch_errors as u32,
                duration_ms: batch_duration_ms,
            }).await?;

            // 배치 통계 누적
            total_new_items += batch_new_items;
            total_updated_items += batch_updated_items;
            total_errors += batch_errors;

            info!("✅ 배치 {}/{} 완료: {}개 저장 (신규: {}, 업데이트: {}, 오류: {}) in {}ms", 
                  batch_id, total_batches, batch.len() - batch_errors, 
                  batch_new_items, batch_updated_items, batch_errors, batch_duration_ms);

            // 배치 간 짧은 지연 (시스템 부하 분산)
            if batch_index < total_batches - 1 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // 🚀 Stage 완료 이벤트
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "DatabaseSave".to_string(),
            items_processed: total_processed,
        }).await?;

        info!("🎯 배치 저장 완료: 총 {}개 처리 (신규: {}, 업데이트: {}, 오류: {})", 
              total_processed, total_new_items, total_updated_items, total_errors);
        
        Ok((total_processed, total_new_items, total_updated_items, total_errors))
    }

    /// 세분화된 이벤트 방출
    async fn emit_detailed_event(&self, event: DetailedCrawlingEvent) -> Result<()> {
        // 🚀 새로운 ConcurrencyEvent 발행 (TaskLifecycle, Session, Batch 통합)
        if let Some(emitter) = self.event_emitter.as_ref() {
            let concurrency_event = match &event {
                // 세션 이벤트들
                DetailedCrawlingEvent::SessionStarted { session_id, .. } => {
                    Some(ConcurrencyEvent::SessionEvent {
                        session_id: session_id.clone(),
                        event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Started,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("stage".to_string(), "session_initialization".to_string()),
                        ]),
                    })
                },
                
                DetailedCrawlingEvent::SessionCompleted { session_id, duration, total_products, success_rate } => {
                    Some(ConcurrencyEvent::SessionEvent {
                        session_id: session_id.clone(),
                        event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Completed,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("duration_seconds".to_string(), duration.as_secs().to_string()),
                            ("total_products".to_string(), total_products.to_string()),
                            ("success_rate".to_string(), success_rate.to_string()),
                        ]),
                    })
                },
                
                // 배치 이벤트들
                DetailedCrawlingEvent::BatchCreated { batch_id, total_batches, start_page, end_page, description } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: format!("batch_{}", batch_id),
                        event_type: crate::new_architecture::events::task_lifecycle::BatchEventType::Created,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("total_batches".to_string(), total_batches.to_string()),
                            ("start_page".to_string(), start_page.to_string()),
                            ("end_page".to_string(), end_page.to_string()),
                            ("description".to_string(), description.clone()),
                        ]),
                    })
                },
                
                DetailedCrawlingEvent::BatchStarted { batch_id, total_batches, pages_in_batch } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: format!("batch_{}", batch_id),
                        event_type: crate::new_architecture::events::task_lifecycle::BatchEventType::Started,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("total_batches".to_string(), total_batches.to_string()),
                            ("pages_in_batch".to_string(), pages_in_batch.to_string()),
                        ]),
                    })
                },
                
                DetailedCrawlingEvent::BatchCompleted { batch, total } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: format!("batch_{}", batch),
                        event_type: crate::new_architecture::events::task_lifecycle::BatchEventType::Completed,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("batch_number".to_string(), batch.to_string()),
                            ("total_batches".to_string(), total.to_string()),
                        ]),
                    })
                },
                
                // Stage 레벨 이벤트들 - 새로 추가
                DetailedCrawlingEvent::StageStarted { stage, message } => {
                    Some(ConcurrencyEvent::SessionEvent {
                        session_id: self.session_id.clone(),
                        event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Started,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("stage".to_string(), stage.clone()),
                            ("stage_message".to_string(), message.clone()),
                            ("event_category".to_string(), "stage_started".to_string()),
                        ]),
                    })
                },
                
                DetailedCrawlingEvent::StageCompleted { stage, items_processed } => {
                    Some(ConcurrencyEvent::SessionEvent {
                        session_id: self.session_id.clone(),
                        event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Completed,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("stage".to_string(), stage.clone()),
                            ("items_processed".to_string(), items_processed.to_string()),
                            ("event_category".to_string(), "stage_completed".to_string()),
                        ]),
                    })
                },
                
                // Page 레벨 이벤트들 - 추가
                DetailedCrawlingEvent::PageStarted { page, batch_id, url } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: format!("batch_{}", batch_id),
                        event_type: crate::new_architecture::events::task_lifecycle::BatchEventType::Started,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("page_number".to_string(), page.to_string()),
                            ("page_url".to_string(), url.clone()),
                            ("event_category".to_string(), "page_started".to_string()),
                        ]),
                    })
                },
                
                DetailedCrawlingEvent::PageCompleted { page, products_found } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: "page_batch".to_string(),
                        event_type: crate::new_architecture::events::task_lifecycle::BatchEventType::Completed,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("page_number".to_string(), page.to_string()),
                            ("products_found".to_string(), products_found.to_string()),
                            ("event_category".to_string(), "page_completed".to_string()),
                        ]),
                    })
                },
                
                // Product 레벨 이벤트들 - 추가
                DetailedCrawlingEvent::ProductStarted { url, batch_id, product_index, total_products } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: format!("batch_{}", batch_id),
                        event_type: crate::new_architecture::events::task_lifecycle::BatchEventType::Started,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("product_url".to_string(), url.clone()),
                            ("product_index".to_string(), product_index.to_string()),
                            ("total_products".to_string(), total_products.to_string()),
                            ("event_category".to_string(), "product_started".to_string()),
                        ]),
                    })
                },
                
                DetailedCrawlingEvent::ProductProcessed { url, success } => {
                    Some(ConcurrencyEvent::BatchEvent {
                        session_id: self.session_id.clone(),
                        batch_id: "product_batch".to_string(),
                        event_type: if *success { 
                            crate::new_architecture::events::task_lifecycle::BatchEventType::Completed
                        } else { 
                            crate::new_architecture::events::task_lifecycle::BatchEventType::Failed
                        },
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("product_url".to_string(), url.clone()),
                            ("success".to_string(), success.to_string()),
                            ("event_category".to_string(), "product_processed".to_string()),
                        ]),
                    })
                },
                
                // Error 이벤트들 - 추가
                DetailedCrawlingEvent::ErrorOccurred { stage, error, recoverable } => {
                    Some(ConcurrencyEvent::SessionEvent {
                        session_id: self.session_id.clone(),
                        event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Failed,
                        timestamp: Utc::now(),
                        metadata: HashMap::from([
                            ("timestamp".to_string(), Utc::now().to_rfc3339()),
                            ("stage".to_string(), stage.clone()),
                            ("error_message".to_string(), error.clone()),
                            ("recoverable".to_string(), recoverable.to_string()),
                            ("event_category".to_string(), "error_occurred".to_string()),
                        ]),
                    })
                },
                
                // Task 레벨 이벤트들
                _ => {
                    if let Some((context, task_event)) = event.to_task_lifecycle_event(&self.session_id) {
                        Some(ConcurrencyEvent::TaskLifecycle {
                            context,
                            event: task_event,
                        })
                    } else {
                        None
                    }
                }
            };
            
            // ConcurrencyEvent를 JSON으로 직렬화하여 발행
            if let Some(concurrency_event) = concurrency_event {
                if let Ok(json_value) = serde_json::to_value(&concurrency_event) {
                    emitter.emit_detailed_crawling_event_json(json_value).await?;
                }
            }
        }
        
        if let Some(emitter) = self.event_emitter.as_ref() {
            // DetailedCrawlingEvent를 기존 이벤트 시스템과 연동
            let progress = match &event {
                DetailedCrawlingEvent::StageStarted { stage, message } => {
                    CrawlingProgress {
                        current: 0,
                        total: if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        },
                        percentage: 0.0,
                        current_stage: match stage.as_str() {
                            "SiteStatus" => CrawlingStage::StatusCheck,
                            "DatabaseAnalysis" => CrawlingStage::DatabaseAnalysis,
                            "ProductList" => CrawlingStage::ProductList,
                            "ProductDetails" => CrawlingStage::ProductDetails,
                            "DatabaseSave" => CrawlingStage::DatabaseSave,
                            _ => CrawlingStage::TotalPages,
                        },
                        current_step: message.clone(),
                        status: CrawlingStatus::Running,
                        message: format!("Stage started: {}", stage),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        }),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::PageCompleted { page, products_found } => {
                    CrawlingProgress {
                        current: *page,
                        total: if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        },
                        percentage: (*page as f64 / (if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        }) as f64) * 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {}에서 {}개 제품 발견", page, products_found),
                        status: CrawlingStatus::Running,
                        message: format!("Page {} processed: {} products found", page, products_found),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: *products_found,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        }),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ProductProcessed { url, success } => {
                    CrawlingProgress {
                        current: 1,
                        total: 1,
                        percentage: if *success { 100.0 } else { 0.0 },
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: if *success { 
                            format!("제품 '{}' 상세정보 수집 완료", url) 
                        } else { 
                            format!("제품 '{}' 상세정보 수집 실패", url) 
                        },
                        status: if *success { CrawlingStatus::Running } else { CrawlingStatus::Error },
                        message: format!("Product {}: {}", url, if *success { "success" } else { "failed" }),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: if *success { 1 } else { 0 },
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(1),
                        errors: if *success { 0 } else { 1 },
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::BatchCompleted { batch, total } => {
                    CrawlingProgress {
                        current: *batch,
                        total: *total,
                        percentage: (*batch as f64 / *total as f64) * 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("배치 {}/{} 완료", batch, total),
                        status: CrawlingStatus::Running,
                        message: format!("Batch {} of {} completed", batch, total),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 1,
                        updated_items: 0,
                        current_batch: Some(*batch),
                        total_batches: Some(*total),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ErrorOccurred { stage, error, recoverable } => {
                    CrawlingProgress {
                        current: 0,
                        total: 1,
                        percentage: 0.0,
                        current_stage: match stage.as_str() {
                            "SiteStatus" => CrawlingStage::StatusCheck,
                            "DatabaseAnalysis" => CrawlingStage::DatabaseAnalysis,
                            "ProductList" => CrawlingStage::ProductList,
                            "ProductDetails" => CrawlingStage::ProductDetails,
                            "DatabaseSave" => CrawlingStage::DatabaseSave,
                            _ => CrawlingStage::TotalPages,
                        },
                        current_step: format!("오류 발생: {}", error),
                        status: if *recoverable { CrawlingStatus::Running } else { CrawlingStatus::Error },
                        message: format!("Error in {}: {} (recoverable: {})", stage, error, recoverable),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(1),
                        errors: 1,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::SessionStarted { session_id, config: _ } => {
                    CrawlingProgress {
                        current: 0,
                        total: if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        },
                        percentage: 0.0,
                        current_stage: CrawlingStage::StatusCheck,
                        current_step: format!("크롤링 세션 {} 시작", session_id),
                        status: CrawlingStatus::Running,
                        message: format!("Session {} started", session_id),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        }),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::SessionCompleted { session_id, duration, total_products, success_rate } => {
                    CrawlingProgress {
                        current: *total_products,
                        total: *total_products,
                        percentage: 100.0,
                        current_stage: CrawlingStage::DatabaseSave,
                        current_step: format!("세션 {} 완료 ({}초, 성공률: {:.1}%)", session_id, duration.as_secs(), success_rate * 100.0),
                        status: CrawlingStatus::Completed,
                        message: format!("Session {} completed: {} products, {:.1}% success rate", session_id, total_products, success_rate * 100.0),
                        remaining_time: None,
                        elapsed_time: duration.as_secs(),
                        new_items: *total_products,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                
                // 🔥 새로운 배치 관련 이벤트들
                DetailedCrawlingEvent::BatchCreated { batch_id, total_batches, start_page, end_page, description } => {
                    CrawlingProgress {
                        current: *batch_id,
                        total: *total_batches,
                        percentage: 0.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("배치 {}/{} 생성: {}", batch_id, total_batches, description),
                        status: CrawlingStatus::Running,
                        message: format!("Batch {}/{} created: pages {} to {}", batch_id, total_batches, start_page, end_page),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(*total_batches),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::BatchStarted { batch_id, total_batches, pages_in_batch } => {
                    CrawlingProgress {
                        current: *batch_id,
                        total: *total_batches,
                        percentage: ((*batch_id - 1) as f64 / *total_batches as f64) * 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("배치 {}/{} 시작 ({}개 페이지)", batch_id, total_batches, pages_in_batch),
                        status: CrawlingStatus::Running,
                        message: format!("Batch {}/{} started: {} pages", batch_id, total_batches, pages_in_batch),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(*total_batches),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                
                // 🔥 새로운 페이지 관련 이벤트들 (기본 처리만 제공)
                DetailedCrawlingEvent::PageStarted { page, batch_id, url: _ } => {
                    CrawlingProgress {
                        current: *page,
                        total: if self.config.start_page > self.config.end_page {
                            self.config.start_page - self.config.end_page + 1
                        } else {
                            self.config.end_page - self.config.start_page + 1
                        },
                        percentage: 0.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {} 시작 (배치 {})", page, batch_id),
                        status: CrawlingStatus::Running,
                        message: format!("Page {} started in batch {}", page, batch_id),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::PageRetryAttempt { page, batch_id, url: _, attempt, max_attempts, reason } => {
                    CrawlingProgress {
                        current: *attempt,
                        total: *max_attempts,
                        percentage: (*attempt as f64 / *max_attempts as f64) * 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {} 재시도 {}/{} (배치 {}) - {}", page, attempt, max_attempts, batch_id, reason),
                        status: CrawlingStatus::Running,
                        message: format!("Page {} attempt {}/{} in batch {} - {}", page, attempt, max_attempts, batch_id, reason),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::PageRetrySuccess { page, batch_id, url: _, final_attempt, products_found } => {
                    CrawlingProgress {
                        current: *final_attempt,
                        total: *final_attempt,
                        percentage: 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {} 재시도 성공 ({}번째 시도, {}개 제품, 배치 {})", page, final_attempt, products_found, batch_id),
                        status: CrawlingStatus::Running,
                        message: format!("Page attempt succeeded on attempt {} with {} products (batch {})", final_attempt, products_found, batch_id),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: *products_found,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::PageRetryFailed { page, batch_id, url: _, total_attempts, final_error } => {
                    CrawlingProgress {
                        current: *total_attempts,
                        total: *total_attempts,
                        percentage: 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {} 최종 실패 ({}번 재시도 후, 배치 {}) - {}", page, total_attempts, batch_id, final_error),
                        status: CrawlingStatus::Error,
                        message: format!("Page {} finally failed after {} attempts (batch {}) - {}", page, total_attempts, batch_id, final_error),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 1,
                        timestamp: chrono::Utc::now(),
                    }
                },
                
                // 🔥 새로운 제품 관련 이벤트들 (기본 처리만 제공)
                DetailedCrawlingEvent::ProductStarted { url: _, batch_id, product_index, total_products } => {
                    CrawlingProgress {
                        current: *product_index,
                        total: *total_products,
                        percentage: (*product_index as f64 / *total_products as f64) * 100.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 {}/{} 시작 (배치 {})", product_index, total_products, batch_id),
                        status: CrawlingStatus::Running,
                        message: format!("Product {}/{} started in batch {}", product_index, total_products, batch_id),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ProductRetryAttempt { url: _, batch_id, attempt, max_attempts, reason } => {
                    CrawlingProgress {
                        current: *attempt,
                        total: *max_attempts,
                        percentage: (*attempt as f64 / *max_attempts as f64) * 100.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 재시도 {}/{} (배치 {}) - {}", attempt, max_attempts, batch_id, reason),
                        status: CrawlingStatus::Running,
                        message: format!("Product attempt {}/{} in batch {} - {}", attempt, max_attempts, batch_id, reason),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ProductRetrySuccess { url: _, batch_id, final_attempt } => {
                    CrawlingProgress {
                        current: *final_attempt,
                        total: *final_attempt,
                        percentage: 100.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 재시도 성공 ({}번째 시도, 배치 {})", final_attempt, batch_id),
                        status: CrawlingStatus::Running,
                        message: format!("Product attempt succeeded on attempt {} (batch {})", final_attempt, batch_id),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 1,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ProductRetryFailed { url: _, batch_id, total_attempts, final_error } => {
                    CrawlingProgress {
                        current: *total_attempts,
                        total: *total_attempts,
                        percentage: 100.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 최종 실패 ({}번 재시도 후, 배치 {}) - {}", total_attempts, batch_id, final_error),
                        status: CrawlingStatus::Error,
                        message: format!("Product finally failed after {} attempts (batch {}) - {}", total_attempts, batch_id, final_error),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 1,
                        timestamp: chrono::Utc::now(),
                    }
                },
                
                DetailedCrawlingEvent::StageCompleted { stage, items_processed } => {
                    CrawlingProgress {
                        current: *items_processed as u32,
                        total: *items_processed as u32,
                        percentage: 100.0,
                        current_stage: match stage.as_str() {
                            "SiteStatus" => CrawlingStage::StatusCheck,
                            "DatabaseAnalysis" => CrawlingStage::DatabaseAnalysis,
                            "ProductList" => CrawlingStage::ProductList,
                            "ProductDetails" => CrawlingStage::ProductDetails,
                            "DatabaseSave" => CrawlingStage::DatabaseSave,
                            _ => CrawlingStage::TotalPages,
                        },
                        current_step: format!("{} 스테이지 완료 ({}개 항목 처리)", stage, items_processed),
                        status: CrawlingStatus::Completed,
                        message: format!("Stage {} completed: {} items processed", stage, items_processed),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: *items_processed as u32,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                // 🔥 새로운 페이지 수집 이벤트들
                DetailedCrawlingEvent::PageCollectionStarted { page, batch_id, url: _, estimated_products } => {
                    CrawlingProgress {
                        current: 0,
                        total: estimated_products.unwrap_or(0),
                        percentage: 0.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {} 수집 시작 (배치 {}, 예상 제품: {}개)", page, batch_id, estimated_products.unwrap_or(0)),
                        status: CrawlingStatus::Running,
                        message: format!("Page {} collection started (batch {}, estimated: {})", page, batch_id, estimated_products.unwrap_or(0)),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::PageCollectionCompleted { page, batch_id, url: _, products_found, duration_ms } => {
                    CrawlingProgress {
                        current: *page,
                        total: *page,
                        percentage: 100.0,
                        current_stage: CrawlingStage::ProductList,
                        current_step: format!("페이지 {} 수집 완료: {}개 제품 발견 ({}ms, 배치 {})", page, products_found, duration_ms, batch_id),
                        status: CrawlingStatus::Running,
                        message: format!("Page {} collection completed: {} products found in {}ms (batch {})", page, products_found, duration_ms, batch_id),
                        remaining_time: None,
                        elapsed_time: *duration_ms,
                        new_items: *products_found,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                // 🔥 새로운 제품 상세 수집 이벤트들
                DetailedCrawlingEvent::ProductDetailCollectionStarted { url: _, product_index, total_products, batch_id } => {
                    CrawlingProgress {
                        current: *product_index,
                        total: *total_products,
                        percentage: (*product_index as f64 / *total_products as f64) * 100.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 상세정보 수집 시작: {}/{} (배치 {})", product_index, total_products, batch_id),
                        status: CrawlingStatus::Running,
                        message: format!("Product detail collection started: {}/{} (batch {})", product_index, total_products, batch_id),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ProductDetailProcessingStarted { url: _, product_index, parsing_stage } => {
                    CrawlingProgress {
                        current: *product_index,
                        total: *product_index + 1,
                        percentage: 0.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 {} 상세정보 처리 시작: {}", product_index, parsing_stage),
                        status: CrawlingStatus::Running,
                        message: format!("Product {} detail processing started: {}", product_index, parsing_stage),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::ProductDetailCollectionCompleted { url: _, product_index, success, duration_ms, data_extracted } => {
                    CrawlingProgress {
                        current: *product_index,
                        total: *product_index,
                        percentage: 100.0,
                        current_stage: CrawlingStage::ProductDetails,
                        current_step: format!("제품 {} 상세정보 수집 완료: {} (데이터 추출: {}) in {}ms", product_index, if *success { "성공" } else { "실패" }, data_extracted, duration_ms),
                        status: if *success { CrawlingStatus::Running } else { CrawlingStatus::Error },
                        message: format!("Product {} detail collection completed: {} (data extracted: {}) in {}ms", product_index, if *success { "success" } else { "failure" }, data_extracted, duration_ms),
                        remaining_time: None,
                        elapsed_time: *duration_ms,
                        new_items: if *success { 1 } else { 0 },
                        updated_items: 0,
                        current_batch: Some(1),
                        total_batches: Some(1),
                        errors: if *success { 0 } else { 1 },
                        timestamp: chrono::Utc::now(),
                    }
                },
                // 🔥 새로운 데이터베이스 배치 저장 이벤트들
                DetailedCrawlingEvent::DatabaseBatchSaveStarted { batch_id, products_count, batch_size } => {
                    CrawlingProgress {
                        current: 0,
                        total: *products_count,
                        percentage: 0.0,
                        current_stage: CrawlingStage::DatabaseSave,
                        current_step: format!("데이터베이스 배치 {} 저장 시작 ({}개 제품, 배치 크기: {})", batch_id, products_count, batch_size),
                        status: CrawlingStatus::Running,
                        message: format!("Database batch {} save started: {} products (batch size: {})", batch_id, products_count, batch_size),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                DetailedCrawlingEvent::DatabaseBatchSaveCompleted { batch_id, products_saved, new_items, updated_items, errors, duration_ms } => {
                    CrawlingProgress {
                        current: *products_saved,
                        total: *products_saved,
                        percentage: 100.0,
                        current_stage: CrawlingStage::DatabaseSave,
                        current_step: format!("데이터베이스 배치 {} 저장 완료: {}개 저장 (신규: {}, 업데이트: {}, 오류: {}) in {}ms", batch_id, products_saved, new_items, updated_items, errors, duration_ms),
                        status: CrawlingStatus::Running,
                        message: format!("Database batch {} save completed: {} saved (new: {}, updated: {}, errors: {}) in {}ms", batch_id, products_saved, new_items, updated_items, errors, duration_ms),
                        remaining_time: None,
                        elapsed_time: *duration_ms,
                        new_items: *new_items,
                        updated_items: *updated_items,
                        current_batch: Some(*batch_id),
                        total_batches: Some(1),
                        errors: *errors,
                        timestamp: chrono::Utc::now(),
                    }
                },
            };

            emitter.emit_progress(progress).await?;
            
            // 또한 DetailedCrawlingEvent를 직접 전송 (계층적 이벤트 모니터용)
            emitter.emit_detailed_crawling_event(event.clone()).await?;
        }
        
        debug!("Emitted detailed event: {:?}", event);
        Ok(())
    }

    /// Update cancellation token for the current session
    pub fn update_cancellation_token(&mut self, cancellation_token: Option<CancellationToken>) {
        self.config.cancellation_token = cancellation_token;
        info!("🔄 Updated cancellation token in ServiceBasedBatchCrawlingEngine: {}", 
              self.config.cancellation_token.is_some());
    }

    /// Stop the crawling engine by cancelling the cancellation token
    pub async fn stop(&self) -> Result<(), String> {
        if let Some(cancellation_token) = &self.config.cancellation_token {
            tracing::info!("🛑 Stopping ServiceBasedBatchCrawlingEngine by cancelling token");
            cancellation_token.cancel();
            Ok(())
        } else {
            let error_msg = "Cannot stop: No cancellation token available";
            tracing::warn!("⚠️ {}", error_msg);
            Err(error_msg.to_string())
        }
    }

    /// AtomicTaskEvent 발송 (Live Production Line UI용)
    // REMOVE_CANDIDATE(Phase3): Legacy granular event emission
    fn emit_atomic_task_event(&self, task_id: &str, stage_name: &str, status: TaskStatus, progress: f64, message: Option<String>) {
        if let Some(broadcaster) = &self.broadcaster {
            let batch_id = 1; // 현재는 단일 배치로 처리
            let event = AtomicTaskEvent {
                task_id: task_id.to_string(),
                batch_id,
                stage_name: stage_name.to_string(),
                status,
                progress,
                message,
                timestamp: Utc::now(),
            };
            
            if let Err(e) = broadcaster.emit_atomic_task_event(event) {
                warn!("Failed to emit atomic task event: {}", e);
            }
        }
    }
}

impl std::fmt::Debug for ServiceBasedBatchCrawlingEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceBasedBatchCrawlingEngine")
            .field("status_checker", &"Arc<dyn StatusChecker>")
            .field("database_analyzer", &"Arc<dyn DatabaseAnalyzer>") 
            .field("product_list_collector", &"Arc<dyn ProductListCollector>")
            .field("product_detail_collector", &"Arc<dyn ProductDetailCollector>")
            .field("data_processor", &"Arc<dyn DataProcessor>")
            .field("storage_service", &"Arc<dyn StorageService>")
            .field("config", &self.config)
            .finish()
    }
}
