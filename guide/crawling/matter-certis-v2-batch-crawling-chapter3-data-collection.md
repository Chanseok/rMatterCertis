# Matter Certis v2 - Rust/Tauri 배치 크롤링 구현 가이드
## Chapter 3: Stage 2-3 - 데이터베이스 분석 및 제품 목록 수집

### 3.1 Stage 2: 데이터베이스 분석 서비스

#### 3.1.1 DatabaseAnalyzer 구현

```rust
// src-tauri/src/application/services/database_analyzer.rs
use crate::domain::entities::*;
use crate::domain::events::ProgressUpdate;
use crate::infrastructure::repositories::DatabaseRepository;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use tracing::{info, warn, debug};

pub struct DatabaseAnalyzer {
    database: Arc<dyn DatabaseRepository>,
    progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
}

impl DatabaseAnalyzer {
    pub fn new(
        database: Arc<dyn DatabaseRepository>,
        progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
    ) -> Self {
        Self {
            database,
            progress_sender,
        }
    }
    
    pub async fn analyze_missing_data(
        &self, 
        site_status: &StatusCheckResult
    ) -> Result<MissingDataAnalysis, CrawlingError> {
        info!("Starting database analysis");
        
        self.send_progress("Starting database analysis", 0.0).await;
        
        // 1. 전체 누락 페이지 분석
        let missing_pages = self.find_missing_pages(site_status).await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        self.send_progress("Analyzed missing pages", 25.0).await;
        info!("Found {} missing pages", missing_pages.len());
        
        // 2. 불완전 페이지 분석
        let incomplete_pages = self.find_incomplete_pages().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        self.send_progress("Analyzed incomplete pages", 50.0).await;
        info!("Found {} incomplete pages", incomplete_pages.len());
        
        // 3. 개별 제품 누락 분석
        let missing_products = self.find_missing_products().await
            .map_err(|e| CrawlingError::Database(e.to_string()))?;
        
        self.send_progress("Analyzed missing products", 75.0).await;
        info!("Found {} missing individual products", missing_products.len());
        
        // 4. 수집 전략 결정
        let collection_strategy = self.determine_collection_strategy(
            &missing_pages, 
            &incomplete_pages, 
            &missing_products
        ).await;
        
        // 5. 우선순위 페이지 결정
        let priority_pages = self.determine_priority_pages(
            &missing_pages, 
            &incomplete_pages
        ).await;
        
        self.send_progress("Database analysis completed", 100.0).await;
        
        let total_missing = missing_pages.len() as u32 + 
                           incomplete_pages.iter().map(|p| p.missing_indices.len() as u32).sum::<u32>() +
                           missing_products.len() as u32;
        
        let analysis = MissingDataAnalysis {
            total_missing_products: total_missing,
            missing_pages,
            incomplete_pages,
            missing_products,
            collection_strategy,
            priority_pages,
        };
        
        info!("Database analysis completed. Total missing: {}", total_missing);
        Ok(analysis)
    }
    
    async fn find_missing_pages(&self, site_status: &StatusCheckResult) -> Result<Vec<u32>, anyhow::Error> {
        let mut missing_pages = Vec::new();
        let target_pages = &site_status.crawling_range.target_pages;
        
        debug!("Checking {} target pages for missing data", target_pages.len());
        
        for &site_page_number in target_pages {
            let has_data = self.database.has_data_for_page(site_page_number).await?;
            
            if !has_data {
                missing_pages.push(site_page_number);
                debug!("Page {} is missing", site_page_number);
            }
        }
        
        Ok(missing_pages)
    }
    
    async fn find_incomplete_pages(&self) -> Result<Vec<IncompletePageInfo>, anyhow::Error> {
        let page_counts = self.database.get_page_product_counts().await?;
        let mut incomplete_pages = Vec::new();
        
        for (page_id, count) in page_counts {
            let expected_count = if page_id == 0 {
                // pageId 0은 offset 때문에 12개 미만일 수 있음
                // 실제 expected count를 계산해야 함
                self.calculate_expected_count_for_first_page().await?
            } else {
                12
            };
            
            if count < expected_count {
                let missing_indices = self.database.find_missing_products_in_page(page_id).await?;
                
                incomplete_pages.push(IncompletePageInfo {
                    page_id,
                    site_page_number: page_id, // pageId와 sitePageNumber가 동일하다고 가정
                    current_count: count,
                    expected_count,
                    missing_indices,
                });
                
                debug!("Page {} is incomplete: {}/{} products", page_id, count, expected_count);
            }
        }
        
        Ok(incomplete_pages)
    }
    
    async fn calculate_expected_count_for_first_page(&self) -> Result<u32, anyhow::Error> {
        // 이 메서드는 StatusChecker에서 계산된 offset 정보를 사용해야 함
        // 현재는 기본값 12를 반환하지만, 실제로는 StatusCheckResult에서 가져와야 함
        Ok(12)
    }
    
    async fn find_missing_products(&self) -> Result<Vec<ProductRef>, anyhow::Error> {
        // 현재는 개별 제품 누락을 감지하는 로직이 복잡하므로 간단히 구현
        // 실제로는 URL 패턴이나 특정 조건을 기반으로 누락된 제품을 찾아야 함
        
        let mut missing_products = Vec::new();
        
        // 예시: 특정 패턴의 URL이 누락된 경우를 찾는 로직
        // 이는 실제 요구사항에 따라 구현되어야 함
        
        Ok(missing_products)
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
        
        info!("Strategy analysis - Missing: {}, Incomplete: {}, Individual: {}", 
              missing_page_count, incomplete_page_count, individual_product_count);
        
        // 전체 작업량 대비 누락된 페이지 비율 계산
        let total_pages = missing_page_count + incomplete_page_count;
        
        if total_pages == 0 && individual_product_count == 0 {
            CollectionStrategy::RecoveryOnlyStrategy
        } else if missing_page_count > incomplete_page_count * 3 {
            // 누락된 전체 페이지가 불완전한 페이지보다 3배 이상 많으면 전체 수집
            CollectionStrategy::FullPageCollection
        } else if incomplete_page_count > missing_page_count * 2 {
            // 불완전한 페이지가 누락된 페이지보다 2배 이상 많으면 점진적 수집
            CollectionStrategy::IncrementalCollection
        } else {
            // 그 외의 경우 혼합 전략
            CollectionStrategy::MixedStrategy
        }
    }
    
    async fn determine_priority_pages(
        &self,
        missing_pages: &[u32],
        incomplete_pages: &[IncompletePageInfo],
    ) -> Vec<u32> {
        let mut priority_pages = Vec::new();
        
        // 1. 완전히 누락된 페이지를 먼저 처리 (낮은 페이지 번호부터)
        let mut sorted_missing = missing_pages.to_vec();
        sorted_missing.sort();
        priority_pages.extend(sorted_missing);
        
        // 2. 불완전한 페이지를 누락 정도에 따라 정렬
        let mut sorted_incomplete: Vec<_> = incomplete_pages.iter().collect();
        sorted_incomplete.sort_by_key(|page| {
            // 누락된 제품 수가 많을수록 우선순위가 높음
            std::cmp::Reverse(page.missing_indices.len())
        });
        
        for page in sorted_incomplete {
            if !priority_pages.contains(&page.site_page_number) {
                priority_pages.push(page.site_page_number);
            }
        }
        
        info!("Determined {} priority pages", priority_pages.len());
        priority_pages
    }
    
    async fn send_progress(&self, message: &str, progress: f64) {
        let update = ProgressUpdate::ProgressUpdate {
            stage_id: 2,
            current_item: (progress as u32).min(100),
            total_items: 100,
            elapsed_time: 0, // 실제로는 시간 측정 필요
            estimated_remaining: None,
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            warn!("Failed to send progress update: {}", e);
        }
    }
}
```

