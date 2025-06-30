# Matter Certis v2 - Rust/Tauri 배치 크롤링 구현 가이드
## Chapter 2: Stage 1 - 상태 체크 및 범위 계산

### 2.1 상태 체크 서비스 구현

#### 2.1.1 StatusChecker 핵심 구현

```rust
// src-tauri/src/application/services/status_checker.rs
use crate::domain::entities::*;
use crate::domain::events::ProgressUpdate;
use crate::infrastructure::repositories::DatabaseRepository;
use crate::infrastructure::crawling::WebCrawler;
use tokio::sync::mpsc;
use std::sync::Arc;
use anyhow::{Result, Context};
use tracing::{info, warn, error};

pub struct StatusChecker {
    crawler: Arc<dyn WebCrawler>,
    database: Arc<dyn DatabaseRepository>,
    config: CrawlingConfig,
    progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
}

impl StatusChecker {
    pub fn new(
        crawler: Arc<dyn WebCrawler>,
        database: Arc<dyn DatabaseRepository>,
        config: CrawlingConfig,
        progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
    ) -> Self {
        Self {
            crawler,
            database,
            config,
            progress_sender,
        }
    }
    
    pub async fn perform_comprehensive_check(&self) -> Result<StatusCheckResult, CrawlingError> {
        info!("Starting comprehensive status check");
        
        self.send_progress("Starting status check", 0.0).await;
        
        // 1. 사이트 총 페이지 수 조회 (재시도 포함)
        let site_total_pages = self.get_site_total_pages_with_retry().await
            .context("Failed to get site total pages")?;
        
        self.send_progress("Retrieved site page count", 25.0).await;
        info!("Site total pages: {}", site_total_pages);
        
        // 2. 마지막 페이지 제품 수 조회
        let last_page_product_count = self.get_last_page_product_count(site_total_pages).await
            .context("Failed to get last page product count")?;
        
        self.send_progress("Analyzed last page", 50.0).await;
        info!("Last page product count: {}", last_page_product_count);
        
        // 3. 로컬 DB 상태 분석
        let local_db_status = self.analyze_local_database().await
            .context("Failed to analyze local database")?;
        
        self.send_progress("Analyzed local database", 75.0).await;
        info!("Local DB products: {}, pages: {}", 
              local_db_status.total_products, 
              local_db_status.total_pages);
        
        // 4. 크롤링 범위 계산
        let crawling_range = self.calculate_crawling_range(
            site_total_pages, 
            last_page_product_count,
            &local_db_status
        ).await.context("Failed to calculate crawling range")?;
        
        // 5. 누락 데이터 분석
        let missing_analysis = self.analyze_missing_data(&local_db_status, &crawling_range).await
            .context("Failed to analyze missing data")?;
        
        self.send_progress("Status check completed", 100.0).await;
        
        let result = StatusCheckResult {
            site_total_pages,
            last_page_product_count,
            local_db_status,
            crawling_range,
            missing_analysis,
            check_timestamp: chrono::Utc::now(),
        };
        
        info!("Status check completed successfully");
        Ok(result)
    }
    
    async fn get_site_total_pages_with_retry(&self) -> Result<u32, CrawlingError> {
        let mut last_error = None;
        
        for attempt in 1..=self.config.max_retries_per_stage {
            self.send_progress(
                &format!("Fetching site page count (attempt {})", attempt), 
                10.0 + (attempt as f64 * 5.0)
            ).await;
            
            match self.fetch_total_pages().await {
                Ok(pages) => {
                    info!("Successfully retrieved total pages: {} (attempt {})", pages, attempt);
                    return Ok(pages);
                }
                Err(e) => {
                    warn!("Attempt {} failed: {}", attempt, e);
                    last_error = Some(e);
                    
                    if attempt < self.config.max_retries_per_stage {
                        let delay = std::time::Duration::from_millis(2000 * attempt as u64);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or(CrawlingError::Network("Unknown error".to_string())))
    }
    
    async fn fetch_total_pages(&self) -> Result<u32, CrawlingError> {
        // 첫 번째 페이지에서 총 페이지 수 추출
        let first_page_url = format!("{}?paged=1", self.config.matter_filter_url);
        
        let html = self.crawler.fetch_html(&first_page_url).await
            .map_err(|e| CrawlingError::Network(e.to_string()))?;
        
        // HTML에서 페이지네이션 정보 추출
        self.extract_total_pages_from_html(&html).await
    }
    
    async fn extract_total_pages_from_html(&self, html: &str) -> Result<u32, CrawlingError> {
        use scraper::{Html, Selector};
        
        let document = Html::parse_document(html);
        
        // 여러 가지 페이지네이션 패턴 시도
        let pagination_selectors = vec![
            ".pagination .page-numbers:last-child",
            ".page-numbers:last-of-type",
            ".pagination a:last-child",
            ".wp-pagenavi a:last-child",
        ];
        
        for selector_str in pagination_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).last() {
                    let text = element.text().collect::<String>();
                    
                    // 숫자 추출
                    if let Ok(page_num) = text.trim().parse::<u32>() {
                        if page_num > 0 && page_num < 10000 { // 합리적인 범위 체크
                            return Ok(page_num);
                        }
                    }
                }
            }
        }
        
        // 페이지네이션을 찾지 못한 경우 URL 기반 탐지
        self.detect_pages_by_probing().await
    }
    
    async fn detect_pages_by_probing(&self) -> Result<u32, CrawlingError> {
        info!("Detecting total pages by probing");
        
        let mut low = 1;
        let mut high = 1000; // 최대 추정치
        
        // 이진 탐색으로 마지막 페이지 찾기
        while low < high {
            let mid = (low + high + 1) / 2;
            let test_url = format!("{}?paged={}", self.config.matter_filter_url, mid);
            
            match self.crawler.fetch_html(&test_url).await {
                Ok(html) => {
                    if self.page_has_products(&html).await {
                        low = mid;
                    } else {
                        high = mid - 1;
                    }
                }
                Err(_) => {
                    high = mid - 1;
                }
            }
            
            // 프로그레스 업데이트
            let progress = 15.0 + ((mid as f64 / 1000.0) * 10.0);
            self.send_progress(&format!("Probing page {}", mid), progress).await;
        }
        
        Ok(low)
    }
    
    async fn page_has_products(&self, html: &str) -> bool {
        use scraper::{Html, Selector};
        
        let document = Html::parse_document(html);
        let product_selectors = vec![
            ".product-item",
            ".product-card",
            ".product-listing-item",
            "article.product",
        ];
        
        for selector_str in product_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if document.select(&selector).next().is_some() {
                    return true;
                }
            }
        }
        
        false
    }
    
    async fn get_last_page_product_count(&self, total_pages: u32) -> Result<u32, CrawlingError> {
        if total_pages == 0 {
            return Ok(0);
        }
        
        let last_page_url = format!("{}?paged={}", self.config.matter_filter_url, total_pages);
        
        let html = self.crawler.fetch_html(&last_page_url).await
            .map_err(|e| CrawlingError::Network(e.to_string()))?;
        
        self.count_products_in_html(&html).await
    }
    
    async fn count_products_in_html(&self, html: &str) -> Result<u32, CrawlingError> {
        use scraper::{Html, Selector};
        
        let document = Html::parse_document(html);
        let product_selectors = vec![
            ".product-item",
            ".product-card", 
            ".product-listing-item",
            "article.product",
        ];
        
        for selector_str in product_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                let count = document.select(&selector).count();
                if count > 0 {
                    return Ok(count as u32);
                }
            }
        }
        
        Err(CrawlingError::Parsing("No products found on page".to_string()))
    }
    
    async fn analyze_local_database(&self) -> Result<LocalDatabaseStatus, CrawlingError> {
        let total_products = self.database.count_products().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        let total_pages = self.database.count_pages().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        let max_page_id = self.database.get_max_page_id().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        let incomplete_pages = self.find_incomplete_pages().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        let missing_products = self.find_missing_products().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        let last_updated = self.database.get_last_update_timestamp().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        Ok(LocalDatabaseStatus {
            total_products,
            total_pages,
            max_page_id,
            incomplete_pages,
            missing_products,
            last_updated,
        })
    }
    
    async fn find_incomplete_pages(&self) -> Result<Vec<IncompletePageInfo>, anyhow::Error> {
        let page_product_counts = self.database.get_page_product_counts().await?;
        let mut incomplete_pages = Vec::new();
        
        for (page_id, count) in page_product_counts {
            let expected_count = if page_id == 0 {
                // pageId 0은 offset 때문에 12개 미만일 수 있음
                continue;
            } else {
                12
            };
            
            if count < expected_count {
                let missing_indices = self.find_missing_indices_for_page(page_id).await?;
                
                incomplete_pages.push(IncompletePageInfo {
                    page_id,
                    site_page_number: page_id, // pageId와 sitePageNumber는 동일하다고 가정
                    current_count: count,
                    expected_count,
                    missing_indices,
                });
            }
        }
        
        Ok(incomplete_pages)
    }
    
    async fn find_missing_indices_for_page(&self, page_id: u32) -> Result<Vec<u32>, anyhow::Error> {
        let existing_indices = self.database.get_product_indices_for_page(page_id).await?;
        let expected_indices: Vec<u32> = (0..12).collect();
        
        let missing_indices = expected_indices
            .into_iter()
            .filter(|&index| !existing_indices.contains(&index))
            .collect();
        
        Ok(missing_indices)
    }
    
    async fn find_missing_products(&self) -> Result<Vec<ProductRef>, anyhow::Error> {
        // 구현 필요: 특정 제품이 누락된 경우를 찾는 로직
        // 현재는 빈 벡터 반환
        Ok(Vec::new())
    }
    
    async fn calculate_crawling_range(
        &self,
        site_total_pages: u32,
        last_page_product_count: u32,
        local_db_status: &LocalDatabaseStatus,
    ) -> Result<CrawlingRange, CrawlingError> {
        // offset 계산: 마지막 페이지가 12개 미만인 경우의 offset
        let offset = if last_page_product_count < 12 {
            12 - last_page_product_count
        } else {
            0
        };
        
        // 크롤링 대상 페이지 범위 결정
        let start_site_page = 0; // 항상 0부터 시작 (sitePageNumber 기준)
        let end_site_page = self.config.target_page_count.saturating_sub(1);
        
        let crawling_range = CrawlingRange::new(start_site_page, end_site_page, offset);
        
        info!("Calculated crawling range: {} to {} (offset: {})", 
              start_site_page, end_site_page, offset);
        
        Ok(crawling_range)
    }
    
    async fn analyze_missing_data(
        &self,
        local_db_status: &LocalDatabaseStatus,
        crawling_range: &CrawlingRange,
    ) -> Result<MissingDataAnalysis, CrawlingError> {
        let mut missing_pages = Vec::new();
        let incomplete_pages = local_db_status.incomplete_pages.clone();
        let missing_products = local_db_status.missing_products.clone();
        
        // 완전히 누락된 페이지 찾기
        for site_page_number in &crawling_range.target_pages {
            let has_data = self.database.has_data_for_page(*site_page_number).await
                .map_err(|e| CrawlingError::Database(e.to_string()))?;
            
            if !has_data {
                missing_pages.push(*site_page_number);
            }
        }
        
        // 수집 전략 결정
        let collection_strategy = self.determine_collection_strategy(
            &missing_pages,
            &incomplete_pages,
            &missing_products,
        ).await;
        
        // 우선순위 페이지 결정
        let priority_pages = self.determine_priority_pages(
            &missing_pages,
            &incomplete_pages,
        ).await;
        
        let total_missing = missing_pages.len() as u32 + 
                           incomplete_pages.iter().map(|p| p.missing_indices.len() as u32).sum::<u32>() +
                           missing_products.len() as u32;
        
        Ok(MissingDataAnalysis {
            total_missing_products: total_missing,
            missing_pages,
            incomplete_pages,
            missing_products,
            collection_strategy,
            priority_pages,
        })
    }
    
    async fn determine_collection_strategy(
        &self,
        missing_pages: &[u32],
        incomplete_pages: &[IncompletePageInfo],
        missing_products: &[ProductRef],
    ) -> CollectionStrategy {
        let missing_page_count = missing_pages.len();
        let incomplete_page_count = incomplete_pages.len();
        let individual_product_count = missing_products.len();
        
        if missing_page_count == 0 && incomplete_page_count == 0 && individual_product_count == 0 {
            CollectionStrategy::RecoveryOnlyStrategy
        } else if missing_page_count > incomplete_page_count * 2 {
            CollectionStrategy::FullPageCollection
        } else if incomplete_page_count > missing_page_count {
            CollectionStrategy::IncrementalCollection
        } else {
            CollectionStrategy::MixedStrategy
        }
    }
    
    async fn determine_priority_pages(
        &self,
        missing_pages: &[u32],
        incomplete_pages: &[IncompletePageInfo],
    ) -> Vec<u32> {
        let mut priority_pages = Vec::new();
        
        // 완전히 누락된 페이지가 우선순위
        priority_pages.extend_from_slice(missing_pages);
        
        // 불완전한 페이지 중 누락이 많은 순으로 정렬
        let mut sorted_incomplete: Vec<_> = incomplete_pages.iter().collect();
        sorted_incomplete.sort_by_key(|p| std::cmp::Reverse(p.missing_indices.len()));
        
        for page in sorted_incomplete {
            priority_pages.push(page.site_page_number);
        }
        
        priority_pages
    }
    
    async fn send_progress(&self, message: &str, progress: f64) {
        let update = ProgressUpdate::StatusCheckProgress {
            current_step: message.to_string(),
            progress,
            details: None,
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            error!("Failed to send progress update: {}", e);
        }
    }
}
```

