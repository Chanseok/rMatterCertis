//! 크롤링 서비스 구현체
//! 
//! domain/services/crawling_services.rs의 트레이트들에 대한 실제 구현체

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::any::Any;
use async_trait::async_trait;
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use scraper;
use regex;

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    SiteStatus, DatabaseAnalysis, FieldAnalysis, DuplicateAnalysis, ProcessingStrategy
};
use crate::domain::product_url::ProductUrl;
use crate::domain::services::crawling_services::{
    SiteDataChangeStatus, DataDecreaseRecommendation, RecommendedAction, SeverityLevel, CrawlingRangeRecommendation
};
use crate::domain::product::{Product, ProductDetail};
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::{AppConfig, CrawlingConfig};
use crate::infrastructure::config::utils as config_utils;

// 상수 정의
const DEFAULT_PRODUCTS_PER_PAGE: u32 = 12;

/// 페이지 분석 결과를 캐싱하기 위한 구조체
#[derive(Debug, Clone)]
struct PageAnalysisCache {
    /// 페이지의 제품 수
    product_count: u32,
    /// 페이지네이션에서 발견된 최대 페이지 번호
    max_pagination_page: u32,
    /// 현재 활성화된 페이지 번호 (페이지네이션에서 확인)
    active_page: u32,
    /// 제품이 있는지 여부
    has_products: bool,
    /// 분석 완료 시각
    analyzed_at: std::time::Instant,
}

/// 사이트 상태 체크 서비스 구현체
/// PageDiscoveryService와 협력하여 사이트 상태를 종합적으로 분석
pub struct StatusCheckerImpl {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
    config: AppConfig,
    /// 페이지 분석 결과 캐시 (페이지 번호 -> 분석 결과)
    page_cache: Arc<tokio::sync::Mutex<HashMap<u32, PageAnalysisCache>>>,
    /// 제품 레포지토리 (로컬 DB 상태 조회용)
    product_repo: Option<Arc<IntegratedProductRepository>>,
}

impl StatusCheckerImpl {
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        config: AppConfig,
    ) -> Self {
        // 470을 초기값으로 설정한 이유 설명:
        // 이는 과거 CSA-IoT 사이트에서 관찰된 대략적인 페이지 수입니다.
        // 그러나 현재는 더 작은 값(100)부터 시작하여 동적으로 탐지합니다.
        // 앱이 사용되면서 실제 마지막 페이지를 학습하고 저장하게 됩니다.
        
        Self {
            http_client: Arc::new(tokio::sync::Mutex::new(http_client)),
            data_extractor: Arc::new(data_extractor),
            config,
            page_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            product_repo: None,
        }
    }

    pub fn with_product_repo(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        config: AppConfig,
        product_repo: Arc<IntegratedProductRepository>,
    ) -> Self {
        let mut instance = Self::new(http_client, data_extractor, config);
        instance.product_repo = Some(product_repo);
        instance
    }

    /// Update the pagination context in the data extractor based on discovered page information
    pub async fn update_pagination_context(&self, total_pages: u32, items_on_last_page: u32) -> Result<()> {
        // Get validated configuration for products per page instead of hardcoding
        let validated_config = crate::application::validated_crawling_config::ValidatedCrawlingConfig::from_app_config(&self.config);
        let products_per_page = validated_config.products_per_page;
        
        // Create pagination context
        let pagination_context = crate::infrastructure::html_parser::PaginationContext {
            total_pages,
            items_per_page: products_per_page, // Use config instead of hardcoded 12
            items_on_last_page,
            target_page_size: products_per_page, // Use config instead of hardcoded 12
        };
        
        // Update the data extractor's pagination context
        self.data_extractor.set_pagination_context(pagination_context)?;
        
        info!("📊 Updated pagination context: total_pages={}, items_on_last_page={}, products_per_page={}", 
               total_pages, items_on_last_page, products_per_page);
        
        Ok(())
    }
}

#[async_trait]
impl StatusChecker for StatusCheckerImpl {
    async fn check_site_status(&self) -> Result<SiteStatus> {
        let start_time = Instant::now();
        info!("Starting comprehensive site status check with detailed page discovery");
        
        // 캐시 초기화
        self.clear_page_cache().await;
        
        info!("Checking site status and discovering pages...");

        // Step 1: 기본 사이트 접근성 확인
        let url = config_utils::matter_products_page_url_simple(1);
        
        // 접근성 테스트
        let access_test = {
            let mut client = self.http_client.lock().await;
            client.fetch_html_string(&url).await
        };
        
        match access_test {
            Ok(_) => info!("Site is accessible"),
            Err(e) => {
                error!("Failed to access site: {}", e);
                return Ok(SiteStatus {
                    is_accessible: false,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    total_pages: 0,
                    estimated_products: 0,
                    products_on_last_page: 0,
                    last_check_time: chrono::Utc::now(),
                    health_score: 0.0,
                    data_change_status: SiteDataChangeStatus::Inaccessible,
                    decrease_recommendation: None,
                    crawling_range_recommendation: CrawlingRangeRecommendation::None,
                });
            }
        }

        // Step 2: 페이지 수 탐지 및 마지막 페이지 제품 수 확인
        let (total_pages, products_on_last_page) = self.discover_total_pages().await?;

        // Step 2.5: 페이지네이션 컨텍스트 업데이트
        if let Err(e) = self.update_pagination_context(total_pages, products_on_last_page).await {
            warn!("Failed to update pagination context: {}", e);
        }

        let response_time_ms = start_time.elapsed().as_millis() as u64;
        let response_time = start_time.elapsed();

        // Step 3: 사이트 건강도 점수 계산
        let health_score = calculate_health_score(response_time, total_pages);

        info!("Site status check completed: {} pages found, {}ms total time, health score: {:.2}", 
              total_pages, response_time_ms, health_score);

        // 정확한 제품 수 계산: (마지막 페이지 - 1) * 페이지당 제품 수 + 마지막 페이지 제품 수
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;
        
        let estimated_products = if total_pages > 1 {
            ((total_pages - 1) * products_per_page) + products_on_last_page
        } else {
            products_on_last_page
        };
        
        info!("Accurate product estimation: ({} full pages * {} products) + {} products on last page = {} total products", 
              total_pages - 1, products_per_page, products_on_last_page, estimated_products);

        // Step 4: 데이터 변화 상태 분석
        let (data_change_status, decrease_recommendation) = self.analyze_data_changes(estimated_products).await;
        
        // Step 5: 크롤링 범위 권장사항 계산
        let crawling_range_recommendation = self.calculate_crawling_range_recommendation_internal(
            total_pages, 
            products_on_last_page, 
            estimated_products
        ).await?;
              
        Ok(SiteStatus {
            is_accessible: true,
            response_time_ms,
            total_pages,
            estimated_products,
            products_on_last_page,
            last_check_time: chrono::Utc::now(),
            health_score,
            data_change_status,
            decrease_recommendation,
            crawling_range_recommendation,
        })
    }

    async fn calculate_crawling_range_recommendation(
        &self, 
        site_status: &SiteStatus, 
        db_analysis: &DatabaseAnalysis
    ) -> Result<CrawlingRangeRecommendation> {
        info!("🔍 Calculating crawling range recommendation from site status and DB analysis...");
        info!("📊 DB Analysis shows: total_products={}, unique_products={}", 
              db_analysis.total_products, db_analysis.unique_products);
        
        // Cross-check with local status to ensure consistency
        let local_status = self.get_local_db_status().await?;
        
        // Verify consistency between different DB access methods
        let db_total = db_analysis.total_products;
        if db_total != local_status.total_saved_products {
            warn!("⚠️  DB inconsistency detected: analysis={}, local_status={}", 
                  db_analysis.total_products, local_status.total_saved_products);
            
            // Use the higher value for safer operation
            let effective_total = db_total.max(local_status.total_saved_products);
            info!("🔧 Using effective total: {}", effective_total);
        }
        
        // If database is empty, recommend full crawl
        if db_analysis.total_products == 0 && local_status.is_empty {
            info!("📊 Local DB is confirmed empty - recommending full crawl");
            return Ok(CrawlingRangeRecommendation::Full);
        }
        
        // If there's inconsistency but some data exists, use partial crawl
        if db_analysis.total_products == 0 && !local_status.is_empty {
            warn!("⚠️  Inconsistent DB state: analysis says empty but local status says not empty");
            warn!("⚠️  This suggests a DB access issue - using local status for safety");
            // Continue with partial crawl logic using local_status data
        }
        
        // Calculate how many new products might have been added
        let effective_total = db_analysis.total_products.max(local_status.total_saved_products);
        let estimated_new_products = if site_status.estimated_products > effective_total {
            site_status.estimated_products - effective_total
        } else {
            0
        };
        
        if estimated_new_products == 0 {
            info!("📊 No new products detected - recommending minimal verification crawl");
            return Ok(CrawlingRangeRecommendation::Partial(5)); // 5 pages for verification
        }
        
        // Calculate pages needed for new products
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;
        let pages_needed = (estimated_new_products as f64 / products_per_page as f64).ceil() as u32;
        let limited_pages = pages_needed.min(self.config.user.crawling.page_range_limit);
        
        info!("📊 Estimated {} new products, recommending {} pages crawl", estimated_new_products, limited_pages);
        Ok(CrawlingRangeRecommendation::Partial(limited_pages))
    }

    async fn estimate_crawling_time(&self, pages: u32) -> Duration {
        // ...
        // 페이지당 평균 2초 + 제품 상세페이지당 1초 추정
        let page_collection_time = pages * 2;
        let product_detail_time = pages * 20; // 페이지당 20개 제품 * 1초
        let total_seconds = page_collection_time + product_detail_time;
        
        Duration::from_secs(total_seconds as u64)
    }

    async fn verify_site_accessibility(&self) -> Result<bool> {
        let status = self.check_site_status().await?;
        // health_score는 성능 정보일 뿐, 크롤링 가능 여부와는 무관
        // 사이트 접근 가능성과 기본적인 페이지 구조만 확인
        Ok(status.is_accessible && status.total_pages > 0)
    }
}