### 3.2 Stage 3: 제품 목록 수집 서비스

#### 3.2.1 ProductListCollector 핵심 구현

```rust
// src-tauri/src/application/services/product_list_collector.rs
use crate::domain::entities::*;
use crate::domain::events::ProgressUpdate;
use crate::infrastructure::repositories::DatabaseRepository;
use crate::infrastructure::crawling::WebCrawler;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{sleep, Duration, Instant};
use std::sync::Arc;
use std::collections::HashMap;
use futures::future::join_all;
use tracing::{info, warn, error, debug};

pub struct ProductListCollector {
    crawler: Arc<dyn WebCrawler>,
    database: Arc<dyn DatabaseRepository>,
    config: CrawlingConfig,
    progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
}

impl ProductListCollector {
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
    
    pub async fn collect_product_lists(
        &self, 
        target_pages: Vec<u32>
    ) -> Result<CollectionResult, CrawlingError> {
        info!("Starting product list collection for {} pages", target_pages.len());
        
        let mut result = CollectionResult::new();
        let start_time = Instant::now();
        
        // 동시 처리를 위한 세마포어
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_limit as usize));
        let mut tasks = Vec::new();
        
        // 각 페이지에 대한 수집 태스크 생성
        for (index, site_page_number) in target_pages.iter().enumerate() {
            let sem = Arc::clone(&semaphore);
            let collector = self.clone();
            let page_number = *site_page_number;
            let task_index = index;
            
            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                
                // 요청 간 지연
                if task_index > 0 {
                    let delay = Duration::from_millis(
                        collector.config.request_delay_ms + (task_index as u64 * 100)
                    );
                    sleep(delay).await;
                }
                
                collector.collect_single_page_with_merge(page_number).await
            });
            
            tasks.push((page_number, task));
        }
        
        // 결과 수집 및 재시도 관리
        let mut completed = 0;
        let total = target_pages.len();
        
        for (site_page_number, task) in tasks {
            match task.await.unwrap() {
                Ok(page_result) => {
                    completed += 1;
                    result.successful_pages.push(page_result.clone());
                    
                    // 데이터베이스에 즉시 저장 (설정에 따라)
                    if self.config.auto_save_to_db {
                        if let Err(e) = self.save_products_to_db(&page_result.products).await {
                            warn!("Failed to save products for page {}: {}", site_page_number, e);
                        }
                    }
                    
                    self.send_page_completed_update(
                        site_page_number,
                        page_result.products.len() as u32,
                        completed,
                        total,
                    ).await;
                }
                Err(e) => {
                    result.failed_pages.push(FailedPageInfo {
                        site_page_number,
                        error: e.clone(),
                        retry_count: 0,
                        last_attempt: chrono::Utc::now(),
                    });
                    
                    self.send_page_failed_update(site_page_number, e.to_string(), 0).await;
                }
            }
        }
        
        // 실패한 페이지 재시도
        if !result.failed_pages.is_empty() {
            info!("Retrying {} failed pages", result.failed_pages.len());
            result = self.retry_failed_pages(result).await?;
        }
        
        // 최종 통계 계산
        result.total_products_collected = result.successful_pages
            .iter()
            .map(|p| p.products.len() as u32)
            .sum();
        
        result.completion_rate = if target_pages.is_empty() {
            100.0
        } else {
            (result.successful_pages.len() as f64 / target_pages.len() as f64) * 100.0
        };
        
        let elapsed = start_time.elapsed();
        info!(
            "Product list collection completed. Success: {}/{}, Total products: {}, Elapsed: {:?}",
            result.successful_pages.len(),
            target_pages.len(),
            result.total_products_collected,
            elapsed
        );
        
        Ok(result)
    }
    
    async fn collect_single_page_with_merge(
        &self, 
        site_page_number: u32
    ) -> Result<PageCollectionResult, CrawlingError> {
        let start_time = Instant::now();
        let url = self.build_page_url(site_page_number);
        let mut collected_products = Vec::new();
        let mut attempt = 0;
        
        debug!("Starting collection for page {} at {}", site_page_number, url);
        
        // 기존에 수집된 불완전 데이터 확인 및 병합
        if let Ok(existing_products) = self.get_existing_products_for_page(site_page_number).await {
            collected_products.extend(existing_products);
            debug!("Found {} existing products for page {}", collected_products.len(), site_page_number);
        }
        
        // 페이지 크롤링 재시도 루프
        while attempt < self.config.max_retries_per_page {
            self.send_page_started_update(site_page_number, &url).await;
            
            match self.crawler.crawl_product_list(&url, site_page_number).await {
                Ok(new_products) => {
                    debug!("Crawled {} new products from page {}", new_products.len(), site_page_number);
                    
                    // 기존 제품과 새 제품 병합 (URL 기준 중복 제거)
                    collected_products = self.merge_products(collected_products, new_products);
                    
                    let expected_count = self.get_expected_count_for_page(site_page_number).await?;
                    
                    if collected_products.len() >= expected_count as usize {
                        let elapsed = start_time.elapsed();
                        info!(
                            "Page {} collection completed: {}/{} products in {:?}",
                            site_page_number, collected_products.len(), expected_count, elapsed
                        );
                        
                        return Ok(PageCollectionResult {
                            site_page_number,
                            products: collected_products,
                            is_complete: true,
                            attempt_count: attempt + 1,
                            elapsed_time: elapsed.as_millis() as u64,
                        });
                    }
                    
                    debug!(
                        "Page {} incomplete: {}/{} products, will retry",
                        site_page_number, collected_products.len(), expected_count
                    );
                }
                Err(e) => {
                    warn!("Page {} collection attempt {} failed: {}", site_page_number, attempt + 1, e);
                }
            }
            
            attempt += 1;
            if attempt < self.config.max_retries_per_page {
                let delay = Duration::from_millis(self.config.request_delay_ms * (attempt + 1) as u64);
                sleep(delay).await;
            }
        }
        
        // 최대 재시도 후에도 불완전한 경우
        let elapsed = start_time.elapsed();
        warn!(
            "Page {} collection incomplete after {} attempts: {} products",
            site_page_number, attempt, collected_products.len()
        );
        
        Ok(PageCollectionResult {
            site_page_number,
            products: collected_products,
            is_complete: false,
            attempt_count: attempt,
            elapsed_time: elapsed.as_millis() as u64,
        })
    }
    
    fn build_page_url(&self, site_page_number: u32) -> String {
        format!("{}?paged={}", self.config.matter_filter_url, site_page_number + 1)
    }
    
    async fn get_existing_products_for_page(&self, site_page_number: u32) -> Result<Vec<Product>, anyhow::Error> {
        // pageId와 sitePageNumber가 동일하다고 가정
        self.database.get_products_by_page_id(site_page_number).await
    }
    
    async fn get_expected_count_for_page(&self, site_page_number: u32) -> Result<u32, CrawlingError> {
        if site_page_number == 0 {
            // 첫 번째 페이지는 offset을 고려해야 함
            // StatusCheckResult에서 계산된 정보를 사용해야 하지만,
            // 현재는 간단히 12를 기본값으로 사용
            Ok(12)
        } else {
            Ok(12)
        }
    }
    
    fn merge_products(&self, existing: Vec<Product>, new: Vec<Product>) -> Vec<Product> {
        let mut merged = HashMap::new();
        
        // 기존 제품들을 URL 키로 저장
        for product in existing {
            merged.insert(product.url.clone(), product);
        }
        
        // 새 제품들을 병합 (URL이 같으면 새 것으로 대체)
        for product in new {
            merged.insert(product.url.clone(), product);
        }
        
        // HashMap에서 벡터로 변환하고 indexInPage로 정렬
        let mut result: Vec<Product> = merged.into_values().collect();
        result.sort_by_key(|p| p.index_in_page);
        
        result
    }
    
    async fn retry_failed_pages(&self, mut result: CollectionResult) -> Result<CollectionResult, CrawlingError> {
        let mut stage_retry_count = 0;
        
        while !result.failed_pages.is_empty() && stage_retry_count < self.config.max_retries_per_stage {
            let failed_pages = std::mem::take(&mut result.failed_pages);
            stage_retry_count += 1;
            
            info!("Stage retry attempt {} for {} pages", stage_retry_count, failed_pages.len());
            
            // 재시도 간격
            sleep(Duration::from_millis(5000)).await;
            
            for mut failed_page in failed_pages {
                self.send_retry_started_update(failed_page.site_page_number, stage_retry_count).await;
                
                match self.collect_single_page_with_merge(failed_page.site_page_number).await {
                    Ok(page_result) => {
                        result.successful_pages.push(page_result.clone());
                        
                        // 성공한 경우 데이터베이스에 저장
                        if self.config.auto_save_to_db {
                            if let Err(e) = self.save_products_to_db(&page_result.products).await {
                                warn!("Failed to save retried products for page {}: {}", 
                                      failed_page.site_page_number, e);
                            }
                        }
                        
                        info!("Retry successful for page {}", failed_page.site_page_number);
                    }
                    Err(e) => {
                        failed_page.retry_count += 1;
                        failed_page.error = e;
                        failed_page.last_attempt = chrono::Utc::now();
                        result.failed_pages.push(failed_page);
                        
                        warn!("Retry failed for page {}", failed_page.site_page_number);
                    }
                }
            }
        }
        
        if stage_retry_count >= self.config.max_retries_per_stage && !result.failed_pages.is_empty() {
            warn!("Maximum stage retries exceeded. {} pages remain failed", result.failed_pages.len());
        }
        
        Ok(result)
    }
    
    async fn save_products_to_db(&self, products: &[Product]) -> Result<(), anyhow::Error> {
        if products.is_empty() {
            return Ok(());
        }
        
        let saved_count = self.database.insert_products(products.to_vec()).await?;
        debug!("Saved {} products to database", saved_count);
        
        Ok(())
    }
    
    // 진행 상황 업데이트 메서드들
    async fn send_page_started_update(&self, site_page_number: u32, url: &str) {
        let update = ProgressUpdate::PageStarted {
            site_page_number,
            page_url: url.to_string(),
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            error!("Failed to send page started update: {}", e);
        }
    }
    
    async fn send_page_completed_update(
        &self, 
        site_page_number: u32, 
        products_count: u32,
        completed: usize,
        total: usize,
    ) {
        let update = ProgressUpdate::PageCompleted {
            site_page_number,
            products_count,
            elapsed_time: 0, // 실제로는 측정된 시간 사용
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            error!("Failed to send page completed update: {}", e);
        }
        
        // 전체 진행률 업데이트
        let progress_update = ProgressUpdate::ProgressUpdate {
            stage_id: 3,
            current_item: completed as u32,
            total_items: total as u32,
            elapsed_time: 0,
            estimated_remaining: None,
        };
        
        if let Err(e) = self.progress_sender.send(progress_update) {
            error!("Failed to send progress update: {}", e);
        }
    }
    
    async fn send_page_failed_update(&self, site_page_number: u32, error: String, retry_count: u32) {
        let update = ProgressUpdate::PageFailed {
            site_page_number,
            error,
            retry_count,
            will_retry: retry_count < self.config.max_retries_per_page,
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            error!("Failed to send page failed update: {}", e);
        }
    }
    
    async fn send_retry_started_update(&self, site_page_number: u32, attempt: u32) {
        let update = ProgressUpdate::RetryStarted {
            stage_id: 3,
            attempt,
            max_attempts: self.config.max_retries_per_stage,
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            error!("Failed to send retry started update: {}", e);
        }
    }
}

impl Clone for ProductListCollector {
    fn clone(&self) -> Self {
        Self {
            crawler: Arc::clone(&self.crawler),
            database: Arc::clone(&self.database),
            config: self.config.clone(),
            progress_sender: self.progress_sender.clone(),
        }
    }
}
```

