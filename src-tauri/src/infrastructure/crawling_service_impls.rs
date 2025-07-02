//! 크롤링 서비스 구현체들
//! 
//! domain/services/crawling_services.rs의 트레이트들에 대한 실제 구현체

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};
use tokio::sync::Semaphore;
use tokio::time::sleep;
use futures::future::try_join_all;

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    SiteStatus, DatabaseAnalysis, FieldAnalysis, DuplicateAnalysis, ProcessingStrategy
};
use crate::domain::product::Product;
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository, csa_iot};

/// 사이트 상태 체크 서비스 구현체
pub struct StatusCheckerImpl {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
}

impl StatusCheckerImpl {
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
    ) -> Self {
        Self {
            http_client: Arc::new(tokio::sync::Mutex::new(http_client)),
            data_extractor: Arc::new(data_extractor),
        }
    }
}

#[async_trait]
impl StatusChecker for StatusCheckerImpl {
    async fn check_site_status(&self) -> Result<SiteStatus> {
        let start_time = Instant::now();
        info!("Checking site status...");

        let url = format!("{}?page=1", csa_iot::PRODUCTS_PAGE_MATTER_ONLY);
        
        let mut client = self.http_client.lock().await;
        let html = match client.fetch_html_string(&url).await {
            Ok(html) => html,
            Err(e) => {
                error!("Failed to access site: {}", e);
                return Ok(SiteStatus {
                    is_accessible: false,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    total_pages: 0,
                    estimated_products: 0,
                    last_check_time: chrono::Utc::now(),
                    health_score: 0.0,
                });
            }
        };

        let response_time = start_time.elapsed().as_millis() as u64;
        
        // 총 페이지 수 추출
        let total_pages = match self.data_extractor.extract_total_pages(&html) {
            Ok(pages) => pages,
            Err(_) => {
                warn!("Could not extract total pages from site");
                50 // 기본값
            }
        };

        // 건강도 점수 계산 (응답시간, 접근성 기반)
        let health_score = if response_time < 3000 { 1.0 }
        else if response_time < 10000 { 0.7 }
        else { 0.3 };

        info!("Site status check completed: {} pages, {}ms response time", total_pages, response_time);

        Ok(SiteStatus {
            is_accessible: true,
            response_time_ms: response_time,
            total_pages,
            estimated_products: total_pages * 20, // 페이지당 약 20개 제품 추정
            last_check_time: chrono::Utc::now(),
            health_score,
        })
    }

    async fn estimate_crawling_time(&self, pages: u32) -> Duration {
        // 페이지당 평균 2초 + 제품 상세페이지당 1초 추정
        let page_collection_time = pages * 2;
        let product_detail_time = pages * 20; // 페이지당 20개 제품 * 1초
        let total_seconds = page_collection_time + product_detail_time;
        
        Duration::from_secs(total_seconds as u64)
    }

    async fn verify_site_accessibility(&self) -> Result<bool> {
        let status = self.check_site_status().await?;
        Ok(status.is_accessible && status.health_score > 0.5)
    }
}

/// 데이터베이스 분석 서비스 구현체
pub struct DatabaseAnalyzerImpl {
    product_repo: Arc<IntegratedProductRepository>,
}

impl DatabaseAnalyzerImpl {
    pub fn new(product_repo: Arc<IntegratedProductRepository>) -> Self {
        Self { product_repo }
    }
}

#[async_trait]
impl DatabaseAnalyzer for DatabaseAnalyzerImpl {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis> {
        info!("Analyzing database state...");

        let statistics = self.product_repo.get_database_statistics().await?;
        let total_products = statistics.total_products as u32;
        
        // 중복 분석 (간단한 버전)
        let duplicate_count = 0; // TODO: 실제 중복 검사 로직 구현
        let unique_products = total_products - duplicate_count;

        // 필드 누락 분석
        let missing_fields = FieldAnalysis {
            missing_company: 0,      // TODO: 실제 누락 필드 분석
            missing_model: 0,
            missing_matter_version: 0,
            missing_connectivity: 0,
            missing_certification_date: 0,
        };

        // 데이터 품질 점수 계산
        let data_quality_score = if total_products > 0 {
            (unique_products as f64 / total_products as f64) * 0.8 + 0.2
        } else {
            1.0
        };