impl StatusCheckerImpl {
    /// 향상된 페이지 탐지 로직 - 사이트 정보 변화 감지 포함
    async fn discover_total_pages(&self) -> Result<(u32, u32)> {
        info!("🔍 Starting enhanced page discovery algorithm with site change detection");
        
        // 1. 시작 페이지 결정
        let start_page = self.config.app_managed.last_known_max_page
            .unwrap_or(self.config.advanced.last_page_search_start);
        
        info!("📍 Starting from page {} (last known: {:?}, default: {})", 
              start_page, 
              self.config.app_managed.last_known_max_page,
              self.config.advanced.last_page_search_start);
        
        // 2. 시작 페이지 분석 (캐시 사용)
        let start_analysis = self.get_or_analyze_page(start_page).await?;
        let mut current_page = start_page;
        
        if !start_analysis.has_products {
            warn!("⚠️  Starting page {} has no products - checking site status", current_page);
            
            // 첫 페이지 확인으로 사이트 접근성 검증
            let first_page_analysis = self.get_or_analyze_page(1).await?;
            if !first_page_analysis.has_products {
                error!("❌ First page also has no products - site may be temporarily unavailable");
                return Err(anyhow::anyhow!(
                    "Site appears to be temporarily unavailable or experiencing issues. Please try again later."
                ));
            }
            
            info!("✅ First page has products - site is accessible, cached page info may be outdated");
            warn!("🔄 Site content may have decreased - will perform full discovery");
            
            // 하향 탐색으로 유효한 페이지 찾기
            current_page = self.find_last_valid_page_downward(current_page).await?;
            info!("✅ Found valid starting page: {}", current_page);
        }
        
        // 3. 반복적 상향 탐색: 페이지네이션에서 더 큰 값을 찾을 때까지 계속
        let mut attempts = 0;
        let max_attempts = self.config.advanced.max_search_attempts;
        
        loop {
            attempts += 1;
            if attempts > max_attempts {
                warn!("🔄 Reached maximum attempts ({}), stopping at page {}", max_attempts, current_page);
                break;
            }
            
            info!("🔍 Iteration {}/{}: Checking page {}", attempts, max_attempts, current_page);
            
            // 현재 페이지를 분석 (캐시 사용)
            let analysis = match self.get_or_analyze_page(current_page).await {
                Ok(analysis) => analysis,
                Err(e) => {
                    warn!("❌ Failed to analyze page {}: {}", current_page, e);
                    // 네트워크 오류 시 하향 탐색
                    current_page = self.find_last_valid_page_downward(current_page).await?;
                    break;
                }
            };
            
            if !analysis.has_products {
                // 제품이 없는 경우 안전성 검사가 포함된 하향 탐색
                info!("🔻 Page {} has no products, performing safe downward search", current_page);
                current_page = self.find_last_valid_page_with_safety_check(current_page).await?;
                break;
            }
            
            // 페이지네이션에서 더 큰 페이지를 찾았는지 확인
            if analysis.max_pagination_page > current_page {
                info!("🔺 Found higher page {} in pagination, jumping there", analysis.max_pagination_page);
                current_page = analysis.max_pagination_page;
                // 새 페이지로 이동하여 다시 탐색
                continue;
            }
            info!("🏁 No higher pages found in pagination, {} appears to be the last page", current_page);
            break;
        }
        
        // 4. 최종 검증: 마지막 페이지 확인 및 제품 수 계산
        let (verified_last_page, products_on_last_page) = self.verify_last_page(current_page).await?;
        
        // 5. 설정 파일에 결과 저장
        if let Err(e) = self.update_last_known_page(verified_last_page).await {
            warn!("⚠️  Failed to update last known page in config: {}", e);
        }
        
        info!("🎉 Final verified last page: {} with {} products", verified_last_page, products_on_last_page);
        Ok((verified_last_page, products_on_last_page))
    }

    /// 하향 탐색으로 마지막 유효한 페이지 찾기
    async fn find_last_valid_page_downward(&self, start_page: u32) -> Result<u32> {
        let mut current_page = start_page.saturating_sub(1);
        let min_page = 1;

        info!("Starting downward search from page {}", current_page);

        while current_page >= min_page {
            let test_url = config_utils::matter_products_page_url_simple(current_page);
            
            let mut client = self.http_client.lock().await;
            match client.fetch_html_string(&test_url).await {
                Ok(html) => {
                    let doc = scraper::Html::parse_document(&html);
                    if self.has_products_on_page(&doc) {
                        info!("Found valid page with products: {}", current_page);
                        return Ok(current_page);
                    }
                },
                Err(e) => {
                    warn!("Failed to fetch page {} during downward search: {}", current_page, e);
                }
            }

            current_page = current_page.saturating_sub(1);
            
            // 요청 간 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.user.request_delay_ms)).await;
        }

        // 모든 페이지에서 제품을 찾지 못한 경우
        warn!("No valid pages found during downward search, returning 1");
        Ok(1)
    }

    /// 안전성 검사가 포함된 하향 탐색 - 연속 빈 페이지 3개 이상 시 fatal error
    async fn find_last_valid_page_with_safety_check(&self, start_page: u32) -> Result<u32> {
        let mut current_page = start_page;
        let mut consecutive_empty_pages = 0;
        const MAX_CONSECUTIVE_EMPTY: u32 = 3;
        let min_page = 1;

        info!("🔍 Starting safe downward search from page {} (max consecutive empty: {})", 
              current_page, MAX_CONSECUTIVE_EMPTY);

        // 먼저 시작 페이지가 비어있는지 확인
        if !self.check_page_has_products(current_page).await? {
            consecutive_empty_pages += 1;
            info!("⚠️  Starting page {} is empty (consecutive: {})", current_page, consecutive_empty_pages);
        }

        while current_page > min_page {
            current_page = current_page.saturating_sub(1);
            
            let test_url = config_utils::matter_products_page_url_simple(current_page);
            info!("🔍 Checking page {} (consecutive empty: {})", current_page, consecutive_empty_pages);
            
            let mut client = self.http_client.lock().await;
            match client.fetch_html_string(&test_url).await {
                Ok(html) => {
                    let doc = scraper::Html::parse_document(&html);
                    if self.has_products_on_page(&doc) {
                        info!("✅ Found valid page with products: {} (after {} consecutive empty pages)", 
                              current_page, consecutive_empty_pages);
                        return Ok(current_page);
                    }
                    consecutive_empty_pages += 1;
                    warn!("⚠️  Page {} is empty (consecutive: {}/{})", 
                          current_page, consecutive_empty_pages, MAX_CONSECUTIVE_EMPTY);
                    
                    // 연속으로 빈 페이지가 3개 이상이면 fatal error
                    if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
                        error!("💥 FATAL ERROR: Found {} consecutive empty pages starting from page {}. This indicates a serious site issue or crawling problem.", 
                               consecutive_empty_pages, start_page);
                        
                        return Err(anyhow!(
                            "Fatal error: {} consecutive empty pages detected. Site may be down or pagination structure changed. Last checked pages: {} to {}",
                            consecutive_empty_pages, 
                            start_page,
                            current_page
                        ));
                    }
                },
                Err(e) => {
                    consecutive_empty_pages += 1;
                    warn!("❌ Failed to fetch page {} during safe downward search: {} (consecutive: {}/{})", 
                          current_page, e, consecutive_empty_pages, MAX_CONSECUTIVE_EMPTY);
                    
                    // 네트워크 오류도 연속 실패로 카운트
                    if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
                        error!("💥 FATAL ERROR: {} consecutive failures (empty pages + network errors) starting from page {}.", 
                               consecutive_empty_pages, start_page);
                        
                        return Err(anyhow!(
                            "Fatal error: {} consecutive failures detected. Network issues or site problems. Last error: {}",
                            consecutive_empty_pages, 
                            e
                        ));
                    }
                }
            }
            
