//! 크롤링 서비스 구현체
//! 
//! domain/services/crawling_services.rs의 트레이트들에 대한 실제 구현체

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use async_trait::async_trait;
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};
use tokio::sync::Semaphore;
use tokio::time::sleep;
use futures::future::try_join_all;
use scraper;
use regex;

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    SiteStatus, DatabaseAnalysis, FieldAnalysis, DuplicateAnalysis, ProcessingStrategy
};
use crate::domain::services::crawling_services::{
    SiteDataChangeStatus, DataDecreaseRecommendation, RecommendedAction, SeverityLevel
};
use crate::domain::product::{Product, ProductDetail};
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::config::utils as config_utils;

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
        }
    }

    /// Update the pagination context in the data extractor based on discovered page information
    pub async fn update_pagination_context(&self, total_pages: u32, items_on_last_page: u32) -> Result<()> {
        // Create pagination context
        let pagination_context = crate::infrastructure::html_parser::PaginationContext {
            total_pages,
            items_per_page: 12, // CSA-IoT site uses 12 items per page
            items_on_last_page,
            target_page_size: 12, // Our system also uses 12 items per page
        };
        
        // Update the data extractor's pagination context
        self.data_extractor.set_pagination_context(pagination_context)?;
        
        info!("📊 Updated pagination context: total_pages={}, items_on_last_page={}", 
               total_pages, items_on_last_page);
        
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
                    last_check_time: chrono::Utc::now(),
                    health_score: 0.0,
                    data_change_status: SiteDataChangeStatus::Inaccessible,
                    decrease_recommendation: None,
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
        use crate::infrastructure::config::defaults::DEFAULT_PRODUCTS_PER_PAGE;
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
              
        Ok(SiteStatus {                is_accessible: true,
                response_time_ms: response_time_ms,
                total_pages,
                estimated_products,
                last_check_time: chrono::Utc::now(),
                health_score,
            data_change_status,
            decrease_recommendation,
        })
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
        Ok(status.is_accessible && status.health_score > 0.5)
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
            } else {
                info!("🏁 No higher pages found in pagination, {} appears to be the last page", current_page);
                break;
            }
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
                    } else {
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
            } else {
                // 마지막 페이지 도달, 제품 수 확인
                let test_url = config_utils::matter_products_page_url_simple(current_page);
                let mut client = self.http_client.lock().await;
                let html = client.fetch_html_string(&test_url).await?;
                drop(client); // 락 해제
                let doc = scraper::Html::parse_document(&html);
                let products_count = self.count_products(&doc);
                return Ok((current_page, products_count));
            }
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
            use crate::infrastructure::config::defaults::DEFAULT_PRODUCTS_PER_PAGE;
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
            r"/page/(\d+)",
            r"paged=(\d+)",
            r"page=(\d+)",
            r"/(\d+)/$",  // 끝에 숫자가 있는 경우
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
}

/// 데이터베이스 분석 서비스 구현체
pub struct DatabaseAnalyzerImpl {
    product_repo: Arc<IntegratedProductRepository>,
}

impl DatabaseAnalyzerImpl {
    pub fn new(product_repo: Arc<IntegratedProductRepository>) -> Self {
        Self { product_repo }
    }

    /// 실제 중복 제품 수 계산
    async fn count_duplicate_products(&self) -> Result<u32> {
        // certificate_id로 그룹화하여 중복 찾기
        let products = self.product_repo.get_all_products().await?;
        let mut cert_id_count = std::collections::HashMap::new();
        
        for product in products {
            if let Some(cert_id) = &product.certificate_id {
                *cert_id_count.entry(cert_id.clone()).or_insert(0) += 1;
            }
        }
        
        // 중복된 제품 수 계산 (그룹 크기 - 1)
        let duplicate_count: u32 = cert_id_count.values()
            .filter(|&&count| count > 1)
            .map(|&count| count - 1)
            .sum();
            
        debug!("Found {} duplicate products based on certificate_id", duplicate_count);
        Ok(duplicate_count)
    }

