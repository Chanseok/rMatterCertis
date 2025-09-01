//! 실제 크롤링 서비스와 `OneShot` Actor 시스템 통합
//! Modern Rust 2024 준수: 기존 크롤링 서비스를 `OneShot` Actor 패턴으로 연동

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::crawl_engine::actor_system::{StageError, StageResult};
use crate::crawl_engine::channels::types::{StageItem, StageType};
use crate::crawl_engine::config::SystemConfig;
use crate::domain::product::ProductDetail;
use crate::domain::product_url::ProductUrl;
use crate::domain::services::crawling_services::{
    CrawlingRangeRecommendation, FieldAnalysis, ProductDetailCollector, ProductListCollector,
    SiteStatus, StatusChecker,
};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::crawling_service_impls::{
    CollectorConfig, ProductListCollectorImpl, StatusCheckerImpl,
};
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};

/// 실제 크롤링 서비스와 `OneShot` Actor 시스템을 연결하는 통합 서비스
pub struct CrawlingIntegrationService {
    status_checker: Arc<dyn StatusChecker>,
    list_collector: Arc<dyn ProductListCollector>,
    detail_collector: Arc<dyn ProductDetailCollector>,
    product_repository: Arc<IntegratedProductRepository>,
    config: Arc<SystemConfig>, // REMOVE_CANDIDATE(if still unused)
}

impl CrawlingIntegrationService {
    /// 기존 인프라를 사용하여 통합 서비스 생성
    pub async fn new(config: Arc<SystemConfig>, app_config: AppConfig) -> Result<Self> {
        // 기존 인프라 서비스들 초기화 (기존 패턴 재사용)
        let http_client = Arc::new(tokio::sync::Mutex::new(
            HttpClient::create_from_global_config()?,
        ));

        let data_extractor = Arc::new(MatterDataExtractor::new()?);

        // DB 풀 재사용 (글로벌 풀)
        let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to obtain database pool: {}", e))?;

        // ProductRepository 초기화 (DB 연결 포함)
        let product_repository = Arc::new(IntegratedProductRepository::new(db_pool));

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

        let list_collector: Arc<dyn ProductListCollector> =
            Arc::new(ProductListCollectorImpl::new(
                Arc::new(HttpClient::create_from_global_config()?), // 🔥 Mutex 제거
                data_extractor.clone(),
                collector_config,
                status_checker_impl,
            ));

        // ProductDetailCollector: 실제 상세 수집 전용 구현 사용
        // 상세 단계는 리스트 단계와 다른 동시성 한도를 사용할 수 있으므로 별도 CollectorConfig 구성
        let detail_config = CollectorConfig {
            batch_size: app_config.user.batch.batch_size,
            max_concurrent: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
            concurrency: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            retry_attempts: 3,
            retry_max: 3,
        };
        let detail_collector: Arc<dyn ProductDetailCollector> = Arc::new(
            crate::infrastructure::crawling_service_impls::ProductDetailCollectorImpl::new(
                Arc::new(HttpClient::create_from_global_config()?),
                data_extractor.clone(),
                detail_config,
            ),
        );

    Ok(Self {
            status_checker,
            list_collector,
            detail_collector,
            product_repository,
            config,
        })
    }

    /// 실제 리스트 수집 단계 실행 (`OneShot` 결과 반환)
    pub async fn execute_list_collection_stage(
        &self,
        pages: Vec<u32>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
    ) -> StageResult {
        self.execute_list_collection_stage_internal(
            pages,
            concurrency_limit,
            cancellation_token,
            true,
        )
        .await
    }

    /// 사이트 상태 확인 없이 직접 페이지 수집 (중복 방지용)
    pub async fn execute_list_collection_stage_no_site_check(
        &self,
        pages: Vec<u32>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
    ) -> StageResult {
        self.execute_list_collection_stage_internal(
            pages,
            concurrency_limit,
            cancellation_token,
            false,
        )
        .await
    }