            // 요청 간 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.user.request_delay_ms)).await;
        }

        // 최소 페이지까지 도달했지만 여전히 연속 빈 페이지가 많다면 fatal error
        if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
            error!("💥 FATAL ERROR: Reached minimum page but still have {} consecutive empty pages. Site appears to be completely empty or broken.", 
                   consecutive_empty_pages);
            
            return Err(anyhow!(
                "Fatal error: Site appears to be empty or broken. {} consecutive empty pages found from page {} down to page {}",
                consecutive_empty_pages, 
                start_page,
                current_page
            ));
        }

        // 모든 페이지에서 제품을 찾지 못했지만 연속 빈 페이지가 3개 미만이면 경고와 함께 1 반환
        warn!("⚠️  No valid pages found during safe downward search, but only {} consecutive empty pages. Returning page 1 as fallback.", 
              consecutive_empty_pages);
        Ok(1)
    }

    /// 마지막 페이지 최종 검증 - 더 철저한 검증 로직
    /// 마지막 페이지 검증 및 제품 수 확인
    async fn verify_last_page(&self, candidate_page: u32) -> Result<(u32, u32)> {
        info!("🔍 Verifying candidate last page: {}", candidate_page);

        // 1. 후보 페이지 분석 (캐시에서 가져오거나 새로 분석)
        let analysis = self.get_or_analyze_page(candidate_page).await?;
        let products_on_last_page = analysis.product_count;
        let has_products = analysis.has_products;
        
        info!("📊 Last page {} has {} products", candidate_page, products_on_last_page);
        
        if !has_products {
            warn!("⚠️  Candidate page {} has no products, performing downward search with safety check", candidate_page);
            let actual_last_page = self.find_last_valid_page_with_safety_check(candidate_page).await?;
            // 실제 마지막 페이지의 제품 수 다시 확인
            let actual_analysis = self.get_or_analyze_page(actual_last_page).await?;
            return Ok((actual_last_page, actual_analysis.product_count));
        }

        // 2. 페이지네이션 분석에서 이미 마지막 페이지임을 확신할 수 있다면 추가 확인 생략
        // 현재 페이지가 페이지네이션에서 발견된 최대 페이지와 같다면 검증 완료
        if analysis.max_pagination_page == candidate_page {
            info!("✅ Page {} confirmed as last page via pagination analysis (max_pagination={})", 
                  candidate_page, analysis.max_pagination_page);
            info!("🚀 Skipping additional verification - pagination analysis is reliable");
            return Ok((candidate_page, products_on_last_page));
        }
        
        // 3. 페이지네이션 분석이 불확실한 경우에만 최소한의 추가 검증 수행
        info!("🔍 Pagination analysis inconclusive (current={}, max_pagination={}), performing minimal verification", 
              candidate_page, analysis.max_pagination_page);
        
        // 바로 다음 페이지 1개만 확인 (과도한 검증 방지)
        let next_page = candidate_page + 1;
        match self.check_page_has_products(next_page).await {
            Ok(true) => {
                warn!("🔍 Found products on page {} after candidate {}, re-discovering", 
                      next_page, candidate_page);
                // 더 높은 페이지에서 제품을 발견했으므로 그 페이지부터 다시 탐색
                return self.discover_from_page_with_count(next_page).await;
            },
            Ok(false) => {
                info!("✅ Verified page {} as the last page with {} products (checked {} page ahead)", 
                      candidate_page, products_on_last_page, 1);
            },
            Err(e) => {
                debug!("❌ Failed to check page {}: {}, assuming {} is last", next_page, e, candidate_page);
            }
        }
        
        Ok((candidate_page, products_on_last_page))
    }

    /// 특정 페이지부터 다시 탐색 시작 (제품 수도 반환)
    async fn discover_from_page_with_count(&self, start_page: u32) -> Result<(u32, u32)> {
        info!("🔄 Re-discovering from page {} with product count", start_page);
        
        let mut current_page = start_page;
        let max_attempts = self.config.advanced.max_search_attempts;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                warn!("🔄 Reached maximum attempts, stopping at page {}", current_page);
                break;
            }

            let test_url = config_utils::matter_products_page_url_simple(current_page);
            
            let (has_products, max_page_in_pagination) = {
                let mut client = self.http_client.lock().await;
                match client.fetch_html_string(&test_url).await {
                    Ok(html) => {
                        let doc = scraper::Html::parse_document(&html);
                        let has_products = self.has_products_on_page(&doc);
                        let max_page = self.find_max_page_in_pagination(&doc);
                        
                        info!("📊 Page {} analysis: has_products={}, max_pagination={}", 
                              current_page, has_products, max_page);
                        
                        (has_products, max_page)
                    },
                    Err(e) => {
                        warn!("❌ Failed to fetch page {}: {}", current_page, e);
                        break;
                    }
                }
            };

            if !has_products {
                // 제품이 없으면 안전성 검사가 포함된 하향 탐색 후 제품 수 확인
                let last_page = self.find_last_valid_page_with_safety_check(current_page).await?;
                let test_url = config_utils::matter_products_page_url_simple(last_page);
                let mut client = self.http_client.lock().await;
                let html = client.fetch_html_string(&test_url).await?;
                drop(client); // 락 해제
                let doc = scraper::Html::parse_document(&html);
                let products_count = self.count_products(&doc);
                return Ok((last_page, products_count));
            }

            if max_page_in_pagination > current_page {
                // 더 큰 페이지가 있으면 이동
                current_page = max_page_in_pagination;
                continue;
            }
            // 마지막 페이지 도달, 제품 수 확인
            let test_url = config_utils::matter_products_page_url_simple(current_page);
            let mut client = self.http_client.lock().await;
            let html = client.fetch_html_string(&test_url).await?;
            drop(client); // 락 해제
            let doc = scraper::Html::parse_document(&html);
            let products_count = self.count_products(&doc);
            return Ok((current_page, products_count));
        }

        // 최대 시도 횟수 도달 시 현재 페이지의 제품 수 확인
        let test_url = config_utils::matter_products_page_url_simple(current_page);
        let mut client = self.http_client.lock().await;
        let html = client.fetch_html_string(&test_url).await?;
        drop(client); // 락 해제
        let doc = scraper::Html::parse_document(&html);
        let products_count = self.count_products(&doc);
        Ok((current_page, products_count))
    }

    /// 특정 페이지부터 다시 탐색 시작
    async fn discover_from_page(&self, start_page: u32) -> Result<u32> {
        info!("🔄 Re-discovering from page {}", start_page);
        
        let mut current_page = start_page;
        let max_attempts = self.config.advanced.max_search_attempts;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                warn!("🔄 Reached maximum attempts, stopping at page {}", current_page);
                break;
            }

            let test_url = config_utils::matter_products_page_url_simple(current_page);
            
            let (has_products, max_page_in_pagination) = {
                let mut client = self.http_client.lock().await;
                match client.fetch_html_string(&test_url).await {
                    Ok(html) => {
                        let doc = scraper::Html::parse_document(&html);
                        let has_products = self.has_products_on_page(&doc);
                        let max_page = self.find_max_page_in_pagination(&doc);
                        
                        info!("📊 Page {} analysis: has_products={}, max_pagination={}", 
                              current_page, has_products, max_page);
                        
                        (has_products, max_page)
                    },
                    Err(e) => {
                        warn!("❌ Failed to fetch page {}: {}", current_page, e);
                        break;
                    }
                }
            };

            if !has_products {
                // 제품이 없으면 안전성 검사가 포함된 하향 탐색
                return self.find_last_valid_page_with_safety_check(current_page).await;
            }

            if max_page_in_pagination > current_page {
                // 더 큰 페이지가 있으면 이동
                current_page = max_page_in_pagination;
            } else {
                // 더 큰 페이지가 없으면 현재 페이지가 마지막
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.user.request_delay_ms)).await;
        }

        Ok(current_page)
    }

    /// 특정 페이지에 제품이 있는지 확인 - 활성 페이지네이션 값도 함께 확인
    async fn check_page_has_products(&self, page: u32) -> Result<bool> {
        let test_url = config_utils::matter_products_page_url_simple(page);
        
        let mut client = self.http_client.lock().await;
        match client.fetch_html_string(&test_url).await {
            Ok(html) => {
                let doc = scraper::Html::parse_document(&html);
                
                // 1. 제품 존재 여부 확인
                let has_products = self.has_products_on_page(&doc);
                
                // 2. 활성 페이지네이션 값 확인 (더 중요한 체크)
                let active_page = self.get_active_page_number(&doc);
                
                // 실제 페이지 번호와 활성 페이지네이션 값이 일치하는지 확인
                let is_correct_page = active_page == page;
                
                if !is_correct_page {
                    info!("⚠️  Page {} was redirected to page {} (pagination mismatch)", page, active_page);
                    return Ok(false);
                }
                
                info!("✅ Page {} verification: has_products={}, active_page={}, is_correct_page={}", 
                      page, has_products, active_page, is_correct_page);
                
                Ok(has_products && is_correct_page)
            },
            Err(_) => Ok(false),
        }
    }

    /// 활성 페이지네이션 값 추출 - 현재 페이지가 실제로 로드되었는지 확인
    fn get_active_page_number(&self, doc: &scraper::Html) -> u32 {
        // 활성 페이지네이션 요소를 찾기 위한 다양한 선택자 시도
        // 사이트 구조에 맞게 우선순위 조정 (페이지네이션 우선 클래스: page-numbers.current)
        let active_selectors = [
            ".page-numbers.current", // 우선순위 가장 높음 (사이트 구조에 맞게 조정)
            "span.page-numbers.current", // 정확한 요소 지정
            "a.page-numbers.current",
            ".current",
            ".active",
            ".pagination .current",
            ".pagination .active",
            "a.current",
            "span.current",
            "[aria-current='page']",
            ".wp-pagenavi .current",
            ".page-item.active a",
            ".page-link.active",
        ];
        
        for selector_str in &active_selectors {
            if let Ok(selector) = scraper::Selector::parse(selector_str) {
                if let Some(element) = doc.select(&selector).next() {
                    // 텍스트 내용에서 페이지 번호 추출
                    let text = element.text().collect::<String>().trim().to_string();
                    if let Ok(page_num) = text.parse::<u32>() {
                        info!("🎯 Found active page number {} using selector '{}'", page_num, selector_str);
                        return page_num;
                    }
                }
            }
        }
        
        // 활성 페이지네이션을 찾지 못한 경우 URL에서 추출 시도
        if let Some(canonical_url) = self.get_canonical_url(doc) {
            if let Some(page_num) = self.extract_page_number(&canonical_url) {
                info!("🎯 Found page number {} from canonical URL", page_num);
                return page_num;
            }
        }
        
        // 모든 방법이 실패한 경우 1 반환 (첫 번째 페이지로 추정)
        warn!("⚠️  Could not determine active page number, assuming page 1");
        1
    }

    /// 페이지의 canonical URL 추출
    fn get_canonical_url(&self, doc: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("link[rel='canonical']") {
            if let Some(element) = doc.select(&selector).next() {
                return element.value().attr("href").map(|s| s.to_string());
            }
        }
        None
    }

    /// 설정 파일에 마지막 페이지 및 메타데이터 업데이트
    async fn update_last_known_page(&self, last_page: u32) -> Result<()> {
        use crate::infrastructure::config::ConfigManager;
        
        let config_manager = ConfigManager::new()?;
        
        // 설정 업데이트를 위한 클로저 사용
        config_manager.update_app_managed(|app_managed| {
            // 마지막 알려진 페이지 업데이트
            app_managed.last_known_max_page = Some(last_page);
            
            // 마지막 성공한 크롤링 시간 업데이트
            app_managed.last_successful_crawl = Some(chrono::Utc::now().to_rfc3339());
            
            // 추정 제품 수 업데이트 (페이지당 12개 제품 - 실제 사이트 구조 기반)
            app_managed.last_crawl_product_count = Some(last_page * 12);
            
            // 페이지당 평균 제품 수 업데이트
            app_managed.avg_products_per_page = Some(DEFAULT_PRODUCTS_PER_PAGE as f64);
            
            info!("📝 Updated config: last_page={}, timestamp={}", 
                  last_page, 
                  app_managed.last_successful_crawl.as_ref().unwrap_or(&"unknown".to_string()));
        }).await?;
        
        info!("✅ Successfully updated last known page to {} in config file", last_page);
        Ok(())
    }

    /// 데이터 변화 상태 분석 및 권장사항 생성
    async fn analyze_data_changes(&self, current_estimated_products: u32) -> (SiteDataChangeStatus, Option<DataDecreaseRecommendation>) {
        // 이전 크롤링 정보 가져오기
        let previous_count = self.config.app_managed.last_crawl_product_count;
        
        match previous_count {
            None => {
                info!("🆕 Initial site check - no previous data available");
                (SiteDataChangeStatus::Initial { count: current_estimated_products }, None)
            },
            Some(prev_count) => {
                let change_percentage = if prev_count > 0 {
                    ((current_estimated_products as f64 - prev_count as f64) / prev_count as f64) * 100.0
                } else {
                    0.0
                };
                
                if current_estimated_products > prev_count {
                    let increase = current_estimated_products - prev_count;
                    info!("📈 Site data increased: {} -> {} (+{}, +{:.1}%)", 
                          prev_count, current_estimated_products, increase, change_percentage);
                    (SiteDataChangeStatus::Increased { 
                        new_count: current_estimated_products, 
                        previous_count: prev_count 
                    }, None)
                } else if current_estimated_products == prev_count {
                    info!("📊 Site data stable: {} products", current_estimated_products);
                    (SiteDataChangeStatus::Stable { count: current_estimated_products }, None)
                } else {
                    let decrease = prev_count - current_estimated_products;
                    let decrease_percentage = (decrease as f64 / prev_count as f64) * 100.0;
                    
                    warn!("📉 Site data decreased: {} -> {} (-{}, -{:.1}%)", 
                          prev_count, current_estimated_products, decrease, decrease_percentage);
                    
                    let severity = if decrease_percentage < 10.0 {
                        SeverityLevel::Low
                    } else if decrease_percentage < 30.0 {
                        SeverityLevel::Medium
                    } else if decrease_percentage < 50.0 {
                        SeverityLevel::High
                    } else {
                        SeverityLevel::Critical
                    };
                    
                    let recommendation = self.generate_decrease_recommendation(decrease_percentage, &severity);
                    
                    (SiteDataChangeStatus::Decreased { 
                        current_count: current_estimated_products,
                        previous_count: prev_count,
                        decrease_amount: decrease
                    }, Some(recommendation))
                }
            }
        }
    }
    
    /// 데이터 감소 시 권장사항 생성
    fn generate_decrease_recommendation(&self, decrease_percentage: f64, severity: &SeverityLevel) -> DataDecreaseRecommendation {
        match severity {
            SeverityLevel::Low => DataDecreaseRecommendation {
                action_type: RecommendedAction::WaitAndRetry,
                description: format!("사이트 데이터가 {:.1}% 감소했습니다. 일시적인 변화일 수 있습니다.", decrease_percentage),
                severity: severity.clone(),
                action_steps: vec![
                    "잠시 후(5-10분) 다시 상태를 확인해보세요".to_string(),
                    "문제가 지속되면 수동으로 사이트를 확인해보세요".to_string(),
                ],
            },
            SeverityLevel::Medium => DataDecreaseRecommendation {
                action_type: RecommendedAction::ManualVerification,
                description: format!("사이트 데이터가 {:.1}% 감소했습니다. 수동 확인이 필요합니다.", decrease_percentage),
                severity: severity.clone(),
                action_steps: vec![
                    "CSA-IoT 사이트에서 직접 제품 수를 확인해보세요".to_string(),
                    "사이트에서 필터 설정이 변경되었는지 확인하세요".to_string(),
                    "데이터베이스를 백업하고 부분 재크롤링을 고려하세요".to_string(),
                ],
            },
            SeverityLevel::High => DataDecreaseRecommendation {
                action_type: RecommendedAction::BackupAndRecrawl,
                description: format!("사이트 데이터가 {:.1}% 크게 감소했습니다. 데이터베이스 백업 후 재크롤링을 권장합니다.", decrease_percentage),
                severity: severity.clone(),
                action_steps: vec![
                    "현재 데이터베이스를 즉시 백업하세요".to_string(),
                    "CSA-IoT 사이트를 수동으로 확인하여 실제 상황을 파악하세요".to_string(),
                    "데이터베이스를 비우고 전체 재크롤링을 수행하세요".to_string(),
                    "크롤링 완료 후 이전 데이터와 비교 분석하세요".to_string(),
                ],
            },
            SeverityLevel::Critical => DataDecreaseRecommendation {
                action_type: RecommendedAction::BackupAndRecrawl,
                description: format!("사이트 데이터가 {:.1}% 심각하게 감소했습니다. 즉시 조치가 필요합니다.", decrease_percentage),
                severity: severity.clone(),
                action_steps: vec![
                    "🚨 즉시 현재 데이터베이스를 백업하세요".to_string(),
                    "CSA-IoT 사이트에 접속하여 실제 상태를 확인하세요".to_string(),
                    "사이트 구조나 필터 조건이 변경되었는지 확인하세요".to_string(),
                    "백업 확인 후 데이터베이스를 초기화하고 전체 재크롤링하세요".to_string(),
                    "크롤링 설정(selector, URL 등)을 재검토하세요".to_string(),
                ],
            },
        }
    }

    /// 페이지에 제품이 있는지 확인
    fn has_products_on_page(&self, doc: &scraper::Html) -> bool {
        let product_count = self.count_products(doc);
        product_count > 0
    }

    /// 페이지네이션에서 최대 페이지 번호 찾기
    fn find_max_page_in_pagination(&self, doc: &scraper::Html) -> u32 {
        let mut max_page = 1;
        
        // 1. 페이지네이션 링크에서 찾기
        let link_selectors = vec![
            "a[href*='page']",
            ".pagination a",
            ".page-numbers", // 모든 페이지 번호 요소 (a와 span 모두 포함)
            ".page-numbers a", 
            ".pager a",
            "a[href*='paged']",
            ".page-numbers:not(.current):not(.dots)" // 현재 페이지와 줄임표를 제외한 페이지 번호
        ];
        
        for selector_str in &link_selectors {
            if let Ok(selector) = scraper::Selector::parse(selector_str) {
                for element in doc.select(&selector) {
                    // href 속성에서 페이지 번호 추출
                    if let Some(href) = element.value().attr("href") {
                        if let Some(page_num) = self.extract_page_number(href) {
                            if page_num > max_page {
                                max_page = page_num;
                                debug!("Found higher page {} in href: {}", page_num, href);
                            }
                        }
                    }
                    
                    // 텍스트에서도 페이지 번호 추출
                    let text = element.text().collect::<String>().trim().to_string();
                    if let Ok(page_num) = text.parse::<u32>() {
                        if page_num > max_page && page_num < 10000 { // 합리적인 상한선
                            max_page = page_num;
                            debug!("Found higher page {} in text: {}", page_num, text);
                        }
                    }
                }
            }
        }
        
        debug!("Max page found in pagination: {}", max_page);
        max_page
    }

    /// URL에서 페이지 번호 추출
    fn extract_page_number(&self, url: &str) -> Option<u32> {
        // URL 패턴: /page/123/ 또는 paged=123
        let patterns = [
            r"/page/(\d+)/?",  // CSA-IoT 사이트의 /page/123/ 패턴
            r"paged=(\d+)",
            r"page=(\d+)",
            r"/(\d+)/$",  // 끝에 숫자가 있는 경우
            r"page/(\d+)/\?",  // page/123/?... 패턴
        ];
        
        for pattern in &patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(url) {
                    if let Some(num_str) = caps.get(1) {
                        if let Ok(num) = num_str.as_str().parse::<u32>() {
                            return Some(num);
                        }
                    }
                }
            }
        }
        
        None
    }

    /// 페이지에서 제품 개수 카운트 (모든 선택자를 시도하고 가장 많은 결과 반환)
    fn count_products(&self, doc: &scraper::Html) -> u32 {
        let mut max_count = 0;
        let mut best_selector = "none";
        
        for selector_str in &self.config.advanced.product_selectors {
            if let Ok(selector) = scraper::Selector::parse(selector_str) {
                let count = doc.select(&selector).count() as u32;
                debug!("Selector '{}' found {} products", selector_str, count);
                if count > max_count {
                    max_count = count;
                    best_selector = selector_str;
                }
            } else {
                debug!("Failed to parse selector: {}", selector_str);
            }
        }
        
        info!("Total products found on page: {} (using selector: {})", max_count, best_selector);
        max_count
    }

    /// 페이지 분석 결과를 캐시에서 가져오거나 새로 분석
    async fn get_or_analyze_page(&self, page_number: u32) -> Result<PageAnalysisCache> {
        // 캐시에서 먼저 확인
        {
            let cache = self.page_cache.lock().await;
            if let Some(cached) = cache.get(&page_number) {
                debug!("📋 Using cached analysis for page {}", page_number);
                return Ok(cached.clone());
            }
        }
        
        // 캐시에 없으면 새로 분석
        debug!("🔍 Analyzing page {} (not in cache)", page_number);
        let url = config_utils::matter_products_page_url_simple(page_number);
        
        let (product_count, max_pagination_page, active_page, has_products) = {
            let mut client = self.http_client.lock().await;
            let html = client.fetch_html_string(&url).await?;
            drop(client); // 락 해제
            
            let doc = scraper::Html::parse_document(&html);
            let product_count = self.count_products(&doc);
            let max_pagination_page = self.find_max_page_in_pagination(&doc);
            let active_page = self.get_active_page_number(&doc);
            let has_products = product_count > 0;
            
            (product_count, max_pagination_page, active_page, has_products)
        };
        
        let analysis = PageAnalysisCache {
            product_count,
            max_pagination_page,
            active_page,
            has_products,
            analyzed_at: std::time::Instant::now(),
        };
        
        // 캐시에 저장
        {
            let mut cache = self.page_cache.lock().await;
            cache.insert(page_number, analysis.clone());
        }
        
        info!("📊 Page {} analysis: has_products={}, product_count={}, max_pagination={}", 
              page_number, has_products, product_count, max_pagination_page);
        
        Ok(analysis)
    }
    
    /// 캐시를 초기화 (새로운 상태 체크 시작 시 호출)
    async fn clear_page_cache(&self) {
        let mut cache = self.page_cache.lock().await;
        cache.clear();
        debug!("🗑️  Page cache cleared");
    }
    
    /// 크롤링 범위 권장사항 계산
    /// 로컬 DB 상태와 사이트 정보를 기반으로 다음 크롤링 대상 페이지 범위를 계산 (내부용)
    async fn calculate_crawling_range_recommendation_internal(
        &self,
        total_pages_on_site: u32,
        products_on_last_page: u32,
        estimated_products: u32,
    ) -> Result<CrawlingRangeRecommendation> {
        info!("🔍 Calculating crawling range recommendation...");
        
        // 현재 로컬 DB 상태 확인
        let local_db_status = self.get_local_db_status().await?;
        
        // DB가 비어있는 경우 전체 크롤링 권장
        if local_db_status.is_empty {
            info!("📊 Local DB is empty - recommending full crawl");
            return Ok(CrawlingRangeRecommendation::Full);
        }
        
        // 사이트 데이터 변화 분석
        let data_change_analysis = self.analyze_site_data_changes(estimated_products).await;
        
        // 크롤링 범위 계산
        let crawling_range = self.calculate_next_crawling_pages(
            &local_db_status,
            total_pages_on_site,
            products_on_last_page,
            estimated_products,
            &data_change_analysis,
        ).await?;
        
        info!("📊 Crawling range recommendation: {:?}", crawling_range);
        Ok(crawling_range)
    }
    
    /// 로컬 DB 상태 조회
    async fn get_local_db_status(&self) -> Result<LocalDbStatus> {
        match &self.product_repo {
            Some(repo) => {
                let products = repo.get_all_products().await?;
                
                if products.is_empty() {
                    return Ok(LocalDbStatus {
                        is_empty: true,
                        max_page_id: 0,
                        max_index_in_page: 0,
                        total_saved_products: 0,
                    });
                }
                
                // 가장 높은 pageId와 해당 페이지에서의 최대 indexInPage 찾기
                let mut max_page_id = 0i32;
                let mut max_index_in_page = 0i32;
                
                for product in &products {
                    if let (Some(page_id), Some(index_in_page)) = (product.page_id, product.index_in_page) {
                        if page_id > max_page_id {
                            max_page_id = page_id;
                            max_index_in_page = index_in_page;
                        } else if page_id == max_page_id && index_in_page > max_index_in_page {
                            max_index_in_page = index_in_page;
                        }
                    }
                }
                
                info!("📊 Local DB status: max_page_id={}, max_index_in_page={}, total_products={}", 
                      max_page_id, max_index_in_page, products.len());
                
                Ok(LocalDbStatus {
                    is_empty: false,
                    max_page_id: max_page_id.max(0) as u32,
                    max_index_in_page: max_index_in_page.max(0) as u32,
                    total_saved_products: products.len() as u32,
                })
            },
            None => {
                warn!("⚠️  Product repository not available - assuming empty DB");
                
                // DB 분석과 로컬 상태가 불일치할 수 있음을 경고
                warn!("⚠️  DB inconsistency possible: repository unavailable but analysis may show different results");
                
                Ok(LocalDbStatus {
                    is_empty: true,
                    max_page_id: 0,
                    max_index_in_page: 0,
                    total_saved_products: 0,
                })
            }
        }
    }
    
    /// 사이트 데이터 변화 분석
    async fn analyze_site_data_changes(&self, current_estimated_products: u32) -> DataChangeAnalysis {
        let previous_count = self.config.app_managed.last_crawl_product_count;
        
        match previous_count {
            None => DataChangeAnalysis::Initial,
            Some(prev_count) => {
                let change_percentage = if prev_count > 0 {
                    ((current_estimated_products as f64 - prev_count as f64) / prev_count as f64) * 100.0
                } else {
                    0.0
                };
                
                if current_estimated_products > prev_count {
                    DataChangeAnalysis::Increased { 
                        new_products: current_estimated_products - prev_count,
                        change_percentage,
                    }
                } else if current_estimated_products == prev_count {
                    DataChangeAnalysis::Stable
                } else {
                    DataChangeAnalysis::Decreased {
                        lost_products: prev_count - current_estimated_products,
                        change_percentage: -change_percentage,
                    }
                }
            }
        }
    }
    
    /// 다음 크롤링 페이지 범위 계산
    async fn calculate_next_crawling_pages(
        &self,
        local_db_status: &LocalDbStatus,
        total_pages_on_site: u32,
        products_on_last_page: u32,
        _estimated_products: u32,
        data_change_analysis: &DataChangeAnalysis,
    ) -> Result<CrawlingRangeRecommendation> {
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;
        
        // 데이터 변화에 따른 크롤링 전략 결정
        match data_change_analysis {
            DataChangeAnalysis::Initial => {
                info!("📊 Initial crawling - recommending full crawl");
                return Ok(CrawlingRangeRecommendation::Full);
            },
            DataChangeAnalysis::Decreased { lost_products, .. } => {
                warn!("📉 Site data decreased by {} products - recommending full recrawl", lost_products);
                return Ok(CrawlingRangeRecommendation::Full);
            },
            DataChangeAnalysis::Increased { new_products, .. } => {
                // 새로운 제품이 많이 추가된 경우 부분 크롤링
                let recommended_pages = (*new_products as f64 / products_per_page as f64).ceil() as u32;
                let limited_pages = recommended_pages.min(self.config.user.crawling.page_range_limit);
                
                info!("📈 Site data increased by {} products - recommending partial crawl of {} pages", 
                      new_products, limited_pages);
                return Ok(CrawlingRangeRecommendation::Partial(limited_pages));
            },
            DataChangeAnalysis::Stable => {
                // 안정적인 경우 기존 로직 적용
            }
        }
        
        // 기존 로직: 로컬 DB 상태 기반 계산
        if local_db_status.is_empty {
            return Ok(CrawlingRangeRecommendation::Full);
        }
        
        // 1단계: 로컬 DB에 마지막으로 저장된 제품의 '역순 절대 인덱스' 계산
        let last_saved_index = (local_db_status.max_page_id * products_per_page) + local_db_status.max_index_in_page;
        info!("📊 Last saved product index: {}", last_saved_index);
        
        // 2단계: 다음에 크롤링해야 할 제품의 '역순 절대 인덱스' 결정
        let next_product_index = last_saved_index + 1;
        info!("📊 Next product index to crawl: {}", last_saved_index);
        
        // 3단계: '역순 절대 인덱스'를 웹사이트 페이지 번호로 변환
        let total_products = ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;
        
        // 다음 제품이 전체 제품 수를 초과하는 경우 (모든 제품 크롤링 완료)
        if next_product_index >= total_products {
            info!("📊 All products have been crawled - no crawling needed");
            return Ok(CrawlingRangeRecommendation::None);
        }
        
        // '순차 인덱스'로 변환 (최신 제품이 0)
        let forward_index = (total_products - 1) - next_product_index;
        
        // 웹사이트 페이지 번호 계산
        let target_page_number = (forward_index / products_per_page) + 1;
        
        info!("📊 Target page to start crawling: {}", target_page_number);
        
        // 4단계: 크롤링 페이지 범위 결정
        let max_crawl_pages = self.config.user.crawling.page_range_limit;
        let start_page = target_page_number;
        let end_page = if start_page >= max_crawl_pages {
            start_page - max_crawl_pages + 1
        } else {
            1
        };
        
        let actual_pages_to_crawl = if start_page >= end_page {
            start_page - end_page + 1
        } else {
            start_page
        };
        
        info!("📊 Crawling range: pages {} to {} (total: {} pages)", 
              start_page, end_page, actual_pages_to_crawl);
        
        Ok(CrawlingRangeRecommendation::Partial(actual_pages_to_crawl))
    }
}