#### 3.2.2 결과 데이터 구조

```rust
// src-tauri/src/domain/entities/collection_result.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionResult {
    pub successful_pages: Vec<PageCollectionResult>,
    pub failed_pages: Vec<FailedPageInfo>,
    pub total_products_collected: u32,
    pub completion_rate: f64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl CollectionResult {
    pub fn new() -> Self {
        Self {
            successful_pages: Vec::new(),
            failed_pages: Vec::new(),
            total_products_collected: 0,
            completion_rate: 0.0,
            started_at: Utc::now(),
            completed_at: None,
        }
    }
    
    pub fn mark_completed(&mut self) {
        self.completed_at = Some(Utc::now());
    }
    
    pub fn get_success_rate(&self) -> f64 {
        let total_pages = self.successful_pages.len() + self.failed_pages.len();
        if total_pages == 0 {
            100.0
        } else {
            (self.successful_pages.len() as f64 / total_pages as f64) * 100.0
        }
    }
    
    pub fn get_elapsed_time(&self) -> Option<chrono::Duration> {
        if let Some(completed_at) = self.completed_at {
            Some(completed_at.signed_duration_since(self.started_at))
        } else {
            Some(Utc::now().signed_duration_since(self.started_at))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCollectionResult {
    pub site_page_number: u32,
    pub products: Vec<Product>,
    pub is_complete: bool,
    pub attempt_count: u32,
    pub elapsed_time: u64, // milliseconds
}

impl PageCollectionResult {
    pub fn get_products_per_second(&self) -> f64 {
        if self.elapsed_time == 0 {
            0.0
        } else {
            (self.products.len() as f64) / (self.elapsed_time as f64 / 1000.0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedPageInfo {
    pub site_page_number: u32,
    pub error: CrawlingError,
    pub retry_count: u32,
    pub last_attempt: DateTime<Utc>,
}

impl FailedPageInfo {
    pub fn should_retry(&self, max_retries: u32) -> bool {
        self.retry_count < max_retries && self.error.is_retryable()
    }
    
    pub fn time_since_last_attempt(&self) -> chrono::Duration {
        Utc::now().signed_duration_since(self.last_attempt)
    }
}
```