### 2.2 재시도 전략 구현

#### 2.2.1 재시도 관리자

```rust
// src-tauri/src/application/services/retry_manager.rs
use std::time::Duration;
use tokio::time::sleep;
use rand::Rng;

pub struct RetryManager {
    config: CrawlingConfig,
}

impl RetryManager {
    pub fn new(config: CrawlingConfig) -> Self {
        Self { config }
    }
    
    pub fn create_retry_strategy(&self, context: RetryContext) -> Box<dyn RetryStrategy + Send + Sync> {
        match context {
            RetryContext::SinglePage => Box::new(ExponentialBackoffStrategy::new(
                Duration::from_millis(1000),
                2.0,
                self.config.max_retries_per_page,
            )),
            RetryContext::Stage => Box::new(LinearBackoffStrategy::new(
                Duration::from_millis(5000),
                self.config.max_retries_per_stage,
            )),
            RetryContext::Network => Box::new(JitteredBackoffStrategy::new(
                Duration::from_millis(2000),
                0.1, // 10% jitter
                3,
            )),
            RetryContext::Database => Box::new(ExponentialBackoffStrategy::new(
                Duration::from_millis(500),
                1.5,
                5,
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RetryContext {
    SinglePage,
    Stage,
    Network,
    Database,
}

#[async_trait::async_trait]
pub trait RetryStrategy {
    async fn should_retry(&self, attempt: u32, error: &CrawlingError) -> bool;
    async fn delay_before_retry(&self, attempt: u32) -> Duration;
    fn max_attempts(&self) -> u32;
}

pub struct ExponentialBackoffStrategy {
    base_delay: Duration,
    multiplier: f64,
    max_attempts: u32,
}

impl ExponentialBackoffStrategy {
    pub fn new(base_delay: Duration, multiplier: f64, max_attempts: u32) -> Self {
        Self {
            base_delay,
            multiplier,
            max_attempts,
        }
    }
}

#[async_trait::async_trait]
impl RetryStrategy for ExponentialBackoffStrategy {
    async fn should_retry(&self, attempt: u32, error: &CrawlingError) -> bool {
        attempt < self.max_attempts && error.is_retryable()
    }
    
    async fn delay_before_retry(&self, attempt: u32) -> Duration {
        let delay_ms = self.base_delay.as_millis() as f64 * self.multiplier.powi(attempt as i32);
        let capped_delay = delay_ms.min(30000.0); // 최대 30초
        Duration::from_millis(capped_delay as u64)
    }
    
    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

pub struct LinearBackoffStrategy {
    base_delay: Duration,
    max_attempts: u32,
}

impl LinearBackoffStrategy {
    pub fn new(base_delay: Duration, max_attempts: u32) -> Self {
        Self { base_delay, max_attempts }
    }
}

#[async_trait::async_trait]
impl RetryStrategy for LinearBackoffStrategy {
    async fn should_retry(&self, attempt: u32, error: &CrawlingError) -> bool {
        attempt < self.max_attempts && error.is_retryable()
    }
    
    async fn delay_before_retry(&self, attempt: u32) -> Duration {
        Duration::from_millis(self.base_delay.as_millis() as u64 * (attempt + 1) as u64)
    }
    
    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

pub struct JitteredBackoffStrategy {
    base_delay: Duration,
    jitter_factor: f64,
    max_attempts: u32,
}

impl JitteredBackoffStrategy {
    pub fn new(base_delay: Duration, jitter_factor: f64, max_attempts: u32) -> Self {
        Self {
            base_delay,
            jitter_factor,
            max_attempts,
        }
    }
}

#[async_trait::async_trait]
impl RetryStrategy for JitteredBackoffStrategy {
    async fn should_retry(&self, attempt: u32, error: &CrawlingError) -> bool {
        attempt < self.max_attempts && error.is_retryable()
    }
    
    async fn delay_before_retry(&self, attempt: u32) -> Duration {
        let base_ms = self.base_delay.as_millis() as f64;
        let jitter = rand::thread_rng().gen_range(-self.jitter_factor..=self.jitter_factor);
        let jittered_delay = base_ms * (1.0 + jitter);
        
        Duration::from_millis(jittered_delay.max(100.0) as u64)
    }
    
    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}
```