/// 로컬 DB 상태 정보
#[derive(Debug, Clone)]
struct LocalDbStatus {
    is_empty: bool,
    max_page_id: u32,
    max_index_in_page: u32,
    total_saved_products: u32,
}

/// 데이터 변화 분석 결과
#[derive(Debug, Clone)]
enum DataChangeAnalysis {
    Initial,
    Increased { new_products: u32, change_percentage: f64 },
    Decreased { lost_products: u32, change_percentage: f64 },
    Stable,
}

/// 컬렉터 설정 (Modern Rust 2024 준수)
/// 
/// ValidatedCrawlingConfig에서 검증된 값을 사용하여 하드코딩을 방지합니다.
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub batch_size: u32,
    pub max_concurrent: u32,
    pub concurrency: u32, // alias for max_concurrent  
    pub delay_between_requests: Duration,
    pub delay_ms: u64, // alias for delay_between_requests in milliseconds
    pub retry_attempts: u32,
    pub retry_max: u32, // alias for retry_attempts
}

impl CollectorConfig {
    /// ValidatedCrawlingConfig에서 CollectorConfig 생성
    /// 
    /// # Arguments
    /// * `validated_config` - 검증된 크롤링 설정
    /// 
    /// # Returns
    /// 설정값이 적용된 CollectorConfig
    #[must_use]
    pub fn from_validated(validated_config: &crate::application::validated_crawling_config::ValidatedCrawlingConfig) -> Self {
        let delay_ms = validated_config.request_delay().as_millis() as u64;
        
        Self {
            batch_size: validated_config.batch_size(),
            max_concurrent: validated_config.max_concurrent(),
            concurrency: validated_config.max_concurrent(), // alias
            delay_between_requests: validated_config.request_delay(),
            delay_ms,
            retry_attempts: validated_config.max_retries(),
            retry_max: validated_config.max_retries(), // alias
        }
    }
}

