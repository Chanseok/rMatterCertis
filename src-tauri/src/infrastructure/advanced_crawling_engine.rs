//! 고급 데이터 처리 파이프라인을 포함한 크롤링 엔진
//! 
//! 이 모듈은 Phase 2의 목표인 고급 데이터 처리 기능을 포함한
//! 엔터프라이즈급 크롤링 엔진을 구현합니다.

use std::sync::Arc;
use std::time::Instant;
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    DeduplicationService, ValidationService, ConflictResolver,
    BatchProgressTracker, BatchRecoveryService, ErrorClassifier
};
use crate::domain::events::{CrawlingProgress, CrawlingStage, CrawlingStatus};
use crate::domain::product::Product;
use crate::application::EventEmitter;
use crate::infrastructure::{
    HttpClient, MatterDataExtractor, IntegratedProductRepository,
    StatusCheckerImpl, DatabaseAnalyzerImpl, ProductListCollectorImpl, 
    ProductDetailCollectorImpl, CollectorConfig,
    DeduplicationServiceImpl, ValidationServiceImpl, ConflictResolverImpl,
    config::AppConfig
};
use crate::infrastructure::data_processing_service_impls::{
    BatchProgressTrackerImpl, BatchRecoveryServiceImpl, RetryManagerImpl, ErrorClassifierImpl
};
use crate::infrastructure::service_based_crawling_engine::{BatchCrawlingConfig, DetailedCrawlingEvent};
use crate::domain::services::data_processing_services::ResolutionStrategy;

/// Phase 2 고급 크롤링 엔진 - 데이터 처리 파이프라인 포함
#[allow(dead_code)] // Phase 2.2에서 모든 필드가 사용될 예정
pub struct AdvancedBatchCrawlingEngine {
    // 기존 서비스 레이어들
    status_checker: Arc<dyn StatusChecker>,
    database_analyzer: Arc<dyn DatabaseAnalyzer>,
    product_list_collector: Arc<dyn ProductListCollector>,
    product_detail_collector: Arc<dyn ProductDetailCollector>,
    
    // 새로운 데이터 처리 서비스들
    deduplication_service: Arc<dyn DeduplicationService>,
    validation_service: Arc<dyn ValidationService>,
    conflict_resolver: Arc<dyn ConflictResolver>,
    
    // 고급 관리 서비스들
    progress_tracker: Arc<dyn BatchProgressTracker>,
    recovery_service: Arc<dyn BatchRecoveryService>,
    retry_manager: Arc<RetryManagerImpl>, // 구체적인 타입 사용 (dyn-compatibility 문제 해결)
    error_classifier: Arc<dyn ErrorClassifier>,
    
    // 기존 컴포넌트들
    product_repo: Arc<IntegratedProductRepository>,
    event_emitter: Arc<Option<EventEmitter>>,
    
    // 설정 및 세션 정보
    config: BatchCrawlingConfig,
    session_id: String,
}