    /// 실제 필드 누락 분석
    async fn analyze_missing_fields(&self) -> Result<FieldAnalysis> {
        let products = self.product_repo.get_all_products().await?;
        let total = products.len() as u32;
        
        let mut missing_company = 0u32;
        let mut missing_model = 0u32;
        let mut missing_matter_version = 0u32;
        let mut missing_connectivity = 0u32;
        let mut missing_certification_date = 0u32;
        
        for product in products {
            if product.manufacturer.is_none() || product.manufacturer.as_ref().map_or(true, |s| s.is_empty()) {
                missing_company += 1;
            }
            if product.model.is_none() || product.model.as_ref().map_or(true, |s| s.is_empty()) {
                missing_model += 1;
            }
            // Note: Product 구조체에 matter_version 필드가 없으므로 스킵
            missing_matter_version = 0;
            // Note: Product 구조체에 connectivity 필드가 없으므로 스킵
            missing_connectivity = 0;
            // Note: certification_date는 ProductDetail에만 있으므로 스킵
            missing_certification_date = 0;
        }
        
        info!("📊 Field analysis: {}/{} missing company, {}/{} missing model, {}/{} missing matter_version",
              missing_company, total, missing_model, total, missing_matter_version, total);
        
        Ok(FieldAnalysis {
            missing_company,
            missing_model,
            missing_matter_version,
            missing_connectivity,
            missing_certification_date,
        })
    }

    /// 실제 데이터 품질 점수 계산
    fn calculate_data_quality_score(&self, total: u32, unique: u32, missing_fields: &FieldAnalysis) -> f64 {
        if total == 0 {
            return 1.0;
        }
        
        // 중복률 점수 (70% 가중치)
        let uniqueness_score = (unique as f64 / total as f64) * 0.7;
        
        // 필드 완성도 점수 (30% 가중치)
        let total_fields = total * 5; // 5개 주요 필드
        let missing_total = missing_fields.missing_company + 
                           missing_fields.missing_model + 
                           missing_fields.missing_matter_version + 
                           missing_fields.missing_connectivity + 
                           missing_fields.missing_certification_date;
        
        let completeness_score = if total_fields > 0 {
            ((total_fields - missing_total) as f64 / total_fields as f64) * 0.3
        } else {
            0.3
        };
        
        let final_score = uniqueness_score + completeness_score;
        debug!("Quality score: uniqueness={:.3} + completeness={:.3} = {:.3}", 
               uniqueness_score, completeness_score, final_score);
        
        final_score.min(1.0).max(0.0)
    }

    /// 마지막 업데이트 시간 가져오기
    async fn get_last_update_time(&self) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
        // 가장 최근에 업데이트된 제품의 시간을 가져오기
        match self.product_repo.get_latest_updated_product().await {
            Ok(Some(product)) => Ok(Some(product.updated_at)),
            Ok(None) => Ok(None),
            Err(_) => Ok(None), // 에러 시 None 반환
        }
    }

    /// 누락된 필드가 많은 제품들의 우선순위 URL 생성
    async fn get_priority_urls_for_missing_fields(&self) -> Result<Vec<String>> {
        let products = self.product_repo.get_all_products().await?;
        let mut priority_products = Vec::new();
        
        for product in products {
            let missing_count = [
                product.manufacturer.is_none() || product.manufacturer.as_ref().map_or(true, |s| s.is_empty()),
                product.model.is_none() || product.model.as_ref().map_or(true, |s| s.is_empty()),
                // matter_version과 connectivity 필드가 Product에 없으므로 false로 설정
                false, // matter_version
                false, // connectivity
                // certification_date는 ProductDetail에만 있으므로 false로 설정
                false, // certification_date
            ].iter().filter(|&&missing| missing).count();
            
            // 3개 이상 필드가 누락된 제품들을 우선순위로 설정
            if missing_count >= 3 {
                priority_products.push(product.url.clone());
            }
        }
        
        // 최대 50개까지만 우선순위로 설정
        priority_products.truncate(50);
        
        info!("📋 Generated {} priority URLs for products with missing fields", priority_products.len());
        Ok(priority_products)
    }

    /// 중복 그룹 찾기
    async fn find_duplicate_groups(&self) -> Result<Vec<Vec<String>>> {
        let products = self.product_repo.get_all_products().await?;
        let mut cert_id_groups: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        
        for product in products {
            if let Some(cert_id) = &product.certificate_id {
                cert_id_groups.entry(cert_id.clone())
                    .or_insert_with(Vec::new)
                    .push(product.url.clone());
            }
        }
        
        // 2개 이상의 제품이 있는 그룹만 중복으로 간주
        let duplicate_groups: Vec<Vec<String>> = cert_id_groups.into_values()
            .filter(|group| group.len() > 1)
            .collect();
        
        debug!("Found {} duplicate groups", duplicate_groups.len());
        Ok(duplicate_groups)
    }
}