impl Default for CollectorConfig {
    /// 기본값은 ValidatedCrawlingConfig::default()에서 가져옴
    /// 하드코딩을 방지하기 위해 ValidatedCrawlingConfig를 사용
    fn default() -> Self {
        let validated_config = crate::application::validated_crawling_config::ValidatedCrawlingConfig::default();
        Self::from_validated(&validated_config)
    }
}

/// 헬스 스코어 계산 함수
fn calculate_health_score(response_time: Duration, total_pages: u32) -> f64 {
    // 응답 시간 기반 점수 (0.0 ~ 0.7) - 더 관대한 기준으로 수정
    let response_score = if response_time.as_millis() <= 2000 {
        0.7  // 2초 이하는 양호
    } else if response_time.as_millis() <= 5000 {
        0.5  // 5초 이하는 보통
    } else if response_time.as_millis() <= 10000 {
        0.3  // 10초 이하는 느림
    } else {
        0.1  // 10초 초과는 문제
    };
    
    // 페이지 수 기반 점수 (0.0 ~ 0.3) - 페이지 발견 여부가 더 중요
    let page_score = if total_pages > 0 {
        0.3
    } else {
        0.0
    };
    
    response_score + page_score
}

/// 제품 목록 수집 서비스 구현체
pub struct ProductListCollectorImpl {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
    config: CollectorConfig,
    status_checker: Arc<StatusCheckerImpl>,
}

impl ProductListCollectorImpl {
    pub fn new(
        http_client: Arc<tokio::sync::Mutex<HttpClient>>,
        data_extractor: Arc<MatterDataExtractor>,
        config: CollectorConfig,
        status_checker: Arc<StatusCheckerImpl>,
    ) -> Self {
        Self {
            http_client,
            data_extractor,
            config,
            status_checker,
        }
    }