impl AdvancedBatchCrawlingEngine {
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
            concurrency: config.concurrency,
            delay_ms: config.delay_ms,
            batch_size: config.batch_size,
            retry_max: config.retry_max,
        };

        // 기본 앱 설정 로드
        let app_config = AppConfig::default();

        // 기존 서비스 인스턴스 생성
        let status_checker = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config,
        )) as Arc<dyn StatusChecker>;

        let database_analyzer = Arc::new(DatabaseAnalyzerImpl::new(
            Arc::clone(&product_repo),
        )) as Arc<dyn DatabaseAnalyzer>;

        let product_list_collector = Arc::new(ProductListCollectorImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            collector_config.clone(),
        )) as Arc<dyn ProductListCollector>;

        let product_detail_collector = Arc::new(ProductDetailCollectorImpl::new(
            http_client,
            data_extractor,
            collector_config,
        )) as Arc<dyn ProductDetailCollector>;

        // 새로운 데이터 처리 서비스 인스턴스 생성
        let deduplication_service = Arc::new(DeduplicationServiceImpl::new(0.85)) as Arc<dyn DeduplicationService>;
        let validation_service = Arc::new(ValidationServiceImpl::new()) as Arc<dyn ValidationService>;
        let conflict_resolver = Arc::new(ConflictResolverImpl::new(ResolutionStrategy::KeepMostComplete)) as Arc<dyn ConflictResolver>;

        // 고급 관리 서비스 인스턴스 생성
        let progress_tracker = Arc::new(BatchProgressTrackerImpl::new()) as Arc<dyn BatchProgressTracker>;
        let recovery_service = Arc::new(BatchRecoveryServiceImpl::new()) as Arc<dyn BatchRecoveryService>;
        let retry_manager = Arc::new(RetryManagerImpl::new(3, 1000)); // 구체적인 타입
        let error_classifier = Arc::new(ErrorClassifierImpl::new()) as Arc<dyn ErrorClassifier>;

        Self {
            status_checker,
            database_analyzer,
            product_list_collector,
            product_detail_collector,
            deduplication_service,
            validation_service,
            conflict_resolver,
            progress_tracker,
            recovery_service,
            retry_manager,
            error_classifier,
            product_repo,
            event_emitter,
            config,
            session_id,
        }
    }

    /// 고급 데이터 처리 파이프라인을 포함한 크롤링 실행
    pub async fn execute(&self) -> Result<()> {
        let start_time = Instant::now();
        info!("Starting advanced batch crawling with data processing pipeline for session: {}", self.session_id);

        // 배치 진행 추적 시작
        let batch_id = format!("batch_{}", self.session_id);
        self.progress_tracker.update_progress(&batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.clone(),
            total_items: 100,
            processed_items: 0,
            successful_items: 0,
            failed_items: 0,
            progress_percentage: 0.0,
            estimated_remaining_time: Some(360), // 6분 예상
            current_stage: "초기화".to_string(),
        }).await?;

        // 세션 시작 이벤트
        self.emit_detailed_event(DetailedCrawlingEvent::SessionStarted {
            session_id: self.session_id.clone(),
            config: self.config.clone(),
        }).await?;

        let mut total_products = 0;
        let mut success_rate = 0.0;

        // 에러 처리 및 복구를 위한 변수들
        let mut execution_result = Ok(());

        // 전체 실행을 try-catch로 감싸서 오류 처리
        match self.execute_with_error_handling(&batch_id).await {
            Ok((products_count, calculated_success_rate)) => {
                total_products = products_count;
                success_rate = calculated_success_rate;
                
                // 배치 완료
                let batch_result = crate::domain::services::data_processing_services::BatchResult {
                    batch_id: batch_id.clone(),
                    total_processed: total_products,
                    successful: (total_products as f64 * success_rate) as u32,
                    failed: total_products - (total_products as f64 * success_rate) as u32,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    errors: vec![],
                };
                self.progress_tracker.complete_batch(&batch_id, batch_result).await?;
            }
            Err(e) => {
                warn!("Batch execution failed: {}", e);
                
                // 에러 분류 및 복구 시도
                let _error_type = self.error_classifier.classify(&e.to_string()).await?;
                let _severity = self.error_classifier.assess_severity(&e.to_string()).await?;
                let is_recoverable = self.error_classifier.assess_recoverability(&e.to_string()).await?;
                
                if is_recoverable {
                    info!("Attempting error recovery for batch {}", batch_id);
                    match self.recovery_service.recover_parsing_error(&e.to_string()).await {
                        Ok(recovery_action) => {
                            info!("Recovery action determined: {:?}", recovery_action);
                            // 복구 액션에 따른 처리는 향후 확장
                        }
                        Err(recovery_err) => {
                            warn!("Recovery failed: {}", recovery_err);
                        }
                    }
                }
                
                execution_result = Err(e);
            }
        }

        let duration = start_time.elapsed();
        info!("Advanced batch crawling completed in {:?}", duration);
        
        // 세션 완료 이벤트
        self.emit_detailed_event(DetailedCrawlingEvent::SessionCompleted {
            session_id: self.session_id.clone(),
            duration,
            total_products,
            success_rate,
        }).await?;
        
        execution_result
    }

    /// 에러 처리가 포함된 실제 실행 로직
    async fn execute_with_error_handling(&self, batch_id: &str) -> Result<(u32, f64)> {

        // Stage 0: 사이트 상태 확인
        let site_status = self.stage0_check_site_status().await?;
        
        // 진행률 업데이트
        self.progress_tracker.update_progress(batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 100, // 예상 총 작업 수
            processed_items: 10,
            successful_items: 10,
            failed_items: 0,
            progress_percentage: 10.0,
            estimated_remaining_time: Some(300), // 5분 예상
            current_stage: "사이트 상태 확인".to_string(),
        }).await?;
        
        // Stage 1: 데이터베이스 분석
        let _db_analysis = self.stage1_analyze_database().await?;
        
        // 진행률 업데이트
        self.progress_tracker.update_progress(batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 100,
            processed_items: 20,
            successful_items: 20,
            failed_items: 0,
            progress_percentage: 20.0,
            estimated_remaining_time: Some(240),
            current_stage: "데이터베이스 분석".to_string(),
        }).await?;
        
        // Stage 2: 제품 목록 수집
        let product_urls = self.stage2_collect_product_list(site_status.total_pages).await?;
        
        // 진행률 업데이트
        self.progress_tracker.update_progress(batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 100,
            processed_items: 40,
            successful_items: 40,
            failed_items: 0,
            progress_percentage: 40.0,
            estimated_remaining_time: Some(180),
            current_stage: "제품 목록 수집".to_string(),
        }).await?;
        
        // Stage 3: 제품 상세정보 수집
        let raw_products = self.stage3_collect_product_details(&product_urls).await?;
        
        // 진행률 업데이트
        self.progress_tracker.update_progress(batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 100,
            processed_items: 70,
            successful_items: 70,
            failed_items: 0,
            progress_percentage: 70.0,
            estimated_remaining_time: Some(90),
            current_stage: "제품 상세정보 수집".to_string(),
        }).await?;
        
        // Stage 4: 고급 데이터 처리 파이프라인
        let processed_products = self.stage4_process_data_pipeline(raw_products).await?;
        let total_products = processed_products.len() as u32;
        
        // 진행률 업데이트
        self.progress_tracker.update_progress(batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 100,
            processed_items: 90,
            successful_items: 90,
            failed_items: 0,
            progress_percentage: 90.0,
            estimated_remaining_time: Some(30),
            current_stage: "고급 데이터 처리".to_string(),
        }).await?;
        
        // Stage 5: 데이터베이스 저장
        let (processed_count, _new_items, _updated_items, errors) = self.stage5_save_to_database(processed_products).await?;
        
        // 성공률 계산
        let success_rate = if processed_count > 0 {
            (processed_count - errors) as f64 / processed_count as f64
        } else {
            0.0
        };

        // 최종 진행률 업데이트
        self.progress_tracker.update_progress(batch_id, crate::domain::services::data_processing_services::BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 100,
            processed_items: 100,
            successful_items: (processed_count - errors) as u32,
            failed_items: errors as u32,
            progress_percentage: 100.0,
            estimated_remaining_time: Some(0),
            current_stage: "완료".to_string(),
        }).await?;

        Ok((total_products, success_rate))
    }

    /// Stage 0: 사이트 상태 확인
    async fn stage0_check_site_status(&self) -> Result<crate::domain::services::SiteStatus> {
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

    /// Stage 1: 데이터베이스 분석
    async fn stage1_analyze_database(&self) -> Result<crate::domain::services::DatabaseAnalysis> {
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

    /// Stage 2: 제품 목록 수집
    async fn stage2_collect_product_list(&self, total_pages: u32) -> Result<Vec<String>> {
        info!("Stage 2: Collecting product list");
        
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

    /// Stage 3: 제품 상세정보 수집
    async fn stage3_collect_product_details(&self, product_urls: &[String]) -> Result<Vec<Product>> {
        info!("Stage 3: Collecting product details");
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "ProductDetails".to_string(),
            message: format!("{}개 제품의 상세정보를 수집하는 중...", product_urls.len()),
        }).await?;

        let products = self.product_detail_collector.collect_details(product_urls).await?;
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "ProductDetails".to_string(),
            items_processed: products.len(),
        }).await?;

        info!("Stage 3 completed: {} products collected", products.len());
        Ok(products)
    }

    /// Stage 4: 고급 데이터 처리 파이프라인 (새로운 단계)
    async fn stage4_process_data_pipeline(&self, raw_products: Vec<Product>) -> Result<Vec<Product>> {
        info!("Stage 4: Processing data through advanced pipeline");
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "DataProcessing".to_string(),
            message: format!("{}개 제품에 대한 고급 데이터 처리 진행 중...", raw_products.len()),
        }).await?;

        // 4.1: 중복 제거
        info!("Step 4.1: Removing duplicates");
        let deduplication_analysis = self.deduplication_service.analyze_duplicates(&raw_products).await?;
        info!("Duplicate analysis: {:.2}% duplicates found", deduplication_analysis.duplicate_rate * 100.0);
        
        let deduplicated_products = self.deduplication_service.remove_duplicates(raw_products).await?;
        info!("Deduplication completed: {} products remaining", deduplicated_products.len());

        // 4.2: 유효성 검사
        info!("Step 4.2: Validating products");
        let validation_result = self.validation_service.validate_all(deduplicated_products).await?;
        info!("Validation completed: {} valid, {} invalid products", 
              validation_result.valid_products.len(), validation_result.invalid_products.len());
        
        if !validation_result.validation_summary.common_errors.is_empty() {
            info!("Common validation errors: {:?}", validation_result.validation_summary.common_errors);
        }

        // 4.3: 충돌 해결
        info!("Step 4.3: Resolving conflicts");
        let resolved_products = self.conflict_resolver.resolve_conflicts(validation_result.valid_products).await?;
        info!("Conflict resolution completed: {} final products", resolved_products.len());

        self.emit_detailed_event(DetailedCrawlingEvent::StageCompleted {
            stage: "DataProcessing".to_string(),
            items_processed: resolved_products.len(),
        }).await?;

        info!("Stage 4 completed: Data processing pipeline finished with {} high-quality products", resolved_products.len());
        Ok(resolved_products)
    }

    /// Stage 5: 데이터베이스 저장
    async fn stage5_save_to_database(&self, products: Vec<Product>) -> Result<(usize, usize, usize, usize)> {
        info!("Stage 5: Saving {} processed products to database", products.len());
        
        self.emit_detailed_event(DetailedCrawlingEvent::StageStarted {
            stage: "DatabaseSave".to_string(),
            message: format!("{}개 제품을 데이터베이스에 저장하는 중...", products.len()),
        }).await?;

        let mut new_items = 0;
        let updated_items = 0;
        let mut errors = 0;

        for (index, product) in products.iter().enumerate() {
            match self.product_repo.create_or_update_product(product).await {
                Ok(_) => {
                    // 임시로 모든 제품을 new_items로 계산
                    new_items += 1;
                    
                    if (index + 1) % 50 == 0 {
                        self.emit_detailed_event(DetailedCrawlingEvent::BatchCompleted {
                            batch: (index + 1) as u32 / 50,
                            total: ((products.len() + 49) / 50) as u32,
                        }).await?;
                    }
                    
                    self.emit_detailed_event(DetailedCrawlingEvent::ProductProcessed {
                        url: product.url.clone(),
                        success: true,
                    }).await?;
                },
                Err(e) => {
                    errors += 1;
                    warn!("Failed to save product from {}: {}", product.url, e);
                    
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

        info!("Stage 5 completed: {} new, {} updated, {} errors", new_items, updated_items, errors);
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
                            "DataProcessing" => CrawlingStage::ProductDetails, // 데이터 처리도 ProductDetails로 분류
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
                DetailedCrawlingEvent::BatchCompleted { batch, total } => {
                    CrawlingProgress {
                        current: *batch,
                        total: *total,
                        percentage: (*batch as f64 / *total as f64) * 100.0,
                        current_stage: CrawlingStage::DatabaseSave,
                        current_step: format!("배치 {}/{} 완료", batch, total),
                        status: CrawlingStatus::Running,
                        message: format!("Batch {} of {} completed", batch, total),
                        remaining_time: None,
                        elapsed_time: 0,
                        new_items: 0,
                        updated_items: 0,
                        current_batch: Some(*batch),
                        total_batches: Some(*total),
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
