//! 크롤링 서비스 구현체
//!
//! `domain/services/crawling_services.rs의` 트레이트들에 대한 실제 구현체

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use regex;
use scraper;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn, trace};

use crate::domain::product::{Product, ProductDetail};
use crate::domain::product_url::ProductUrl;
use crate::domain::services::crawling_services::{
    CrawlingRangeRecommendation, DataDecreaseRecommendation, DatabaseAnalysis, RecommendedAction,
    SeverityLevel, SiteDataChangeStatus,
};
use crate::domain::services::{
    DatabaseAnalyzer, DuplicateAnalysis, FieldAnalysis, ProcessingStrategy, ProductDetailCollector,
    ProductListCollector, SiteStatus, StatusChecker,
};
use crate::infrastructure::config::utils as config_utils;
use crate::infrastructure::config::{AppConfig, CrawlingConfig};
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};
use crate::crawl_engine::actors::types::AppEvent;
// Canonical pagination calculator (legacy utils::PageIdCalculator via domain alias)
use crate::domain::pagination::CanonicalPageIdCalculator;

// 상수 정의
const DEFAULT_PRODUCTS_PER_PAGE: u32 = 12;

// Reintroduced struct definitions (accidentally disrupted during method removal phase)
pub struct StatusCheckerImpl {
    pub http_client: Arc<HttpClient>,
    pub data_extractor: Arc<MatterDataExtractor>,
    pub config: AppConfig,
    page_cache: Arc<tokio::sync::Mutex<HashMap<u32, PageAnalysisCache>>>,
    pub product_repo: Option<Arc<IntegratedProductRepository>>,
}

#[derive(Clone, Debug)]
struct PageAnalysisCache {
    max_page_in_pagination: u32,
    product_count: u32,
    has_products: bool,
}

impl StatusCheckerImpl {
    #[must_use]
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        config: AppConfig,
    ) -> Self {
        Self {
            http_client: Arc::new(http_client),
            data_extractor: Arc::new(data_extractor),
            config,
            page_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            product_repo: None,
        }
    }

    pub fn create_configured_http_client(&self) -> Result<HttpClient> {
        HttpClient::create_from_global_config()
            .map_err(|e| anyhow!("HttpClient create failed: {}", e))
    }

    pub async fn clear_page_cache(&self) {
        let mut guard = self.page_cache.lock().await;
        guard.clear();
    }
}

impl StatusCheckerImpl {
    /// Associate a product repository after initial creation (legacy helper)
    #[must_use]
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
    pub async fn update_pagination_context(
        &self,
        total_pages: u32,
        items_on_last_page: u32,
    ) -> Result<()> {
        let validated_config =
            crate::application::validated_crawling_config::ValidatedCrawlingConfig::from_app_config(
                &self.config,
            );
        let products_per_page = validated_config.products_per_page;
        let pagination_context = crate::infrastructure::html_parser::PaginationContext {
            total_pages,
            items_per_page: products_per_page,
            items_on_last_page,
            target_page_size: products_per_page,
        };
        self.data_extractor
            .set_pagination_context(pagination_context)?;
        info!(
            "📊 Updated pagination context: total_pages={}, items_on_last_page={}, products_per_page={}",
            total_pages, items_on_last_page, products_per_page
        );
        Ok(())
    }
}

#[async_trait]
impl StatusChecker for StatusCheckerImpl {
    async fn check_site_status(&self) -> Result<SiteStatus> {
        let start_time = Instant::now();
        info!("Starting comprehensive site status check with detailed page discovery");

        // 캐시 초기화
        self.clear_page_cache_internal().await;

        info!("Checking site status and discovering pages...");

        // Step 1: 기본 사이트 접근성 확인
        let url = config_utils::matter_products_page_url_simple(1);

        // 접근성 테스트 (네트워크/HTTP 오류를 사용자 친화적으로 처리)
        // 이전 구현은 fetch_response() 뒤 '?'로 인해 네트워크 오류가 상위로 전파되어
        // 프론트엔드에 즉시 실패로 표시되었습니다. 여기서는 재시도 정책을 가진
        // fetch_response_with_policy()를 사용하고, 모든 오류를 "접근 불가"로 매핑합니다.
        let accessibility_result = {
            let _client = self.create_configured_http_client()?; // config 검증 목적
            match self.http_client.fetch_response_with_policy(&url).await {
                Ok(resp) => {
                    // 본 단계에서는 본문 파싱이 필요 없으므로 body 를 소비하지 않고 성공 처리
                    Ok(())
                }
                Err(e) => Err(e),
            }
        };

        if let Err(e) = accessibility_result {
            error!("Failed to access site (treated as inaccessible): {}", e);
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
        } else {
            info!("Site is accessible");
        }

        // Step 2: 페이지 수 탐지 및 마지막 페이지 제품 수 확인
        let (total_pages, products_on_last_page) = self.discover_total_pages().await?;

        // Step 2.5: 페이지네이션 컨텍스트 업데이트
        if let Err(e) = self
            .update_pagination_context(total_pages, products_on_last_page)
            .await
        {
            warn!("Failed to update pagination context: {}", e);
        }

        let response_time_ms = start_time.elapsed().as_millis() as u64;
        let response_time = start_time.elapsed();

        // Step 3: 사이트 건강도 점수 계산
        let health_score = calculate_health_score(response_time, total_pages);

        info!(
            "Site status check completed: {} pages found, {}ms total time, health score: {:.2}",
            total_pages, response_time_ms, health_score
        );

        // 정확한 제품 수 계산: (마지막 페이지 - 1) * 페이지당 제품 수 + 마지막 페이지 제품 수
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;

        let estimated_products =
            total_pages.saturating_sub(1) * products_per_page + products_on_last_page;

        info!(
            "Accurate product estimation: ({} full pages * {} products) + {} products on last page = {} total products",
            total_pages.saturating_sub(1),
            products_per_page,
            products_on_last_page,
            estimated_products
        );

        // Step 4: 데이터 변화 상태 분석
        let (data_change_status, decrease_recommendation) =
            self.analyze_data_changes(estimated_products).await;

        // Step 5: 크롤링 범위 권장사항 계산 (StatusCheckerImpl 내부에서는 CrawlingPlanner를 생성하지 않음)
        // 중복 분석을 방지하기 위해 플래너 생성과 시스템 분석은 상위 오케스트레이션 레이어에서 수행합니다.
        let crawling_range_recommendation = self
            .calculate_crawling_range_recommendation_internal(
                total_pages,
                products_on_last_page,
                estimated_products,
            )
            .await?;

        // Step 6: Anomaly detection against stored maxima
        {
            let mut cfg_guard = self.config.clone();
            let mut mutated = false;
            let now_ts = chrono::Utc::now().to_rfc3339();
            // Track previous maxima
            let prev_max_page = cfg_guard.app_managed.last_known_max_page;
            let prev_max_products = cfg_guard.app_managed.last_known_max_total_products;

            // Detect page count drop
            if let Some(prev) = prev_max_page {
                if total_pages < prev {
                    // Potential anomaly: page count decreased
                    if cfg_guard.app_managed.first_degradation_at.is_none() {
                        cfg_guard.app_managed.first_degradation_at = Some(now_ts.clone());
                    }
                    cfg_guard.app_managed.last_degradation_note = Some(format!(
                        "page_drop prev={} current={}", prev, total_pages
                    ));
                } else if total_pages > prev {
                    // New high water mark resets degradation note
                    cfg_guard.app_managed.last_known_max_page = Some(total_pages);
                    cfg_guard.app_managed.last_degradation_note = None;
                    mutated = true;
                }
            } else {
                cfg_guard.app_managed.last_known_max_page = Some(total_pages);
                mutated = true;
            }

            // Detect total products estimation drop
            if let Some(prev_p) = prev_max_products {
                if estimated_products < prev_p {
                    if cfg_guard.app_managed.first_degradation_at.is_none() {
                        cfg_guard.app_managed.first_degradation_at = Some(now_ts.clone());
                    }
                    cfg_guard.app_managed.last_degradation_note = Some(format!(
                        "product_drop prev={} current={}", prev_p, estimated_products
                    ));
                } else if estimated_products > prev_p {
                    cfg_guard.app_managed.last_known_max_total_products = Some(estimated_products);
                    cfg_guard.app_managed.last_degradation_note = None;
                    mutated = true;
                }
            } else {
                cfg_guard.app_managed.last_known_max_total_products = Some(estimated_products);
                mutated = true;
            }

            if mutated {
                // Persist only the app_managed section (best-effort)
                match crate::infrastructure::config::ConfigManager::new() {
                    Ok(manager) => {
                        let persist_result = async {
                            let mut disk_cfg = manager.load_config().await?;
                            disk_cfg.app_managed = cfg_guard.app_managed.clone();
                            manager.save_config(&disk_cfg).await
                        }.await;
                        match persist_result {
                            Ok(_) => {
                                tracing::info!(target="site_health", max_page=?cfg_guard.app_managed.last_known_max_page, max_products=?cfg_guard.app_managed.last_known_max_total_products, "Updated stored maxima");
                            }
                            Err(e) => {
                                tracing::warn!(target="site_health", error=%e, "Failed to persist updated maxima");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target="site_health", error=%e, "Failed to init ConfigManager for maxima persistence");
                    }
                }
            } else if cfg_guard.app_managed.last_degradation_note.is_some() {
                tracing::warn!(target="site_health", note=?cfg_guard.app_managed.last_degradation_note, first_at=?cfg_guard.app_managed.first_degradation_at, "Site pagination anomaly detected (non-fatal)");
            }
        }

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
        db_analysis: &DatabaseAnalysis,
    ) -> Result<CrawlingRangeRecommendation> {
        info!("🔍 Calculating crawling range recommendation from site status and DB analysis...");
        info!(
            "📊 DB Analysis shows: total_products={}, unique_products={}",
            db_analysis.total_products, db_analysis.unique_products
        );

        // Cross-check with local status to ensure consistency
        let local_status = self.get_local_db_status().await?;

        // Verify consistency between different DB access methods
        let db_total = db_analysis.total_products;
        if db_total != local_status.total_saved_products {
            warn!(
                "⚠️  DB inconsistency detected: analysis={}, local_status={}",
                db_analysis.total_products, local_status.total_saved_products
            );
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
        let effective_total = db_analysis
            .total_products
            .max(local_status.total_saved_products);
        let estimated_new_products = site_status
            .estimated_products
            .saturating_sub(effective_total);

        if estimated_new_products == 0 {
            info!("📊 No new products detected - recommending minimal verification crawl");
            return Ok(CrawlingRangeRecommendation::Partial(5)); // 5 pages for verification
        }

        // Calculate pages needed for new products
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;
        let pages_needed =
            (f64::from(estimated_new_products) / f64::from(products_per_page)).ceil() as u32;
        let limited_pages = pages_needed.min(self.config.user.crawling.page_range_limit);

        info!(
            "📊 Estimated {} new products, recommending {} pages crawl",
            estimated_new_products, limited_pages
        );
        Ok(CrawlingRangeRecommendation::Partial(limited_pages))
    }

    async fn estimate_crawling_time(&self, pages: u32) -> Duration {
        // ...
        // 페이지당 평균 2초 + 제품 상세페이지당 1초 추정
        let page_collection_time = pages * 2;
        let product_detail_time = pages * 20; // 페이지당 20개 제품 * 1초
        let total_seconds = page_collection_time + product_detail_time;

        Duration::from_secs(u64::from(total_seconds))
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
        let start_page = self
            .config
            .app_managed
            .last_known_max_page
            .unwrap_or(self.config.advanced.last_page_search_start);

        info!(
            "📍 Starting from page {} (last known: {:?}, default: {})",
            start_page,
            self.config.app_managed.last_known_max_page,
            self.config.advanced.last_page_search_start
        );

        // 2. 시작 페이지 분석 (캐시 사용)
        let start_analysis = self.get_or_analyze_page(start_page).await?;
        let mut current_page = start_page;

        if !start_analysis.has_products {
            warn!(
                "⚠️  Starting page {} has no products - checking site status",
                current_page
            );
            // 첫 페이지 확인으로 사이트 접근성 검증
            let first_page_analysis = self.get_or_analyze_page(1).await?;
            if !first_page_analysis.has_products {
                error!("❌ First page also has no products - site may be temporarily unavailable");
                return Err(anyhow::anyhow!(
                    "Site appears to be temporarily unavailable or experiencing issues. Please try again later."
                ));
            }

            info!(
                "✅ First page has products - site is accessible, cached page info may be outdated"
            );
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
                warn!(
                    "🔄 Reached maximum attempts ({}), stopping at page {}",
                    max_attempts, current_page
                );
                break;
            }

            info!(
                "🔍 Iteration {}/{}: Checking page {}",
                attempts, max_attempts, current_page
            );

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
                info!(
                    "🔻 Page {} has no products, performing safe downward search",
                    current_page
                );
                current_page = self
                    .find_last_valid_page_with_safety_check(current_page)
                    .await?;
                break;
            }

            // 페이지네이션에서 더 큰 페이지를 찾았는지 확인
            if analysis.max_page_in_pagination > current_page {
                info!(
                    "🔺 Found higher page {} in pagination, jumping there",
                    analysis.max_page_in_pagination
                );
                current_page = analysis.max_page_in_pagination;
                // 새 페이지로 이동하여 다시 탐색
                continue;
            }
            info!(
                "🏁 No higher pages found in pagination, {} appears to be the last page",
                current_page
            );
            break;
        }

        // 4. 최종 검증: 마지막 페이지 확인 및 제품 수 계산
        let (verified_last_page, products_on_last_page) =
            self.verify_last_page(current_page).await?;

        // 5. 설정 파일에 결과 저장
        if let Err(e) = self
            .update_last_known_page(verified_last_page, Some(products_on_last_page))
            .await
        {
            warn!("⚠️  Failed to update last known page in config: {}", e);
        }

        info!(
            "🎉 Final verified last page: {} with {} products",
            verified_last_page, products_on_last_page
        );
        Ok((verified_last_page, products_on_last_page))
    }