### 2.3 데이터베이스 리포지토리 인터페이스

#### 2.3.1 데이터베이스 접근 추상화

```rust
// src-tauri/src/infrastructure/repositories/database_repository.rs
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[async_trait]
pub trait DatabaseRepository: Send + Sync {
    // 기본 카운트 조회
    async fn count_products(&self) -> Result<u32, anyhow::Error>;
    async fn count_pages(&self) -> Result<u32, anyhow::Error>;
    async fn get_max_page_id(&self) -> Result<Option<u32>, anyhow::Error>;
    
    // 페이지별 제품 정보
    async fn get_page_product_counts(&self) -> Result<HashMap<u32, u32>, anyhow::Error>;
    async fn get_product_indices_for_page(&self, page_id: u32) -> Result<Vec<u32>, anyhow::Error>;
    async fn has_data_for_page(&self, page_id: u32) -> Result<bool, anyhow::Error>;
    
    // 제품 CRUD
    async fn insert_products(&self, products: Vec<Product>) -> Result<u32, anyhow::Error>;
    async fn insert_matter_products(&self, products: Vec<MatterProduct>) -> Result<u32, anyhow::Error>;
    async fn get_products_by_page_id(&self, page_id: u32) -> Result<Vec<Product>, anyhow::Error>;
    
    // 메타데이터
    async fn get_last_update_timestamp(&self) -> Result<Option<DateTime<Utc>>, anyhow::Error>;
    async fn update_last_crawl_time(&self) -> Result<(), anyhow::Error>;
    
    // 누락 데이터 분석
    async fn find_incomplete_pages(&self) -> Result<Vec<IncompletePageInfo>, anyhow::Error>;
    async fn find_missing_products_in_page(&self, page_id: u32) -> Result<Vec<u32>, anyhow::Error>;
}

// 구체적인 SQLite 구현
pub struct SqliteRepository {
    pool: sqlx::SqlitePool,
}

impl SqliteRepository {
    pub async fn new(database_url: &str) -> Result<Self, anyhow::Error> {
        let pool = sqlx::SqlitePool::connect(database_url).await?;
        
        // 마이그레이션 실행
        sqlx::migrate!("./migrations").run(&pool).await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl DatabaseRepository for SqliteRepository {
    async fn count_products(&self) -> Result<u32, anyhow::Error> {
        let result = sqlx::query_scalar!("SELECT COUNT(*) FROM matter_products")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(result as u32)
    }
    
    async fn count_pages(&self) -> Result<u32, anyhow::Error> {
        let result = sqlx::query_scalar!("SELECT COUNT(DISTINCT pageId) FROM matter_products")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(result as u32)
    }
    
    async fn get_max_page_id(&self) -> Result<Option<u32>, anyhow::Error> {
        let result = sqlx::query_scalar!("SELECT MAX(pageId) FROM matter_products")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(result.map(|v| v as u32))
    }
    
    async fn get_page_product_counts(&self) -> Result<HashMap<u32, u32>, anyhow::Error> {
        let rows = sqlx::query!(
            "SELECT pageId, COUNT(*) as count FROM matter_products GROUP BY pageId"
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut counts = HashMap::new();
        for row in rows {
            counts.insert(row.pageId as u32, row.count as u32);
        }
        
        Ok(counts)
    }
    
    async fn get_product_indices_for_page(&self, page_id: u32) -> Result<Vec<u32>, anyhow::Error> {
        let rows = sqlx::query!(
            "SELECT indexInPage FROM matter_products WHERE pageId = ? ORDER BY indexInPage",
            page_id
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|row| row.indexInPage as u32).collect())
    }
    
    async fn has_data_for_page(&self, page_id: u32) -> Result<bool, anyhow::Error> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM matter_products WHERE pageId = ?",
            page_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(count > 0)
    }
    
    async fn insert_products(&self, products: Vec<Product>) -> Result<u32, anyhow::Error> {
        let mut tx = self.pool.begin().await?;
        let mut inserted_count = 0;
        
        for product in products {
            let result = sqlx::query!(
                r#"
                INSERT OR REPLACE INTO matter_products 
                (url, manufacturer, model, category, pageId, indexInPage)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
                product.url,
                product.brand,
                product.model_name,
                product.category,
                product.page_id,
                product.index_in_page
            )
            .execute(&mut *tx)
            .await?;
            
            if result.rows_affected() > 0 {
                inserted_count += 1;
            }
        }
        
        tx.commit().await?;
        Ok(inserted_count)
    }
    
    async fn insert_matter_products(&self, products: Vec<MatterProduct>) -> Result<u32, anyhow::Error> {
        let mut tx = self.pool.begin().await?;
        let mut inserted_count = 0;
        
        for product in products {
            let result = sqlx::query!(
                r#"
                INSERT OR REPLACE INTO matter_products 
                (url, manufacturer, model, category, vid, pid, certificationDate, 
                 certificationType, productDescription, additionalInfo)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                product.url,
                product.brand,
                product.model_name,
                product.category,
                product.vid,
                product.pid,
                product.certification_date,
                product.certification_type,
                product.product_description,
                product.additional_info
            )
            .execute(&mut *tx)
            .await?;
            
            if result.rows_affected() > 0 {
                inserted_count += 1;
            }
        }
        
        tx.commit().await?;
        Ok(inserted_count)
    }
    
    async fn get_products_by_page_id(&self, page_id: u32) -> Result<Vec<Product>, anyhow::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT url, manufacturer, model, category, pageId, indexInPage
            FROM matter_products 
            WHERE pageId = ?
            ORDER BY indexInPage
            "#,
            page_id
        )
        .fetch_all(&self.pool)
        .await?;
        
        let products = rows.into_iter().map(|row| Product {
            url: row.url,
            brand: row.manufacturer,
            model_name: row.model,
            category: row.category,
            page_id: row.pageId as u32,
            index_in_page: row.indexInPage as u32,
        }).collect();
        
        Ok(products)
    }
    
    async fn get_last_update_timestamp(&self) -> Result<Option<DateTime<Utc>>, anyhow::Error> {
        let result = sqlx::query_scalar!(
            "SELECT value FROM app_metadata WHERE key = 'lastUpdated'"
        )
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(timestamp_str) = result {
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)?;
            Ok(Some(timestamp.with_timezone(&Utc)))
        } else {
            Ok(None)
        }
    }
    
    async fn update_last_crawl_time(&self) -> Result<(), anyhow::Error> {
        let now = Utc::now();
        let timestamp_str = now.to_rfc3339();
        
        sqlx::query!(
            "INSERT OR REPLACE INTO app_metadata (key, value, updatedAt) VALUES (?, ?, ?)",
            "lastUpdated",
            timestamp_str,
            timestamp_str
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn find_incomplete_pages(&self) -> Result<Vec<IncompletePageInfo>, anyhow::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT pageId, COUNT(*) as count
            FROM matter_products 
            WHERE pageId > 0  -- pageId 0은 offset 때문에 제외
            GROUP BY pageId
            HAVING count < 12
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut incomplete_pages = Vec::new();
        
        for row in rows {
            let page_id = row.pageId as u32;
            let current_count = row.count as u32;
            let missing_indices = self.find_missing_products_in_page(page_id).await?;
            
            incomplete_pages.push(IncompletePageInfo {
                page_id,
                site_page_number: page_id, // 동일하다고 가정
                current_count,
                expected_count: 12,
                missing_indices,
            });
        }
        
        Ok(incomplete_pages)
    }
    
    async fn find_missing_products_in_page(&self, page_id: u32) -> Result<Vec<u32>, anyhow::Error> {
        let existing_indices = self.get_product_indices_for_page(page_id).await?;
        let all_indices: Vec<u32> = (0..12).collect();
        
        let missing = all_indices
            .into_iter()
            .filter(|&index| !existing_indices.contains(&index))
            .collect();
        
        Ok(missing)
    }
}
```

이것으로 Chapter 2가 완료되었습니다. 이 장에서는 Stage 1의 상태 체크 및 범위 계산 알고리즘을 상세히 구현했습니다.

다음 Chapter 3을 생성할까요? Chapter 3에서는 Stage 2 (데이터베이스 분석)와 Stage 3 (제품 목록 수집)을 다룰 예정입니다.
