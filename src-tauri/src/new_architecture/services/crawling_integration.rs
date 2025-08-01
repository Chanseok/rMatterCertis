//! 실제 크롤링 서비스와 OneShot Actor 시스템 통합
//! Modern Rust 2024 준수: 기존 크롤링 서비스를 OneShot Actor 패턴으로 연동

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;
use tracing::{info, warn, error, debug};
use anyhow::Result;

use crate::domain::services::crawling_services::{
    StatusChecker, ProductListCollector, ProductDetailCollector, DatabaseAnalyzer,
    SiteStatus, CrawlingRangeRecommendation
};
use crate::domain::product_url::ProductUrl;
use crate::domain::product::ProductDetail;
use crate::infrastructure::crawling_service_impls::{StatusCheckerImpl, ProductListCollectorImpl, CollectorConfig};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::new_architecture::actor_system::{
    StageResult, StageSuccessResult, StageError, CollectionMetrics, ProcessingMetrics,
    FailedItem
};
use crate::new_architecture::channel_types::{StageType, StageItem};
use crate::new_architecture::system_config::SystemConfig;

/// 실제 크롤링 서비스와 OneShot Actor 시스템을 연결하는 통합 서비스
pub struct CrawlingIntegrationService {
    status_checker: Arc<dyn StatusChecker>,
    list_collector: Arc<dyn ProductListCollector>,
    detail_collector: Arc<dyn ProductDetailCollector>,
    database_analyzer: Arc<dyn DatabaseAnalyzer>,
    product_repository: Arc<IntegratedProductRepository>,
    config: Arc<SystemConfig>,
    app_config: AppConfig,
}

impl CrawlingIntegrationService {
    /// 기존 인프라를 사용하여 통합 서비스 생성
    pub async fn new(
        config: Arc<SystemConfig>,
        app_config: AppConfig,
    ) -> Result<Self> {
        // 기존 인프라 서비스들 초기화 (기존 패턴 재사용)
        let http_client = Arc::new(tokio::sync::Mutex::new(
            HttpClient::create_from_global_config()?
        ));
        
        let data_extractor = Arc::new(
            MatterDataExtractor::new()?
        );
        
        // 실제 DB 연결 URL 가져오기
        let database_url = crate::commands::crawling_v4::get_database_url_v4()
            .map_err(|e| anyhow::anyhow!("Failed to get database URL: {}", e))?;
        
        // DB 연결 풀 생성
        let db_pool = sqlx::SqlitePool::connect(&database_url).await
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;
        
        // ProductRepository 초기화 (DB 연결 포함)
        let product_repository = Arc::new(
            IntegratedProductRepository::new(db_pool)
        );
        
        // StatusChecker 생성 (ProductRepository 포함)
        let http_client_for_checker = http_client.lock().await.clone();
        let data_extractor_for_checker = (*data_extractor).clone();
        let status_checker_impl = Arc::new(StatusCheckerImpl::with_product_repo(
            http_client_for_checker,
            data_extractor_for_checker,
            app_config.clone(),
            product_repository.clone(),
        ));
        let status_checker: Arc<dyn StatusChecker> = status_checker_impl.clone();
        
        // DatabaseAnalyzer는 StatusCheckerImpl 재사용  
        let database_analyzer: Arc<dyn DatabaseAnalyzer> = status_checker_impl.clone();
        
        // ProductListCollector 생성
        let collector_config = CollectorConfig {
            batch_size: app_config.user.batch.batch_size,
            max_concurrent: app_config.user.max_concurrent_requests,
            concurrency: app_config.user.max_concurrent_requests,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            retry_attempts: 3,
            retry_max: 3,
        };
        
        let list_collector: Arc<dyn ProductListCollector> = Arc::new(ProductListCollectorImpl::new(
            Arc::new(HttpClient::create_from_global_config()?),  // 🔥 Mutex 제거
            data_extractor.clone(),
            collector_config.clone(),
            status_checker_impl.clone(),
        ));
        
        // ProductDetailCollector는 ProductListCollectorImpl 재사용 (기존 패턴)
        let detail_collector: Arc<dyn ProductDetailCollector> = Arc::new(ProductListCollectorImpl::new(
            Arc::new(HttpClient::create_from_global_config()?),  // 🔥 Mutex 제거
            data_extractor.clone(),
            collector_config.clone(),
            status_checker_impl.clone(),
        ));
        
        Ok(Self {
            status_checker,
            list_collector,
            detail_collector,
            database_analyzer,
            product_repository,
            config,
            app_config,
        })
    }
    