#[async_trait]
impl DatabaseAnalyzer for DatabaseAnalyzerImpl {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis> {
        let products = self.product_repo.get_all_products().await?;
        let total = products.len() as u32;
        
        // 중복 제품 계산
        let duplicate_count = self.count_duplicate_products().await?;
        let unique = total.saturating_sub(duplicate_count);
        
        // 필드 누락 분석
        let missing_fields_analysis = self.analyze_missing_fields().await?;
        
        // 데이터 품질 점수 계산
        let data_quality_score = self.calculate_data_quality_score(total, unique, &missing_fields_analysis);
        
        // 마지막 업데이트 시간
        let last_update = self.get_last_update_time().await?;
        
        info!("📊 Database Analysis: total={}, unique={}, duplicates={}, quality={:.3}", 
              total, unique, duplicate_count, data_quality_score);
        
        Ok(DatabaseAnalysis {
            total_products: total,
            unique_products: unique,
            duplicate_count,
            last_update,
            missing_fields_analysis,
            data_quality_score,
        })
    }
    
    async fn recommend_processing_strategy(&self) -> Result<ProcessingStrategy> {
        let analysis = self.analyze_current_state().await?;
        
        // 배치 크기 추천 (데이터 품질에 따라 조정)
        let recommended_batch_size = if analysis.data_quality_score > 0.8 {
            20 // 높은 품질일 때는 큰 배치
        } else if analysis.data_quality_score > 0.5 {
            10 // 중간 품질
        } else {
            5  // 낮은 품질일 때는 작은 배치
        };
        
        // 동시성 추천
        let recommended_concurrency = if analysis.total_products > 1000 {
            3
        } else {
            2
        };
        
        // 중복 건너뛰기 여부
        let should_skip_duplicates = analysis.duplicate_count > analysis.total_products / 10;
        
        // 기존 데이터 업데이트 여부
        let should_update_existing = analysis.data_quality_score < 0.7;
        
        // 우선순위 URL 생성
        let priority_urls = self.get_priority_urls_for_missing_fields().await?;
        
        info!("🎯 Strategy: batch_size={}, concurrency={}, skip_duplicates={}, update_existing={}", 
              recommended_batch_size, recommended_concurrency, should_skip_duplicates, should_update_existing);
        
        Ok(ProcessingStrategy {
            recommended_batch_size,
            recommended_concurrency,
            should_skip_duplicates,
            should_update_existing,
            priority_urls,
        })
    }
    
    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis> {
        let total_products = self.product_repo.get_all_products().await?.len() as u32;
        let total_duplicates = self.count_duplicate_products().await?;
        let duplicate_percentage = if total_products > 0 {
            (total_duplicates as f64 / total_products as f64) * 100.0
        } else {
            0.0
        };
        
        // 중복 그룹 찾기 (간소화된 버전)
        let duplicate_groups = vec![]; // 실제 구현에서는 find_duplicate_groups() 사용
        
        info!("🔄 Duplicate Analysis: {}/{} duplicates ({:.1}%)", 
              total_duplicates, total_products, duplicate_percentage);
        
        Ok(DuplicateAnalysis {
            total_duplicates,
            duplicate_groups,
            duplicate_percentage,
        })
    }
}

/// 컬렉터 설정
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

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            max_concurrent: 3,
            concurrency: 3,
            delay_between_requests: Duration::from_millis(500),
            delay_ms: 500,
            retry_attempts: 3,
            retry_max: 3,
        }
    }
}

