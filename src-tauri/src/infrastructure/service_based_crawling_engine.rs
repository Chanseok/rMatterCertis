//! 개선된 배치 크롤링 엔진 - 서비스 레이어 분리 버전
//! 
//! 이 모듈은 guide/crawling 문서의 요구사항에 따라 각 단계를 
//! 독립적인 서비스로 분리하여 구현한 엔터프라이즈급 크롤링 엔진입니다.

use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};

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

        // Stage 0: 사이트 상태 확인
        let site_status = self.stage0_check_site_status().await?;
        
        // Stage 1: 데이터베이스 분석
        let _db_analysis = self.stage1_analyze_database().await?;
        
        // Stage 2: 제품 목록 수집
        let product_urls = self.stage2_collect_product_list(site_status.total_pages).await?;
        
        // Stage 3: 제품 상세정보 수집
        let products = self.stage3_collect_product_details(&product_urls).await?;
        let total_products = products.len() as u32;
        
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
        
        if !site_status.is_accessible || site_status.health_score < 0.5 {
            let error_msg = format!("Site is not accessible or unhealthy (score: {})", site_status.health_score);
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
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "ProductList".to_string(),
            message: format!("{}페이지에서 제품 목록을 수집하는 중...", total_pages),
        }).await?;

        let effective_end = total_pages.min(self.config.end_page);
        let product_urls = self.product_list_collector.collect_all_pages(effective_end).await?;
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "ProductList".to_string(),
            items_processed: product_urls.len(),
        }).await?;

        info!("Stage 2 completed: {} product URLs collected", product_urls.len());
        Ok(product_urls)
    }

    /// Stage 3: 제품 상세정보 수집 (서비스 기반 + 재시도 메커니즘)
    async fn stage3_collect_product_details(&self, product_urls: &[String]) -> Result<Vec<Product>> {
        info!("Stage 3: Collecting product details using ProductDetailCollector service with retry mechanism");
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "ProductDetails".to_string(),
            message: format!("{}개 제품의 상세정보를 수집하는 중... (재시도 지원)", product_urls.len()),
        }).await?;

        // 초기 시도
        let mut successful_products = Vec::new();
        let mut failed_urls = Vec::new();

        match self.product_detail_collector.collect_details(product_urls).await {
            Ok(product_details) => {
                // ProductDetail을 Product로 변환
                successful_products = product_details.into_iter()
                    .map(|detail| crate::infrastructure::crawling_service_impls::product_detail_to_product(detail))
                    .collect();
                info!("✅ Initial collection successful: {} products", successful_products.len());
            }
            Err(e) => {
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

        // 재시도 처리
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
            let ready_items = self.retry_manager.get_ready_items().await?;
            if ready_items.is_empty() {
                debug!("No items ready for retry in cycle {}", cycle);
                break;
            }
            
            info!("🔄 Retry cycle {}: Processing {} items", cycle, ready_items.len());
            
            for retry_item in ready_items {
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
                    
                    // 재시도 간 지연
                    tokio::time::sleep(Duration::from_millis(self.config.delay_ms)).await;
                }
            }
            
            // 사이클 간 지연
            if cycle < 3 {
                tokio::time::sleep(Duration::from_secs(5)).await;
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
}