    /// 하향 탐색으로 마지막 유효한 페이지 찾기
    async fn find_last_valid_page_downward(&self, start_page: u32) -> Result<u32> {
        let mut current_page = start_page.saturating_sub(1);
        let min_page = 1;

        info!("Starting downward search from page {}", current_page);

        while current_page >= min_page {
            let test_url = config_utils::matter_products_page_url_simple(current_page);

            // Use configured HttpClient
            let _client = self.create_configured_http_client()?;
            match self.http_client.fetch_response(&test_url).await {
                Ok(response) => match response.text().await {
                    Ok(html) => {
                        let doc = scraper::Html::parse_document(&html);
                        if self.has_products_on_page(&doc) {
                            info!("Found valid page with products: {}", current_page);
                            return Ok(current_page);
                        }
                    }
                    Err(e) => {
                        error!("Failed to get HTML for page {}: {}", current_page, e);
                    }
                },
                Err(e) => {
                    warn!(
                        "Failed to fetch page {} during downward search: {}",
                        current_page, e
                    );
                }
            }

            current_page = current_page.saturating_sub(1);

            // 요청 간 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.user.request_delay_ms,
            ))
            .await;
        }

        // 모든 페이지에서 제품을 찾지 못한 경우
        warn!("No valid pages found during downward search, returning 1");
        Ok(1)
    }

    /// 안전성 검사가 포함된 하향 탐색 - 연속 빈 페이지 3개 이상 시 fatal error
    async fn find_last_valid_page_with_safety_check(&self, start_page: u32) -> Result<u32> {
        let mut current_page = start_page;
        let mut consecutive_empty_pages = 0;
        const MAX_CONSECUTIVE_EMPTY: u32 = 12;
        let min_page = 1;

        info!(
            "🔍 Starting safe downward search from page {} (max consecutive empty: {})",
            current_page, MAX_CONSECUTIVE_EMPTY
        );

        // 먼저 시작 페이지가 비어있는지 확인
        if !self.check_page_has_products(current_page).await? {
            consecutive_empty_pages += 1;
            info!(
                "⚠️  Starting page {} is empty (consecutive: {})",
                current_page, consecutive_empty_pages
            );
        }

        while current_page > min_page {
            current_page = current_page.saturating_sub(1);

            let test_url = config_utils::matter_products_page_url_simple(current_page);
            info!(
                "🔍 Checking page {} (consecutive empty: {})",
                current_page, consecutive_empty_pages
            );

            // Use configured HttpClient
            let _client = self.create_configured_http_client()?; // unused (legacy path)
            match self.http_client.fetch_response(&test_url).await {
                Ok(response) => match response.text().await {
                    Ok(html) => {
                        let doc = scraper::Html::parse_document(&html);
                        if self.has_products_on_page(&doc) {
                            info!(
                                "✅ Found valid page with products: {} (after {} consecutive empty pages)",
                                current_page, consecutive_empty_pages
                            );
                            return Ok(current_page);
                        }
                        consecutive_empty_pages += 1;
                        warn!(
                            "⚠️  Page {} is empty (consecutive: {}/{})",
                            current_page, consecutive_empty_pages, MAX_CONSECUTIVE_EMPTY
                        );

                        // 연속으로 빈 페이지가 3개 이상이면 fatal error
                        if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
                            error!(
                                "💥 FATAL ERROR: Found {} consecutive empty pages starting from page {}. This indicates a serious site issue or crawling problem.",
                                consecutive_empty_pages, start_page
                            );

                            return Err(anyhow!(
                                "Fatal error: {} consecutive empty pages detected. Site may be down or pagination structure changed. Last checked pages: {} to {}",
                                consecutive_empty_pages,
                                start_page,
                                current_page
                            ));
                        }
                    }
                    Err(e) => {
                        consecutive_empty_pages += 1;
                        warn!(
                            "❌ Failed to get HTML for page {} during safe downward search: {} (consecutive: {}/{})",
                            current_page, e, consecutive_empty_pages, MAX_CONSECUTIVE_EMPTY
                        );

                        if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
                            error!(
                                "💥 FATAL ERROR: {} consecutive failures starting from page {}.",
                                consecutive_empty_pages, start_page
                            );

                            return Err(anyhow!(
                                "Fatal error: {} consecutive failures detected. HTML parsing issues or site problems. Last error: {}",
                                consecutive_empty_pages,
                                e
                            ));
                        }
                    }
                },
                Err(e) => {
                    consecutive_empty_pages += 1;
                    warn!(
                        "❌ Failed to fetch page {} during safe downward search: {} (consecutive: {}/{})",
                        current_page, e, consecutive_empty_pages, MAX_CONSECUTIVE_EMPTY
                    );

                    // 네트워크 오류도 연속 실패로 카운트
                    if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
                        error!(
                            "💥 FATAL ERROR: {} consecutive failures (empty pages + network errors) starting from page {}.",
                            consecutive_empty_pages, start_page
                        );

                        return Err(anyhow!(
                            "Fatal error: {} consecutive failures detected. Network issues or site problems. Last error: {}",
                            consecutive_empty_pages,
                            e
                        ));
                    }
                }
            }

            // 요청 간 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.user.request_delay_ms,
            ))
            .await;
        }

        // 최소 페이지까지 도달했지만 여전히 연속 빈 페이지가 많다면 fatal error
        if consecutive_empty_pages >= MAX_CONSECUTIVE_EMPTY {
            error!(
                "💥 FATAL ERROR: Reached minimum page but still have {} consecutive empty pages. Site appears to be completely empty or broken.",
                consecutive_empty_pages
            );

            return Err(anyhow!(
                "Fatal error: Site appears to be empty or broken. {} consecutive empty pages found from page {} down to page {}",
                consecutive_empty_pages,
                start_page,
                current_page
            ));
        }

        // 모든 페이지에서 제품을 찾지 못했지만 연속 빈 페이지가 3개 미만이면 경고와 함께 1 반환
        warn!(
            "⚠️  No valid pages found during safe downward search, but only {} consecutive empty pages. Returning page 1 as fallback.",
            consecutive_empty_pages
        );
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

        info!(
            "📊 Last page {} has {} products",
            candidate_page, products_on_last_page
        );

        if !has_products {
            warn!(
                "⚠️  Candidate page {} has no products, performing downward search with safety check",
                candidate_page
            );
            let actual_last_page = self
                .find_last_valid_page_with_safety_check(candidate_page)
                .await?;
            // 실제 마지막 페이지의 제품 수 다시 확인
            let actual_analysis = self.get_or_analyze_page(actual_last_page).await?;
            return Ok((actual_last_page, actual_analysis.product_count));
        }

        // 2. 페이지네이션 분석에서 이미 마지막 페이지임을 확신할 수 있다면 추가 확인 생략
        // 현재 페이지가 페이지네이션에서 발견된 최대 페이지와 같다면 검증 완료
        if analysis.max_page_in_pagination == candidate_page {
            info!(
                "✅ Page {} confirmed as last page via pagination analysis (max_pagination={})",
                candidate_page, analysis.max_page_in_pagination
            );
            info!("🚀 Skipping additional verification - pagination analysis is reliable");
            return Ok((candidate_page, products_on_last_page));
        }

        // 3. 페이지네이션 분석이 불확실한 경우에만 최소한의 추가 검증 수행
        info!(
            "🔍 Pagination analysis inconclusive (current={}, max_pagination={}), performing minimal verification",
            candidate_page, analysis.max_page_in_pagination
        );

        // 바로 다음 페이지 1개만 확인 (과도한 검증 방지)
        let next_page = candidate_page + 1;
        match self.check_page_has_products(next_page).await {
            Ok(true) => {
                warn!(
                    "🔍 Found products on page {} after candidate {}, re-discovering",
                    next_page, candidate_page
                );
                // 더 높은 페이지에서 제품을 발견했으므로 그 페이지부터 다시 탐색
                return self.discover_from_page_with_count(next_page).await;
            }
            Ok(false) => {
                info!(
                    "✅ Verified page {} as the last page with {} products (checked {} page ahead)",
                    candidate_page, products_on_last_page, 1
                );
            }
            Err(e) => {
                debug!(
                    "❌ Failed to check page {}: {}, assuming {} is last",
                    next_page, e, candidate_page
                );
            }
        }

        Ok((candidate_page, products_on_last_page))
    }

    /// 특정 페이지부터 다시 탐색 시작 (제품 수도 반환)
    async fn discover_from_page_with_count(&self, start_page: u32) -> Result<(u32, u32)> {
        info!(
            "🔄 Re-discovering from page {} with product count",
            start_page
        );

        let mut current_page = start_page;
        let max_attempts = self.config.advanced.max_search_attempts;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                warn!(
                    "🔄 Reached maximum attempts, stopping at page {}",
                    current_page
                );
                break;
            }

            let test_url = config_utils::matter_products_page_url_simple(current_page);

            let (has_products, max_page_in_pagination) = {
                match self.http_client.fetch_html_string(&test_url).await {
                    Ok(html) => {
                        let doc = scraper::Html::parse_document(&html);
                        let has_products = self.has_products_on_page(&doc);
                        let max_page = self.find_max_page_in_pagination(&doc);

                        info!(
                            "📊 Page {} analysis: has_products={}, max_pagination={}",
                            current_page, has_products, max_page
                        );

                        (has_products, max_page)
                    }
                    Err(e) => {
                        warn!("❌ Failed to fetch page {}: {}", current_page, e);
                        break;
                    }
                }
            };

            if !has_products {
                // 제품이 없으면 안전성 검사가 포함된 하향 탐색 후 제품 수 확인
                let last_page = self
                    .find_last_valid_page_with_safety_check(current_page)
                    .await?;
                let test_url = config_utils::matter_products_page_url_simple(last_page);

                let html = self.http_client.fetch_html_string(&test_url).await?;

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

            let html = self.http_client.fetch_html_string(&test_url).await?;

            let doc = scraper::Html::parse_document(&html);
            let products_count = self.count_products(&doc);
            return Ok((current_page, products_count));
        }

        // 최대 시도 횟수 도달 시 현재 페이지의 제품 수 확인
        let test_url = config_utils::matter_products_page_url_simple(current_page);

        let html = self.http_client.fetch_html_string(&test_url).await?;

        let doc = scraper::Html::parse_document(&html);
        let products_count = self.count_products(&doc);
        Ok((current_page, products_count))
    }

    /// 특정 페이지에 제품이 있는지 확인 - 활성 페이지네이션 값도 함께 확인
    async fn check_page_has_products(&self, page: u32) -> Result<bool> {
        let test_url = config_utils::matter_products_page_url_simple(page);

        // Use configured HttpClient
        let _client = self.create_configured_http_client()?;
        match self.http_client.fetch_response(&test_url).await {
            Ok(response) => {
                match response.text().await {
                    Ok(html) => {
                        let doc = scraper::Html::parse_document(&html);

                        // 1. 제품 존재 여부 확인
                        let has_products = self.has_products_on_page(&doc);

                        // 2. 활성 페이지네이션 값 확인 (더 중요한 체크)
                        let active_page = self.get_active_page_number(&doc);

                        // 실제 페이지 번호와 활성 페이지네이션 값이 일치하는지 확인
                        let is_correct_page = active_page == page;

                        if !is_correct_page {
                            info!(
                                "⚠️  Page {} was redirected to page {} (pagination mismatch)",
                                page, active_page
                            );
                            return Ok(false);
                        }

                        info!(
                            "✅ Page {} verification: has_products={}, active_page={}, is_correct_page={}",
                            page, has_products, active_page, is_correct_page
                        );

                        Ok(has_products && is_correct_page)
                    }
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// 활성 페이지네이션 값 추출 - 현재 페이지가 실제로 로드되었는지 확인
    fn get_active_page_number(&self, doc: &scraper::Html) -> u32 {
        // 활성 페이지네이션 요소를 찾기 위한 다양한 선택자 시도
        // 사이트 구조에 맞게 우선순위 조정 (페이지네이션 우선 클래스: page-numbers.current)
        let active_selectors = [
            ".page-numbers.current",     // 우선순위 가장 높음 (사이트 구조에 맞게 조정)
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
                        info!(
                            "🎯 Found active page number {} using selector '{}'",
                            page_num, selector_str
                        );
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
                return element
                    .value()
                    .attr("href")
                    .map(std::string::ToString::to_string);
            }
        }
        None
    }

    /// 설정 파일에 마지막 페이지 및 메타데이터 업데이트
    async fn update_last_known_page(
        &self,
        last_page: u32,
        items_on_last_page: Option<u32>,
    ) -> Result<()> {
        use crate::infrastructure::config::ConfigManager;

        let config_manager = ConfigManager::new()?;

        // 설정 업데이트를 위한 클로저 사용
        config_manager.update_app_managed(|app_managed| {
            // 마지막 알려진 페이지 업데이트
            app_managed.last_known_max_page = Some(last_page);

            // 마지막 성공한 크롤링 시간 업데이트
            app_managed.last_successful_crawl = Some(chrono::Utc::now().to_rfc3339());

            // 총 제품 수 정확 계산
            let items_per_page: u32 = DEFAULT_PRODUCTS_PER_PAGE;
        let last_partial = items_on_last_page.unwrap_or(items_per_page);
        let accurate_total = if last_page == 0 { 0 } else { (last_page - 1) * items_per_page + last_partial };
            app_managed.last_crawl_product_count = Some(accurate_total);

            // 페이지당 평균 제품 수 업데이트
            app_managed.avg_products_per_page = Some(f64::from(DEFAULT_PRODUCTS_PER_PAGE));

        info!("📝 Updated config: last_page={}, items_on_last_page={}, accurate_total_products={}, timestamp={}", 
            last_page,
            last_partial,
            accurate_total,
            app_managed.last_successful_crawl.as_ref().unwrap_or(&"unknown".to_string()));
        }).await?;

        info!(
            "✅ Successfully updated last known page to {} in config file",
            last_page
        );
        Ok(())
    }

    /// 데이터 변화 상태 분석 및 권장사항 생성
    async fn analyze_data_changes(
        &self,
        current_estimated_products: u32,
    ) -> (SiteDataChangeStatus, Option<DataDecreaseRecommendation>) {
        // 이전 크롤링 정보 가져오기
        let previous_count = self.config.app_managed.last_crawl_product_count;

        match previous_count {
            None => {
                info!("🆕 Initial site check - no previous data available");
                (
                    SiteDataChangeStatus::Initial {
                        count: current_estimated_products,
                    },
                    None,
                )
            }
            Some(prev_count) => {
                let change_percentage = if prev_count > 0 {
                    ((f64::from(current_estimated_products) - f64::from(prev_count))
                        / f64::from(prev_count))
                        * 100.0
                } else {
                    0.0
                };
                // 허용 오차 (마지막 페이지 partial 차이 등) - 0.5% 미만 변화는 Stable 처리
                let decrease_tolerance_pct = 0.5_f64;

                if current_estimated_products > prev_count {
                    let increase = current_estimated_products - prev_count;
                    info!(
                        "📈 Site data increased: {} -> {} (+{}, +{:.1}%)",
                        prev_count, current_estimated_products, increase, change_percentage
                    );
                    (
                        SiteDataChangeStatus::Increased {
                            new_count: current_estimated_products,
                            previous_count: prev_count,
                        },
                        None,
                    )
                } else if current_estimated_products == prev_count
                    || change_percentage.abs() < decrease_tolerance_pct
                {
                    if change_percentage.abs() < decrease_tolerance_pct
                        && current_estimated_products != prev_count
                    {
                        info!(
                            "📊 Site data change {:.2}% within tolerance (<{:.2}%), treating as stable ({} -> {})",
                            change_percentage,
                            decrease_tolerance_pct,
                            prev_count,
                            current_estimated_products
                        );
                    } else {
                        info!(
                            "📊 Site data stable: {} products",
                            current_estimated_products
                        );
                    }
                    (
                        SiteDataChangeStatus::Stable {
                            count: current_estimated_products,
                        },
                        None,
                    )
                } else {
                    let decrease = prev_count - current_estimated_products;
                    let decrease_percentage = (f64::from(decrease) / f64::from(prev_count)) * 100.0;

                    warn!(
                        "📉 Site data decreased: {} -> {} (-{}, -{:.1}%)",
                        prev_count, current_estimated_products, decrease, decrease_percentage
                    );

                    let severity = if decrease_percentage < 10.0 {
                        SeverityLevel::Low
                    } else if decrease_percentage < 30.0 {
                        SeverityLevel::Medium
                    } else if decrease_percentage < 50.0 {
                        SeverityLevel::High
                    } else {
                        SeverityLevel::Critical
                    };

                    let recommendation =
                        self.generate_decrease_recommendation(decrease_percentage, &severity);

                    (
                        SiteDataChangeStatus::Decreased {
                            current_count: current_estimated_products,
                            previous_count: prev_count,
                            decrease_amount: decrease,
                        },
                        Some(recommendation),
                    )
                }
            }
        }
    }

    /// 데이터 감소 시 권장사항 생성
    fn generate_decrease_recommendation(
        &self,
        decrease_percentage: f64,
        severity: &SeverityLevel,
    ) -> DataDecreaseRecommendation {
        match severity {
            SeverityLevel::Low => DataDecreaseRecommendation {
                action_type: RecommendedAction::WaitAndRetry,
                description: format!(
                    "사이트 데이터가 {:.1}% 감소했습니다. 일시적인 변화일 수 있습니다.",
                    decrease_percentage
                ),
                severity: severity.clone(),
                action_steps: vec![
                    "잠시 후(5-10분) 다시 상태를 확인해보세요".to_string(),
                    "문제가 지속되면 수동으로 사이트를 확인해보세요".to_string(),
                ],
            },
            SeverityLevel::Medium => DataDecreaseRecommendation {
                action_type: RecommendedAction::ManualVerification,
                description: format!(
                    "사이트 데이터가 {:.1}% 감소했습니다. 수동 확인이 필요합니다.",
                    decrease_percentage
                ),
                severity: severity.clone(),
                action_steps: vec![
                    "CSA-IoT 사이트에서 직접 제품 수를 확인해보세요".to_string(),
                    "사이트에서 필터 설정이 변경되었는지 확인하세요".to_string(),
                    "데이터베이스를 백업하고 부분 재크롤링을 고려하세요".to_string(),
                ],
            },
            SeverityLevel::High => DataDecreaseRecommendation {
                action_type: RecommendedAction::BackupAndRecrawl,
                description: format!(
                    "사이트 데이터가 {:.1}% 크게 감소했습니다. 데이터베이스 백업 후 재크롤링을 권장합니다.",
                    decrease_percentage
                ),
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
                description: format!(
                    "사이트 데이터가 {:.1}% 심각하게 감소했습니다. 즉시 조치가 필요합니다.",
                    decrease_percentage
                ),
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
            ".page-numbers:not(.current):not(.dots)", // 현재 페이지와 줄임표를 제외한 페이지 번호
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
                        if page_num > max_page && page_num < 10000 {
                            // 합리적인 상한선
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
            r"/page/(\d+)/?", // CSA-IoT 사이트의 /page/123/ 패턴
            r"paged=(\d+)",
            r"page=(\d+)",
            r"/(\d+)/$",      // 끝에 숫자가 있는 경우
            r"page/(\d+)/\?", // page/123/?... 패턴
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

        info!(
            "Total products found on page: {} (using selector: {})",
            max_count, best_selector
        );
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

        let (product_count, max_pagination_page, _active_page, has_products) = {
            // Use consistent HttpClient
            let _client = self.create_configured_http_client()?;
            let response = self.http_client.fetch_response(&url).await?;
            let html_string: String = response.text().await?;

            let doc = scraper::Html::parse_document(&html_string);
            let product_count = self.count_products(&doc);
            let max_pagination_page = self.find_max_page_in_pagination(&doc);
            let active_page = self.get_active_page_number(&doc);
            let has_products = product_count > 0;

            (
                product_count,
                max_pagination_page,
                active_page,
                has_products,
            )
        };

        let analysis = PageAnalysisCache {
            max_page_in_pagination: max_pagination_page,
            product_count,
            has_products,
        };

        // 캐시에 저장
        {
            let mut cache = self.page_cache.lock().await;
            cache.insert(page_number, analysis.clone());
        }

        info!(
            "📊 Page {} analysis: has_products={}, product_count={}, max_pagination={}",
            page_number, has_products, product_count, max_pagination_page
        );

        Ok(analysis)
    }

    /// 캐시를 초기화 (새로운 상태 체크 시작 시 호출)
    async fn clear_page_cache_internal(&self) {
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
        let crawling_range = self
            .calculate_next_crawling_pages(
                &local_db_status,
                total_pages_on_site,
                products_on_last_page,
                estimated_products,
                &data_change_analysis,
            )
            .await?;

        info!("📊 Crawling range recommendation: {:?}", crawling_range);
        Ok(crawling_range)
    }

    /// 로컬 DB 상태 조회
    async fn get_local_db_status(&self) -> Result<LocalDbStatus> {
        if let Some(repo) = &self.product_repo {
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
                if let (Some(page_id), Some(index_in_page)) =
                    (product.page_id, product.index_in_page)
                {
                    if page_id > max_page_id {
                        max_page_id = page_id;
                        max_index_in_page = index_in_page;
                    } else if page_id == max_page_id && index_in_page > max_index_in_page {
                        max_index_in_page = index_in_page;
                    }
                }
            }

            info!(
                "📊 Local DB status: max_page_id={}, max_index_in_page={}, total_products={}",
                max_page_id,
                max_index_in_page,
                products.len()
            );

            Ok(LocalDbStatus {
                is_empty: false,
                max_page_id: max_page_id.max(0) as u32,
                max_index_in_page: max_index_in_page.max(0) as u32,
                total_saved_products: products.len() as u32,
            })
        } else {
            warn!("⚠️  Product repository not available - assuming empty DB");

            // DB 분석과 로컬 상태가 불일치할 수 있음을 경고
            warn!(
                "⚠️  DB inconsistency possible: repository unavailable but analysis may show different results"
            );

            Ok(LocalDbStatus {
                is_empty: true,
                max_page_id: 0,
                max_index_in_page: 0,
                total_saved_products: 0,
            })
        }
    }

    /// 사이트 데이터 변화 분석
    async fn analyze_site_data_changes(
        &self,
        current_estimated_products: u32,
    ) -> DataChangeAnalysis {
        let previous_count = self.config.app_managed.last_crawl_product_count;

        match previous_count {
            None => DataChangeAnalysis::Initial,
            Some(prev_count) => {
                let _change_percentage = if prev_count > 0 {
                    ((f64::from(current_estimated_products) - f64::from(prev_count))
                        / f64::from(prev_count))
                        * 100.0
                } else {
                    0.0
                };

                if current_estimated_products > prev_count {
                    DataChangeAnalysis::Increased {
                        new_products: current_estimated_products - prev_count,
                    }
                } else if current_estimated_products == prev_count {
                    DataChangeAnalysis::Stable
                } else {
                    DataChangeAnalysis::Decreased {
                        lost_products: prev_count - current_estimated_products,
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
            }
            DataChangeAnalysis::Decreased { lost_products, .. } => {
                warn!(
                    "📉 Site data decreased by {} products - recommending full recrawl",
                    lost_products
                );
                return Ok(CrawlingRangeRecommendation::Full);
            }
            DataChangeAnalysis::Increased { new_products, .. } => {
                // 새로운 제품이 많이 추가된 경우 부분 크롤링
                let recommended_pages =
                    (f64::from(*new_products) / f64::from(products_per_page)).ceil() as u32;
                let limited_pages =
                    recommended_pages.min(self.config.user.crawling.page_range_limit);

                info!(
                    "📈 Site data increased by {} products - recommending partial crawl of {} pages",
                    new_products, limited_pages
                );
                return Ok(CrawlingRangeRecommendation::Partial(limited_pages));
            }
            DataChangeAnalysis::Stable => {
                // 안정적인 경우 기존 로직 적용
            }
        }

        // 기존 로직: 로컬 DB 상태 기반 계산
        if local_db_status.is_empty {
            return Ok(CrawlingRangeRecommendation::Full);
        }

        // 1단계: 로컬 DB에 마지막으로 저장된 제품의 '역순 절대 인덱스' 계산
        let last_saved_index =
            (local_db_status.max_page_id * products_per_page) + local_db_status.max_index_in_page;
        info!("📊 Last saved product index: {}", last_saved_index);

        // 2단계: 다음에 크롤링해야 할 제품의 '역순 절대 인덱스' 결정
        let next_product_index = last_saved_index + 1;
        info!("📊 Next product index to crawl: {}", last_saved_index);

        // 3단계: '역순 절대 인덱스'를 웹사이트 페이지 번호로 변환
        let total_products =
            (total_pages_on_site.saturating_sub(1) * products_per_page) + products_on_last_page;

        // 다음 제품이 전체 제품 수를 초과하는 경우 (모든 제품 크롤링 완료)
        if next_product_index >= total_products {
            info!("📊 All products have been crawled - no crawling needed");
            return Ok(CrawlingRangeRecommendation::None);
        }

        // '순차 인덱스'로 변환 (최신 제품이 0)
        let forward_index = total_products
            .saturating_sub(1)
            .saturating_sub(next_product_index);

        // 웹사이트 페이지 번호 계산
        let target_page_number = (forward_index / products_per_page) + 1;

        info!("📊 Target page to start crawling: {}", target_page_number);

        // 4단계: 크롤링 페이지 범위 결정
        let max_crawl_pages = self.config.user.crawling.page_range_limit;
        let start_page = target_page_number;
        let end_page = if start_page >= max_crawl_pages {
            start_page.saturating_sub(max_crawl_pages).saturating_add(1)
        } else {
            1
        };

        let actual_pages_to_crawl = if start_page >= end_page {
            start_page.saturating_sub(end_page).saturating_add(1)
        } else {
            start_page
        };

        info!(
            "📊 Crawling range: pages {} to {} (total: {} pages)",
            start_page, end_page, actual_pages_to_crawl
        );

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
    Increased { new_products: u32 },
    Decreased { lost_products: u32 },
    Stable,
}

/// 컬렉터 설정 (Modern Rust 2024 준수)
///
/// `ValidatedCrawlingConfig에서` 검증된 값을 사용하여 하드코딩을 방지합니다.
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
    /// `ValidatedCrawlingConfig에서` `CollectorConfig` 생성
    ///
    /// # Arguments
    /// * `validated_config` - 검증된 크롤링 설정
    ///
    /// # Returns
    /// 설정값이 적용된 `CollectorConfig`
    #[must_use]
    pub fn from_validated(
        validated_config: &crate::application::validated_crawling_config::ValidatedCrawlingConfig,
    ) -> Self {
        // as_millis returns u128; convert safely to u64, saturating on overflow
        let delay_ms = match u64::try_from(validated_config.request_delay().as_millis()) {
            Ok(v) => v,
            Err(_) => u64::MAX,
        };

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
    /// 기본값은 `ValidatedCrawlingConfig::default()에서` 가져옴
    /// 하드코딩을 방지하기 위해 `ValidatedCrawlingConfig를` 사용
    fn default() -> Self {
        let validated_config =
            crate::application::validated_crawling_config::ValidatedCrawlingConfig::default();
        Self::from_validated(&validated_config)
    }
}

/// 헬스 스코어 계산 함수
fn calculate_health_score(response_time: Duration, total_pages: u32) -> f64 {
    // 응답 시간 기반 점수 (0.0 ~ 0.7) - 더 관대한 기준으로 수정
    let response_score = if response_time.as_millis() <= 2000 {
        0.7 // 2초 이하는 양호
    } else if response_time.as_millis() <= 5000 {
        0.5 // 5초 이하는 보통
    } else if response_time.as_millis() <= 10000 {
        0.3 // 10초 이하는 느림
    } else {
        0.1 // 10초 초과는 문제
    };

    // 페이지 수 기반 점수 (0.0 ~ 0.3) - 페이지 발견 여부가 더 중요
    let page_score = if total_pages > 0 { 0.3 } else { 0.0 };

    response_score + page_score
}

/// 제품 목록 수집 서비스 구현체
pub struct ProductListCollectorImpl {
    http_client: Arc<HttpClient>, // 🔥 Mutex 제거 - 페이지 수집도 진정한 동시성 구현
    data_extractor: Arc<MatterDataExtractor>,
    config: CollectorConfig,
    status_checker: Arc<StatusCheckerImpl>,
}

impl ProductListCollectorImpl {
    #[must_use]
    pub const fn new(
        http_client: Arc<HttpClient>, // 🔥 Mutex 제거
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

    /// 🔥 동시성을 보장하는 이벤트 기반 페이지 수집 메서드 (비동기 이벤트 큐 사용)
    pub async fn collect_page_range_with_async_events(
        &self,
        start_page: u32,
        end_page: u32,
        cancellation_token: Option<CancellationToken>,
        session_id: String,
        batch_id: String,
    ) -> Result<Vec<ProductUrl>> {
        use tokio::sync::mpsc;

        // 🔥 비동기 이벤트 큐 생성 (논블로킹)
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PageEvent>();

        // 🔥 이벤트 처리기를 별도 태스크로 분리 (메인 작업과 독립적)
        let session_id_clone = session_id.clone();
        let batch_id_clone = batch_id.clone();
        tokio::spawn(async move {
            while let Some(page_event) = event_rx.recv().await {
                // 이벤트 처리는 메인 작업 흐름과 완전히 독립적
                // 실패해도 메인 작업에 영향 없음
                if let Err(e) =
                    Self::handle_page_event(page_event, &session_id_clone, &batch_id_clone).await
                {
                    debug!("Event handling failed (non-critical): {}", e);
                }
            }
        });

        // Handle descending range (older to newer) - typical for our use case
        let pages: Vec<u32> = if start_page > end_page {
            (end_page..=start_page).rev().collect()
        } else {
            (start_page..=end_page).collect()
        };

        info!(
            "🔍 Collecting from {} pages in range {} to {} with true concurrent execution + async events",
            pages.len(),
            start_page,
            end_page
        );

        // 사이트 분석 정보를 가져와서 정확한 총 페이지 수와 마지막 페이지 제품 수 확인
        let (total_pages, products_on_last_page) =
            self.status_checker.discover_total_pages().await?;
        let last_page_number = total_pages;
        let products_in_last_page = products_on_last_page;

        // CanonicalPageIdCalculator 초기화 (legacy 구현 alias)
        let page_calculator =
            CanonicalPageIdCalculator::new(last_page_number, products_in_last_page as usize);
        let max_concurrent = self.config.max_concurrent as usize;

        // 진정한 동시성 실행을 위한 세마포어 기반 처리
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        let mut tasks = Vec::new();

        info!(
            "🚀 Creating {} concurrent tasks with semaphore control (max: {})",
            pages.len(),
            max_concurrent
        );

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
            let semaphore_clone = Arc::clone(&semaphore);
            let calculator = page_calculator.clone();
            let event_tx_clone = event_tx.clone();
            let cancellation_token_clone = cancellation_token.clone();

            // 각 태스크는 완전히 독립적으로 실행
            let task = tokio::spawn(async move {
                // 🔥 논블로킹 이벤트 발송 (실패해도 메인 작업 계속)
                let _ = event_tx_clone.send(PageEvent::Started { page_number: page });

                // 실행 허가를 받을 때까지 대기 (진정한 동시성 제어)
                let _permit = if let Ok(permit) = semaphore_clone.acquire().await {
                    debug!("🔓 Acquired permit for page {}", page);
                    permit
                } else {
                    let _ = event_tx_clone.send(PageEvent::Failed {
                        page_number: page,
                        error: "Semaphore acquisition failed".to_string(),
                    });
                    return Err(anyhow!("Semaphore acquisition failed"));
                };

                // 취소 확인
                if let Some(ref token) = cancellation_token_clone {
                    if token.is_cancelled() {
                        let _ = event_tx_clone.send(PageEvent::Cancelled { page_number: page });
                        return Err(anyhow!("Task cancelled"));
                    }
                }

                // 실제 페이지 수집 작업 (완전히 독립적)
                let start_time = std::time::Instant::now();
                let result = Self::collect_single_page_independently(
                    http_client,
                    data_extractor,
                    calculator,
                    page,
                )
                .await;

                let duration_ms = start_time.elapsed().as_millis() as u64;

                // 🔥 결과에 따른 논블로킹 이벤트 발송
                match &result {
                    Ok(products) => {
                        let _ = event_tx_clone.send(PageEvent::Completed {
                            page_number: page,
                            products_found: products.len() as u32,
                            duration_ms,
                        });
                    }
                    Err(e) => {
                        let _ = event_tx_clone.send(PageEvent::Failed {
                            page_number: page,
                            error: e.to_string(),
                        });
                    }
                }

                debug!("🔗 Page {} processing completed (permit released)", page);
                result.map(|products| (page, products))
            });

            tasks.push(task);
        }

        info!(
            "✅ Created {} tasks, waiting for all to complete with concurrent execution",
            tasks.len()
        );

        // 모든 태스크가 완료될 때까지 기다림 (진정한 파이프라인 실행)
        let results = futures::future::join_all(tasks).await;

        // 결과 수집
        let mut all_urls = Vec::new();
        let mut successful_pages = 0;
        let mut failed_pages = 0;

        for result in results {
            match result {
                Ok(Ok((page, mut urls))) => {
                    debug!("✅ Page {} completed with {} URLs", page, urls.len());
                    all_urls.append(&mut urls);
                    successful_pages += 1;
                }
                Ok(Err(e)) => {
                    warn!("❌ Page processing failed: {}", e);
                    failed_pages += 1;
                }
                Err(e) => {
                    warn!("❌ Task join failed: {}", e);
                    failed_pages += 1;
                }
            }
        }

        info!(
            "🎯 Phase 5 concurrent collection completed: {} pages successful, {} failed, {} total URLs",
            successful_pages,
            failed_pages,
            all_urls.len()
        );

        Ok(all_urls)
    }

    /// 🔥 완전히 독립적인 단일 페이지 수집 (의존성 최소화)
    async fn collect_single_page_independently(
        http_client: Arc<HttpClient>, // 🔥 Mutex 제거 - 페이지 수집도 진정한 동시성
        data_extractor: Arc<MatterDataExtractor>,
        calculator: CanonicalPageIdCalculator,
        page: u32,
    ) -> Result<Vec<ProductUrl>> {
        let url = format!(
            "https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver",
            page
        );

        // 🔥 Mutex 제거 - HTTP 클라이언트 직접 사용으로 진정한 동시성
        let response = http_client.fetch_response(&url).await?;
        let html_string: String = response.text().await?;

        let doc = scraper::Html::parse_document(&html_string);
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

        // 🔎 Debug summary for verification of page_id/index_in_page mapping
        if product_urls.is_empty() {
            debug!(
                "📐 Page {} produced no product URLs for mapping summary",
                page
            );
        } else {
            let min_page_id = product_urls.iter().map(|p| p.page_id).min().unwrap_or(0);
            let max_page_id = product_urls.iter().map(|p| p.page_id).max().unwrap_or(0);
            let min_index = product_urls
                .iter()
                .map(|p| p.index_in_page)
                .min()
                .unwrap_or(0);
            let max_index = product_urls
                .iter()
                .map(|p| p.index_in_page)
                .max()
                .unwrap_or(0);
            let sample: Vec<String> = product_urls
                .iter()
                .take(6)
                .enumerate()
                .map(|(i, p)| format!("i{}=>p{}_i{}", i, p.page_id, p.index_in_page))
                .collect();
            debug!(
                "📐 Page {} mapping summary: count={}, page_id=[{}..{}], index_in_page=[{}..{}], sample={:?}",
                page,
                product_urls.len(),
                min_page_id,
                max_page_id,
                min_index,
                max_index,
                sample
            );
        }

        Ok(product_urls)
    }

    /// 🔥 이벤트 처리기 (비동기, 논블로킹)
    async fn handle_page_event(event: PageEvent, _session_id: &str, _batch_id: &str) -> Result<()> {
        // 실제 이벤트 브로드캐스팅 로직
        // 이 함수는 메인 작업과 완전히 독립적으로 실행됨
        match event {
            PageEvent::Started { page_number } => {
                debug!("📄 Page {} started", page_number);
                // SystemStateBroadcaster::emit_product_list_page_event() 호출
            }
            PageEvent::Completed {
                page_number,
                products_found,
                duration_ms,
            } => {
                debug!(
                    "✅ Page {} completed: {} products in {}ms",
                    page_number, products_found, duration_ms
                );
                // 완료 이벤트 발송
            }
            PageEvent::Failed { page_number, error } => {
                debug!("❌ Page {} failed: {}", page_number, error);
                // 실패 이벤트 발송
            }
            PageEvent::Cancelled { page_number } => {
                debug!("🛑 Page {} cancelled", page_number);
                // 취소 이벤트 발송
            }
        }
        Ok(())
    }
}

/// 🔥 `ProductDetail` 태스크 이벤트 타입
#[derive(Debug, Clone)]
enum ProductDetailEvent {
    TaskStarted {
        product_url: String,
        product_name: Option<String>,
        task_id: String,
    },
    HttpRequestStarted {
        product_url: String,
        task_id: String,
    },
    ParsingStarted {
        product_url: String,
        task_id: String,
        html_size: usize,
    },
    TaskCompleted {
        product_url: String,
        product_name: Option<String>,
        task_id: String,
        processing_time: std::time::Duration,
        extracted_fields: u32,
    },
    TaskFailed {
        product_url: String,
        task_id: String,
        error: String,
        processing_time: std::time::Duration,
    },
}

#[async_trait]
impl ProductListCollector for ProductListCollectorImpl {
    async fn collect_page_batch(
        &self,
        pages: &[u32],
        total_pages: u32,
        products_on_last_page: u32,
    ) -> Result<Vec<ProductUrl>> {
        info!(
            "🔍 Collecting batch of {} pages with stateless design",
            pages.len()
        );

        let mut all_urls = Vec::new();
        for &page in pages {
            match self
                .collect_single_page(page, total_pages, products_on_last_page)
                .await
            {
                Ok(mut urls) => {
                    all_urls.append(&mut urls);
                    debug!("✅ Page {} completed with {} URLs", page, urls.len());
                }
                Err(e) => {
                    error!("❌ Failed to collect page {}: {}", page, e);
                }
            }
        }

        info!(
            "📊 Batch collection completed: {} total URLs from {} pages",
            all_urls.len(),
            pages.len()
        );
        Ok(all_urls)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn collect_all_pages(
        &self,
        total_pages: u32,
        products_on_last_page: u32,
    ) -> Result<Vec<ProductUrl>> {
        info!(
            "🔍 Collecting from {} pages with stateless parallel processing",
            total_pages
        );

        // Use the existing parallel implementation from collect_page_range
        self.collect_page_range(1, total_pages, total_pages, products_on_last_page)
            .await
    }

    async fn collect_page_range(
        &self,
        start_page: u32,
        end_page: u32,
        total_pages: u32,
        products_on_last_page: u32,
    ) -> Result<Vec<ProductUrl>> {
        // Handle descending range (older to newer) - typical for our use case
        let pages: Vec<u32> = if start_page > end_page {
            // Descending range: start from oldest (highest page number) to newest (lower page number)
            (end_page..=start_page).rev().collect()
        } else {
            // Ascending range: start from lowest to highest page number
            (start_page..=end_page).collect()
        };

        info!(
            "🔍 Collecting from {} pages in range {} to {} with stateless execution",
            pages.len(),
            start_page,
            end_page
        );

        // ✅ Clean Code: 명시적 파라미터 사용 (상태 의존성 제거)
        info!(
            "📊 Using explicit parameters: total_pages={}, products_on_last_page={}",
            total_pages, products_on_last_page
        );

        // CanonicalPageIdCalculator 초기화 (한 번만 생성)
        let page_calculator =
            CanonicalPageIdCalculator::new(total_pages, products_on_last_page as usize);

        let max_concurrent = self.config.max_concurrent as usize;

        // Phase 5 Implementation: 진정한 동시성 실행을 위한 세마포어 기반 처리
        // 1. 세마포어 생성: max_concurrent 개의 permit만 허용
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        // 2. 모든 페이지에 대해 즉시 태스크 생성 (하지만 세마포어로 제어)
        let mut tasks = Vec::new();

        info!(
            "🚀 Creating {} concurrent tasks with semaphore control (max: {})",
            pages.len(),
            max_concurrent
        );

        for page in pages {
            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let status_checker = Arc::clone(&self.status_checker);
            let semaphore_clone = Arc::clone(&semaphore);
            let calculator = page_calculator.clone(); // CanonicalPageIdCalculator 클론

            // 3. 각 태스크는 세마포어 permit을 획득한 후 실행
            let task = tokio::spawn(async move {
                // 실행 허가를 받을 때까지 대기 (진정한 동시성 제어)
                let _permit = if let Ok(permit) = semaphore_clone.acquire().await {
                    debug!("🔓 Acquired permit for page {}", page);
                    permit
                } else {
                    error!("Failed to acquire semaphore permit for page {}", page);
                    return Err(anyhow!("Semaphore acquisition failed"));
                };

                // ✅ PageIdCalculator를 사용한 크롤링 및 URL 생성
                let url = format!(
                    "https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver",
                    page
                );
                // Use consistent HttpClient for true concurrency
                let response = http_client.fetch_response(&url).await?;
                let html_string: String = response.text().await?;

                let doc = scraper::Html::parse_document(&html_string);
                let url_strings =
                    data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;

                // 활성 페이지 번호 추출 (리다이렉트/페이지네이션 불일치 방지)
                let active_page = status_checker.get_active_page_number(&doc);
                if active_page != page {
                    tracing::warn!(
                        "⚠️ Requested page {} but active pagination indicates {}. Using {} for page_id calculation.",
                        page,
                        active_page,
                        active_page
                    );
                }
                let effective_page = active_page.max(1);

                // ✅ PageIdCalculator를 사용한 ProductUrl 생성
                let product_urls: Vec<ProductUrl> = url_strings
                    .into_iter()
                    .enumerate()
                    .map(|(index, url)| {
                        let calculation = calculator.calculate(effective_page, index);
                        ProductUrl {
                            url,
                            page_id: calculation.page_id,
                            index_in_page: calculation.index_in_page,
                        }
                    })
                    .collect();

                debug!(
                    "🔗 Extracted {} URLs from page {} (permit released)",
                    product_urls.len(),
                    page
                );
                Ok::<(u32, Vec<ProductUrl>), anyhow::Error>((page, product_urls))
                // _permit이 여기서 자동으로 drop되어 다음 태스크가 실행될 수 있음
            });

            tasks.push(task);
        }

        info!(
            "✅ Created {} tasks, waiting for all to complete with concurrent execution",
            tasks.len()
        );

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
                }
                Ok(Err(e)) => {
                    error!("❌ Page collection failed: {}", e);
                    failed_pages += 1;
                }
                Err(e) => {
                    error!("❌ Task join failed: {}", e);
                    failed_pages += 1;
                }
            }
        }

        info!(
            "📊 Concurrent collection completed: {} successful, {} failed, {} total URLs",
            successful_pages,
            failed_pages,
            all_urls.len()
        );

        Ok(all_urls)
    }

    async fn collect_single_page(
        &self,
        page: u32,
        total_pages: u32,
        products_on_last_page: u32,
    ) -> Result<Vec<ProductUrl>> {
        // ✅ Clean Code: 명시적 파라미터 사용 (상태 의존성 제거)

        use tracing::{debug, info, warn};

    // 기대 규칙: 비말단(마지막이 아닌) 페이지는 정확히 PRODUCTS_PER_PAGE 개(현재 12)여야 하며
    // index_in_page 는 0..PRODUCTS_PER_PAGE-1 범위에서 중복 없이 채워져야 함.
    let expected_per_page: usize = crate::domain::constants::site::PRODUCTS_PER_PAGE as usize;
        let max_retries = self.config.retry_attempts.max(1); // 최소 1회는 시도
        let base_delay_ms: u64 = self.config.delay_ms.max(300);
        let max_delay_ms: u64 = 8_000;

        info!(
            "📊 Using cached site analysis for single page {}: total_pages={}, products_on_last_page={}, max_retries={}",
            page, total_pages, products_on_last_page, max_retries
        );

        let page_calculator =
            CanonicalPageIdCalculator::new(total_pages, products_on_last_page as usize);

        let is_last_page = page >= total_pages;

        let mut last_error: Option<anyhow::Error> = None;
        // Determine if persistent attempt logging is enabled (default: disabled for simple retry control stage)
        let attempt_log_enabled = std::env::var("MC_ATTEMPT_LOG_SQLITE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let pool_opt = if attempt_log_enabled {
            match crate::infrastructure::database_connection::get_or_init_global_pool().await {
                Ok(p) => Some(p),
                Err(e) => { tracing::debug!(target="crawler", error=%e, "attempt_log_pool_unavailable"); None }
            }
        } else { None };
        for attempt in 0..=max_retries {
            let attempt_start = std::time::Instant::now();
            // Predeclare attempt metrics for logging (filled progressively)
            let mut product_count: usize = 0;
            let mut distinct_indices: usize = 0;
            let mut contiguous_ok: bool = false;
            let mut count_mismatch_flag: bool = false;
            let mut index_mismatch_flag: bool = false;
            let mut success_final: bool = false;
            let mut error_code: Option<String> = None;
            let mut error_detail: Option<String> = None;
            let mut retry_scheduled: bool = false;
            let url = crate::infrastructure::config::utils::matter_products_page_url_simple(page);
            // 변형 전략: 재시도 시 UA/Referer를 회전하고 캐시 버스터 파라미터를 추가하여 서버측 변동 가능성 증가
            let ua_pool: &[&str] = &[
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
                "Mozilla/5.0 (iPhone; CPU iPhone OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Mobile/15E148 Safari/604.1",
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0 Safari/537.36",
            ];
            let referer_base = "https://csa-iot.org/csa-iot_products/";
            let variant_suffix = if attempt == 0 { String::new() } else { format!("&mc_variant={}", attempt) };
            let url_with_variant = format!("{}{}", url, variant_suffix);
            let opts = if attempt == 0 {
                crate::infrastructure::simple_http_client::RequestOptions::default()
            } else {
                let ua = ua_pool[(attempt as usize) % ua_pool.len()].to_string();
                let mut o = crate::infrastructure::simple_http_client::RequestOptions::default();
                o.user_agent_override = Some(ua);
                o.referer = Some(referer_base.to_string());
                // Removed trivial numeric casts (attempt/max_retries already u32)
                o.attempt = Some(attempt + 1);
                o.max_attempts = Some(max_retries + 1);
                o
            };
            // 정책 기반 HttpClient 사용 (상태 기반 재시도, Retry-After 준수) + 옵션 적용
            let response = match self.http_client.fetch_response_with_options(&url_with_variant, &opts).await {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(e);
                    error_code = Some("network_fetch_failed".to_string());
                    if attempt < max_retries {
                        retry_scheduled = true;
                        // 지수 백오프 + 지터
                        let shift = attempt.min(20);
                        let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
                        let base = (base_delay_ms.saturating_mul(factor)).min(max_delay_ms);
                        // Full jitter in [base/2, base]
                        let jitter = fastrand::u64(..=(base / 2));
                        let delay = (base / 2).saturating_add(jitter);
                        warn!(
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            delay_ms = delay,
                            "List page fetch failed (network). Retrying... page={}",
                            page
                        );
                        // Adaptive RPS reduction on repeated failures
                        if attempt >= 2 {
                            let () = crate::infrastructure::simple_http_client::HttpClient::set_global_max_rps(8).await;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        // Attempt log (network failure) before continue
                        let duration_ms = attempt_start.elapsed().as_millis() as i64;
                        if attempt_log_enabled {
                            if let Some(pool) = &pool_opt {
                            let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                                .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                                .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                                .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(retry_scheduled as i64)
                                .execute(pool).await;
                            }
                        }
                        continue;
                    }
                    // Final network failure (no retry)
                    let duration_ms = attempt_start.elapsed().as_millis() as i64;
                    if attempt_log_enabled {
                        if let Some(pool) = &pool_opt {
                        let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                            .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                            .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                            .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(retry_scheduled as i64)
                            .execute(pool).await;
                        }
                    }
                    break;
                }
            };

            let html_string: String = match response.text().await {
                Ok(s) => s,
                Err(e) => {
                    last_error = Some(anyhow::anyhow!(e));
                    error_code = Some("body_read_failed".to_string());
                    if attempt < max_retries {
                        retry_scheduled = true;
                        let shift = attempt.min(20);
                        let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
                        let base = (base_delay_ms.saturating_mul(factor)).min(max_delay_ms);
                        let jitter = fastrand::u64(..=(base / 2));
                        let delay = (base / 2).saturating_add(jitter);
                        warn!(
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            delay_ms = delay,
                            "List page body read failed. Retrying... page={}",
                            page
                        );
                        if attempt >= 2 {
                            let () = crate::infrastructure::simple_http_client::HttpClient::set_global_max_rps(8).await;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        let duration_ms = attempt_start.elapsed().as_millis() as i64;
                        if attempt_log_enabled {
                            if let Some(pool) = &pool_opt {
                            let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                                .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                                .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                                .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(retry_scheduled as i64)
                                .execute(pool).await;
                            }
                        }
                        continue;
                    }
                    let duration_ms = attempt_start.elapsed().as_millis() as i64;
                    if attempt_log_enabled {
                        if let Some(pool) = &pool_opt {
                        let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                            .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                            .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                            .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(retry_scheduled as i64)
                            .execute(pool).await;
                        }
                    }
                    break;
                }
            };

            // 비-Send 객체가 await를 넘지 않도록 스코프 한정
            let mut retry_needed = false;
            let mut out_urls: Option<Vec<ProductUrl>> = None;
            {
                let doc = scraper::Html::parse_document(&html_string);
                let url_strings_res: Result<Vec<String>, _> = self
                    .data_extractor
                    .extract_product_urls(&doc, "https://csa-iot.org");

                if let Err(e) = url_strings_res {
                    last_error = Some(anyhow::anyhow!(e));
                    if attempt < max_retries {
                        retry_needed = true;
                    }
                } else if last_error.is_some() && retry_needed {
                    // no further work in this scope
                } else if last_error.is_none() {
                    let url_strings = url_strings_res.unwrap();
                    // 활성 페이지 번호 확인 후 보정
                    let active_page = self.status_checker.get_active_page_number(&doc);
                    if active_page != page {
                        tracing::warn!(
                            "⚠️ Requested page {} but active pagination indicates {}. Using {} for page_id calculation.",
                            page,
                            active_page,
                            active_page
                        );
                    }
                    let effective_page = active_page.max(1);

                    // ✅ PageIdCalculator를 사용한 ProductUrl 생성
                    let product_urls: Vec<ProductUrl> = url_strings
                        .into_iter()
                        .enumerate()
                        .map(|(index, url)| {
                            let calculation = page_calculator.calculate(effective_page, index);
                            ProductUrl {
                                url,
                                page_id: calculation.page_id,
                                index_in_page: calculation.index_in_page,
                            }
                        })
                        .collect();

                    let count = product_urls.len();
                    product_count = count;
                    debug!(
                        "🔗 Extracted {} URLs from page {} (attempt {}/{})",
                        count,
                        page,
                        attempt + 1,
                        max_retries + 1
                    );

                    // 성공 판정:
                    //  - 마지막 페이지: 개수 제한 없음 (단 음수 불가)
                    //  - 비말단 페이지: 정확히 expected_per_page 개 AND 인덱스 커버리지(0..expected_per_page-1) 유니크 충족
                    use std::collections::HashSet;
                    let mut count_mismatch = false;
                    let mut index_mismatch = false;
                    let success = if is_last_page {
                        true
                    } else if count == expected_per_page {
                        let indices: HashSet<i32> = product_urls.iter().map(|p| p.index_in_page).collect();
                        distinct_indices = indices.len();
                        // 커버리지: 0..expected-1 모두 포함 여부
                        let has_full_coverage = distinct_indices == expected_per_page
                            && (0..expected_per_page as i32).all(|v| indices.contains(&v));
                        index_mismatch = !has_full_coverage;
                        has_full_coverage
                    } else {
                        count_mismatch = true;
                        false
                    };
                    // 기록용 플래그/지표
                    if distinct_indices == 0 { distinct_indices = product_urls.iter().map(|p| p.index_in_page).collect::<HashSet<_>>().len(); }
                    contiguous_ok = !is_last_page && count == expected_per_page && !index_mismatch;
                    count_mismatch_flag = count_mismatch;
                    index_mismatch_flag = index_mismatch;
                    success_final = success;
                    if success {
                        out_urls = Some(product_urls);
                    } else {
                        // 미충족: 재시도 필요 표시
                        if !is_last_page {
                            let reason = if count_mismatch { "count_mismatch" } else if index_mismatch { "index_mismatch" } else { "unknown_mismatch" };
                            let msg = format!(
                                "Page validation failed ({reason}): page={} expected_count={} actual_count={} attempt={}/{}",
                                page, expected_per_page, count, attempt+1, max_retries+1
                            );
                            if attempt < max_retries { retry_needed = true; }
                            last_error = Some(anyhow::anyhow!(msg.clone()));
                            error_code = Some(reason.to_string());
                            error_detail = Some(msg);
                            tracing::warn!(target="crawler", page, attempt=attempt+1, max_attempts=max_retries+1, reason, actual=count, expected=expected_per_page, "List page retry classification");
                        } else {
                            // Last page shouldn't come here; log defensively
                            tracing::warn!(target="crawler", page, actual=count, expected=expected_per_page, "Last page unexpected validation branch");
                        }
                    }
                }
            }

            // 스코프 밖: 비-Send 해제됨
            if let Some(urls) = out_urls {
                // Successful attempt log
                let duration_ms = attempt_start.elapsed().as_millis() as i64;
                if attempt_log_enabled {
                    if let Some(pool) = &pool_opt {
                    let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                        .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                        .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                        .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(retry_scheduled as i64)
                        .execute(pool).await;
                    }
                }
                return Ok(urls);
            }
            if retry_needed {
                let shift = attempt.min(20);
                let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
                let base = (base_delay_ms.saturating_mul(factor)).min(max_delay_ms);
                let jitter = fastrand::u64(..=(base / 2));
                let delay = (base / 2).saturating_add(jitter);
                warn!(
                    attempt = attempt + 1,
                    max = max_retries + 1,
                    delay_ms = delay,
                    "List page retry due to parse/insufficient items. page={}",
                    page
                );
                if attempt >= 2 {
                    let () =
                        crate::infrastructure::simple_http_client::HttpClient::set_global_max_rps(
                            8,
                        )
                        .await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                // Failed attempt with retry scheduled
                let duration_ms = attempt_start.elapsed().as_millis() as i64;
                if attempt_log_enabled {
                    if let Some(pool) = &pool_opt {
                    let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                        .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                        .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                        .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(true as i64)
                        .execute(pool).await;
                    }
                }
                continue;
            }
            // 에러가 있으나 재시도 불가하면 종료
            if last_error.is_some() {
                let duration_ms = attempt_start.elapsed().as_millis() as i64;
                if attempt_log_enabled {
                    if let Some(pool) = &pool_opt {
                    let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")
                        .bind(page as i64).bind(attempt as i64).bind(product_count as i64).bind(distinct_indices as i64).bind(contiguous_ok as i64)
                        .bind(count_mismatch_flag as i64).bind(index_mismatch_flag as i64).bind(is_last_page as i64).bind(success_final as i64)
                        .bind(error_code.as_deref()).bind(error_detail.as_deref()).bind(duration_ms).bind(retry_scheduled as i64)
                        .execute(pool).await;
                    }
                }
                break;
            }
        }

        // 최종 실패
        let final_err = last_error.unwrap_or_else(|| anyhow::anyhow!("List page collection failed for page {} (unknown error)", page));
        tracing::warn!(target="crawler", page, error=%final_err, "List page final failure after retries");
        // Final failure post-loop log already recorded; ensure a terminal attempt row exists if none captured success
        // (If loop broke due to last_error on final attempt we inserted above; otherwise insert here as safeguard.)
        if attempt_log_enabled {
            if let Some(pool) = &pool_opt {
                let safeguard_exists: Option<i64> = sqlx::query_scalar::<_, Option<i64>>("SELECT 1 FROM page_fetch_attempts WHERE logical_page_id=?1 AND attempt_no=?2 LIMIT 1")
                    .bind(page as i64).bind(max_retries as i64)
                    .fetch_optional(pool).await?
                    .flatten();
                if safeguard_exists.is_none() {
                    let _ = sqlx::query("INSERT OR IGNORE INTO page_fetch_attempts (logical_page_id, attempt_no, product_count, distinct_indices, contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, success_final, error_code, error_detail, duration_ms, retry_scheduled) VALUES (?1, ?2, 0, 0, 0, 0, 0, ?3, 0, 'unknown_final_failure', ?4, 0, 0)")
                        .bind(page as i64).bind(max_retries as i64).bind(is_last_page as i64).bind(format!("final failure: {final_err}"))
                        .execute(pool).await;
                }
            }
        }
        Err(final_err)
    }

    async fn collect_page_range_with_cancellation(
        &self,
        start_page: u32,
        end_page: u32,
        total_pages: u32,
        products_on_last_page: u32,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<ProductUrl>> {
        // Handle descending range (older to newer) - typical for our use case
        let pages: Vec<u32> = if start_page > end_page {
            // Descending range: start from oldest (highest page number) to newest (lower page number)
            (end_page..=start_page).rev().collect()
        } else {
            // Ascending range: start from lowest to highest page number
            (start_page..=end_page).collect()
        };

        info!(
            "🔍 Collecting from {} pages in range {} to {} with cancellation support and stateless execution",
            pages.len(),
            start_page,
            end_page
        );

        // ✅ Clean Code: 명시적 파라미터 사용 (상태 의존성 제거)

        info!(
            "📊 Using explicit parameters: total_pages={}, products_on_last_page={}",
            total_pages, products_on_last_page
        );

        // CanonicalPageIdCalculator 초기화 (한 번만 생성)
        let page_calculator =
            CanonicalPageIdCalculator::new(total_pages, products_on_last_page as usize);

        let max_concurrent = self.config.max_concurrent as usize;

        // Phase 5 Implementation: 진정한 동시성 실행을 위한 세마포어 기반 처리
        // 1. 세마포어 생성: max_concurrent 개의 permit만 허용
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        // 2. 모든 페이지에 대해 즉시 태스크 생성 (하지만 세마포어로 제어)
        let mut tasks = Vec::new();

        info!(
            "🚀 Creating {} concurrent tasks with semaphore control (max: {})",
            pages.len(),
            max_concurrent
        );

        for page in pages {
            // 취소 토큰 확인
            if cancellation_token.is_cancelled() {
                warn!("🛑 Task creation cancelled for page {}", page);
                break;
            }

            let http_client = Arc::clone(&self.http_client);
            let data_extractor = Arc::clone(&self.data_extractor);
            let status_checker = Arc::clone(&self.status_checker);
            let token_clone = cancellation_token.clone();
            let semaphore_clone = Arc::clone(&semaphore);
            let calculator = page_calculator.clone(); // CanonicalPageIdCalculator 클론

            // 3. 각 태스크는 세마포어 permit을 획득한 후 실행
            let task = tokio::spawn(async move {
                // 실행 허가를 받을 때까지 대기
                let _permit = if let Ok(permit) = semaphore_clone.acquire().await {
                    debug!("🔓 Acquired permit for page {}", page);
                    permit
                } else {
                    error!("Failed to acquire semaphore permit for page {}", page);
                    return Err(anyhow!("Semaphore acquisition failed"));
                };

                // 작업 시작 전 취소 확인
                if token_clone.is_cancelled() {
                    warn!("🛑 Task cancelled before execution for page {}", page);
                    return Err(anyhow!("Task cancelled"));
                }

                // 실제 페이지 수집 작업
                let url =
                    crate::infrastructure::config::utils::matter_products_page_url_simple(page);
                // Use consistent HttpClient for true concurrency
                let response = http_client.fetch_response(&url).await?;
                let html_string: String = response.text().await?;

                // 중간에 취소 확인
                if token_clone.is_cancelled() {
                    warn!("🛑 Task cancelled during processing for page {}", page);
                    return Err(anyhow!("Task cancelled during processing"));
                }

                let doc = scraper::Html::parse_document(&html_string);
                let url_strings =
                    data_extractor.extract_product_urls(&doc, "https://csa-iot.org")?;

                // 활성 페이지 번호 추출 및 보정
                let active_page = status_checker.get_active_page_number(&doc);
                if active_page != page {
                    tracing::warn!(
                        "⚠️ Requested page {} but active pagination indicates {}. Using {} for page_id calculation.",
                        page,
                        active_page,
                        active_page
                    );
                }
                let effective_page = active_page.max(1);

                // ✅ PageIdCalculator를 사용한 ProductUrl 생성
                let product_urls: Vec<ProductUrl> = url_strings
                    .into_iter()
                    .enumerate()
                    .map(|(index, url)| {
                        let calculation = calculator.calculate(effective_page, index);
                        ProductUrl {
                            url,
                            page_id: calculation.page_id,
                            index_in_page: calculation.index_in_page,
                        }
                    })
                    .collect();

                debug!(
                    "🔗 Extracted {} URLs from page {} (permit released)",
                    product_urls.len(),
                    page
                );
                Ok::<(u32, Vec<ProductUrl>), anyhow::Error>((page, product_urls))
                // _permit이 여기서 자동으로 drop되어 다음 태스크가 실행될 수 있음
            });

            tasks.push(task);
        }

        info!(
            "✅ Created {} tasks, waiting for all to complete with concurrent execution",
            tasks.len()
        );

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
                }
                Ok(Err(e)) => {
                    error!("❌ Page collection failed: {}", e);
                    failed_pages += 1;
                }
                Err(e) => {
                    error!("❌ Task join failed: {}", e);
                    failed_pages += 1;
                }
            }
        }

        info!(
            "📊 Concurrent collection completed: {} successful, {} failed, {} total URLs",
            successful_pages,
            failed_pages,
            all_urls.len()
        );

        Ok(all_urls)
    }
}

/// 데이터베이스 분석 서비스 구현체
pub struct DatabaseAnalyzerImpl {
    product_repo: Arc<IntegratedProductRepository>,
}

impl DatabaseAnalyzerImpl {
    #[must_use]
    pub const fn new(product_repo: Arc<IntegratedProductRepository>) -> Self {
        Self { product_repo }
    }
}

#[async_trait]
impl DatabaseAnalyzer for DatabaseAnalyzerImpl {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis> {
        // 💾 실제 DB에서 제품 정보를 가져옵니다
        info!("🔍 [DatabaseAnalyzer] Starting database analysis...");

        // 🔧 Debug: IntegratedProductRepository가 올바르게 설정되었는지 확인
        info!("🔍 [DatabaseAnalyzer] Using database pool from IntegratedProductRepository");

        // 🔍 Test database connection first
        info!("🔍 [DatabaseAnalyzer] Testing database connection...");

        // 🚀 Performance optimization: Use count query instead of loading all products
        let total_products = match self.product_repo.get_product_count().await {
            Ok(count) => {
                info!(
                    "✅ [DatabaseAnalyzer] Successfully retrieved total count from database: {}",
                    count
                );
                count as usize
            }
            Err(e) => {
                error!(
                    "❌ [DatabaseAnalyzer] Failed to get product count from database: {:?}",
                    e
                );
                error!("❌ [DatabaseAnalyzer] Error details: {}", e);
                error!("❌ [DatabaseAnalyzer] Error source: {:?}", e.source());
                error!(
                    "❌ [DatabaseAnalyzer] This is the exact error location that generates 'Product repository not available'"
                );

                // 🔧 Additional debugging: Try to check if the database exists
                info!("🔍 [DatabaseAnalyzer] Attempting additional diagnostics...");

                warn!("⚠️  Product repository not available - assuming empty DB");
                warn!(
                    "⚠️  DB inconsistency possible: repository unavailable but analysis may show different results"
                );
                return Ok(DatabaseAnalysis {
                    total_products: 0,
                    unique_products: 0,
                    missing_products_count: 0,
                    duplicate_count: 0,
                    last_update: Some(chrono::Utc::now()),
                    missing_fields_analysis: FieldAnalysis {
                        missing_company: 0,
                        missing_model: 0,
                        missing_matter_version: 0,
                        missing_connectivity: 0,
                        missing_certification_date: 0,
                    },
                    data_quality_score: 0.0,
                });
            }
        };

        info!(
            "📊 [DatabaseAnalyzer] Database analysis completed: {} total products",
            total_products
        );

        // 기본 분석 반환 - 필드 스키마에 맞게 수정
        Ok(DatabaseAnalysis {
            total_products: total_products as u32,
            unique_products: total_products as u32,
            missing_products_count: 0, // duplicate_count를 missing_products_count로 변경
            duplicate_count: 0,
            last_update: Some(chrono::Utc::now()),
            missing_fields_analysis: FieldAnalysis {
                missing_company: 0,
                missing_model: 0,
                missing_matter_version: 0,
                missing_connectivity: 0,
                missing_certification_date: 0,
            },
            data_quality_score: if total_products > 0 { 0.85 } else { 0.0 },
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
    http_client: Arc<HttpClient>, // 🔥 Mutex 제거 - GlobalRateLimiter가 동시성 관리
    data_extractor: Arc<MatterDataExtractor>,
    config: CollectorConfig,
    /// Optional emitter closure for ProductDetailKeyed AppEvents (Option A)
    event_emitter: Option<Arc<dyn Fn(AppEvent) + Send + Sync>>,
}

// Retry/Validation enums & helpers
#[derive(Debug)]
enum AttemptResult {
    Success(ProductDetail),
    Incomplete { detail: Option<ProductDetail>, reason: IncompleteReason },
    TransientError(anyhow::Error),
    FatalError(anyhow::Error),
}

#[derive(Debug)]
enum IncompleteReason {
    MissingCertificateId,
    MissingRequiredField(&'static str),
}

#[derive(Clone, Copy)]
struct DetailValidationRules {
    require_certificate_id: bool,
    required_fields: &'static [&'static str],
}

fn default_validation_rules() -> DetailValidationRules {
    DetailValidationRules { 
        require_certificate_id: true, 
        required_fields: &["certification_date"] 
    }
}

fn classify_network_error(e: &anyhow::Error) -> bool {
    let msg = format!("{}", e);
    msg.contains("timeout") || msg.contains("temporarily") || msg.contains("connection")
}

async fn attempt_collect_product(
    collector: &ProductDetailCollectorImpl,
    product_url: &ProductUrl,
    _attempt: u32,
    rules: &DetailValidationRules,
) -> AttemptResult {
    let fetch_res = collector.http_client.fetch_response_with_policy(&product_url.url).await;
    let response = match fetch_res {
        Ok(r) => r,
        Err(e) => {
            let ae = anyhow!(e);
            return if classify_network_error(&ae) { AttemptResult::TransientError(ae) } else { AttemptResult::FatalError(ae) };
        }
    };
    let body = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            let ae = anyhow!(e);
            return if classify_network_error(&ae) { AttemptResult::TransientError(ae) } else { AttemptResult::FatalError(ae) };
        }
    };
    let doc = scraper::Html::parse_document(&body);
    let extracted = match collector.data_extractor.extract_product_detail(&doc, product_url.url.clone()) {
        Ok(d) => d,
        Err(e) => {
            let ae = anyhow!(e);
            return AttemptResult::TransientError(ae);
        }
    };
    let mut detail = extracted;
    detail.page_id = Some(product_url.page_id);
    detail.index_in_page = Some(product_url.index_in_page);
    detail.id = Some(format!("p{:04}i{:02}", product_url.page_id, product_url.index_in_page));
    if rules.require_certificate_id && detail.certificate_id.as_deref().unwrap_or("").trim().is_empty() {
        return AttemptResult::Incomplete { detail: Some(detail), reason: IncompleteReason::MissingCertificateId };
    }
    for f in rules.required_fields {
        match *f {
            "manufacturer" if detail.manufacturer.as_deref().unwrap_or("").trim().is_empty() => {
                return AttemptResult::Incomplete { detail: Some(detail), reason: IncompleteReason::MissingRequiredField(*f) };
            }
            "model" if detail.model.as_deref().unwrap_or("").trim().is_empty() => {
                return AttemptResult::Incomplete { detail: Some(detail), reason: IncompleteReason::MissingRequiredField(*f) };
            }
            "certification_date" if detail.certification_date.as_deref().unwrap_or("").trim().is_empty() => {
                return AttemptResult::Incomplete { detail: Some(detail), reason: IncompleteReason::MissingRequiredField(*f) };
            }
            _ => {}
        }
    }
    AttemptResult::Success(detail)
}

impl ProductDetailCollectorImpl {
    async fn backoff_sleep(&self, attempt: u32) {
        let base = 500_u64;
        let delay = base * attempt as u64;
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
}

impl ProductDetailCollectorImpl {
    #[must_use]
    pub const fn new(
        http_client: Arc<HttpClient>, // 🔥 Mutex 제거
        data_extractor: Arc<MatterDataExtractor>,
        config: CollectorConfig,
    ) -> Self {
        Self {
            http_client,
            data_extractor,
            config,
            event_emitter: None,
        }
    }
    /// Attach an emitter closure (builder style)
    pub fn with_event_emitter(mut self, emitter: Arc<dyn Fn(AppEvent) + Send + Sync>) -> Self {
        self.event_emitter = Some(emitter);
        self
    }

    /// 🔥 `ProductDetail` 이벤트 처리기 (비동기, 논블로킹)
    async fn handle_product_detail_event(
        event: ProductDetailEvent,
        session_id: &str,
        batch_id: &str,
    ) -> Result<()> {
        // 🔥 이벤트 처리를 로그로만 남기고 실제 브로드캐스트는 ServiceBasedCrawlingEngine에서 처리
        // SystemStateBroadcaster는 AppHandle이 필요하므로 여기서는 직접 사용하지 않음

        match event {
            ProductDetailEvent::TaskStarted {
                product_url,
                product_name,
                task_id,
            } => {
                info!("🚀 Product task started: {} ({})", product_url, task_id);
                debug!(
                    "📝 Product: {} | Task: {} | Session: {} | Batch: {}",
                    product_name.unwrap_or_else(|| "Unknown".to_string()),
                    task_id,
                    session_id,
                    batch_id
                );
            }
            ProductDetailEvent::HttpRequestStarted {
                product_url,
                task_id,
            } => {
                debug!(
                    "🌐 HTTP request started for product: {} (task: {})",
                    product_url, task_id
                );
            }
            ProductDetailEvent::ParsingStarted {
                product_url,
                task_id,
                html_size,
            } => {
                debug!(
                    "🔍 Parsing started for product: {} (task: {}, HTML size: {})",
                    product_url, task_id, html_size
                );
            }
            ProductDetailEvent::TaskCompleted {
                product_url,
                product_name,
                task_id,
                processing_time,
                extracted_fields,
            } => {
                info!(
                    "✅ Product task completed: {} ({}) - {} fields extracted in {:?}",
                    product_url, task_id, extracted_fields, processing_time
                );
                debug!(
                    "📊 Product: {} | Fields: {} | Time: {:?} | Session: {} | Batch: {}",
                    product_name.unwrap_or_else(|| "Unknown".to_string()),
                    extracted_fields,
                    processing_time,
                    session_id,
                    batch_id
                );
            }
            ProductDetailEvent::TaskFailed {
                product_url,
                task_id,
                error,
                processing_time,
            } => {
                warn!(
                    "❌ Product task failed: {} ({}) - {} (took {:?})",
                    product_url, task_id, error, processing_time
                );
                debug!(
                    "💥 Error: {} | Time: {:?} | Session: {} | Batch: {}",
                    error, processing_time, session_id, batch_id
                );
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ProductDetailCollector for ProductDetailCollectorImpl {
    async fn collect_details(&self, product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>> {
        debug!("Collecting details (improved retry) for {} products", product_urls.len());
        let mut collected = Vec::with_capacity(product_urls.len());
        let max_retries = self.config.retry_attempts.max(1);
        let rules = default_validation_rules();
        for product_url in product_urls {
            let mut last_incomplete_reason: Option<String> = None;
            for attempt in 1..=max_retries {
                match attempt_collect_product(self, product_url, attempt, &rules).await {
                    AttemptResult::Success(detail) => { collected.push(detail); break; }
                    AttemptResult::Incomplete { detail: _d, reason } => {
                        let reason_str = match reason {
                            IncompleteReason::MissingCertificateId => "missing_certificate_id".to_string(),
                            IncompleteReason::MissingRequiredField(f) => format!("missing_field:{}", f),
                        };
                        last_incomplete_reason = Some(reason_str.clone());
                        if attempt == max_retries {
                            warn!("Incomplete product detail after {} attempts ({}): url={}", attempt, reason_str, product_url.url);
                        } else {
                            debug!("Retrying incomplete (attempt {} of {}): {} -> {}", attempt, max_retries, product_url.url, reason_str);
                            self.backoff_sleep(attempt).await;
                        }
                    }
                    AttemptResult::TransientError(e) => {
                        if attempt == max_retries {
                            warn!("Transient error final attempt ({}): url={} err={}", attempt, product_url.url, e);
                        } else {
                            debug!("Transient error attempt {} of {} (retrying): url={} err={}", attempt, max_retries, product_url.url, e);
                            self.backoff_sleep(attempt).await;
                        }
                    }
                    AttemptResult::FatalError(e) => {
                        warn!("Fatal error collecting detail (no retry): url={} err={}", product_url.url, e);
                        break;
                    }
                }
            }
            if let Some(r) = last_incomplete_reason.take() { trace!("Final incomplete reason for {}: {}", product_url.url, r); }
        }
        debug!("Collected {} complete product details (improved retry)", collected.len());
        Ok(collected)
    }

    async fn collect_details_with_cancellation(
        &self,
        product_urls: &[ProductUrl],
        cancellation_token: CancellationToken,
    ) -> Result<Vec<ProductDetail>> {
        debug!("Collecting details (improved retry + cancellation) for {} products", product_urls.len());
        let mut collected = Vec::with_capacity(product_urls.len());
        let max_retries = self.config.retry_attempts.max(1);
        let rules = default_validation_rules();
        'outer: for product_url in product_urls {
            if cancellation_token.is_cancelled() { warn!("Cancellation requested early; stopping"); break 'outer; }
            let mut last_incomplete_reason: Option<String> = None;
            for attempt in 1..=max_retries {
                if cancellation_token.is_cancelled() { warn!("Cancellation inside attempts for {}", product_url.url); break 'outer; }
                match attempt_collect_product(self, product_url, attempt, &rules).await {
                    AttemptResult::Success(detail) => { collected.push(detail); break; }
                    AttemptResult::Incomplete { detail: _d, reason } => {
                        let reason_str = match reason {
                            IncompleteReason::MissingCertificateId => "missing_certificate_id".to_string(),
                            IncompleteReason::MissingRequiredField(f) => format!("missing_field:{}", f),
                        };
                        last_incomplete_reason = Some(reason_str.clone());
                        if attempt == max_retries { warn!("Incomplete after {} attempts ({}): url={}", attempt, reason_str, product_url.url); }
                        else { self.backoff_sleep(attempt).await; }
                    }
                    AttemptResult::TransientError(e) => {
                        if attempt == max_retries { warn!("Transient final ({}): url={} err={}", attempt, product_url.url, e); }
                        else { self.backoff_sleep(attempt).await; }
                    }
                    AttemptResult::FatalError(e) => { warn!("Fatal (no retry): url={} err={}", product_url.url, e); break; }
                }
            }
            if let Some(r) = last_incomplete_reason.take() { trace!("Final incomplete reason for {}: {}", product_url.url, r); }
        }
        debug!("Collected {} complete product details (improved retry + cancellation)", collected.len());
        Ok(collected)
    }

    async fn collect_single_product(&self, product_url: &ProductUrl) -> Result<ProductDetail> {
        self.collect_details(&[product_url.clone()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to collect product detail"))
    }

    async fn collect_product_batch(
        &self,
        product_urls: &[ProductUrl],
    ) -> Result<Vec<ProductDetail>> {
        self.collect_details(product_urls).await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ProductDetailCollectorImpl {
    /// 🔥 동시성을 보장하는 이벤트 기반 제품 상세정보 수집 메서드 (비동기 이벤트 큐 사용)
    ///
    /// Errors
    /// Returns an error if HTTP fetching or parsing fails for all tasks in a way that prevents
    /// producing any results. Partial failures are logged and skipped; successful details are
    /// still returned.
    ///
    /// Panics
    /// This function does not intentionally panic. Internal task joins are awaited safely.
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cognitive_complexity)]
    #[allow(clippy::missing_errors_doc)]
    #[allow(clippy::missing_panics_doc)]
    pub async fn collect_details_with_async_events(
        &self,
        product_urls: &[ProductUrl],
        cancellation_token: Option<CancellationToken>,
        session_id: String,
        batch_id: String,
    ) -> Result<Vec<ProductDetail>> {
        info!(
            "🚀 Collecting details sequentially for {} products with async events",
            product_urls.len()
        );

        // 🔥 비동기 이벤트 큐 생성 (ProductDetail 태스크용)
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ProductDetailEvent>();

        // 🔥 이벤트 처리기 생성 (완전히 독립적)
        let session_id_clone = session_id.clone();
        let batch_id_clone = batch_id.clone();
        let event_handler = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if let Err(e) =
                    Self::handle_product_detail_event(event, &session_id_clone, &batch_id_clone)
                        .await
                {
                    warn!("🔥 Event handler error: {}", e);
                }
            }
        });

        let mut details = Vec::with_capacity(product_urls.len());
        let max_retries = self.config.retry_attempts.max(1);

        for product_url in product_urls {
            if let Some(ref token) = cancellation_token {
                if token.is_cancelled() {
                    warn!("Cancellation requested; stopping async-events detail collection early");
                    break;
                }
            }

            let url = product_url.url.clone();
            let page_id = product_url.page_id;
            let index_in_page = product_url.index_in_page;

            let start_time = std::time::Instant::now();
            let task_id = format!("product-{}", url);

            // Emit keyed detail fetch_started (phase=fetch, status=started)
            if let Some(em) = &self.event_emitter {
                em(AppEvent::ProductDetailKeyed {
                    session_id: session_id.clone(),
                    batch_id: Some(batch_id.clone()),
                    product_key: url.clone(),
                    product_url: url.clone(),
                    phase: "fetch".into(),
                    status: "started".into(),
                    attempt: Some(1),
                    duration_ms: None,
                    html_size: None,
                    extracted_fields: None,
                    error: None,
                    timestamp: Utc::now(),
                });
            }

            // 태스크 시작 이벤트
            let _ = event_tx.send(ProductDetailEvent::TaskStarted {
                product_url: url.clone(),
                product_name: None,
                task_id: task_id.clone(),
            });

            // HTTP 요청 시작 이벤트
            let _ = event_tx.send(ProductDetailEvent::HttpRequestStarted {
                product_url: url.clone(),
                task_id: task_id.clone(),
            });

            // Retry-aware HTTP fetch (sequential) with optional cancellation checks
            let mut attempts: u32 = 0;
            let mut html_opt: Option<String> = None;
            loop {
                if let Some(ref token) = cancellation_token {
                    if token.is_cancelled() {
                        warn!("Cancellation during retry loop for {}", url);
                        drop(event_tx);
                        let _ = event_handler.await;
                        return Ok(details);
                    }
                }

                attempts += 1;
                match self.http_client.fetch_response_with_policy(&url).await {
                    Ok(response) => match response.text().await {
                        Ok(s) => {
                            html_opt = Some(s);
                            break;
                        }
                        Err(e) => {
                            if attempts < max_retries {
                                tokio::time::sleep(Duration::from_millis(
                                    500 * u64::from(attempts),
                                ))
                                .await;
                            } else {
                                let _ = event_tx.send(ProductDetailEvent::TaskFailed {
                                    product_url: url.clone(),
                                    task_id: task_id.clone(),
                                    error: format!("Failed to read response: {}", e),
                                    processing_time: start_time.elapsed(),
                                });
                                if let Some(em) = &self.event_emitter {
                                    em(AppEvent::ProductDetailKeyed {
                                        session_id: session_id.clone(),
                                        batch_id: Some(batch_id.clone()),
                                        product_key: url.clone(),
                                        product_url: url.clone(),
                                        phase: "fetch".into(),
                                        status: "failed".into(),
                                        attempt: Some(attempts),
                                        duration_ms: Some(start_time.elapsed().as_millis() as u64),
                                        html_size: None,
                                        extracted_fields: None,
                                        error: Some("read_response".into()),
                                        timestamp: Utc::now(),
                                    });
                                }
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        if attempts < max_retries {
                            tokio::time::sleep(Duration::from_millis(500 * u64::from(attempts)))
                                .await;
                        } else {
                            let _ = event_tx.send(ProductDetailEvent::TaskFailed {
                                product_url: url.clone(),
                                task_id: task_id.clone(),
                                error: format!("HTTP request failed: {}", e),
                                processing_time: start_time.elapsed(),
                            });
                            if let Some(em) = &self.event_emitter {
                                em(AppEvent::ProductDetailKeyed {
                                    session_id: session_id.clone(),
                                    batch_id: Some(batch_id.clone()),
                                    product_key: url.clone(),
                                    product_url: url.clone(),
                                    phase: "fetch".into(),
                                    status: "failed".into(),
                                    attempt: Some(attempts),
                                    duration_ms: Some(start_time.elapsed().as_millis() as u64),
                                    html_size: None,
                                    extracted_fields: None,
                                    error: Some("http_failed".into()),
                                    timestamp: Utc::now(),
                                });
                            }
                            break;
                        }
                    }
                }
            }

            let Some(html) = html_opt else { continue };

            if let Some(ref token) = cancellation_token {
                if token.is_cancelled() {
                    warn!("Cancellation after fetch for {}", url);
                    break;
                }
            }

            // 파싱 시작 이벤트
            let _ = event_tx.send(ProductDetailEvent::ParsingStarted {
                product_url: url.clone(),
                task_id: task_id.clone(),
                html_size: html.len(),
            });
            if let Some(em) = &self.event_emitter {
                em(AppEvent::ProductDetailKeyed {
                    session_id: session_id.clone(),
                    batch_id: Some(batch_id.clone()),
                    product_key: url.clone(),
                    product_url: url.clone(),
                    phase: "parse".into(),
                    status: "started".into(),
                    attempt: Some(1),
                    duration_ms: None,
                    html_size: Some(html.len() as u32),
                    extracted_fields: None,
                    error: None,
                    timestamp: Utc::now(),
                });
            }

            let doc = scraper::Html::parse_document(&html);
            match self
                .data_extractor
                .extract_product_detail(&doc, url.clone())
            {
                Ok(mut detail) => {
                    detail.page_id = Some(page_id);
                    detail.index_in_page = Some(index_in_page);
                    detail.id = Some(format!("p{:04}i{:02}", page_id, index_in_page));

                    let _ = event_tx.send(ProductDetailEvent::TaskCompleted {
                        product_url: url.clone(),
                        product_name: detail.manufacturer.clone().or_else(|| detail.model.clone()),
                        task_id: task_id.clone(),
                        processing_time: start_time.elapsed(),
                        extracted_fields: calculate_extracted_fields(&detail),
                    });
                    if let Some(em) = &self.event_emitter {
                        em(AppEvent::ProductDetailKeyed {
                            session_id: session_id.clone(),
                            batch_id: Some(batch_id.clone()),
                            product_key: url.clone(),
                            product_url: url.clone(),
                            phase: "persist".into(),
                            status: "succeeded".into(),
                            attempt: Some(1),
                            duration_ms: Some(start_time.elapsed().as_millis() as u64),
                            html_size: Some(html.len() as u32),
                            extracted_fields: Some(calculate_extracted_fields(&detail)),
                            error: None,
                            timestamp: Utc::now(),
                        });
                    }

                    details.push(detail);
                }
                Err(e) => {
                    let _ = event_tx.send(ProductDetailEvent::TaskFailed {
                        product_url: url.clone(),
                        task_id: task_id.clone(),
                        error: format!("Parsing failed: {}", e),
                        processing_time: start_time.elapsed(),
                    });
                    if let Some(em) = &self.event_emitter {
                        em(AppEvent::ProductDetailKeyed {
                            session_id: session_id.clone(),
                            batch_id: Some(batch_id.clone()),
                            product_key: url.clone(),
                            product_url: url.clone(),
                            phase: "parse".into(),
                            status: "failed".into(),
                            attempt: Some(1),
                            duration_ms: Some(start_time.elapsed().as_millis() as u64),
                            html_size: Some(html.len() as u32),
                            extracted_fields: None,
                            error: Some("parse_failed".into()),
                            timestamp: Utc::now(),
                        });
                    }
                }
            }
        }

        // 🔥 이벤트 큐 정리
        drop(event_tx);
        let _ = event_handler.await;

        info!(
            "✅ Successfully collected {} product details with async events (sequential)",
            details.len()
        );
        Ok(details)
    }
}

/// 지능형 크롤링 범위 계산기
pub struct CrawlingRangeCalculator {
    product_repo: Arc<IntegratedProductRepository>,
    config: AppConfig,
}

/// Simplified progress snapshot used by smart_crawling without legacy domain::events types
#[derive(Debug, Clone)]
pub struct RangeSimpleProgress {
    pub current: u32,
    pub total: u32,
    pub percentage: f64,
    pub current_batch: Option<u32>,
    pub total_batches: Option<u32>,
    pub is_completed: bool,
}

impl CrawlingRangeCalculator {
    #[must_use]
    pub const fn new(product_repo: Arc<IntegratedProductRepository>, config: AppConfig) -> Self {
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
        info!(
            "🎯 입력 파라미터: total_pages={}, products_on_last_page={}",
            total_pages, products_on_last_page
        );

        // 사용자 설정에서 페이지 범위 제한 가져오기
        let user_page_limit = self.config.user.crawling.page_range_limit;
        let intelligent_mode = &self.config.user.crawling.intelligent_mode;

        info!(
            "⚙️ User settings: page_range_limit={}, intelligent_mode.enabled={}, max_range_limit={}",
            user_page_limit, intelligent_mode.enabled, intelligent_mode.max_range_limit
        );

        // 실제 사용할 페이지 제한 계산
        let effective_page_limit =
            if intelligent_mode.enabled && intelligent_mode.override_config_limit {
                // 지능형 모드에서 override가 허용된 경우, max_range_limit과 user_page_limit 중 작은 값 사용
                user_page_limit.min(intelligent_mode.max_range_limit)
            } else {
                // 일반 모드이거나 override가 비활성화된 경우, 사용자 설정값 그대로 사용
                user_page_limit
            };

        info!(
            "📊 Effective page limit for this crawling: {}",
            effective_page_limit
        );

        // 데이터베이스에서 현재 제품 정보 가져오기
        let all_products = match self.product_repo.get_all_products().await {
            Ok(products) => {
                info!(
                    "✅ Successfully retrieved {} products from database",
                    products.len()
                );
                products
            }
            Err(e) => {
                error!("❌ Failed to get products from database: {}", e);
                Vec::new()
            }
        };

        if all_products.is_empty() {
            info!(
                "📋 Database is empty - starting from the last page (page {})",
                total_pages
            );
            let end_page = (total_pages.saturating_sub(effective_page_limit - 1)).max(1);
            return Ok(Some((total_pages, end_page)));
        }

        // 가장 높은 page_id 찾기 (역순이므로 가장 작은 실제 페이지 번호)
        let max_page_id = all_products
            .iter()
            .filter_map(|p| p.page_id)
            .max()
            .unwrap_or(0);

        info!("🔍 Current max page_id in database: {}", max_page_id);

        // page_id에서 실제 페이지 번호로 변환
        // page_id 0 = 485페이지, page_id 1 = 484페이지, ..., page_id 5 = 480페이지
        // Overflow 방지: max_page_id가 total_pages보다 클 수 있음 (사이트 변경 등)
        let last_crawled_page = if max_page_id as u32 >= total_pages {
            warn!(
                "⚠️  Database max_page_id ({}) >= total_pages ({}), assuming no valid crawled pages",
                max_page_id, total_pages
            );
            0 // 유효한 크롤링된 페이지가 없다고 간주
        } else {
            total_pages - max_page_id as u32
        };
        info!(
            "📍 Last crawled page: {} (page_id: {})",
            last_crawled_page, max_page_id
        );

        // 다음 크롤링할 범위 계산
        // 현재 페이지의 제품 수집 상태 확인
        let current_page_products = all_products
            .iter()
            .filter(|p| p.page_id == Some(max_page_id))
            .count();

        let expected_products_on_current_page = if last_crawled_page == total_pages {
            // 마지막 페이지 (485페이지)라면 products_on_last_page만큼 있어야 함
            products_on_last_page as usize
        } else {
            // 다른 페이지라면 12개가 있어야 함
            DEFAULT_PRODUCTS_PER_PAGE as usize
        };

        info!(
            "🔍 Current page {} has {}/{} products",
            last_crawled_page, current_page_products, expected_products_on_current_page
        );

        // 다음 크롤링 시작 페이지 결정
        let start_page = if current_page_products < expected_products_on_current_page {
            // 현재 페이지가 완전히 수집되지 않았다면 현재 페이지부터 시작
            last_crawled_page.max(1)
        } else {
            // 현재 페이지가 완료되었다면 다음 페이지부터 시작
            if last_crawled_page > 1 {
                last_crawled_page - 1
            } else {
                info!("🏁 All pages have been crawled");
                return Ok(None);
            }
        };

        // 크롤링 범위 제한 적용 (사용자 설정 존중)
        let end_page = (start_page.saturating_sub(effective_page_limit - 1)).max(1);

        info!(
            "✅ Next crawling range: {}페이지 → {}페이지 (역순, 최대 {}페이지)",
            start_page, end_page, effective_page_limit
        );
        Ok(Some((start_page, end_page)))
    }
}

/// `ProductDetail을` Product로 변환하는 헬퍼 함수
#[must_use]
pub fn product_detail_to_product(detail: ProductDetail) -> Product {
    let mut product = Product {
        id: detail.id.clone(), // Use detail's id if available
        url: detail.url,
        manufacturer: detail.manufacturer,
        model: detail.model,
        certificate_id: detail.certificate_id,
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

/// 🔥 `ProductDetail에서` 추출된 필드 개수를 계산하는 헬퍼 함수
const fn calculate_extracted_fields(detail: &crate::domain::product::ProductDetail) -> u32 {
    let mut count = 0u32;

    if detail.manufacturer.is_some() {
        count += 1;
    }
    if detail.model.is_some() {
        count += 1;
    }
    if detail.device_type.is_some() {
        count += 1;
    }
    if detail.certificate_id.is_some() {
        count += 1;
    }
    if detail.certification_date.is_some() {
        count += 1;
    }
    if detail.hardware_version.is_some() {
        count += 1;
    }
    if detail.vid.is_some() {
        count += 1;
    }
    if detail.pid.is_some() {
        count += 1;
    }
    if detail.family_sku.is_some() {
        count += 1;
    }
    if detail.family_variant_sku.is_some() {
        count += 1;
    }
    if detail.firmware_version.is_some() {
        count += 1;
    }
    if detail.family_id.is_some() {
        count += 1;
    }
    if detail.specification_version.is_some() {
        count += 1;
    }
    if detail.transport_interface.is_some() {
        count += 1;
    }
    // primary_device_type_id removed; normalized list handled elsewhere
    if detail.application_categories.is_some() {
        count += 1;
    }

    count
}

// Additional trait implementations for service-based architecture

#[async_trait]
impl DatabaseAnalyzer for StatusCheckerImpl {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis> {
        // Placeholder implementation for service-based architecture
        Ok(DatabaseAnalysis {
            total_products: 0,
            unique_products: 0,
            missing_products_count: 0, // duplicate_count를 missing_products_count로 변경
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
        _cancellation_token: CancellationToken,
    ) -> Result<Vec<ProductDetail>> {
        // Placeholder implementation for service-based architecture
        Ok(Vec::new())
    }

    async fn collect_single_product(&self, _product_url: &ProductUrl) -> Result<ProductDetail> {
        // Placeholder implementation for service-based architecture
        Err(anyhow!("Not implemented"))
    }

    async fn collect_product_batch(
        &self,
        _product_urls: &[ProductUrl],
    ) -> Result<Vec<ProductDetail>> {
        // Placeholder implementation for service-based architecture
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl CrawlingRangeCalculator {
    /// 간단한 진행 상황 분석 (`smart_crawling` 명령어용)
    pub async fn analyze_simple_progress(
        &self,
        total_pages_on_site: u32,
        products_on_last_page: u32,
    ) -> Result<RangeSimpleProgress> {
        // 로컬 DB 상태 확인
        let all_products = match self.product_repo.get_all_products().await {
            Ok(products) => {
                info!(
                    "📊 Successfully retrieved {} products from database",
                    products.len()
                );
                products
            }
            Err(e) => {
                error!("❌ Failed to get products from database: {}", e);
                Vec::new()
            }
        };
        let saved_products = all_products.len() as u32;

        // 총 제품 수 추정
        let products_per_page = DEFAULT_PRODUCTS_PER_PAGE;
        // 안전한 총 제품 수 추정 (언더플로우/오버플로우 방지)
        let pages_except_last = total_pages_on_site.saturating_sub(1);
        let total_estimated_products = pages_except_last
            .saturating_mul(products_per_page)
            .saturating_add(products_on_last_page.min(products_per_page));

        // 진행률 계산
        let percentage = if total_estimated_products > 0 {
            (f64::from(saved_products) / f64::from(total_estimated_products)) * 100.0
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

        // 실제 페이지 번호로 변환 (page_id 0 = 마지막 페이지) — 언더플로우 방지
        let actual_last_crawled_page = if max_page_id >= 0 {
            let mp = max_page_id as u32;
            if mp > total_pages_on_site {
                warn!(
                    "⚠️ Detected inconsistency: max_page_id ({}) > total_pages_on_site ({}). Using 0 for actual_last_crawled_page.",
                    mp, total_pages_on_site
                );
            }
            total_pages_on_site.saturating_sub(mp)
        } else {
            0
        };

        info!(
            "📊 Progress: {}/{} products ({:.1}%), last crawled page: {} (page_id: {})",
            saved_products,
            total_estimated_products,
            percentage,
            actual_last_crawled_page,
            max_page_id
        );

        Ok(RangeSimpleProgress {
            current: saved_products,
            total: total_estimated_products,
            percentage,
            current_batch: Some(max_page_id as u32),
            total_batches: Some(total_pages_on_site),
            is_completed: percentage >= 100.0,
        })
    }

    pub async fn analyze_crawling_progress(
        &self,
        _url: &str,
        _config: &CrawlingConfig,
        _database_analysis: &DatabaseAnalysis,
    ) -> Result<RangeSimpleProgress> {
        // Placeholder implementation
        Ok(RangeSimpleProgress {
            current: 0,
            total: 1,
            percentage: 0.0,
            current_batch: Some(0),
            total_batches: Some(1),
            is_completed: false,
        })
    }
}

/// 🔥 페이지 처리 이벤트 (논블로킹 큐용)
#[derive(Debug, Clone)]
enum PageEvent {
    Started {
        page_number: u32,
    },
    Completed {
        page_number: u32,
        products_found: u32,
        duration_ms: u64,
    },
    Failed {
        page_number: u32,
        error: String,
    },
    Cancelled {
        page_number: u32,
    },
}