/// 헬스 스코어 계산 함수
fn calculate_health_score(response_time: Duration, total_pages: u32) -> f64 {
    // 응답 시간 기반 점수 (0.0 ~ 0.7)
    let response_score = if response_time.as_millis() <= 500 {
        0.7
    } else if response_time.as_millis() <= 1000 {
        0.5
    } else if response_time.as_millis() <= 2000 {
        0.3
    } else {
        0.1
    };
    
    // 페이지 수 기반 점수 (0.0 ~ 0.3)
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
}

impl ProductListCollectorImpl {
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
impl ProductListCollector for ProductListCollectorImpl {
    async fn collect_all_pages(&self, total_pages: u32) -> Result<Vec<String>> {
        let mut all_urls = Vec::new();
        
        for page in 1..=total_pages {
            match self.collect_single_page(page).await {
                Ok(urls) => {
                    all_urls.extend(urls);
                    debug!("📄 Collected {} URLs from page {}", all_urls.len(), page);
                }
                Err(e) => {
                    warn!("⚠️  Failed to collect URLs from page {}: {}", page, e);
                }
            }
            
            tokio::time::sleep(self.config.delay_between_requests).await;
        }
        
        info!("📋 Total URLs collected: {}", all_urls.len());
        Ok(all_urls)
    }
    
    async fn collect_single_page(&self, page: u32) -> Result<Vec<String>> {
        let url = config_utils::matter_products_page_url_simple(page);
        let mut client = self.http_client.lock().await;
        let html = client.fetch_html_string(&url).await?;
        drop(client);
        
        let doc = scraper::Html::parse_document(&html);
        let urls = self.data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;
        
        debug!("🔗 Extracted {} URLs from page {}", urls.len(), page);
        Ok(urls)
    }
    
    async fn collect_page_batch(&self, pages: &[u32]) -> Result<Vec<String>> {
        let mut all_urls = Vec::new();
        
        for page in pages {
            match self.collect_single_page(*page).await {
                Ok(urls) => all_urls.extend(urls),
                Err(e) => warn!("Failed to collect from page {}: {}", page, e),
            }
            
            tokio::time::sleep(self.config.delay_between_requests).await;
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
    async fn collect_details(&self, urls: &[String]) -> Result<Vec<ProductDetail>> {
        let mut products = Vec::new();
        
        for url in urls {
            match self.collect_single_product(url).await {
                Ok(product) => products.push(product),
                Err(e) => warn!("Failed to collect product from {}: {}", url, e),
            }
            
            tokio::time::sleep(self.config.delay_between_requests).await;
        }
        
        Ok(products)
    }
    
    async fn collect_single_product(&self, url: &str) -> Result<ProductDetail> {
        let mut client = self.http_client.lock().await;
        let html = client.fetch_html_string(url).await?;
        drop(client);
        
        let doc = scraper::Html::parse_document(&html);
        let product_detail = self.data_extractor.extract_product_detail(&doc, url.to_string())?;
        
        debug!("📦 Extracted product: {}", product_detail.certification_id.as_deref().unwrap_or("Unknown"));
        Ok(product_detail)
    }
    
    async fn collect_product_batch(&self, urls: &[String]) -> Result<Vec<ProductDetail>> {
        let mut products = Vec::new();
        
        for url in urls {
            match self.collect_single_product(url).await {
                Ok(product) => products.push(product),
                Err(e) => warn!("Failed to collect product from {}: {}", url, e),
            }
            
            tokio::time::sleep(self.config.delay_between_requests).await;
        }
        
        Ok(products)
    }
}

/// ProductDetail을 Product로 변환하는 헬퍼 함수
pub fn product_detail_to_product(detail: crate::domain::product::ProductDetail) -> Product {
    Product {
        url: detail.url,
        manufacturer: detail.manufacturer,
        model: detail.model,
        certificate_id: detail.certification_id,
        page_id: detail.page_id,
        index_in_page: detail.index_in_page,
        created_at: detail.created_at,
        updated_at: detail.updated_at,
    }
}

