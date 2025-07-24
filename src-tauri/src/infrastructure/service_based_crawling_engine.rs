//! 개선된 배치 크롤링 엔진 - 서비스 레이어 분리 버전
//! 
//! 이 모듈은 guide/crawling 문서의 요구사항에 따라 각 단계를 
//! 독립적인 서비스로 분리하여 구현한 엔터프라이즈급 크롤링 엔진입니다.

use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};
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
    StatusCheckerImpl, ProductListCollectorImpl,
    CrawlingRangeCalculator, CollectorConfig, product_detail_to_product
};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::system_broadcaster::SystemStateBroadcaster;
use crate::events::{AtomicTaskEvent, TaskStatus};

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
            Arc::new(tokio::sync::Mutex::new(http_client.clone())),
            Arc::new(data_extractor.clone()),
            list_collector_config,
            status_checker_impl.clone(),
        ));

        // ProductDetailCollector는 ProductListCollectorImpl을 재사용 (trait 구현 추가됨)
        let product_detail_collector: Arc<dyn ProductDetailCollector> = Arc::new(ProductListCollectorImpl::new(
            Arc::new(tokio::sync::Mutex::new(http_client.clone())),
            Arc::new(data_extractor.clone()),
            detail_collector_config,
            status_checker_impl,
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

        // 🔥 크롤링 시작 이벤트 발송 (UI 연결)
        if let Some(broadcaster) = &mut self.broadcaster {
            if let Err(e) = broadcaster.emit_crawling_started() {
                warn!("Failed to emit crawling-started event: {}", e);
            }
        }

        // 세션 시작 이벤트
        self.emit_detailed_event(DetailedCrawlingEvent::SessionStarted {
            session_id: self.session_id.clone(),
            config: self.config.clone(),
        }).await?;

        // 시작 전 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled before starting");
                return Err(anyhow!("Crawling session cancelled before starting"));
            }
        }

        // Stage 0: 사이트 상태 확인
        let site_status = self.stage0_check_site_status().await?;
        
        // Stage 0 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 0");
                return Err(anyhow!("Crawling session cancelled after site status check"));
            }
        }
        
        // Stage 0.5: 지능형 범위 재계산 및 실제 적용 - Phase 4 Implementation
        info!("🧠 Stage 0.5: Performing intelligent range recalculation");
        info!("📊 Site analysis: total_pages={}, products_on_last_page={}", 
              site_status.total_pages, site_status.products_on_last_page);
        
        let optimal_range = self.range_calculator.calculate_next_crawling_range(
            site_status.total_pages,
            site_status.products_on_last_page, // ✅ 실제 값 사용 (이전: 하드코딩 10)
        ).await?;
        
        // 계산된 범위를 실제로 적용하여 최종 범위 결정
        let (actual_start_page, actual_end_page) = if let Some((optimal_start, optimal_end)) = optimal_range {
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
        
        // Stage 2: 제품 목록 수집 - 계산된 최적 범위 사용
        let product_urls = self.stage2_collect_product_list_optimized(actual_start_page, actual_end_page).await?;
        
        // Stage 2 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 2");
                return Err(anyhow!("Crawling session cancelled after product list collection"));
            }
        }
        
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
        
        // 세션 완료 이벤트
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
    async fn stage2_collect_product_list(&self, total_pages: u32) -> Result<Vec<ProductUrl>> {
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
                cancellation_token.clone()
            ).await?
        } else {
            warn!("⚠️  No cancellation token - using parallel collection without cancellation");
            // 취소 토큰이 없어도 병렬 처리 사용
            self.product_list_collector.collect_page_range(self.config.start_page, effective_end).await?
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
    async fn stage2_collect_product_list_optimized(&mut self, start_page: u32, end_page: u32) -> Result<Vec<ProductUrl>> {
        info!("Stage 2: Collecting product list using optimized range {} to {} with TRUE concurrent execution", start_page, end_page);
        
        // 🔥 배치 생성 이벤트 발송 (UI 연결)
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
                if let Some(ref mut b) = broadcaster {
                    match event_type.as_str() {
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

        // 🔥 이벤트 콜백 함수 정의
        let event_tx_clone = event_tx.clone();
        let page_callback = move |page_id: u32, url: String, product_count: u32, success: bool| -> Result<()> {
            let payload = serde_json::to_value((page_id, url, product_count, success))?;
            if let Err(e) = event_tx_clone.try_send(("page-crawled".to_string(), payload)) {
                warn!("Failed to send page-crawled event: {}", e);
            }
            Ok(())
        };

        let event_tx_clone2 = event_tx.clone();
        let retry_callback = move |item_id: String, item_type: String, url: String, attempt: u32, max_attempts: u32, reason: String| -> Result<()> {
            let payload = serde_json::to_value((item_id, item_type, url, attempt, max_attempts, reason))?;
            if let Err(e) = event_tx_clone2.try_send(("retry-attempt".to_string(), payload)) {
                warn!("Failed to send retry-attempt event: {}", e);
            }
            Ok(())
        };

        // 실제 크롤링 실행 (동시성 유지)
        let product_urls = if let Some(cancellation_token) = &self.config.cancellation_token {
            // 기존 동시성 메서드 사용하지만 이벤트 콜백과 함께
            let collector = self.product_list_collector.clone();
            let collector_impl = collector.as_ref()
                .as_any()
                .downcast_ref::<ProductListCollectorImpl>()
                .ok_or_else(|| anyhow!("Failed to downcast ProductListCollector"))?;
            
            collector_impl.collect_page_range_with_events(
                start_page,
                end_page,
                Some(cancellation_token.clone()),
                page_callback,
                retry_callback,
            ).await?
        } else {
            // 기존 동시성 메서드 사용
            self.product_list_collector.collect_page_range(
                start_page,
                end_page,
            ).await.map_err(|e| anyhow!("Product list collection failed: {}", e))?
        };

        // 이벤트 채널 종료
        drop(event_tx);
        
        // 이벤트 처리 완료 대기 및 브로드캐스터 복구
        if let Ok(broadcaster_opt) = event_handler.await {
            self.broadcaster = broadcaster_opt;
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
        let mut failed_urls = Vec::new();

        // 항상 취소 토큰을 사용하도록 강제 - 없으면 기본 토큰 생성
        let result = if let Some(cancellation_token) = &self.config.cancellation_token {
            info!("🛑 USING PROVIDED CANCELLATION TOKEN for product detail collection");
            info!("🛑 Cancellation token is_cancelled: {}", cancellation_token.is_cancelled());
            self.product_detail_collector.collect_details_with_cancellation(product_urls, cancellation_token.clone()).await
        } else {
            warn!("⚠️  NO CANCELLATION TOKEN - creating default token for consistent behavior");
            let default_token = CancellationToken::new();
            self.product_detail_collector.collect_details_with_cancellation(product_urls, default_token).await
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
                        
                        // 🔥 제품 수집 완료 이벤트 발송 (UI 연결)
                        if let Some(broadcaster) = &mut self.broadcaster {
                            if let Some(product_url) = product_urls.get(index) {
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
                failed_urls = product_urls.to_vec();
                
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
                    
                    // 🔥 재시도 시도 이벤트 발송
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
                                
                                // 🔥 재시도 성공 이벤트 발송
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
                                
                                // 🔥 재시도 최종 실패 이벤트 발송
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

    /// Stage 4: 데이터베이스 저장
    async fn stage4_save_to_database(&mut self, products: Vec<(Product, ProductDetail)>) -> Result<(usize, usize, usize, usize)> {
        info!("Stage 4: Saving {} products to database", products.len());
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "DatabaseSave".to_string(),
            message: format!("{}개 제품을 데이터베이스에 저장하는 중...", products.len()),
        }).await?;

        let mut new_items = 0;
        let mut updated_items = 0;
        let mut errors = 0;
        let total_items = products.len();

        for (index, (product, product_detail)) in products.into_iter().enumerate() {
            // 주기적으로 취소 확인 (100개마다)
            if index % 100 == 0 {
                if let Some(cancellation_token) = &self.config.cancellation_token {
                    if cancellation_token.is_cancelled() {
                        warn!("🛑 Database save cancelled after saving {} products", index);
                        break;
                    }
                }
            }

            let item_id = format!("db_save_{}_{}", index, product.url.replace("https://", "").replace("/", "_"));
            
            // 🔥 DB 저장 시도 이벤트 발송
            if let Some(broadcaster) = &mut self.broadcaster {
                if let Err(e) = broadcaster.emit_database_save_attempt(
                    item_id.clone(),
                    "product".to_string(),
                    product.url.clone()
                ) {
                    warn!("Failed to emit database-save-attempt event: {}", e);
                }
            }
            
            // 제품이 기존에 존재하는지 확인
            let existing_product = self.product_repo.get_product_by_url(&product.url).await?;
            let is_update = existing_product.is_some();
            
            // Product와 ProductDetail을 모두 저장
            let product_save_result = self.product_repo.create_or_update_product(&product).await;
            let product_detail_save_result = self.product_detail_repo.create_or_update_product_detail(&product_detail).await;
            
            match (product_save_result, product_detail_save_result) {
                (Ok(_), Ok(_)) => {
                    if is_update {
                        updated_items += 1;
                    } else {
                        new_items += 1;
                    }
                    
                    // 🔥 DB 저장 성공 이벤트 발송
                    if let Some(broadcaster) = &mut self.broadcaster {
                        if let Err(e) = broadcaster.emit_database_save_success(
                            item_id.clone(),
                            "product".to_string(),
                            product.url.clone(),
                            is_update
                        ) {
                            warn!("Failed to emit database-save-success event: {}", e);
                        }
                    }
                    
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                        url: product.url.clone(),
                        success: true,
                    }).await?;
                },
                (Err(e), _) => {
                    errors += 1;
                    warn!("Failed to save product {:?}: {}", product.model, e);
                    
                    // 🔥 DB 저장 실패 이벤트 발송
                    if let Some(broadcaster) = &mut self.broadcaster {
                        if let Err(emit_err) = broadcaster.emit_database_save_failed(
                            item_id.clone(),
                            "product".to_string(),
                            product.url.clone(),
                            e.to_string()
                        ) {
                            warn!("Failed to emit database-save-failed event: {}", emit_err);
                        }
                    }
                    
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                        url: product.url.clone(),
                        success: false,
                    }).await?;
                }
                (_, Err(e)) => {
                    errors += 1;
                    warn!("Failed to save product detail for {:?}: {}", product.model, e);
                    
                    // 🔥 DB 저장 실패 이벤트 발송 (ProductDetail 저장 실패)
                    if let Some(broadcaster) = &mut self.broadcaster {
                        if let Err(emit_err) = broadcaster.emit_database_save_failed(
                            format!("{}_detail", item_id),
                            "product_detail".to_string(),
                            product.url.clone(),
                            e.to_string()
                        ) {
                            warn!("Failed to emit database-save-failed event: {}", emit_err);
                        }
                    }
                    
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                        url: product.url.clone(),
                        success: false,
                    }).await?;
                }
            }

            // 🔥 배치 진행 상황 업데이트 (10개마다)
            if index % 10 == 0 || index == total_items - 1 {
                let progress = (index + 1) as f64 / total_items as f64;
                let completed = new_items + updated_items + errors;
                
                if let Some(broadcaster) = &mut self.broadcaster {
                    if let Err(e) = broadcaster.emit_batch_progress(
                        "DatabaseSave".to_string(),
                        progress,
                        total_items as u32,
                        completed as u32,
                        0, // items_active (현재 처리 중인 항목)
                        errors as u32
                    ) {
                        warn!("Failed to emit batch-progress event: {}", e);
                    }
                }
            }
        }

        let total_processed = new_items + updated_items + errors;

        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "DatabaseSave".to_string(),
            items_processed: total_processed,
        }).await?;

        info!("Stage 4 completed: {} new, {} updated, {} errors", new_items, updated_items, errors);
        Ok((total_processed, new_items, updated_items, errors))
    }

    /// 세분화된 이벤트 방출
    async fn emit_detailed_event(&self, event: DetailedCrawlingEvent) -> Result<()> {
        if let Some(emitter) = self.event_emitter.as_ref() {
            // DetailedCrawlingEvent를 기존 이벤트 시스템과 연동
            let progress = match &event {
                DetailedCrawlingEvent::StageStarted { stage, message } => {
                    CrawlingProgress {
                        current: 0,
                        total: self.config.end_page - self.config.start_page + 1,
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
                        total_batches: Some(self.config.end_page - self.config.start_page + 1),
                        errors: 0,
                        timestamp: chrono::Utc::now(),
                    }
                },
                _ => return Ok(()), // 다른 이벤트들은 기본 진행률 업데이트를 사용하지 않음
            };

            emitter.emit_progress(progress).await?;
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