    /// 🔥 이벤트 콜백을 지원하는 동시성 페이지 수집 메서드
    pub async fn collect_page_range_with_events<F1, F2>(
        &self,
        start_page: u32,
        end_page: u32,
        cancellation_token: Option<CancellationToken>,
        page_callback: F1,
        retry_callback: F2,
    ) -> Result<Vec<ProductUrl>>
    where
        F1: Fn(u32, String, u32, bool) -> Result<()> + Send + Sync + 'static,
        F2: Fn(String, String, String, u32, u32, String) -> Result<()> + Send + Sync + 'static,
    {
        let page_callback = Arc::new(page_callback);
        let retry_callback = Arc::new(retry_callback);
        
        // Handle descending range (older to newer) - typical for our use case
        let pages: Vec<u32> = if start_page > end_page {
            // Descending range: start from oldest (highest page number) to newest (lower page number)
            (end_page..=start_page).rev().collect()
        } else {
            // Ascending range: start from lowest to highest page number
            (start_page..=end_page).collect()
        };
        
        info!("🔍 Collecting from {} pages in range {} to {} with concurrent execution and events", 
              pages.len(), start_page, end_page);
        
        // 사이트 분석 정보를 가져와서 정확한 총 페이지 수와 마지막 페이지 제품 수 확인
        let (total_pages, products_on_last_page) = self.status_checker.discover_total_pages().await?;
        
        // 가장 큰 페이지 번호가 마지막 페이지 번호임
        let last_page_number = total_pages;
        let products_in_last_page = products_on_last_page;
        
        info!("📊 Site analysis result: total_pages={}, products_on_last_page={}", 
              total_pages, products_on_last_page);
        
        // PageIdCalculator 초기화
        let page_calculator = crate::utils::PageIdCalculator::new(last_page_number, products_in_last_page as usize);
        
        let max_concurrent = self.config.max_concurrent as usize;
        
        // 진정한 동시성 실행을 위한 세마포어 기반 처리
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        
        // 모든 페이지에 대해 즉시 태스크 생성 (하지만 세마포어로 제어)
        let mut tasks = Vec::new();
        
        info!("🚀 Creating {} concurrent tasks with semaphore control (max: {})", pages.len(), max_concurrent);
        
        for page in pages {
            // 취소 토큰 확인
            if let Some(ref token) = cancellation_token {
                if token.is_cancelled() {
                    warn!("🛑 Task creation cancelled for page {}", page);
                    break;
                }
            }
            
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let token_clone = cancellation_token.clone();
            let semaphore_clone = Arc::clone(&semaphore);
            let calculator = page_calculator.clone();
            let page_callback_clone = Arc::clone(&page_callback);
            let retry_callback_clone = Arc::clone(&retry_callback);
            
            let task = tokio::spawn(async move {
                // 실행 허가를 받을 때까지 대기
                let _permit = match semaphore_clone.acquire().await {
                    Ok(permit) => {
                        debug!("🔓 Acquired permit for page {}", page);
                        permit
                    },
                    Err(_) => {
                        error!("Failed to acquire semaphore permit for page {}", page);
                        return Err(anyhow!("Semaphore acquisition failed"));
                    }
                };
                
                // 취소 토큰 확인
                if let Some(ref token) = token_clone {
                    if token.is_cancelled() {
                        warn!("🛑 Task cancelled for page {}", page);
                        return Err(anyhow!("Task cancelled"));
                    }
                }
                
                // 실제 페이지 수집 작업
                let url = format!("https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver", page);
                
                let collect_result = async {
                    let mut client = http_client.lock().await;
                    let html = client.fetch_html_string(&url).await?;
                    drop(client);
                    
                    let doc = scraper::Html::parse_document(&html);
                    let url_strings = data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;
                    
                    // Convert URLs to ProductUrl with proper pageId and indexInPage calculation
                    let product_urls: Vec<ProductUrl> = url_strings
                        .into_iter()
                        .enumerate()
                        .map(|(index, url)| {
                            let calculation = calculator.calculate(page, index);
                            ProductUrl {
                                url,
                                page_id: calculation.page_id,
                                index_in_page: calculation.index_in_page,
                            }
                        })
                        .collect();
                    
                    Ok::<Vec<ProductUrl>, anyhow::Error>(product_urls)
                }.await;
                
                match collect_result {
                    Ok(product_urls) => {
                        debug!("✅ Page {} completed successfully with {} URLs", page, product_urls.len());
                        
                        // 🔥 성공 이벤트 발송
                        if let Err(e) = page_callback_clone(page, url.clone(), product_urls.len() as u32, true) {
                            warn!("Failed to emit page-crawled success event for page {}: {}", page, e);
                        }
                        
                        Ok((page, product_urls))
                    }
                    Err(e) => {
                        warn!("❌ Page {} collection failed: {}", page, e);
                        
                        // 🔥 실패 이벤트 발송
                        if let Err(emit_err) = page_callback_clone(page, url.clone(), 0, false) {
                            warn!("Failed to emit page-crawled failure event for page {}: {}", page, emit_err);
                        }
                        
                        // 🔥 재시도 시도 이벤트 발송
                        if let Err(emit_err) = retry_callback_clone(
                            format!("page_{}", page),
                            "page".to_string(),
                            url.clone(),
                            1,
                            2,
                            e.to_string()
                        ) {
                            warn!("Failed to emit retry-attempt event for page {}: {}", page, emit_err);
                        }
                        
                        // 페이지 재시도 (1회만)
                        info!("🔄 Retrying page {} collection", page);
                        
                        let retry_result = async {
                            let mut client = http_client.lock().await;
                            let html = client.fetch_html_string(&url).await?;
                            drop(client);
                            
                            let doc = scraper::Html::parse_document(&html);
                            let url_strings = data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;
                            
                            let product_urls: Vec<ProductUrl> = url_strings
                                .into_iter()
                                .enumerate()
                                .map(|(index, url)| {
                                    let calculation = calculator.calculate(page, index);
                                    ProductUrl {
                                        url,
                                        page_id: calculation.page_id,
                                        index_in_page: calculation.index_in_page,
                                    }
                                })
                                .collect();
                            
                            Ok::<Vec<ProductUrl>, anyhow::Error>(product_urls)
                        }.await;
                        
                        match retry_result {
                            Ok(retry_urls) => {
                                info!("✅ Page {} retry successful with {} URLs", page, retry_urls.len());
                                
                                // 🔥 재시도 성공 이벤트 발송
                                if let Err(emit_err) = retry_callback_clone(
                                    format!("page_{}", page),
                                    "page".to_string(),
                                    url.clone(),
                                    2,
                                    2,
                                    "Retry successful".to_string()
                                ) {
                                    warn!("Failed to emit retry-success event for page {}: {}", page, emit_err);
                                }
                                
                                // 최종 성공 이벤트 발송
                                if let Err(emit_err) = page_callback_clone(page, url, retry_urls.len() as u32, true) {
                                    warn!("Failed to emit page-crawled retry success event for page {}: {}", page, emit_err);
                                }
                                
                                Ok((page, retry_urls))
                            }
                            Err(retry_e) => {
                                warn!("❌ Page {} retry also failed: {}", page, retry_e);
                                
                                // 🔥 재시도 최종 실패 이벤트 발송
                                if let Err(emit_err) = retry_callback_clone(
                                    format!("page_{}", page),
                                    "page".to_string(),
                                    url,
                                    2,
                                    2,
                                    retry_e.to_string()
                                ) {
                                    warn!("Failed to emit retry-failed event for page {}: {}", page, emit_err);
                                }
                                
                                // 페이지 실패는 전체 작업을 중단시키지 않음 - 빈 결과 반환
                                Ok((page, Vec::new()))
                            }
                        }
                    }
                }
            });
            
            tasks.push(task);
        }
        
        info!("✅ Created {} tasks, waiting for all to complete with concurrent execution", tasks.len());
        
        // 모든 태스크가 완료될 때까지 기다림
        let results = futures::future::join_all(tasks).await;
        
        // 결과 수집
        let mut all_urls = Vec::new();
        let mut successful_pages = 0;
        let mut failed_pages = 0;
        
        for result in results {
            match result {
                Ok(Ok((page, urls))) => {
                    all_urls.extend(urls);
                    successful_pages += 1;
                    debug!("✅ Page {} completed successfully", page);
                },
                Ok(Err(e)) => {
                    error!("❌ Page collection failed: {}", e);
                    failed_pages += 1;
                },
                Err(e) => {
                    error!("❌ Task join failed: {}", e);
                    failed_pages += 1;
                }
            }
        }
        
        info!("📊 Concurrent collection with events completed: {} successful, {} failed, {} total URLs", 
              successful_pages, failed_pages, all_urls.len());
        
        Ok(all_urls)
    }
}