### 3.3 배치 수준 재시도 관리

#### 3.3.1 BatchRetryManager 구현

```rust
// src-tauri/src/application/services/batch_retry_manager.rs
use crate::domain::entities::*;
use crate::domain::events::ProgressUpdate;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};
use std::collections::HashMap;
use tracing::{info, warn, error};

pub struct BatchRetryManager {
    failed_batches: Vec<FailedBatch>,
    max_batch_retries: u32,
    progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
    retry_delay_base: Duration,
}

impl BatchRetryManager {
    pub fn new(
        max_batch_retries: u32,
        progress_sender: mpsc::UnboundedSender<ProgressUpdate>,
    ) -> Self {
        Self {
            failed_batches: Vec::new(),
            max_batch_retries,
            progress_sender,
            retry_delay_base: Duration::from_secs(30),
        }
    }
    
    pub fn add_failed_batch(&mut self, batch: FailedBatch) {
        self.failed_batches.push(batch);
    }
    
    pub async fn retry_failed_batches(&mut self) -> Result<Vec<BatchResult>, CrawlingError> {
        if self.failed_batches.is_empty() {
            return Ok(Vec::new());
        }
        
        info!("Starting batch retry process for {} batches", self.failed_batches.len());
        
        let mut results = Vec::new();
        let mut retry_count = 0;
        
        while !self.failed_batches.is_empty() && retry_count < self.max_batch_retries {
            retry_count += 1;
            
            self.send_batch_retry_started(retry_count).await;
            
            let current_failures = std::mem::take(&mut self.failed_batches);
            info!("Batch retry attempt {} for {} batches", retry_count, current_failures.len());
            
            // 배치 재시도 간격 (지수 백오프)
            if retry_count > 1 {
                let delay = self.retry_delay_base * 2_u32.pow(retry_count - 1);
                let capped_delay = delay.min(Duration::from_secs(300)); // 최대 5분
                
                info!("Waiting {:?} before batch retry", capped_delay);
                sleep(capped_delay).await;
            }
            
            for failed_batch in current_failures {
                match self.retry_single_batch(failed_batch.clone()).await {
                    Ok(result) => {
                        results.push(result);
                        info!("Batch retry successful for batch {}", failed_batch.batch_id);
                    }
                    Err(e) => {
                        let mut updated_batch = failed_batch;
                        updated_batch.retry_count += 1;
                        updated_batch.last_error = e.to_string();
                        updated_batch.last_attempt = chrono::Utc::now();
                        
                        // 재시도 가능한 에러인지 확인
                        if e.is_retryable() && updated_batch.retry_count < self.max_batch_retries {
                            self.failed_batches.push(updated_batch);
                            warn!("Batch retry failed, will try again: {}", e);
                        } else {
                            warn!("Batch permanently failed: {}", e);
                            results.push(BatchResult::Failed {
                                batch_id: updated_batch.batch_id,
                                error: e.to_string(),
                                attempted_retries: updated_batch.retry_count,
                            });
                        }
                    }
                }
            }
        }
        
        if !self.failed_batches.is_empty() {
            warn!("Maximum batch retries exceeded. {} batches permanently failed", 
                  self.failed_batches.len());
            
            // 영구적으로 실패한 배치들을 결과에 추가
            for failed_batch in &self.failed_batches {
                results.push(BatchResult::Failed {
                    batch_id: failed_batch.batch_id.clone(),
                    error: failed_batch.last_error.clone(),
                    attempted_retries: failed_batch.retry_count,
                });
            }
        }
        
        info!("Batch retry process completed. {} results", results.len());
        Ok(results)
    }
    
    async fn retry_single_batch(&self, failed_batch: FailedBatch) -> Result<BatchResult, CrawlingError> {
        info!("Retrying batch: {}", failed_batch.batch_id);
        
        let start_time = Instant::now();
        
        match failed_batch.batch_type {
            BatchType::ProductListCollection { pages } => {
                // ProductListCollector를 재실행
                // 이 부분은 실제 구현에서 적절한 서비스를 호출해야 함
                self.retry_product_list_batch(pages).await
            }
            BatchType::ProductDetailCollection { urls } => {
                // ProductDetailCollector를 재실행
                self.retry_product_detail_batch(urls).await
            }
            BatchType::DatabaseOperation { operation } => {
                // 데이터베이스 작업 재시도
                self.retry_database_batch(operation).await
            }
        }
    }
    
    async fn retry_product_list_batch(&self, pages: Vec<u32>) -> Result<BatchResult, CrawlingError> {
        // 실제 구현에서는 ProductListCollector 인스턴스를 사용해야 함
        // 현재는 예시 구현
        
        info!("Retrying product list collection for {} pages", pages.len());
        
        // 성공적으로 처리된 페이지와 실패한 페이지 추적
        let mut successful_pages = Vec::new();
        let mut failed_pages = Vec::new();
        
        for page in pages {
            // 실제 크롤링 로직 호출
            // let result = self.product_list_collector.collect_single_page(page).await;
            
            // 예시: 임시 성공 처리
            successful_pages.push(page);
        }
        
        Ok(BatchResult::Success {
            batch_id: format!("product_list_retry_{}", chrono::Utc::now().timestamp()),
            processed_items: successful_pages.len() as u32,
            elapsed_time: 0, // 실제 측정된 시간 사용
        })
    }
    
    async fn retry_product_detail_batch(&self, urls: Vec<String>) -> Result<BatchResult, CrawlingError> {
        info!("Retrying product detail collection for {} products", urls.len());
        
        // 실제 구현은 ProductDetailCollector를 사용
        Ok(BatchResult::Success {
            batch_id: format!("product_detail_retry_{}", chrono::Utc::now().timestamp()),
            processed_items: urls.len() as u32,
            elapsed_time: 0,
        })
    }
    
    async fn retry_database_batch(&self, operation: DatabaseBatchOperation) -> Result<BatchResult, CrawlingError> {
        info!("Retrying database operation: {:?}", operation);
        
        // 실제 구현은 데이터베이스 작업을 수행
        Ok(BatchResult::Success {
            batch_id: format!("database_retry_{}", chrono::Utc::now().timestamp()),
            processed_items: 1,
            elapsed_time: 0,
        })
    }
    
    async fn send_batch_retry_started(&self, attempt: u32) {
        let update = ProgressUpdate::RetryStarted {
            stage_id: 0, // 배치 레벨 재시도
            attempt,
            max_attempts: self.max_batch_retries,
        };
        
        if let Err(e) = self.progress_sender.send(update) {
            error!("Failed to send batch retry started update: {}", e);
        }
    }
}

#[derive(Debug, Clone)]
pub struct FailedBatch {
    pub batch_id: String,
    pub batch_type: BatchType,
    pub retry_count: u32,
    pub last_error: String,
    pub last_attempt: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum BatchType {
    ProductListCollection { pages: Vec<u32> },
    ProductDetailCollection { urls: Vec<String> },
    DatabaseOperation { operation: DatabaseBatchOperation },
}

#[derive(Debug, Clone)]
pub enum DatabaseBatchOperation {
    BulkInsert { table: String, records: u32 },
    BulkUpdate { table: String, records: u32 },
    Migration { version: String },
    Cleanup { operation: String },
}

#[derive(Debug, Clone)]
pub enum BatchResult {
    Success {
        batch_id: String,
        processed_items: u32,
        elapsed_time: u64,
    },
    PartialSuccess {
        batch_id: String,
        successful_items: u32,
        failed_items: u32,
        elapsed_time: u64,
    },
    Failed {
        batch_id: String,
        error: String,
        attempted_retries: u32,
    },
}
```

이것으로 Chapter 3이 완료되었습니다. 이 장에서는 Stage 2의 데이터베이스 분석과 Stage 3의 제품 목록 수집, 그리고 배치 수준의 재시도 관리를 다뤘습니다.

다음 Chapter 4를 생성할까요? Chapter 4에서는 Stage 4 (제품 상세 정보 수집), 누락 데이터 복구, 그리고 UI 연동을 다룰 예정입니다.
