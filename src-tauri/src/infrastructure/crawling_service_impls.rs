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
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::config::utils as config_utils;

/// 사이트 상태 체크 서비스 구현체
/// PageDiscoveryService와 협력하여 사이트 상태를 종합적으로 분석
pub struct StatusCheckerImpl {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
    config: AppConfig,
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
        }
    }
}

#[async_trait]
impl StatusChecker for StatusCheckerImpl {
    async fn check_site_status(&self) -> Result<SiteStatus> {
        let start_time = Instant::now();
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
                });
            }
        }

        // Step 2: 페이지 수 탐지 (PageDiscoveryService의 로직을 안전하게 활용)
        let total_pages = self.discover_total_pages().await?;

        let response_time = start_time.elapsed().as_millis() as u64;

        // Step 3: 사이트 건강도 점수 계산
        let health_score = calculate_health_score(response_time, total_pages);

        info!("Site status check completed: {} pages found, {}ms total time, health score: {:.2}", 
              total_pages, response_time, health_score);

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

impl StatusCheckerImpl {
    /// 향상된 페이지 탐지 로직 - 페이지네이션을 반복적으로 확인하여 정확한 마지막 페이지 찾기
    async fn discover_total_pages(&self) -> Result<u32> {
        info!("🔍 Starting enhanced page discovery algorithm");
        
        // 1. 시작 페이지 결정
        let start_page = self.config.app_managed.last_known_max_page
            .unwrap_or(self.config.advanced.last_page_search_start);
        
        info!("📍 Starting from page {} (last known: {:?}, default: {})", 
              start_page, 
              self.config.app_managed.last_known_max_page,
              self.config.advanced.last_page_search_start);
        
        // 2. 첫 번째 단계: 시작 페이지가 유효한지 확인
        let mut current_page = start_page;
        if !self.check_page_has_products(current_page).await? {
            info!("⚠️  Starting page {} has no products, searching downward", current_page);
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
            
            // 현재 페이지를 로드하고 분석
            let test_url = config_utils::matter_products_page_url_simple(current_page);
            debug!("📄 Loading page: {}", test_url);
            
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
                        // 네트워크 오류 시 하향 탐색
                        current_page = self.find_last_valid_page_downward(current_page).await?;
                        break;
                    }
                }
            };
            
            if !has_products {
                // 제품이 없는 경우 안전성 검사가 포함된 하향 탐색
                info!("🔻 Page {} has no products, performing safe downward search", current_page);
                current_page = self.find_last_valid_page_with_safety_check(current_page).await?;
                break;
            }
            
            // 페이지네이션에서 더 큰 페이지를 찾았는지 확인
            if max_page_in_pagination > current_page {
                info!("🔺 Found higher page {} in pagination, jumping there", max_page_in_pagination);
                current_page = max_page_in_pagination;
                // 새 페이지로 이동하여 다시 탐색
                continue;
            } else {
                info!("🏁 No higher pages found in pagination, {} appears to be the last page", current_page);
                break;
            }
        }
        
        // 4. 최종 검증: 마지막 페이지 확인
        let verified_last_page = self.verify_last_page(current_page).await?;
        
        // 5. 설정 파일에 결과 저장
        if let Err(e) = self.update_last_known_page(verified_last_page).await {
            warn!("⚠️  Failed to update last known page in config: {}", e);
        }
        
        info!("🎉 Final verified last page: {}", verified_last_page);
        Ok(verified_last_page)
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
    async fn verify_last_page(&self, candidate_page: u32) -> Result<u32> {
        info!("🔍 Verifying candidate last page: {}", candidate_page);

        // 1. 후보 페이지에 제품이 있는지 확인
        let has_products = self.check_page_has_products(candidate_page).await?;
        if !has_products {
            warn!("⚠️  Candidate page {} has no products, performing downward search with safety check", candidate_page);
            return self.find_last_valid_page_with_safety_check(candidate_page).await;
        }

        // 2. 다음 페이지들을 확인하여 정말 마지막인지 검증
        let verification_range = 5; // 최대 5페이지까지 확인
        
        for offset in 1..=verification_range {
            let next_page = candidate_page + offset;
            
            match self.check_page_has_products(next_page).await {
                Ok(true) => {
                    warn!("🔍 Found products on page {} after candidate {}, re-discovering", 
                          next_page, candidate_page);
                    
                    // 더 높은 페이지에서 제품을 발견했으므로 그 페이지부터 다시 탐색
                    return self.discover_from_page(next_page).await;
                },
                Ok(false) => {
                    debug!("✅ Page {} confirmed empty", next_page);
                },
                Err(e) => {
                    debug!("❌ Failed to check page {}: {}", next_page, e);
                    // 네트워크 오류는 무시하고 계속 진행
                }
            }
            
            // 각 확인 사이에 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.user.request_delay_ms)).await;
        }

        info!("✅ Verified page {} as the last page (checked {} pages ahead)", 
              candidate_page, verification_range);
        Ok(candidate_page)
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

    /// 특정 페이지에 제품이 있는지 확인
    async fn check_page_has_products(&self, page: u32) -> Result<bool> {
        let test_url = config_utils::matter_products_page_url_simple(page);
        
        let mut client = self.http_client.lock().await;
        match client.fetch_html_string(&test_url).await {
            Ok(html) => {
                let doc = scraper::Html::parse_document(&html);
                Ok(self.has_products_on_page(&doc))
            },
            Err(_) => Ok(false),
        }
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
            
            // 추정 제품 수 업데이트 (페이지당 20개 제품 추정)
            app_managed.last_crawl_product_count = Some(last_page * 20);
            
            // 페이지당 평균 제품 수 업데이트
            app_managed.avg_products_per_page = Some(20.0);
            
            info!("📝 Updated config: last_page={}, timestamp={}", 
                  last_page, 
                  app_managed.last_successful_crawl.as_ref().unwrap_or(&"unknown".to_string()));
        }).await?;
        
        info!("✅ Successfully updated last known page to {} in config file", last_page);
        Ok(())
    }

    /// 페이지에 제품이 있는지 확인 (PageDiscoveryService 로직 활용)
    fn has_products_on_page(&self, doc: &scraper::Html) -> bool {
        let mut max_count = 0;
        
        for selector_str in &self.config.advanced.product_selectors {
            if let Ok(selector) = scraper::Selector::parse(selector_str) {
                let count = doc.select(&selector).count() as u32;
                if count > max_count {
                    max_count = count;
                }
            }
        }
        
        // 기본 선택자들도 시도
        if max_count == 0 {
            if let Ok(article_selector) = scraper::Selector::parse("article") {
                max_count = doc.select(&article_selector).count() as u32;
            }
        }
        
        max_count > 0
    }

    /// 페이지네이션에서 최대 페이지 번호 찾기 (PageDiscoveryService 로직 활용)
    fn find_max_page_in_pagination(&self, doc: &scraper::Html) -> u32 {
        let link_selector = scraper::Selector::parse("a[href*='page']").unwrap();
        let mut max_page = 1;
        
        for element in doc.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                if let Some(page_num) = self.extract_page_number(href) {
                    max_page = max_page.max(page_num);
                }
            }
        }
        
        max_page
    }

    /// URL에서 페이지 번호 추출 (PageDiscoveryService 로직 활용)
    fn extract_page_number(&self, url: &str) -> Option<u32> {
        if let Some(captures) = regex::Regex::new(r"[?&]page[d]?=(\d+)")
            .ok()
            .and_then(|re| re.captures(url)) 
        {
            if let Some(num_match) = captures.get(1) {
                return num_match.as_str().parse().ok();
            }
        }
        
        if let Some(captures) = regex::Regex::new(r"/page/(\d+)")
            .ok()
            .and_then(|re| re.captures(url))
        {
            if let Some(num_match) = captures.get(1) {
                return num_match.as_str().parse().ok();
            }
        }
        
        None
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
        let url = config_utils::matter_products_page_url_simple(page);
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

                let url = config_utils::matter_products_page_url_simple(page_num);
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

/// 사이트 건강도 점수 계산
fn calculate_health_score(response_time_ms: u64, total_pages: u32) -> f64 {
    // 응답시간 기반 점수 (0.0 ~ 1.0)
    let time_score = if response_time_ms < 2000 { 1.0 }
    else if response_time_ms < 5000 { 0.8 }
    else if response_time_ms < 10000 { 0.6 }
    else if response_time_ms < 20000 { 0.4 }
    else { 0.2 };
    
    // 페이지 수 기반 점수 (페이지가 너무 적으면 사이트에 문제가 있을 수 있음)
    let page_score = if total_pages >= 10 { 1.0 }
    else if total_pages >= 5 { 0.8 }
    else if total_pages >= 1 { 0.6 }
    else { 0.0 };
    
    // 가중 평균 (응답시간이 더 중요)
    (time_score * 0.7) + (page_score * 0.3)
}