#[async_trait]
impl ProductListCollector for ProductListCollectorImpl {
    async fn collect_page_batch(&self, pages: &[u32]) -> Result<Vec<ProductUrl>> {
        let mut all_urls = Vec::new();
        for &page in pages {
            match self.collect_page_range(page, page).await {
                Ok(mut urls) => all_urls.append(&mut urls),
                Err(e) => {
                    error!("Failed to collect page {}: {}", page, e);
                    continue;
                }
            }
        }
        Ok(all_urls)
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn collect_all_pages(&self, total_pages: u32) -> Result<Vec<ProductUrl>> {
        info!("🔍 Collecting from {} pages with parallel processing", total_pages);
        
        // Use the existing parallel implementation from collect_page_range
        self.collect_page_range(1, total_pages).await
    }
    
    async fn collect_page_range(&self, start_page: u32, end_page: u32) -> Result<Vec<ProductUrl>> {
        // Handle descending range (older to newer) - typical for our use case
        let pages: Vec<u32> = if start_page > end_page {
            // Descending range: start from oldest (highest page number) to newest (lower page number)
            (end_page..=start_page).rev().collect()
        } else {
            // Ascending range: start from lowest to highest page number
            (start_page..=end_page).collect()
        };
        
        info!("🔍 Collecting from {} pages in range {} to {} with true concurrent execution", 
              pages.len(), start_page, end_page);
        
        // 사이트 분석 정보를 가져와서 정확한 총 페이지 수와 마지막 페이지 제품 수 확인
        let (total_pages, products_on_last_page) = self.status_checker.discover_total_pages().await?;
        
        // 가장 큰 페이지 번호가 마지막 페이지 번호임
        let last_page_number = total_pages;
        let products_in_last_page = products_on_last_page;
        
        info!("📊 Site analysis result: total_pages={}, products_on_last_page={}", 
              total_pages, products_on_last_page);
        
        // PageIdCalculator 초기화
        let page_calculator = crate::utils::PageIdCalculator::new(last_page_number, products_in_last_page as usize);
        
        let max_concurrent = self.config.max_concurrent as usize;
        
        // Phase 5 Implementation: 진정한 동시성 실행을 위한 세마포어 기반 처리
        // 1. 세마포어 생성: max_concurrent 개의 permit만 허용
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        
        // 2. 모든 페이지에 대해 즉시 태스크 생성 (하지만 세마포어로 제어)
        let mut tasks = Vec::new();
        
        info!("🚀 Creating {} concurrent tasks with semaphore control (max: {})", pages.len(), max_concurrent);
        
        for page in pages {
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let semaphore_clone = Arc::clone(&semaphore);
            let calculator = page_calculator.clone();
            
            // 3. 각 태스크는 세마포어 permit을 획득한 후 실행
            let task = tokio::spawn(async move {
                // 실행 허가를 받을 때까지 대기 (진정한 동시성 제어)
                let _permit = match semaphore_clone.acquire().await {
                    Ok(permit) => {
                        debug!("🔓 Acquired permit for page {}", page);
                        permit
                    },
                    Err(_) => {
                        error!("Failed to acquire semaphore permit for page {}", page);
                        return Err(anyhow!("Semaphore acquisition failed"));
                    }
                };
                
                // 실제 페이지 수집 작업
                let url = format!("https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver", page);
                let mut client = http_client.lock().await;
                let html = client.fetch_html_string(&url).await?;
                drop(client);
                
                let doc = scraper::Html::parse_document(&html);
                let url_strings = data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;
                
                // Convert URLs to ProductUrl with proper pageId and indexInPage calculation
                let product_urls: Vec<ProductUrl> = url_strings
                    .into_iter()
                    .enumerate()
                    .map(|(index, url)| {
                        let calculation = calculator.calculate(page, index);
                        ProductUrl {
                            url,
                            page_id: calculation.page_id,
                            index_in_page: calculation.index_in_page,
                        }
                    })
                    .collect();
                
                debug!("🔗 Extracted {} URLs from page {} (permit released)", product_urls.len(), page);
                Ok::<(u32, Vec<ProductUrl>), anyhow::Error>((page, product_urls))
                // _permit이 여기서 자동으로 drop되어 다음 태스크가 실행될 수 있음
            });
            
            tasks.push(task);
        }
        
        info!("✅ Created {} tasks, waiting for all to complete with concurrent execution", tasks.len());
        
        // 4. 모든 태스크가 완료될 때까지 기다림 (진정한 파이프라인 실행)
        let results = futures::future::join_all(tasks).await;
        
        // 결과 수집
        let mut all_urls = Vec::new();
        let mut successful_pages = 0;
        let mut failed_pages = 0;
        
        for result in results {
            match result {
                Ok(Ok((page, urls))) => {
                    all_urls.extend(urls);
                    successful_pages += 1;
                    debug!("✅ Page {} completed successfully", page);
                },
                Ok(Err(e)) => {
                    error!("❌ Page collection failed: {}", e);
                    failed_pages += 1;
                },
                Err(e) => {
                    error!("❌ Task join failed: {}", e);
                    failed_pages += 1;
                }
            }
        }
        
        info!("📊 Concurrent collection completed: {} successful, {} failed, {} total URLs", 
              successful_pages, failed_pages, all_urls.len());
        
        Ok(all_urls)
    }
    
    async fn collect_single_page(&self, page: u32) -> Result<Vec<ProductUrl>> {
        // 사이트 분석 정보를 먼저 가져오기
        let (total_pages, products_on_last_page) = self.status_checker.discover_total_pages().await?;
        
        // 가장 큰 페이지 번호가 마지막 페이지 번호임
        let last_page_number = total_pages;
        let products_in_last_page = products_on_last_page;
        
        let page_calculator = crate::utils::PageIdCalculator::new(last_page_number, products_in_last_page as usize);
        
        let url = crate::infrastructure::config::utils::matter_products_page_url_simple(page);
        let mut client = self.http_client.lock().await;
        let html = client.fetch_html_string(&url).await?;
        drop(client);
        
        let doc = scraper::Html::parse_document(&html);
        let url_strings = self.data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;
        
        // Convert URLs to ProductUrl with proper pageId and indexInPage calculation
        let product_urls: Vec<ProductUrl> = url_strings
            .into_iter()
            .enumerate()
            .map(|(index, url)| {
                let calculation = page_calculator.calculate(page, index);
                ProductUrl {
                    url,
                    page_id: calculation.page_id,
                    index_in_page: calculation.index_in_page,
                                              }
            })
            .collect();
        
        debug!("🔗 Extracted {} URLs from page {}", product_urls.len(), page);
        Ok(product_urls)
    }
    
    async fn collect_page_range_with_cancellation(&self, start_page: u32, end_page: u32, cancellation_token: CancellationToken) -> Result<Vec<ProductUrl>> {
        // Handle descending range (older to newer) - typical for our use case
        let pages: Vec<u32> = if start_page > end_page {
            // Descending range: start from oldest (highest page number) to newest (lower page number)
            (end_page..=start_page).rev().collect()
        } else {
            // Ascending range: start from lowest to highest page number
            (start_page..=end_page).collect()
        };
        
        info!("🔍 Collecting from {} pages in range {} to {} with cancellation support", 
              pages.len(), start_page, end_page);
        
        // 사이트 분석 정보를 가져와서 정확한 총 페이지 수와 마지막 페이지 제품 수 확인
        let (total_pages, products_on_last_page) = self.status_checker.discover_total_pages().await?;
        
        // 가장 큰 페이지 번호가 마지막 페이지 번호임
        let last_page_number = total_pages;
        let products_in_last_page = products_on_last_page;
        
        info!("📊 Site analysis result: total_pages={}, products_on_last_page={}", 
              total_pages, products_on_last_page);
        
        let max_concurrent = self.config.max_concurrent as usize;
        
        // Phase 5 Implementation: 진정한 동시성 실행을 위한 세마포어 기반 처리
        // 1. 세마포어 생성: max_concurrent 개의 permit만 허용
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        
        // 2. 모든 페이지에 대해 즉시 태스크 생성 (하지만 세마포어로 제어)
        let mut tasks = Vec::new();
        
        info!("🚀 Creating {} concurrent tasks with semaphore control (max: {})", pages.len(), max_concurrent);
        
        for page in pages {
            // 취소 토큰 확인
            if cancellation_token.is_cancelled() {
                warn!("🛑 Task creation cancelled for page {}", page);
                break;
            }
            
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let token_clone = cancellation_token.clone();
            let semaphore_clone = Arc::clone(&semaphore);
            
            // 사이트 분석 결과를 클로저로 전달
            let last_page_number_clone = last_page_number;
            let products_in_last_page_clone = products_in_last_page;
            
            // 3. 각 태스크는 세마포어 permit을 획득한 후 실행
            let task = tokio::spawn(async move {
                // 실행 허가를 받을 때까지 대기
                let _permit = match semaphore_clone.acquire().await {
                    Ok(permit) => {
                        debug!("🔓 Acquired permit for page {}", page);
                        permit
                    },
                    Err(_) => {
                        error!("Failed to acquire semaphore permit for page {}", page);
                        return Err(anyhow!("Semaphore acquisition failed"));
                    }
                };
                
                // 작업 시작 전 취소 확인
                if token_clone.is_cancelled() {
                    warn!("🛑 Task cancelled before execution for page {}", page);
                    return Err(anyhow!("Task cancelled"));
                }
                
                // 실제 페이지 수집 작업
                let url = crate::infrastructure::config::utils::matter_products_page_url_simple(page);
                let mut client = http_client.lock().await;
                let html = client.fetch_html_string(&url).await?;
                drop(client);
                
                // 중간에 취소 확인
                if token_clone.is_cancelled() {
                    warn!("🛑 Task cancelled during processing for page {}", page);
                    return Err(anyhow!("Task cancelled during processing"));
                }
                
                let doc = scraper::Html::parse_document(&html);
                let url_strings = data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;
                
                // 사이트 분석 결과를 사용하여 정확한 PageIdCalculator 생성
                let page_calculator = crate::utils::PageIdCalculator::new(last_page_number_clone, products_in_last_page_clone as usize);
                
                // Convert URLs to ProductUrl with proper pageId and indexInPage calculation
                let product_urls: Vec<ProductUrl> = url_strings
                    .into_iter()
                    .enumerate()
                    .map(|(index, url)| {
                        let calculation = page_calculator.calculate(page, index);
                        ProductUrl {
                            url,
                            page_id: calculation.page_id,
                            index_in_page: calculation.index_in_page,
                        }
                    })
                    .collect();
                
                debug!("🔗 Extracted {} URLs from page {} (permit released)", product_urls.len(), page);
                Ok::<(u32, Vec<ProductUrl>), anyhow::Error>((page, product_urls))
                // _permit이 여기서 자동으로 drop되어 다음 태스크가 실행될 수 있음
            });
            
            tasks.push(task);
        }
        
        info!("✅ Created {} tasks, waiting for all to complete with concurrent execution", tasks.len());
        
        // 4. 모든 태스크가 완료될 때까지 기다림 (진정한 파이프라인 실행)
        let results = futures::future::join_all(tasks).await;
        
        // 결과 수집
        let mut all_urls = Vec::new();
        let mut successful_pages = 0;
        let mut failed_pages = 0;
        
        for result in results {
            match result {
                Ok(Ok((page, urls))) => {
                    all_urls.extend(urls);
                    successful_pages += 1;
                    debug!("✅ Page {} completed successfully", page);
                },
                Ok(Err(e)) => {
                    error!("❌ Page collection failed: {}", e);
                    failed_pages += 1;
                },
                Err(e) => {
                    error!("❌ Task join failed: {}", e);
                    failed_pages += 1;
                }
            }
        }
        
        info!("📊 Concurrent collection completed: {} successful, {} failed, {} total URLs", 
              successful_pages, failed_pages, all_urls.len());
        
        Ok(all_urls)
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
        // IntegratedProductRepository는 get_all_products 메서드를 가지므로 이를 사용
        let all_products = self.product_repo.get_all_products().await.unwrap_or_default();
        let total_products = all_products.len();
        
        // 기본 분석 반환 - 필드 스키마에 맞게 수정
        Ok(DatabaseAnalysis {
            total_products: total_products as u32,
            unique_products: total_products as u32,
            duplicate_count: 0,
            last_update: Some(chrono::Utc::now()),
            missing_fields_analysis: FieldAnalysis {
                missing_company: 0,
                missing_model: 0,
                missing_matter_version: 0,
                missing_connectivity: 0,
                missing_certification_date: 0,
            },
            data_quality_score: 0.85,
        })
    }

    async fn recommend_processing_strategy(&self) -> Result<ProcessingStrategy> {
        Ok(ProcessingStrategy {
            recommended_batch_size: 100,
            recommended_concurrency: 10,
            should_skip_duplicates: true,
            should_update_existing: false,
            priority_urls: Vec::new(),
        })
    }

    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis> {
        Ok(DuplicateAnalysis {
            total_duplicates: 0,
            duplicate_groups: Vec::new(),
            duplicate_percentage: 0.0,
        })
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
        http_client: Arc<tokio::sync::Mutex<HttpClient>>,
        data_extractor: Arc<MatterDataExtractor>,
        config: CollectorConfig,
    ) -> Self {
        Self {
            http_client,
            data_extractor,
            config,
        }
    }
}

#[async_trait]
impl ProductDetailCollector for ProductDetailCollectorImpl {
    async fn collect_details(&self, product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>> {
        info!("Collecting details for {} products", product_urls.len());
        
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent as usize));
        let mut tasks = Vec::new();
        
        for product_url in product_urls {
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let url = product_url.url.clone();
            let permit = Arc::clone(&semaphore);
            let delay = self.config.delay_ms;
            
            let task = tokio::spawn(async move {
                let _permit = permit.acquire().await.unwrap();
                
                tokio::time::sleep(Duration::from_millis(delay)).await;
                
                let mut client = http_client.lock().await;
                let html = client.fetch_html_string(&url).await?;
                drop(client);
                
                let doc = scraper::Html::parse_document(&html);
                let detail = data_extractor.extract_product_detail(&doc, url.clone())?;
                
                Ok::<ProductDetail, anyhow::Error>(detail)
            });
            
            tasks.push(task);
        }
        
        let results = futures::future::join_all(tasks).await;
        let mut details = Vec::new();
        
        for result in results {
            match result {
                Ok(Ok(detail)) => details.push(detail),
                Ok(Err(e)) => warn!("Failed to collect product detail: {}", e),
                Err(e) => warn!("Task failed: {}", e),
            }
        }
        
        info!("Successfully collected {} product details", details.len());
        Ok(details)
    }

