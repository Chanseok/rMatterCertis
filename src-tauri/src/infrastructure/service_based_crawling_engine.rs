//! 개선된 배치 크롤링 엔진 - 서비스 레이어 분리 버전
//! 
//! 이 모듈은 guide/crawling 문서의 요구사항에 따라 각 단계를 
//! 독립적인 서비스로 분리하여 구현한 엔터프라이즈급 크롤링 엔진입니다.

use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};
use tokio_util::sync::CancellationToken;

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    SiteStatus, DatabaseAnalysis
};
use crate::domain::events::{CrawlingProgress, CrawlingStage, CrawlingStatus};
use crate::domain::product::Product;
use crate::application::EventEmitter;
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository, RetryManager};
use crate::infrastructure::crawling_service_impls::*;
use crate::infrastructure::config::AppConfig;

/// 배치 크롤링 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchCrawlingConfig {
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency: u32,
    pub delay_ms: u64,
    pub batch_size: u32,
    pub retry_max: u32,
    pub timeout_ms: u64,
    #[serde(skip)]
    pub cancellation_token: Option<CancellationToken>,
}

impl Default for BatchCrawlingConfig {
    fn default() -> Self {
        Self {
            start_page: 1,
            end_page: 100,
            concurrency: 3,
            delay_ms: 1000,
            batch_size: 10,
            retry_max: 3,
            timeout_ms: 30000,
            cancellation_token: None,
        }
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
    
    // 기존 컴포넌트들
    product_repo: Arc<IntegratedProductRepository>,
    event_emitter: Arc<Option<EventEmitter>>,
    
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
    ) -> Self {
        // 서비스 설정
        let collector_config = CollectorConfig {
            max_concurrent: config.concurrency,
            concurrency: config.concurrency,
            delay_between_requests: Duration::from_millis(config.delay_ms),
            delay_ms: config.delay_ms,
            batch_size: config.batch_size,
            retry_attempts: config.retry_max,
            retry_max: config.retry_max,
        };
        
        // 기본 앱 설정 로드
        let app_config = AppConfig::default();

        // 서비스 인스턴스 생성
        let status_checker = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config,
        )) as Arc<dyn StatusChecker>;

        let database_analyzer = Arc::new(DatabaseAnalyzerImpl::new(
            Arc::clone(&product_repo),
        )) as Arc<dyn DatabaseAnalyzer>;

        let product_list_collector = Arc::new(ProductListCollectorImpl::new(
            Arc::new(tokio::sync::Mutex::new(http_client.clone())),
            Arc::new(data_extractor.clone()),
            collector_config.clone(),
        )) as Arc<dyn ProductListCollector>;

        let product_detail_collector = Arc::new(ProductDetailCollectorImpl::new(
            Arc::new(tokio::sync::Mutex::new(http_client)),
            Arc::new(data_extractor),
            collector_config,
        )) as Arc<dyn ProductDetailCollector>;

        Self {
            status_checker,
            database_analyzer,
            product_list_collector,
            product_detail_collector,
            product_repo,
            event_emitter,
            retry_manager: Arc::new(RetryManager::new(config.retry_max)),
            config,
            session_id,
        }
    }

    /// 4단계 서비스 기반 크롤링 실행
    pub async fn execute(&self) -> Result<()> {
        let start_time = Instant::now();
        info!("Starting service-based 4-stage batch crawling for session: {}", self.session_id);

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
        
        // Stage 1: 데이터베이스 분석
        let _db_analysis = self.stage1_analyze_database().await?;
        
        // Stage 1 완료 후 취소 확인
        if let Some(cancellation_token) = &self.config.cancellation_token {
            if cancellation_token.is_cancelled() {
                warn!("🛑 Crawling session cancelled after Stage 1");
                return Err(anyhow!("Crawling session cancelled after database analysis"));
            }
        }
        
        // Stage 2: 제품 목록 수집
        let product_urls = self.stage2_collect_product_list(site_status.total_pages).await?;
        
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

        let duration = start_time.elapsed();
        info!("Service-based batch crawling completed in {:?}: {} products collected, {:.2}% success rate", 
            duration, total_products, success_rate * 100.0);
        
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
    async fn stage2_collect_product_list(&self, total_pages: u32) -> Result<Vec<String>> {
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
        
        // 취소 가능한 제품 목록 수집 실행
        let product_urls = if let Some(cancellation_token) = &self.config.cancellation_token {
            info!("🛑 Using cancellation token for product list collection");
            
            // 취소 토큰과 함께 제품 목록 수집 - 개선된 ProductListCollector 사용
            self.product_list_collector.collect_page_range_with_cancellation(
                self.config.start_page, 
                effective_end, 
                cancellation_token.clone()
            ).await?
        } else {
            warn!("⚠️  No cancellation token available for product list collection");
            self.product_list_collector.collect_all_pages(effective_end).await?
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

    /// 취소 가능한 제품 목록 수집 메서드
    async fn collect_product_list_with_cancellation(
        &self,
        total_pages: u32,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<String>> {
        info!("🔄 Starting cancellable product list collection for {} pages", total_pages);

        let start_page = self.config.start_page;
        let end_page = total_pages.min(self.config.end_page);
        let pages_to_process: Vec<u32> = (start_page..=end_page).collect();

        let batch_size = self.config.batch_size as usize;
        let mut all_urls = Vec::new();

        for (batch_index, batch_pages) in pages_to_process.chunks(batch_size).enumerate() {
            // 배치 시작 전 취소 확인
            if cancellation_token.is_cancelled() {
                warn!("🛑 Product list collection cancelled at batch {}", batch_index + 1);
                return Err(anyhow!("Product list collection cancelled"));
            }

            info!("📄 Processing batch {}: pages {:?}", batch_index + 1, batch_pages);

            // 배치 내 페이지들을 병렬로 처리
            let mut batch_tasks = Vec::new();
            for &page in batch_pages {
                let product_list_collector = Arc::clone(&self.product_list_collector);
                let token_clone = cancellation_token.clone();

                let task = tokio::spawn(async move {
                    // 작업 시작 전 취소 확인
                    if token_clone.is_cancelled() {
                        debug!("🛑 Page {} collection cancelled before start", page);
                        return Err(anyhow!("Page collection cancelled"));
                    }

                    // 페이지 수집 실행
                    let result = product_list_collector.collect_single_page(page).await;
                    
                    // 작업 완료 후 취소 확인
                    if token_clone.is_cancelled() {
                        debug!("🛑 Page {} collection cancelled after completion", page);
                        return Err(anyhow!("Page collection cancelled after completion"));
                    }

                    result
                });
                batch_tasks.push(task);
            }

            // 배치 작업 완료 대기
            let batch_results = futures::future::join_all(batch_tasks).await;

            // 결과 처리
            for (i, result) in batch_results.into_iter().enumerate() {
                match result {
                    Ok(Ok(urls)) => {
                        let urls_len = urls.len();
                        all_urls.extend(urls);
                        debug!("✅ Page {} completed: {} URLs", batch_pages[i], urls_len);
                    }
                    Ok(Err(e)) => {
                        if e.to_string().contains("cancelled") {
                            warn!("🛑 Page {} was cancelled", batch_pages[i]);
                            return Err(e);
                        } else {
                            warn!("❌ Page {} failed: {}", batch_pages[i], e);
                        }
                    }
                    Err(e) => {
                        warn!("💥 Page {} task panicked: {}", batch_pages[i], e);
                    }
                }
            }
            
            // 배치 완료 후 취소 확인
            if cancellation_token.is_cancelled() {
                warn!("🛑 Product list collection cancelled after batch {}", batch_index + 1);
                return Err(anyhow!("Product list collection cancelled"));
            }

            // 배치 간 지연 (마지막 배치가 아닌 경우)
            if batch_index < pages_to_process.chunks(batch_size).count() - 1 {
                let delay = Duration::from_millis(self.config.delay_ms);
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {},
                    _ = cancellation_token.cancelled() => {
                        warn!("🛑 Product list collection cancelled during batch delay");
                        return Err(anyhow!("Product list collection cancelled during delay"));
                    }
                }
            }
        }

        info!("✅ Cancellable product list collection completed: {} URLs collected", all_urls.len());
        Ok(all_urls)
    }

    /// Stage 3: 제품 상세정보 수집 (서비스 기반 + 재시도 메커니즘)
    async fn stage3_collect_product_details(&self, product_urls: &[String]) -> Result<Vec<Product>> {
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

        // cancellation token 사용 여부 확인
        let result = if let Some(cancellation_token) = &self.config.cancellation_token {
            info!("🛑 USING CANCELLATION TOKEN for product detail collection (token present)");
            info!("🛑 Cancellation token is_cancelled: {}", cancellation_token.is_cancelled());
            self.product_detail_collector.collect_details_with_cancellation(product_urls, cancellation_token.clone()).await
        } else {
            warn!("⚠️  NO CANCELLATION TOKEN provided - using standard collection (this is the problem!)");
            self.product_detail_collector.collect_details(product_urls).await
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
                
                // ProductDetail을 Product로 변환
                successful_products = product_details.into_iter()
                    .map(|detail| crate::infrastructure::crawling_service_impls::product_detail_to_product(detail))
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
                
                // 실패한 URL들을 재시도 큐에 추가
                for (index, url) in failed_urls.iter().enumerate() {
                    let item_id = format!("product_detail_{}_{}", self.session_id, index);
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("url".to_string(), url.clone());
                    metadata.insert("stage".to_string(), "product_details".to_string());
                    
                    if let Err(retry_err) = self.retry_manager.add_failed_item(
                        item_id,
                        CrawlingStage::ProductDetails,
                        e.to_string(),
                        url.clone(),
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
    async fn process_retries_for_product_details(&self) -> Result<Vec<Product>> {
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
                    
                    match self.product_detail_collector.collect_details(&[url.clone()]).await {
                        Ok(mut product_details) => {
                            if let Some(detail) = product_details.pop() {
                                let product = crate::infrastructure::crawling_service_impls::product_detail_to_product(detail);
                                info!("✅ Retry successful for: {}", url);
                                retry_products.push(product);
                                
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
                                item_id,
                                CrawlingStage::ProductDetails,
                                e.to_string(),
                                url.clone(),
                                metadata,
                            ).await {
                                debug!("Item exceeded retry limit or not retryable: {}", retry_err);
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
    async fn stage4_save_to_database(&self, products: Vec<Product>) -> Result<(usize, usize, usize, usize)> {
        info!("Stage 4: Saving {} products to database", products.len());
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "DatabaseSave".to_string(),
            message: format!("{}개 제품을 데이터베이스에 저장하는 중...", products.len()),
        }).await?;

        let mut new_items = 0;
        let mut updated_items = 0;
        let mut errors = 0;

        for product in products {
            match self.product_repo.create_or_update_product(&product).await {
                Ok(_) => {
                    // 제품이 새로 추가되었는지 업데이트되었는지 확인하기 위해
                    // 기존 제품을 조회해보겠습니다
                    match self.product_repo.get_product_by_url(&product.url).await? {
                        Some(_existing) => updated_items += 1,
                        None => new_items += 1,
                    }
                    
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                        url: product.url.clone(),
                        success: true,
                    }).await?;
                },
                Err(e) => {
                    errors += 1;
                    warn!("Failed to save product {:?}: {}", product.model, e);
                    
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                        url: product.url.clone(),
                        success: false,
                    }).await?;
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
}