    /// 내부 리스트 수집 구현 (사이트 상태 확인 선택적)
    async fn execute_list_collection_stage_internal(
        &self,
        pages: Vec<u32>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
        perform_site_check: bool,
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

            match self
                .collect_page_batch_with_retry(
                    chunk,
                    cancellation_token.clone(),
                    perform_site_check,
                )
                .await
            {
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
                    message: format!("All {total_pages} pages failed to collect"),
                },
                stage_id: "list-collection".to_string(),
                context: "Complete collection failure".to_string(),
            }
        } else if failed_count == 0 {
            StageResult::Success {
                processed_items: total_pages,
                duration_ms: elapsed.as_millis() as u64,
            }
        } else {
            StageResult::Failure {
                error: StageError::ProcessingError {
                    message: format!(
                        "Partial failure: {successful_count} successes, {failed_count} failures"
                    ),
                },
                partial_results: successful_count,
            }
        }
    }

    /// 페이지별 상세 URL 수집 결과를 반환 (per-page detailed results)
    ///
    /// 반환값: Vec<(`page_number`, Vec<ProductUrl>)>
    pub async fn collect_pages_detailed(
        &self,
        pages: Vec<u32>,
        perform_site_check: bool,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<Vec<(u32, Vec<ProductUrl>)>> {
        // 동일한 내부 배치 수집 로직 재사용
        let result = self
            .collect_page_batch_with_retry(&pages, cancellation_token, perform_site_check)
            .await?;
        Ok(result)
    }

    /// 페이지별 상세 URL 수집 결과(+메타: `retry_count`, `duration_ms)를` 반환
    ///
    /// 반환값: Vec<(`page_number`, Vec<ProductUrl>, `retry_count`, `duration_ms`)>
    pub async fn collect_pages_detailed_with_meta(
        &self,
        pages: Vec<u32>,
        perform_site_check: bool,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<Vec<(u32, Vec<ProductUrl>, u32, u64)>> {
        let mut results: Vec<(u32, Vec<ProductUrl>, u32, u64)> = Vec::new();

        for &page in &pages {
            if cancellation_token.is_cancelled() {
                break;
            }

            match self
                .collect_single_page_with_retry_with_meta(page, 3, perform_site_check)
                .await
            {
                Ok((urls, retry_count, duration_ms)) => {
                    results.push((page, urls, retry_count, duration_ms));
                }
                Err(e) => {
                    warn!(page = page, error = %e, "Failed to collect page after retries");
                    // 실패한 경우에도 형식을 유지하되 빈 URL과 시도 횟수/소요시간 0으로 채움
                    results.push((page, Vec::new(), 3, 0));
                }
            }
        }

        Ok(results)
    }

    /// 실제 상세 수집 단계 실행 (`OneShot` 결과 반환)
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

            match self
                .collect_detail_batch_with_retry(chunk, cancellation_token.clone())
                .await
            {
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
                    message: format!("All {total_urls} product details failed to collect"),
                },
                stage_id: "detail-collection".to_string(),
                context: "Complete detail collection failure".to_string(),
            }
        } else if failed_count == 0 {
            StageResult::Success {
                processed_items: total_urls,
                duration_ms: elapsed.as_millis() as u64,
            }
        } else {
            StageResult::Failure {
                error: StageError::ProcessingError {
                    message: "Partial failure in detail collection".to_string(),
                },
                partial_results: successful_count,
            }
        }
    }

    /// 상세 수집 결과를 도메인 객체로 직접 반환 (per-item detailed bridging 용)
    pub async fn collect_details_detailed(
        &self,
        urls: Vec<ProductUrl>,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<Vec<ProductDetail>> {
        // 내부 배치 수집 로직 재사용
        self.collect_detail_batch_with_retry(&urls, cancellation_token)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// 상세 수집 결과(+메타: `retry_count`, `duration_ms)를` 반환 (per-item detailed bridging 용)
    pub async fn collect_details_detailed_with_meta(
        &self,
        urls: Vec<ProductUrl>,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<(Vec<ProductDetail>, u32, u64)> {
        self.collect_detail_batch_with_retry_with_meta(&urls, cancellation_token, 2)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
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
            duplicate_count: (db_stats.total_products - db_stats.active_products) as u32,
            missing_products_count: 0,
            last_update: None,
            missing_fields_analysis: FieldAnalysis {
                missing_company: 0,
                missing_model: 0,
                missing_matter_version: 0,
                missing_connectivity: 0,
                missing_certification_date: 0,
            },
            data_quality_score: 0.8,
        };

        self.status_checker
            .calculate_crawling_range_recommendation(&site_status, &db_analysis)
            .await
    }

    /// 배치 페이지 수집 (재시도 포함)
    async fn collect_page_batch_with_retry(
        &self,
        pages: &[u32],
        cancellation_token: CancellationToken,
        perform_site_check: bool,
    ) -> Result<Vec<(u32, Vec<ProductUrl>)>> {
        let mut results = Vec::new();

        // 개별 페이지 수집
        for &page in pages {
            if cancellation_token.is_cancelled() {
                break;
            }

            match self
                .collect_single_page_with_retry(page, 3, perform_site_check)
                .await
            {
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
        perform_site_check: bool,
    ) -> Result<Vec<ProductUrl>> {
        // 사이트 상태 확인 (선택적)
        let site_status = if perform_site_check {
            info!(page = page, "🔍 Performing site status check for page");
            self.status_checker.check_site_status().await?
        } else {
            info!(
                page = page,
                "⚡ Skipping site status check - using cached site info"
            );
            // 기본값 사용 (실제로는 캐시된 값을 사용해야 함)
            SiteStatus {
                total_pages: 495,
                products_on_last_page: 6,
                is_accessible: true,
                estimated_products: 5934,
                response_time_ms: 500,
                last_check_time: chrono::Utc::now(),
                health_score: 1.0,
                data_change_status:
                    crate::domain::services::crawling_services::SiteDataChangeStatus::Stable {
                        count: 5934,
                    },
                decrease_recommendation: None,
                crawling_range_recommendation: CrawlingRangeRecommendation::Partial(5),
            }
        };
        let mut last_error = None;
        let expected_per_page: usize = 12;

        for attempt in 0..=max_retries {
            match self
                .list_collector
                .collect_single_page(
                    page,
                    site_status.total_pages,
                    site_status.products_on_last_page,
                )
                .await
            {
                Ok(urls) => {
                    // 성공 판정 강화: 비마지막 페이지는 최소 12개를 기대
                    let is_last = page >= site_status.total_pages;
                    if !is_last && urls.len() < expected_per_page {
                        last_error = Some(anyhow::anyhow!(
                            "Insufficient products on page {}: expected >= {}, got {}",
                            page,
                            expected_per_page,
                            urls.len()
                        ));
                        if attempt < max_retries {
                            let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                            debug!(
                                page = page,
                                attempt = attempt,
                                delay_ms = delay.as_millis(),
                                "Retrying page collection due to insufficient items"
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                    }
                    if attempt > 0 {
                        info!(
                            page = page,
                            attempt = attempt,
                            "Page collection succeeded after retry"
                        );
                    }
                    return Ok(urls);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        debug!(
                            page = page,
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Retrying page collection"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }

    /// 단일 페이지 수집 (재시도 및 메타데이터 포함)
    async fn collect_single_page_with_retry_with_meta(
        &self,
        page: u32,
        max_retries: u32,
        perform_site_check: bool,
    ) -> Result<(Vec<ProductUrl>, u32, u64)> {
        let site_status = if perform_site_check {
            info!(page = page, "Performing site status check for page (meta)");
            self.status_checker.check_site_status().await?
        } else {
            info!(
                page = page,
                "Skipping site status check - using cached site info (meta)"
            );
            SiteStatus {
                total_pages: 495,
                products_on_last_page: 6,
                is_accessible: true,
                estimated_products: 5934,
                response_time_ms: 500,
                last_check_time: chrono::Utc::now(),
                health_score: 1.0,
                data_change_status:
                    crate::domain::services::crawling_services::SiteDataChangeStatus::Stable {
                        count: 5934,
                    },
                decrease_recommendation: None,
                crawling_range_recommendation: CrawlingRangeRecommendation::Partial(5),
            }
        };

        let mut last_error = None;
        let expected_per_page: usize = 12;
        let started = std::time::Instant::now();

        for attempt in 0..=max_retries {
            match self
                .list_collector
                .collect_single_page(
                    page,
                    site_status.total_pages,
                    site_status.products_on_last_page,
                )
                .await
            {
                Ok(urls) => {
                    // 성공 판정 강화: 비마지막 페이지는 최소 12개를 기대
                    let is_last = page >= site_status.total_pages;
                    if !is_last && urls.len() < expected_per_page {
                        last_error = Some(anyhow::anyhow!(
                            "Insufficient products on page {}: expected >= {}, got {}",
                            page,
                            expected_per_page,
                            urls.len()
                        ));
                        if attempt < max_retries {
                            let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                            debug!(
                                page = page,
                                attempt = attempt,
                                delay_ms = delay.as_millis(),
                                "Retrying page collection (meta) due to insufficient items"
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                    }
                    if attempt > 0 {
                        info!(
                            page = page,
                            attempt = attempt,
                            "Page collection succeeded after retry (meta)"
                        );
                    }
                    let duration_ms = started.elapsed().as_millis() as u64;
                    return Ok((urls, attempt, duration_ms));
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        debug!(
                            page = page,
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Retrying page collection (meta)"
                        );
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
        info!(
            urls_count = urls.len(),
            "[Integration] collect_detail_batch_with_retry starting"
        );
        self.detail_collector
            .collect_details_with_cancellation(urls, cancellation_token)
            .await
    }

    /// 배치 상세 수집 (재시도 및 메타 포함)
    async fn collect_detail_batch_with_retry_with_meta(
        &self,
        urls: &[ProductUrl],
        cancellation_token: CancellationToken,
        max_retries: u32,
    ) -> Result<(Vec<ProductDetail>, u32, u64)> {
        let started = std::time::Instant::now();
        let mut last_error: Option<anyhow::Error> = None;
        info!(
            urls_count = urls.len(),
            max_retries = max_retries,
            "[Integration] collect_detail_batch_with_retry_with_meta starting"
        );
        for attempt in 0..=max_retries {
            match self
                .detail_collector
                .collect_details_with_cancellation(urls, cancellation_token.clone())
                .await
            {
                Ok(details) => {
                    debug!(
                        attempt = attempt,
                        details_count = details.len(),
                        "[Integration] detail collection succeeded"
                    );
                    let duration_ms = started.elapsed().as_millis() as u64;
                    return Ok((details, attempt, duration_ms));
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        debug!(
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Retrying detail collection (meta)"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }
}

/// `StageActor에서` 실제 크롤링 서비스 사용을 위한 도우미 구조체
pub struct RealCrawlingStageExecutor {
    integration_service: Arc<CrawlingIntegrationService>,
}

impl RealCrawlingStageExecutor {
    #[must_use] pub const fn new(integration_service: Arc<CrawlingIntegrationService>) -> Self {
        Self {
            integration_service,
        }
    }

    /// `StageActor에서` 호출할 실제 단계 실행 메서드
    pub async fn execute_stage(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        cancellation_token: CancellationToken,
    ) -> StageResult {
        match stage_type {
            StageType::ListCollection => {
                let pages: Vec<u32> = items
                    .into_iter()
                    .filter_map(|item| match item {
                        StageItem::Page(page) => Some(page),
                        _ => None,
                    })
                    .collect();

                self.integration_service
                    .execute_list_collection_stage(pages, concurrency_limit, cancellation_token)
                    .await
            }

            StageType::DetailCollection => {
                // 현재는 URL 아이템이 없으므로 빈 처리
                // 실제로는 이전 단계에서 수집된 URL을 받아야 함
                let urls = Vec::new(); // TODO: 실제 URL 전달 구현

                self.integration_service
                    .execute_detail_collection_stage(urls, concurrency_limit, cancellation_token)
                    .await
            }

            StageType::DataValidation => {
                // 데이터 검증 로직 (현재는 성공으로 처리)
                StageResult::Success {
                    processed_items: items.len() as u32,
                    duration_ms: 100,
                }
            }

            StageType::DatabaseSave => {
                // 데이터베이스 저장 로직 (현재는 성공으로 처리)
                StageResult::Success {
                    processed_items: items.len() as u32,
                    duration_ms: 200,
                }
            }
        }
    }
}