        info!("Database analysis completed: {} total, {} unique products", total_products, unique_products);

        Ok(DatabaseAnalysis {
            total_products,
            unique_products,
            duplicate_count,
            last_update: None, // TODO: 마지막 업데이트 시간 추적
            missing_fields_analysis: missing_fields,
            data_quality_score,
        })
    }

    async fn recommend_processing_strategy(&self) -> Result<ProcessingStrategy> {
        let analysis = self.analyze_current_state().await?;
        
        // 데이터베이스 크기에 따른 전략 조정
        let (batch_size, concurrency) = if analysis.total_products < 1000 {
            (20, 5)
        } else if analysis.total_products < 5000 {
            (15, 3)
        } else {
            (10, 2)
        };

        Ok(ProcessingStrategy {
            recommended_batch_size: batch_size,
            recommended_concurrency: concurrency,
            should_skip_duplicates: analysis.duplicate_count > 0,
            should_update_existing: analysis.data_quality_score < 0.8,
            priority_urls: Vec::new(), // TODO: 우선순위 URL 로직
        })
    }

    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis> {
        // TODO: 실제 중복 분석 로직 구현
        Ok(DuplicateAnalysis {
            total_duplicates: 0,
            duplicate_groups: Vec::new(),
            duplicate_percentage: 0.0,
        })
    }
}

/// 제품 목록 수집 서비스 구현체
pub struct ProductListCollectorImpl {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
    config: CollectorConfig,
}

#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub concurrency: u32,
    pub delay_ms: u64,
    pub batch_size: u32,
    pub retry_max: u32,
}

impl ProductListCollectorImpl {
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        config: CollectorConfig,
    ) -> Self {
        Self {
            http_client: Arc::new(tokio::sync::Mutex::new(http_client)),
            data_extractor: Arc::new(data_extractor),
            config,
        }
    }
}

#[async_trait]
impl ProductListCollector for ProductListCollectorImpl {
    async fn collect_all_pages(&self, total_pages: u32) -> Result<Vec<String>> {
        info!("Collecting product URLs from {} pages", total_pages);

        let _semaphore = Arc::new(Semaphore::new(self.config.concurrency as usize));
        let mut all_product_urls = Vec::new();

        // 배치별로 페이지 처리
        for batch_start in (1..=total_pages).step_by(self.config.batch_size as usize) {
            let batch_end = (batch_start + self.config.batch_size - 1).min(total_pages);
            let batch_pages: Vec<u32> = (batch_start..=batch_end).collect();
            
            let batch_urls = self.collect_page_batch(&batch_pages).await?;
            all_product_urls.extend(batch_urls);

            debug!("Completed batch {}-{}, total URLs: {}", batch_start, batch_end, all_product_urls.len());
        }

        info!("Product URL collection completed: {} URLs collected", all_product_urls.len());
        Ok(all_product_urls)
    }

    async fn collect_single_page(&self, page: u32) -> Result<Vec<String>> {
        let url = format!("{}?page={}", csa_iot::PRODUCTS_PAGE_MATTER_ONLY, page);
        debug!("Fetching page: {}", url);

        if self.config.delay_ms > 0 {
            sleep(Duration::from_millis(self.config.delay_ms)).await;
        }

        let mut client = self.http_client.lock().await;
        let html_str = client.fetch_html_string(&url).await?;
        
        let urls = self.data_extractor.extract_product_urls_from_content(&html_str)
            .map_err(|e| anyhow!("Failed to extract URLs from page {}: {}", page, e))?;

        debug!("Extracted {} URLs from page {}", urls.len(), page);
        Ok(urls)
    }