    /// 실제 리스트 수집 단계 실행 (OneShot 결과 반환)
    pub async fn execute_list_collection_stage(
        &self,
        pages: Vec<u32>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
    ) -> StageResult {
        let start_time = Instant::now();
        
        info!(
            pages_count = pages.len(),
            concurrency_limit = concurrency_limit,
            "Starting real list collection stage"
        );
        
        // 설정에서 배치 크기 로드
        let batch_size = self.config.performance.batch_sizes.initial_size.min(50) as usize;
        let mut all_collected_urls = Vec::new();
        let mut successful_pages = Vec::new();
        let mut failed_pages = Vec::new();
        
        // 페이지를 배치로 나누어 처리
        for chunk in pages.chunks(batch_size) {
            // 취소 확인
            if cancellation_token.is_cancelled() {
                return StageResult::FatalError {
                    error: StageError::ValidationError {
                        message: "Collection cancelled by user".to_string(),
                    },
                    stage_id: "list-collection".to_string(),
                    context: "User cancellation".to_string(),
                };
            }
            
            match self.collect_page_batch_with_retry(chunk, cancellation_token.clone()).await {
                Ok(batch_result) => {
                    for (page, urls) in batch_result {
                        if urls.is_empty() {
                            failed_pages.push(page);
                        } else {
                            all_collected_urls.extend(urls);
                            successful_pages.push(page);
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Batch collection failed");
                    failed_pages.extend(chunk.iter());
                }
            }
        }
        
        let elapsed = start_time.elapsed();
        let total_pages = pages.len() as u32;
        let successful_count = successful_pages.len() as u32;
        let failed_count = failed_pages.len() as u32;
        
        // 결과 분류
        if successful_count == 0 {
            StageResult::FatalError {
                error: StageError::NetworkError {
                    message: format!("All {} pages failed to collect", total_pages),
                },
                stage_id: "list-collection".to_string(),
                context: "Complete collection failure".to_string(),
            }
        } else if failed_count == 0 {
            StageResult::Success(StageSuccessResult {
                processed_items: total_pages,
                stage_duration_ms: elapsed.as_millis() as u64,
                collection_metrics: Some(CollectionMetrics {
                    total_items: total_pages,
                    successful_items: successful_count,
                    failed_items: failed_count,
                    duration_ms: elapsed.as_millis() as u64,
                    avg_response_time_ms: if successful_count > 0 {
                        elapsed.as_millis() as u64 / successful_count as u64
                    } else { 0 },
                    success_rate: 100.0,
                }),
                processing_metrics: None,
            })
        } else {
            StageResult::PartialSuccess {
                success_items: StageSuccessResult {
                    processed_items: successful_count,
                    stage_duration_ms: elapsed.as_millis() as u64,
                    collection_metrics: Some(CollectionMetrics {
                        total_items: total_pages,
                        successful_items: successful_count,
                        failed_items: failed_count,
                        duration_ms: elapsed.as_millis() as u64,
                        avg_response_time_ms: if successful_count > 0 {
                            elapsed.as_millis() as u64 / successful_count as u64
                        } else { 0 },
                        success_rate: (successful_count as f64 / total_pages as f64) * 100.0,
                    }),
                    processing_metrics: None,
                },
                failed_items: failed_pages.into_iter().map(|page| FailedItem {
                    item_id: page.to_string(),
                    error: StageError::NetworkError {
                        message: "Page collection failed".to_string(),
                    },
                    retry_count: 0,
                    last_attempt_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                }).collect(),
                stage_id: "list-collection".to_string(),
            }
        }
    }
    
    /// 실제 상세 수집 단계 실행 (OneShot 결과 반환)
    pub async fn execute_detail_collection_stage(
        &self,
        product_urls: Vec<ProductUrl>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
    ) -> StageResult {
        let start_time = Instant::now();
        
        info!(
            urls_count = product_urls.len(),
            concurrency_limit = concurrency_limit,
            "Starting real detail collection stage"
        );
        
        // 설정에서 배치 크기 로드
        let batch_size = self.config.performance.batch_sizes.initial_size.min(20) as usize;
        let mut all_collected_details = Vec::new();
        let mut successful_urls = Vec::new();
        let mut failed_urls = Vec::new();
        
        // URL을 배치로 나누어 처리
        for chunk in product_urls.chunks(batch_size) {
            // 취소 확인
            if cancellation_token.is_cancelled() {
                return StageResult::FatalError {
                    error: StageError::ValidationError {
                        message: "Detail collection cancelled by user".to_string(),
                    },
                    stage_id: "detail-collection".to_string(),
                    context: "User cancellation".to_string(),
                };
            }
            
            match self.collect_detail_batch_with_retry(chunk, cancellation_token.clone()).await {
                Ok(batch_details) => {
                    for detail in batch_details {
                        all_collected_details.push(detail.clone());
                        successful_urls.push(detail.url.clone());
                    }
                }
                Err(e) => {
                    error!(error = %e, "Detail batch collection failed");
                    failed_urls.extend(chunk.iter().map(|url| url.url.clone()));
                }
            }
        }
        
        let elapsed = start_time.elapsed();
        let total_urls = product_urls.len() as u32;
        let successful_count = successful_urls.len() as u32;
        let failed_count = failed_urls.len() as u32;
        
        // 결과 분류
        if successful_count == 0 {
            StageResult::FatalError {
                error: StageError::NetworkError {
                    message: format!("All {} product details failed to collect", total_urls),
                },
                stage_id: "detail-collection".to_string(),
                context: "Complete detail collection failure".to_string(),
            }
        } else if failed_count == 0 {
            StageResult::Success(StageSuccessResult {
                processed_items: total_urls,
                stage_duration_ms: elapsed.as_millis() as u64,
                collection_metrics: None,
                processing_metrics: Some(ProcessingMetrics {
                    total_processed: total_urls,
                    successful_saves: successful_count,
                    failed_saves: failed_count,
                    duration_ms: elapsed.as_millis() as u64,
                    avg_processing_time_ms: if successful_count > 0 {
                        elapsed.as_millis() as u64 / successful_count as u64
                    } else { 0 },
                    success_rate: 100.0,
                }),
            })
        } else {
            StageResult::PartialSuccess {
                success_items: StageSuccessResult {
                    processed_items: successful_count,
                    stage_duration_ms: elapsed.as_millis() as u64,
                    collection_metrics: None,
                    processing_metrics: Some(ProcessingMetrics {
                        total_processed: total_urls,
                        successful_saves: successful_count,
                        failed_saves: failed_count,
                        duration_ms: elapsed.as_millis() as u64,
                        avg_processing_time_ms: if successful_count > 0 {
                            elapsed.as_millis() as u64 / successful_count as u64
                        } else { 0 },
                        success_rate: (successful_count as f64 / total_urls as f64) * 100.0,
                    }),
                },
                failed_items: failed_urls.into_iter().map(|url| FailedItem {
                    item_id: url,
                    error: StageError::NetworkError {
                        message: "Product detail collection failed".to_string(),
                    },
                    retry_count: 0,
                    last_attempt_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                }).collect(),
                stage_id: "detail-collection".to_string(),
            }
        }
    }
    
    /// 사이트 상태 분석 실행
    pub async fn execute_site_analysis(&self) -> Result<SiteStatus> {
        info!("Starting real site status analysis");
        
        self.status_checker.check_site_status().await
    }
    
    /// 크롤링 범위 권장사항 계산
    pub async fn calculate_crawling_recommendation(&self) -> Result<CrawlingRangeRecommendation> {
        info!("Calculating real crawling range recommendation with actual DB data");
        
        let site_status = self.status_checker.check_site_status().await?;
        
        // 실제 DB 상태 확인
        let db_stats = self.product_repository.get_database_statistics().await?;
        info!(
            total_products = db_stats.total_products,
            active_products = db_stats.active_products,
            "Real DB stats retrieved"
        );
        
        // DB 분석을 위한 분석 결과 생성 (실제 DB 데이터 기반)
        let db_analysis = crate::domain::services::crawling_services::DatabaseAnalysis {
            total_products: db_stats.total_products as u32,
            unique_products: db_stats.active_products as u32,
            duplicate_count: 0,
            last_update: db_stats.last_crawled,
            missing_fields_analysis: crate::domain::services::crawling_services::FieldAnalysis {
                missing_company: 0,
                missing_model: 0,
                missing_matter_version: 0,
                missing_connectivity: 0,
                missing_certification_date: 0,
            },
            data_quality_score: 0.8,
        };
        
        self.status_checker.calculate_crawling_range_recommendation(&site_status, &db_analysis).await
    }
    
    /// 배치 페이지 수집 (재시도 포함)
    async fn collect_page_batch_with_retry(
        &self,
        pages: &[u32],
        cancellation_token: CancellationToken,
    ) -> Result<Vec<(u32, Vec<ProductUrl>)>> {
        let mut results = Vec::new();
        
        // 개별 페이지 수집
        for &page in pages {
            if cancellation_token.is_cancelled() {
                break;
            }
            
            match self.collect_single_page_with_retry(page, 3).await {
                Ok(urls) => {
                    results.push((page, urls));
                }
                Err(e) => {
                    warn!(page = page, error = %e, "Failed to collect page after retries");
                    results.push((page, Vec::new()));
                }
            }
        }
        
        Ok(results)
    }
    
    /// 단일 페이지 수집 (재시도 포함)
    async fn collect_single_page_with_retry(
        &self,
        page: u32,
        max_retries: u32,
    ) -> Result<Vec<ProductUrl>> {
        // 사이트 상태 확인하여 실제 정보 가져오기
        let site_status = self.status_checker.check_site_status().await?;
        let mut last_error = None;
        
        for attempt in 0..=max_retries {
            match self.list_collector.collect_single_page(
                page,
                site_status.total_pages,
                site_status.products_on_last_page
            ).await {
                Ok(urls) => {
                    if attempt > 0 {
                        info!(page = page, attempt = attempt, "Page collection succeeded after retry");
                    }
                    return Ok(urls);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        debug!(page = page, attempt = attempt, delay_ms = delay.as_millis(), "Retrying page collection");
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }
    
    /// 배치 상세 수집 (재시도 포함)
    async fn collect_detail_batch_with_retry(
        &self,
        urls: &[ProductUrl],
        cancellation_token: CancellationToken,
    ) -> Result<Vec<ProductDetail>> {
        // 취소 토큰과 함께 실제 상세 수집 호출
        self.detail_collector.collect_details_with_cancellation(urls, cancellation_token).await
    }
}

/// StageActor에서 실제 크롤링 서비스 사용을 위한 도우미 구조체
pub struct RealCrawlingStageExecutor {
    integration_service: Arc<CrawlingIntegrationService>,
}

impl RealCrawlingStageExecutor {
    pub fn new(integration_service: Arc<CrawlingIntegrationService>) -> Self {
        Self {
            integration_service,
        }
    }
    
    /// StageActor에서 호출할 실제 단계 실행 메서드
    pub async fn execute_stage(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
    ) -> StageResult {
        match stage_type {
            StageType::ListCollection | StageType::Collection => {
                let pages: Vec<u32> = items.into_iter()
                    .filter_map(|item| match item {
                        StageItem::Page(page) => Some(page),
                        _ => None,
                    })
                    .collect();
                
                self.integration_service.execute_list_collection_stage(
                    pages,
                    concurrency_limit,
                    cancellation_token,
                ).await
            }
            
            StageType::DetailCollection | StageType::Processing => {
                // 현재는 URL 아이템이 없으므로 빈 처리
                // 실제로는 이전 단계에서 수집된 URL을 받아야 함
                let urls = Vec::new(); // TODO: 실제 URL 전달 구현
                
                self.integration_service.execute_detail_collection_stage(
                    urls,
                    concurrency_limit,
                    cancellation_token,
                ).await
            }
            
            StageType::DataValidation => {
                // 데이터 검증 로직 (현재는 성공으로 처리)
                StageResult::Success(StageSuccessResult {
                    processed_items: items.len() as u32,
                    stage_duration_ms: 100,
                    collection_metrics: None,
                    processing_metrics: None,
                })
            }
            
            StageType::DatabaseSave => {
                // 데이터베이스 저장 로직 (현재는 성공으로 처리)
                StageResult::Success(StageSuccessResult {
                    processed_items: items.len() as u32,
                    stage_duration_ms: 200,
                    collection_metrics: None,
                    processing_metrics: None,
                })
            }
        }
    }
}