    async fn collect_details_with_cancellation(
        &self,
        product_urls: &[ProductUrl],
        cancellation_token: CancellationToken,
    ) -> Result<Vec<ProductDetail>> {
        info!("Collecting details for {} products with cancellation support", product_urls.len());
        
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent as usize));
        let mut tasks = Vec::new();
        
        for product_url in product_urls {
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let url = product_url.url.clone();
            let permit = Arc::clone(&semaphore);
            let delay = self.config.delay_ms;
            let token = cancellation_token.clone();
            
            let task = tokio::spawn(async move {
                if token.is_cancelled() {
                    return Err(anyhow!("Task cancelled"));
                }
                
                let _permit = permit.acquire().await.unwrap();
                
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(delay)) => {},
                    _ = token.cancelled() => return Err(anyhow!("Task cancelled during delay")),
                }
                
                if token.is_cancelled() {
                    return Err(anyhow!("Task cancelled"));
                }
                
                let mut client = http_client.lock().await;
                let html = client.fetch_html_string(&url).await?;
                drop(client);
                
                if token.is_cancelled() {
                    return Err(anyhow!("Task cancelled"));
                }
                
                let doc = scraper::Html::parse_document(&html);
                let detail = data_extractor.extract_product_detail(&doc, url.clone())?;
                
                Ok::<ProductDetail, anyhow::Error>(detail)
            });
            
            tasks.push(task);
        }
        
        let results = futures::future::join_all(tasks).await;
        let mut details = Vec::new();
        
        for result in results {
            match result {
                Ok(Ok(detail)) => details.push(detail),
                Ok(Err(e)) => {
                    if !cancellation_token.is_cancelled() {
                        warn!("Failed to collect product detail: {}", e);
                    }
                },
                Err(e) => {
                    if !cancellation_token.is_cancelled() {
                        warn!("Task failed: {}", e);
                    }
                },
            }
        }
        
        info!("Successfully collected {} product details", details.len());
        Ok(details)
    }

    async fn collect_single_product(&self, product_url: &ProductUrl) -> Result<ProductDetail> {
        self.collect_details(&[product_url.clone()]).await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to collect product detail"))
    }

    async fn collect_product_batch(&self, product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>> {
        self.collect_details(product_urls).await
    }
}

/// 지능형 크롤링 범위 계산기
pub struct CrawlingRangeCalculator {
    product_repo: Arc<IntegratedProductRepository>,
    config: AppConfig,
}

impl CrawlingRangeCalculator {
    pub fn new(
        product_repo: Arc<IntegratedProductRepository>,
        config: AppConfig,
    ) -> Self {
        Self {
            product_repo,
            config,
        }
    }
    
    /// 다음 크롤링 범위 계산
    pub async fn calculate_next_crawling_range(
        &self,
        total_pages: u32,
        products_on_last_page: u32,
    ) -> Result<Option<(u32, u32)>> {
        info!("🎯 입력 파라미터: total_pages={}, products_on_last_page={}", 
              total_pages, products_on_last_page);
        
        // 데이터베이스에서 현재 제품 정보 가져오기
        let all_products = match self.product_repo.get_all_products().await {
            Ok(products) => {
                info!("✅ Successfully retrieved {} products from database", products.len());
                products
            },
            Err(e) => {
                error!("❌ Failed to get products from database: {}", e);
                Vec::new()
            }
        };
        
        if all_products.is_empty() {
            info!("📋 Database is empty - starting from the last page (page {})", total_pages);
            let end_page = (total_pages.saturating_sub(self.config.user.crawling.page_range_limit - 1)).max(1);
            return Ok(Some((total_pages, end_page)));
        }
        
        // 가장 높은 page_id 찾기 (역순이므로 가장 작은 실제 페이지 번호)
        let max_page_id = all_products.iter()
            .filter_map(|p| p.page_id)
            .max()
            .unwrap_or(0);
            
        info!("🔍 Current max page_id in database: {}", max_page_id);
        
        // page_id에서 실제 페이지 번호로 변환
        // page_id 0 = 485페이지, page_id 1 = 484페이지, ..., page_id 5 = 480페이지
        let last_crawled_page = total_pages - max_page_id as u32;
        info!("📍 Last crawled page: {} (page_id: {})", last_crawled_page, max_page_id);
        
        // 다음 크롤링할 범위 계산
        // 현재 페이지의 제품 수집 상태 확인
        let current_page_products = all_products.iter()
            .filter(|p| p.page_id == Some(max_page_id))
            .count();
        
        let expected_products_on_current_page = if last_crawled_page == total_pages {
            // 마지막 페이지 (485페이지)라면 products_on_last_page만큼 있어야 함
            products_on_last_page as usize
        } else {
            // 다른 페이지라면 12개가 있어야 함
            DEFAULT_PRODUCTS_PER_PAGE as usize
        };
        
        info!("🔍 Current page {} has {}/{} products", 
              last_crawled_page, current_page_products, expected_products_on_current_page);
        
        // 다음 크롤링 시작 페이지 결정
        let start_page = if current_page_products < expected_products_on_current_page {
            // 현재 페이지가 완전히 수집되지 않았다면 현재 페이지부터 시작
            last_crawled_page
        } else {
            // 현재 페이지가 완료되었다면 다음 페이지부터 시작
            if last_crawled_page > 1 {
                last_crawled_page - 1
            } else {
                info!("🏁 All pages have been crawled");
                return Ok(None);
            }
        };
        
        // 크롤링 범위 제한 적용
        let end_page = (start_page.saturating_sub(self.config.user.crawling.page_range_limit - 1)).max(1);
        
        info!("✅ Next crawling range: {}페이지 → {}페이지 (역순, 최대 {}페이지)", 
              start_page, end_page, self.config.user.crawling.page_range_limit);
        Ok(Some((start_page, end_page)))
    }
}

/// ProductDetail을 Product로 변환하는 헬퍼 함수
pub fn product_detail_to_product(detail: ProductDetail) -> Product {
    let mut product = Product {
        id: detail.id.clone(), // Use detail's id if available
        url: detail.url,
        manufacturer: detail.manufacturer,
        model: detail.model,
        certificate_id: detail.certification_id,
        page_id: detail.page_id,
        index_in_page: detail.index_in_page,
        created_at: detail.created_at,
        updated_at: detail.updated_at,
    };
    
    // Generate ID if not already set
    if product.id.is_none() {
        product.generate_id();
    }
    
    product
}

// Additional trait implementations for service-based architecture

#[async_trait]
impl DatabaseAnalyzer for StatusCheckerImpl {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis> {
        // Placeholder implementation for service-based architecture
        Ok(DatabaseAnalysis {
            total_products: 0,
            unique_products: 0,
            duplicate_count: 0,
            missing_fields_analysis: FieldAnalysis {
                missing_company: 0,
                missing_model: 0,
                missing_matter_version: 0,
                missing_connectivity: 0,
                missing_certification_date: 0,
            },
            data_quality_score: 0.0,
            last_update: Some(chrono::Utc::now()),
        })
    }

    async fn recommend_processing_strategy(&self) -> Result<ProcessingStrategy> {
        Ok(ProcessingStrategy {
            recommended_batch_size: 100,
            recommended_concurrency: 10,
            should_skip_duplicates: true,
            should_update_existing: false,
            priority_urls: Vec::new(),
        })
    }

    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis> {
        Ok(DuplicateAnalysis {
            total_duplicates: 0,
            duplicate_groups: Vec::new(),
            duplicate_percentage: 0.0,
        })
    }
}

#[async_trait]
impl ProductDetailCollector for ProductListCollectorImpl {
    async fn collect_details(&self, _product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>> {
        // Placeholder implementation for service-based architecture
        Ok(Vec::new())
    }

    async fn collect_details_with_cancellation(
        &self, 
        _product_urls: &[ProductUrl], 
        _cancellation_token: CancellationToken
    ) -> Result<Vec<ProductDetail>> {
        // Placeholder implementation for service-based architecture
        Ok(Vec::new())
    }

    async fn collect_single_product(&self, _product_url: &ProductUrl) -> Result<ProductDetail> {
        // Placeholder implementation for service-based architecture
        Err(anyhow!("Not implemented"))
    }

    async fn collect_product_batch(&self, _product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>> {
        // Placeholder implementation for service-based architecture
        Ok(Vec::new())
    }
}

impl CrawlingRangeCalculator {
    /// 간단한 진행 상황 분석 (smart_crawling 명령어용)
    pub async fn analyze_simple_progress(
        &self,
        total_pages_on_site: u32,
        products_on_last_page: u32,
    ) -> Result<crate::domain::events::CrawlingProgress> {
        // 로컬 DB 상태 확인
        let all_products = match self.product_repo.get_all_products().await {
            Ok(products) => {
                info!("📊 Successfully retrieved {} products from database", products.len());
                products
            },
            Err(e) => {
                error!("❌ Failed to get products from database: {}", e);
                Vec::new()
            }
        };
        let saved_products = all_products.len() as u32;
        
        // 총 제품 수 추정
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;
        let total_estimated_products = ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;
        
        // 진행률 계산
        let percentage = if total_estimated_products > 0 {
            (saved_products as f64 / total_estimated_products as f64) * 100.0
        } else {
            0.0
        };
        
        // 가장 높은 pageId와 indexInPage 찾기
        let mut max_page_id = 0i32;
        let mut max_index_in_page = 0i32;
        
        for product in &all_products {
            let page_id = product.page_id.unwrap_or(0);
            let index_in_page = product.index_in_page.unwrap_or(0);
            
            if page_id > max_page_id {
                max_page_id = page_id;
                max_index_in_page = index_in_page;
            } else if page_id == max_page_id && index_in_page > max_index_in_page {
                max_index_in_page = index_in_page;
            }
        }
        
        // 실제 페이지 번호로 변환 (page_id 0 = 마지막 페이지)
        let actual_last_crawled_page = if max_page_id >= 0 {
            total_pages_on_site - max_page_id as u32
        } else {
            0
        };
        
        info!("📊 Progress: {}/{} products ({:.1}%), last crawled page: {} (page_id: {})", 
              saved_products, total_estimated_products, percentage, 
              actual_last_crawled_page, max_page_id);
        
        Ok(crate::domain::events::CrawlingProgress {
            current: saved_products,
            total: total_estimated_products,
            percentage,
            current_stage: if percentage >= 100.0 { 
                crate::domain::events::CrawlingStage::DatabaseSave 
            } else { 
                crate::domain::events::CrawlingStage::Idle 
            },
            current_step: format!("Saved {} of {} products", saved_products, total_estimated_products),
            status: if percentage >= 100.0 { 
                crate::domain::events::CrawlingStatus::Completed 
            } else { 
                crate::domain::events::CrawlingStatus::Idle 
            },
            message: format!("Progress: {:.1}%", percentage),
            remaining_time: None,
            elapsed_time: 0,
            new_items: 0,
            updated_items: 0,
            current_batch: Some(max_page_id as u32),
            total_batches: Some(total_pages_on_site),
            errors: 0,
            timestamp: chrono::Utc::now(),
        })
    }
    
    pub async fn analyze_crawling_progress(
        &self,
        _url: &str,
        _config: &CrawlingConfig,
        _database_analysis: &DatabaseAnalysis,
    ) -> Result<crate::domain::events::CrawlingProgress> {
        // Placeholder implementation
        Ok(crate::domain::events::CrawlingProgress {
            current: 0,
            total: 1,
            percentage: 0.0,
            current_stage: crate::domain::events::CrawlingStage::Idle,
            current_step: "Waiting".to_string(),
            status: crate::domain::events::CrawlingStatus::Idle,
            message: "Ready".to_string(),
            remaining_time: None,
            elapsed_time: 0,
            new_items: 0,
            updated_items: 0,
            current_batch: Some(0),
            total_batches: Some(1),
            errors: 0,
            timestamp: chrono::Utc::now(),
        })
    }
}