    async fn collect_page_batch(&self, pages: &[u32]) -> Result<Vec<String>> {
        let semaphore = Arc::new(Semaphore::new(self.config.concurrency as usize));
        
        let batch_tasks: Vec<_> = pages.iter().map(|&page_num| {
            let semaphore = Arc::clone(&semaphore);
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let delay_ms = self.config.delay_ms;

            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                
                if delay_ms > 0 {
                    sleep(Duration::from_millis(delay_ms)).await;
                }

                let url = format!("{}?page={}", csa_iot::PRODUCTS_PAGE_MATTER_ONLY, page_num);
                debug!("Fetching page: {}", url);

                let mut client = http_client.lock().await;
                match client.fetch_html_string(&url).await {
                    Ok(html_str) => {
                        match data_extractor.extract_product_urls_from_content(&html_str) {
                            Ok(urls) => {
                                debug!("Extracted {} URLs from page {}", urls.len(), page_num);
                                Ok(urls)
                            },
                            Err(e) => {
                                warn!("Failed to extract URLs from page {}: {}", page_num, e);
                                Ok(Vec::new())
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to fetch page {}: {}", page_num, e);
                        Err(e)
                    }
                }
            })
        }).collect();

        let batch_results = try_join_all(batch_tasks).await?;
        let mut all_urls = Vec::new();
        
        for result in batch_results {
            match result {
                Ok(urls) => all_urls.extend(urls),
                Err(e) => warn!("Batch task failed: {}", e),
            }
        }

        Ok(all_urls)
    }
}

/// 제품 상세정보 수집 서비스 구현체
pub struct ProductDetailCollectorImpl {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
    config: CollectorConfig,
}

impl ProductDetailCollectorImpl {
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        config: CollectorConfig,
    ) -> Self {
        Self {
            http_client: Arc::new(tokio::sync::Mutex::new(http_client)),
            data_extractor: Arc::new(data_extractor),
            config,
        }
    }
}

#[async_trait]
impl ProductDetailCollector for ProductDetailCollectorImpl {
    async fn collect_details(&self, urls: &[String]) -> Result<Vec<Product>> {
        info!("Collecting product details from {} URLs", urls.len());

        let mut all_products = Vec::new();

        // 배치별로 제품 처리
        for batch in urls.chunks(self.config.batch_size as usize) {
            let batch_products = self.collect_product_batch(batch).await?;
            all_products.extend(batch_products);
            
            debug!("Completed batch, total products: {}", all_products.len());
        }

        info!("Product detail collection completed: {} products collected", all_products.len());
        Ok(all_products)
    }

    async fn collect_single_product(&self, url: &str) -> Result<Product> {
        debug!("Fetching product detail: {}", url);

        if self.config.delay_ms > 0 {
            sleep(Duration::from_millis(self.config.delay_ms)).await;
        }

        let mut client = self.http_client.lock().await;
        let html_str = client.fetch_html_string(url).await?;
        
        // HTML 파싱하여 Product 구조체 생성
        let html = scraper::Html::parse_document(&html_str);
        let products = self.data_extractor.extract_products_from_list(&html, 0)?;
        
        if let Some(product) = products.into_iter().next() {
            debug!("Extracted product: {:?} - {:?}", product.manufacturer, product.model);
            Ok(product)
        } else {
            Err(anyhow!("No product found at URL: {}", url))
        }
    }

    async fn collect_product_batch(&self, urls: &[String]) -> Result<Vec<Product>> {
        let semaphore = Arc::new(Semaphore::new(self.config.concurrency as usize));
        
        let batch_tasks: Vec<_> = urls.iter().map(|url| {
            let semaphore = Arc::clone(&semaphore);
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let delay_ms = self.config.delay_ms;
            let url = url.clone();

            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                
                if delay_ms > 0 {
                    sleep(Duration::from_millis(delay_ms)).await;
                }

                let mut client = http_client.lock().await;
                match client.fetch_html_string(&url).await {
                    Ok(html_str) => {
                        let html = scraper::Html::parse_document(&html_str);
                        match data_extractor.extract_products_from_list(&html, 0) {
                            Ok(mut products) => {
                                if let Some(product) = products.pop() {
                                    debug!("Extracted product: {:?} - {:?}", product.manufacturer, product.model);
                                    Ok(Some(product))
                                } else {
                                    warn!("No product found at URL: {}", url);
                                    Ok(None)
                                }
                            },
                            Err(e) => {
                                warn!("Failed to extract product from {}: {}", url, e);
                                Ok(None)
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to fetch product {}: {}", url, e);
                        Err(e)
                    }
                }
            })
        }).collect();

        let batch_results = try_join_all(batch_tasks).await?;
        let mut products = Vec::new();
        
        for result in batch_results {
            match result {
                Ok(Some(product)) => products.push(product),
                Ok(None) => {}, // 스킵
                Err(e) => warn!("Product collection task failed: {}", e),
            }
        }

        Ok(products)
    }
}
