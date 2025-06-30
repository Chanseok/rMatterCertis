# Matter Certis v2 - Batch Crawling Implementation Chapter 4: Product Detail Collection and Missing Data Recovery

## Table of Contents
1. [Stage 4: Product Detail Collection](#stage-4-product-detail-collection)
2. [Missing Data Recovery](#missing-data-recovery)
3. [Real-time UI Progress Reporting](#real-time-ui-progress-reporting)
4. [Error Handling and Recovery](#error-handling-and-recovery)
5. [Implementation Patterns](#implementation-patterns)

---

## Stage 4: Product Detail Collection

### 4.1 Core Architecture

The product detail collection stage (Stage 4) processes individual products collected from Stage 3 to gather comprehensive product information.

```rust
// Core structs for Stage 4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetailCollector {
    state: Arc<CrawlerState>,
    abort_controller: Arc<AtomicBool>,
    config: CrawlerConfig,
    browser_manager: Arc<BrowserManager>,
    progress_callback: Option<Arc<dyn ProductDetailProgressCallback + Send + Sync>>,
    current_batch: Option<u32>,
    total_batches: Option<u32>,
    retry_count: u32,
}

#[derive(Debug, Clone)]
pub struct DetailCrawlResult {
    pub url: String,
    pub product: Option<MatterProduct>,
    pub is_new_item: bool,
    pub success: bool,
    pub error: Option<String>,
    pub processing_time_ms: u64,
}

// Progress callback trait
#[async_trait]
pub trait ProductDetailProgressCallback {
    async fn update_progress(
        &self,
        processed_items: u32,
        total_items: u32,
        start_time: SystemTime,
        is_completed: bool,
        new_items: u32,
        updated_items: u32,
        current_batch: Option<u32>,
        total_batches: Option<u32>,
        retry_count: Option<u32>,
    ) -> Result<(), CrawlerError>;
}
```

### 4.2 Collection Process Implementation

```rust
impl ProductDetailCollector {
    pub async fn collect(&mut self, products: Vec<Product>) -> Result<Vec<MatterProduct>, CrawlerError> {
        if products.is_empty() {
            return Ok(Vec::new());
        }

        // Stage 4 initialization
        self.state.set_detail_stage_product_count(products.len() as u32).await?;
        self.state.initialize_detail_stage().await?;
        
        // Set initial progress state
        self.state.update_progress(ProgressUpdate {
            current: 0,
            total: products.len() as u32,
            total_items: products.len() as u32,
            processed_items: 0,
            percentage: 0.0,
            new_items: 0,
            updated_items: 0,
            current_stage: CrawlingStage::ProductDetail,
            current_step: "Stage 4: Product detail collection".to_string(),
            status: CrawlingStatus::Running,
            message: format!("Stage 4: Starting product detail collection (0/{})", products.len()),
            current_batch: self.current_batch,
            total_batches: self.total_batches,
        }).await?;

        // Initialize tracking
        let mut matter_products = Vec::new();
        let mut failed_products = Vec::new();
        let mut failed_product_errors: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize product task states
        self.initialize_product_task_states(&products).await?;

        // Execute parallel collection
        self.execute_parallel_product_detail_crawling(
            &products,
            &mut matter_products,
            &mut failed_products,
            &mut failed_product_errors,
        ).await?;

        // Handle failures with retry logic
        if !failed_products.is_empty() {
            self.retry_failed_product_details(
                &mut failed_products,
                &products,
                &mut matter_products,
                &mut failed_product_errors,
            ).await?;
        }

        // Finalize and validate results
        self.finalize_collection_results(&products, &matter_products, &failed_products).await?;

        Ok(matter_products)
    }

    async fn execute_parallel_product_detail_crawling(
        &mut self,
        products: &[Product],
        matter_products: &mut Vec<MatterProduct>,
        failed_products: &mut Vec<String>,
        failed_product_errors: &mut HashMap<String, Vec<String>>,
    ) -> Result<(), CrawlerError> {
        let config = &self.config;
        let total_items = products.len() as u32;
        let start_time = SystemTime::now();
        let mut last_progress_update = Instant::now();
        let progress_update_interval = Duration::from_millis(3000);

        // Parallel processing with semaphore
        let concurrency = config.detail_concurrency.unwrap_or(1);
        let semaphore = Arc::new(Semaphore::new(concurrency as usize));
        
        let mut tasks = Vec::new();
        
        for product in products {
            if self.abort_controller.load(Ordering::Relaxed) {
                break;
            }

            let semaphore_clone = semaphore.clone();
            let product_clone = product.clone();
            let state_clone = self.state.clone();
            let abort_controller_clone = self.abort_controller.clone();
            
            let task = tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.unwrap();
                
                Self::process_product_detail_crawl(
                    product_clone,
                    state_clone,
                    abort_controller_clone,
                    1, // initial attempt
                ).await
            });
            
            tasks.push(task);
        }

        // Process results as they complete
        let mut completed_count = 0u32;
        let results = try_join_all(tasks).await?;
        
        for result in results {
            if let Ok(Some(detail_result)) = result {
                if detail_result.success {
                    if let Some(product) = detail_result.product {
                        // Check if this is a new item or update
                        let is_new = !matter_products.iter().any(|p| p.url == product.url);
                        
                        if is_new {
                            matter_products.push(product);
                        } else {
                            // Update existing product
                            if let Some(existing) = matter_products.iter_mut().find(|p| p.url == product.url) {
                                *existing = product;
                            }
                        }

                        // Record the processing in state
                        self.state.record_detail_item_processed(is_new, Some(detail_result.url.clone())).await?;
                    }
                } else {
                    failed_products.push(detail_result.url.clone());
                    if let Some(error) = detail_result.error {
                        failed_product_errors
                            .entry(detail_result.url)
                            .or_insert_with(Vec::new)
                            .push(error);
                    }
                }
            }

            completed_count += 1;

            // Periodic progress updates
            if last_progress_update.elapsed() >= progress_update_interval {
                self.update_collection_progress(completed_count, total_items, start_time).await?;
                last_progress_update = Instant::now();
            }
        }

        // Final progress update
        self.update_collection_progress(completed_count, total_items, start_time).await?;

        Ok(())
    }
}
```

### 4.3 Individual Product Processing

```rust
impl ProductDetailCollector {
    async fn process_product_detail_crawl(
        product: Product,
        state: Arc<CrawlerState>,
        abort_controller: Arc<AtomicBool>,
        attempt: u32,
    ) -> Result<Option<DetailCrawlResult>, CrawlerError> {
        let start_time = Instant::now();
        
        // Check for abort signal
        if abort_controller.load(Ordering::Relaxed) {
            return Ok(None);
        }

        // Update task status
        state.update_product_task_status(&product.url, TaskStatus::Running).await?;

        let config = &state.get_config().await?;
        let mut detail_product: Option<MatterProduct> = None;

        // Implement exponential backoff retry logic
        let base_retry_delay = config.base_retry_delay_ms.unwrap_or(1000);
        let max_retry_delay = config.max_retry_delay_ms.unwrap_or(15000);
        let retry_max = config.retry_max.unwrap_or(2);

        let result = Self::retry_with_exponential_backoff(
            retry_max,
            base_retry_delay,
            max_retry_delay,
            || async {
                Self::crawl_single_product_detail(&product, config).await
            },
            |retry_attempt, err| {
                // Log retry attempt
                log::warn!(
                    "Retrying ({}/{}) after {}ms for {}: {}",
                    retry_attempt,
                    retry_max,
                    Self::calculate_backoff_delay(retry_attempt, base_retry_delay, max_retry_delay),
                    product.url,
                    err
                );
                
                // Emit retry event
                state.emit_crawling_task_status(CrawlingTaskStatus {
                    task_id: format!("product-retry-{}", product.url),
                    status: TaskStatus::Attempting,
                    message: serde_json::to_string(&json!({
                        "stage": 4,
                        "type": "retry",
                        "url": product.url,
                        "attempt": retry_attempt,
                        "maxAttempts": retry_max,
                        "delay": Self::calculate_backoff_delay(retry_attempt, base_retry_delay, max_retry_delay),
                        "error": err.to_string(),
                        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
                    })).unwrap_or_default(),
                });

                // Continue retrying unless aborted
                !abort_controller.load(Ordering::Relaxed)
            },
        ).await;

        match result {
            Ok(product_detail) => {
                detail_product = Some(product_detail);
                state.update_product_task_status(&product.url, TaskStatus::Success).await?;
                
                let processing_time = start_time.elapsed().as_millis() as u64;
                let is_new_item = true; // Will be determined later in the calling function
                
                // Emit success event
                state.emit_crawling_task_status(CrawlingTaskStatus {
                    task_id: format!("product-{}", product.url),
                    status: TaskStatus::Success,
                    message: serde_json::to_string(&json!({
                        "stage": 4,
                        "type": "product",
                        "url": product.url,
                        "manufacturer": product.manufacturer.unwrap_or_else(|| "Unknown".to_string()),
                        "model": product.model.unwrap_or_else(|| "Unknown".to_string()),
                        "isNewItem": is_new_item,
                        "endTime": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
                    })).unwrap_or_default(),
                });

                Ok(Some(DetailCrawlResult {
                    url: product.url.clone(),
                    product: detail_product,
                    is_new_item,
                    success: true,
                    error: None,
                    processing_time_ms: processing_time,
                }))
            }
            Err(error) => {
                let error_msg = error.to_string();
                state.update_product_task_status(&product.url, TaskStatus::Error).await?;
                
                // Emit error event
                state.emit_crawling_task_status(CrawlingTaskStatus {
                    task_id: format!("product-{}", product.url),
                    status: TaskStatus::Error,
                    message: serde_json::to_string(&json!({
                        "stage": 4,
                        "type": "product",
                        "url": product.url,
                        "manufacturer": product.manufacturer.unwrap_or_else(|| "Unknown".to_string()),
                        "model": product.model.unwrap_or_else(|| "Unknown".to_string()),
                        "error": error_msg,
                        "attempt": attempt,
                        "endTime": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
                    })).unwrap_or_default(),
                });

                Ok(Some(DetailCrawlResult {
                    url: product.url.clone(),
                    product: None,
                    is_new_item: false,
                    success: false,
                    error: Some(error_msg),
                    processing_time_ms: start_time.elapsed().as_millis() as u64,
                }))
            }
        }
    }

    async fn crawl_single_product_detail(
        product: &Product,
        config: &CrawlerConfig,
    ) -> Result<MatterProduct, CrawlerError> {
        // Create HTTP client with enhanced headers
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.page_timeout_ms.unwrap_or(30000)))
            .user_agent(config.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT))
            .build()?;

        // Fetch the product page
        let response = client
            .get(&product.url)
            .headers(Self::get_enhanced_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(CrawlerError::HttpError(format!(
                "HTTP {} for {}",
                response.status(),
                product.url
            )));
        }

        let html = response.text().await?;
        
        // Parse the HTML content
        let document = Html::parse_document(&html);
        
        // Use the MatterProductParser to extract product details
        let mut matter_product = MatterProductParser::parse_from_html(&document, &product.url)?;
        
        // Enrich with data from the product list
        matter_product.manufacturer = product.manufacturer.clone();
        matter_product.model = product.model.clone();
        matter_product.certification_type = product.certification_type.clone();
        
        Ok(matter_product)
    }
}
```

---

## Missing Data Recovery

### 5.1 Missing Data Analysis

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingDataAnalysis {
    pub missing_details: Vec<MissingProductDetail>,
    pub incomplete_pages: Vec<IncompletePage>,
    pub total_missing_details: u32,
    pub total_incomplete_pages: u32,
    pub summary: MissingDataSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingProductDetail {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub last_attempt: Option<SystemTime>,
    pub error_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncompletePage {
    pub page_id: u32,
    pub missing_indices: Vec<u32>,
    pub expected_count: u32,
    pub actual_count: u32,
    pub last_crawl_attempt: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingDataSummary {
    pub products_count: u32,
    pub product_details_count: u32,
    pub difference: i32,
}

impl MissingDataAnalyzer {
    pub async fn analyze_missing_data(
        &self,
        config: &CrawlerConfig,
    ) -> Result<MissingDataAnalysis, CrawlerError> {
        // Get current database state
        let db_products = self.database_repository.get_all_products().await?;
        let db_product_details = self.database_repository.get_all_product_details().await?;
        
        // Compare with site data to identify missing items
        let site_products = self.get_site_product_list(config).await?;
        
        let mut missing_details = Vec::new();
        let mut incomplete_pages = Vec::new();
        
        // Identify missing product details
        for site_product in &site_products {
            if !db_product_details.iter().any(|db_prod| db_prod.url == site_product.url) {
                missing_details.push(MissingProductDetail {
                    url: site_product.url.clone(),
                    manufacturer: site_product.manufacturer.clone(),
                    model: site_product.model.clone(),
                    last_attempt: None,
                    error_count: 0,
                    last_error: None,
                });
            }
        }
        
        // Identify incomplete pages
        let pages_per_site = self.calculate_total_pages(config).await?;
        for page_id in 1..=pages_per_site {
            let page_products = self.get_products_for_page(page_id, config).await?;
            let expected_count = config.products_per_page.unwrap_or(12);
            
            if page_products.len() < expected_count as usize {
                let missing_indices: Vec<u32> = (page_products.len() as u32..expected_count)
                    .collect();
                
                incomplete_pages.push(IncompletePage {
                    page_id,
                    missing_indices,
                    expected_count,
                    actual_count: page_products.len() as u32,
                    last_crawl_attempt: None,
                });
            }
        }
        
        Ok(MissingDataAnalysis {
            total_missing_details: missing_details.len() as u32,
            total_incomplete_pages: incomplete_pages.len() as u32,
            summary: MissingDataSummary {
                products_count: site_products.len() as u32,
                product_details_count: db_product_details.len() as u32,
                difference: site_products.len() as i32 - db_product_details.len() as i32,
            },
            missing_details,
            incomplete_pages,
        })
    }
}
```

### 5.2 Missing Data Recovery Implementation

```rust
#[derive(Debug)]
pub struct MissingDataRecovery {
    crawler_engine: Arc<CrawlerEngine>,
    database_repository: Arc<DatabaseRepository>,
    state: Arc<CrawlerState>,
}

impl MissingDataRecovery {
    pub async fn recover_missing_products(
        &mut self,
        analysis_result: MissingDataAnalysis,
        config: CrawlerConfig,
    ) -> Result<RecoveryResult, CrawlerError> {
        let mut recovery_result = RecoveryResult::default();
        
        // Phase 1: Recover incomplete pages (Stage 1-3 workflow)
        if !analysis_result.incomplete_pages.is_empty() {
            log::info!("Starting incomplete pages recovery: {} pages", analysis_result.incomplete_pages.len());
            
            let page_ranges = self.convert_incomplete_pages_to_ranges(&analysis_result.incomplete_pages);
            let recovery_products = self.crawler_engine
                .crawl_missing_product_pages(page_ranges, config.clone())
                .await?;
                
            recovery_result.recovered_from_pages = recovery_products.len() as u32;
        }
        
        // Phase 2: Recover specific missing product details
        if !analysis_result.missing_details.is_empty() {
            log::info!("Starting missing product details recovery: {} products", analysis_result.missing_details.len());
            
            let missing_products: Vec<Product> = analysis_result.missing_details
                .into_iter()
                .map(|detail| Product {
                    url: detail.url,
                    manufacturer: detail.manufacturer,
                    model: detail.model,
                    certification_type: None,
                    matter_support: None,
                })
                .collect();
            
            let detail_collector = ProductDetailCollector::new(
                self.state.clone(),
                Arc::new(AtomicBool::new(false)),
                config.clone(),
                self.crawler_engine.get_browser_manager(),
                None,
                None,
            );
            
            let recovered_details = detail_collector.collect(missing_products).await?;
            recovery_result.recovered_details = recovered_details.len() as u32;
            
            // Save recovered details to database
            if config.auto_add_to_local_db && !recovered_details.is_empty() {
                let save_result = self.database_repository
                    .save_products_to_db(&recovered_details)
                    .await?;
                    
                recovery_result.saved_to_db = save_result.added + save_result.updated;
            }
        }
        
        Ok(recovery_result)
    }

    fn convert_incomplete_pages_to_ranges(&self, incomplete_pages: &[IncompletePage]) -> Vec<CrawlingRange> {
        let mut ranges = Vec::new();
        let mut current_range: Option<CrawlingRange> = None;
        
        for page in incomplete_pages {
            match current_range.as_mut() {
                Some(range) if range.end_page + 1 == page.page_id => {
                    // Extend current range
                    range.end_page = page.page_id;
                }
                _ => {
                    // Start new range
                    if let Some(range) = current_range {
                        ranges.push(range);
                    }
                    current_range = Some(CrawlingRange {
                        start_page: page.page_id,
                        end_page: page.page_id,
                        priority: 1,
                        estimated_products: page.expected_count - page.actual_count,
                    });
                }
            }
        }
        
        if let Some(range) = current_range {
            ranges.push(range);
        }
        
        ranges
    }
}

#[derive(Debug, Default)]
pub struct RecoveryResult {
    pub recovered_from_pages: u32,
    pub recovered_details: u32,
    pub saved_to_db: u32,
    pub errors: Vec<String>,
}
```

---

## Real-time UI Progress Reporting

### 6.1 Progress Event System

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub current: u32,
    pub total: u32,
    pub total_items: u32,
    pub processed_items: u32,
    pub percentage: f64,
    pub current_stage: CrawlingStage,
    pub current_step: String,
    pub status: CrawlingStatus,
    pub message: String,
    pub remaining_time: Option<Duration>,
    pub elapsed_time: Duration,
    pub start_time: SystemTime,
    pub estimated_end_time: Option<SystemTime>,
    pub new_items: u32,
    pub updated_items: u32,
    pub current_batch: Option<u32>,
    pub total_batches: Option<u32>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub errors: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingTaskStatus {
    pub task_id: String,
    pub status: TaskStatus,
    pub message: String,
    pub timestamp: SystemTime,
    pub stage: CrawlingStage,
    pub details: Option<serde_json::Value>,
}

#[async_trait]
pub trait ProgressReporter: Send + Sync {
    async fn emit_progress(&self, update: ProgressUpdate) -> Result<(), CrawlerError>;
    async fn emit_task_status(&self, status: CrawlingTaskStatus) -> Result<(), CrawlerError>;
    async fn emit_stage_changed(&self, stage: CrawlingStage, message: String) -> Result<(), CrawlerError>;
}
```

### 6.2 Progress Manager Implementation

```rust
pub struct ProgressManager {
    event_sender: tokio::sync::broadcast::Sender<CrawlingEvent>,
    time_estimation_service: Arc<TimeEstimationService>,
    state: Arc<CrawlerState>,
}

impl ProgressManager {
    pub fn new(
        event_sender: tokio::sync::broadcast::Sender<CrawlingEvent>,
        time_estimation_service: Arc<TimeEstimationService>,
        state: Arc<CrawlerState>,
    ) -> Self {
        Self {
            event_sender,
            time_estimation_service,
            state,
        }
    }

    pub async fn update_product_detail_progress(
        &self,
        processed_items: u32,
        total_items: u32,
        start_time: SystemTime,
        new_items: u32,
        updated_items: u32,
        current_batch: Option<u32>,
        total_batches: Option<u32>,
        retry_count: u32,
    ) -> Result<(), CrawlerError> {
        let now = SystemTime::now();
        let elapsed_time = now.duration_since(start_time)
            .unwrap_or_else(|_| Duration::from_secs(0));
        
        let percentage = if total_items > 0 {
            (processed_items as f64 / total_items as f64) * 100.0
        } else {
            0.0
        };

        // Get time estimation
        let stage_id = if let (Some(batch), Some(total)) = (current_batch, total_batches) {
            format!("stage4_batch_{}_{}", batch, total)
        } else {
            "stage4_product_detail".to_string()
        };

        let estimation = self.time_estimation_service
            .update_estimation(
                &stage_id,
                percentage,
                elapsed_time,
                retry_count,
                total_items,
                processed_items,
                None, // No global context for detail stage
            )
            .await?;

        let remaining_time = if estimation.remaining_time.seconds > 0 {
            Some(Duration::from_secs(estimation.remaining_time.seconds))
        } else {
            None
        };

        let estimated_end_time = remaining_time.map(|remaining| now + remaining);

        // Create batch message if applicable
        let message = if let (Some(batch), Some(total)) = (current_batch, total_batches) {
            if percentage >= 100.0 {
                format!(
                    "Batch {}/{} - Product detail collection completed: {}/{} products",
                    batch, total, processed_items, total_items
                )
            } else {
                format!(
                    "Batch {}/{} - Product detail collection: {}/{} products ({:.1}%)",
                    batch, total, processed_items, total_items, percentage
                )
            }
        } else {
            format!(
                "Stage 4: Product detail collection {}/{} ({:.1}%)",
                processed_items, total_items, percentage
            )
        };

        let progress_update = ProgressUpdate {
            current: processed_items,
            total: total_items,
            total_items,
            processed_items,
            percentage,
            current_stage: CrawlingStage::ProductDetail,
            current_step: "Stage 4: Product detail collection".to_string(),
            status: if percentage >= 100.0 {
                CrawlingStatus::CompletedStage4
            } else {
                CrawlingStatus::Running
            },
            message,
            remaining_time,
            elapsed_time,
            start_time,
            estimated_end_time,
            new_items,
            updated_items,
            current_batch,
            total_batches,
            retry_count,
            max_retries: 3, // Default max retries
            errors: None,
        };

        self.emit_progress(progress_update).await?;

        // Emit parallel task update
        self.state.update_parallel_tasks(
            std::cmp::min(total_items - processed_items, 5), // Active tasks
            5, // Max concurrency
        ).await?;

        Ok(())
    }
}

#[async_trait]
impl ProgressReporter for ProgressManager {
    async fn emit_progress(&self, update: ProgressUpdate) -> Result<(), CrawlerError> {
        let event = CrawlingEvent::Progress(update);
        if let Err(e) = self.event_sender.send(event) {
            log::warn!("Failed to send progress event: {}", e);
        }
        Ok(())
    }

    async fn emit_task_status(&self, status: CrawlingTaskStatus) -> Result<(), CrawlerError> {
        let event = CrawlingEvent::TaskStatus(status);
        if let Err(e) = self.event_sender.send(event) {
            log::warn!("Failed to send task status event: {}", e);
        }
        Ok(())
    }

    async fn emit_stage_changed(&self, stage: CrawlingStage, message: String) -> Result<(), CrawlerError> {
        let event = CrawlingEvent::StageChanged { stage, message };
        if let Err(e) = self.event_sender.send(event) {
            log::warn!("Failed to send stage changed event: {}", e);
        }
        Ok(())
    }
}
```

### 6.3 UI Integration (Tauri Commands)

```rust
#[tauri::command]
pub async fn get_crawling_progress(state: tauri::State<'_, AppState>) -> Result<ProgressUpdate, String> {
    let crawler_state = state.crawler_state.lock().await;
    crawler_state.get_progress_data()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn subscribe_to_crawling_events(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut event_receiver = state.event_receiver.lock().await;
    let app_handle_clone = app_handle.clone();
    
    tokio::spawn(async move {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                CrawlingEvent::Progress(progress) => {
                    let _ = app_handle_clone.emit_all("crawling-progress", &progress);
                }
                CrawlingEvent::TaskStatus(status) => {
                    let _ = app_handle_clone.emit_all("crawling-task-status", &status);
                }
                CrawlingEvent::StageChanged { stage, message } => {
                    let _ = app_handle_clone.emit_all("crawling-stage-changed", &json!({
                        "stage": stage,
                        "message": message
                    }));
                }
                CrawlingEvent::Error(error) => {
                    let _ = app_handle_clone.emit_all("crawling-error", &error);
                }
            }
        }
    });
    
    Ok(())
}
```

---

## Error Handling and Recovery

### 7.1 Error Types and Classification

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProductDetailError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),
    
    #[error("HTML parsing failed: {0}")]
    ParseError(String),
    
    #[error("Required field missing: {0}")]
    MissingField(String),
    
    #[error("Timeout occurred: {0}ms")]
    Timeout(u64),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Browser error: {0}")]
    BrowserError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub url: String,
    pub attempt: u32,
    pub error_type: String,
    pub error_message: String,
    pub timestamp: SystemTime,
    pub stack_trace: Option<String>,
}
```

### 7.2 Retry Strategy Implementation

```rust
impl ProductDetailCollector {
    async fn retry_with_exponential_backoff<F, Fut, T>(
        max_retries: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
        operation: F,
        should_retry: impl Fn(u32, &CrawlerError) -> bool,
    ) -> Result<T, CrawlerError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, CrawlerError>>,
    {
        let mut last_error = None;
        
        for attempt in 1..=max_retries + 1 {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempt > max_retries || !should_retry(attempt, &error) {
                        return Err(error);
                    }
                    
                    let delay = Self::calculate_backoff_delay(attempt, base_delay_ms, max_delay_ms);
                    log::warn!("Attempt {} failed, retrying in {}ms: {}", attempt, delay, error);
                    
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    last_error = Some(error);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            CrawlerError::Generic("Maximum retries exceeded".to_string())
        }))
    }

    fn calculate_backoff_delay(attempt: u32, base_delay: u64, max_delay: u64) -> u64 {
        let exponential_delay = base_delay * 2_u64.pow(attempt.saturating_sub(1));
        std::cmp::min(exponential_delay, max_delay)
    }

    async fn retry_failed_product_details(
        &mut self,
        failed_products: &mut Vec<String>,
        all_products: &[Product],
        matter_products: &mut Vec<MatterProduct>,
        failed_product_errors: &mut HashMap<String, Vec<String>>,
    ) -> Result<(), CrawlerError> {
        let config = &self.config;
        let retry_count = config.product_detail_retry_count.unwrap_or(2);
        
        if retry_count == 0 || failed_products.is_empty() {
            return Ok();
        }

        log::info!(
            "Starting retry for {} failed product details with {} retry cycles",
            failed_products.len(),
            retry_count
        );

        let mut retry_success_count = 0;
        
        for attempt in 1..=retry_count {
            if self.abort_controller.load(Ordering::Relaxed) {
                log::info!("Abort signal received during retry, stopping");
                break;
            }

            if failed_products.is_empty() {
                break;
            }

            self.update_retry_status(
                "detail-retry",
                attempt,
                failed_products.len() as u32,
                failed_products.clone(),
            ).await?;

            let retry_urls = failed_products.clone();
            failed_products.clear();

            for url in retry_urls {
                if let Some(product) = all_products.iter().find(|p| p.url == url) {
                    match Self::process_product_detail_crawl(
                        product.clone(),
                        self.state.clone(),
                        self.abort_controller.clone(),
                        attempt + 1,
                    ).await? {
                        Some(result) if result.success => {
                            if let Some(matter_product) = result.product {
                                // Check for duplicates and add/update
                                let is_new = !matter_products.iter().any(|p| p.url == matter_product.url);
                                
                                if is_new {
                                    matter_products.push(matter_product);
                                } else {
                                    if let Some(existing) = matter_products.iter_mut().find(|p| p.url == matter_product.url) {
                                        *existing = matter_product;
                                    }
                                }
                                
                                self.state.record_detail_item_processed(is_new, Some(url.clone())).await?;
                                retry_success_count += 1;
                            }
                        }
                        Some(result) => {
                            // Still failed, add back to failed list
                            failed_products.push(url.clone());
                            if let Some(error) = result.error {
                                failed_product_errors
                                    .entry(url)
                                    .or_insert_with(Vec::new)
                                    .push(format!("Attempt {}: {}", attempt + 1, error));
                            }
                        }
                        None => {
                            // Aborted
                            break;
                        }
                    }
                }
            }
        }

        if retry_success_count > 0 {
            log::info!(
                "Retry completed: {} additional products successfully collected",
                retry_success_count
            );
        }

        if !failed_products.is_empty() {
            log::warn!(
                "After {} retry attempts, {} products still failed",
                retry_count,
                failed_products.len()
            );
            
            // Log final failure details
            for url in failed_products {
                if let Some(errors) = failed_product_errors.get(url) {
                    log::error!("Final failure for {}: {}", url, errors.join("; "));
                }
            }
        }

        Ok(())
    }
}
```

---

## Implementation Patterns

### 8.1 State Management Pattern

```rust
// Centralized state management for Stage 4
impl CrawlerState {
    pub async fn initialize_detail_stage(&mut self) -> Result<(), CrawlerError> {
        self.detail_stage_processed_count = 0;
        self.detail_stage_new_count = 0;
        self.detail_stage_updated_count = 0;
        self.processed_product_urls.clear();
        self.current_stage = CrawlingStage::ProductDetail;
        Ok(())
    }

    pub async fn record_detail_item_processed(
        &mut self,
        is_new_item: bool,
        product_url: Option<String>,
    ) -> Result<(), CrawlerError> {
        // Overflow detection
        if self.detail_stage_processed_count >= self.detail_stage_total_product_count {
            log::warn!(
                "Detail stage overflow detected: processed={}, total={}",
                self.detail_stage_processed_count,
                self.detail_stage_total_product_count
            );
            return Ok(()); // Prevent overflow
        }

        self.detail_stage_processed_count += 1;
        
        if is_new_item {
            self.detail_stage_new_count += 1;
        } else {
            self.detail_stage_updated_count += 1;
        }

        // Track processed URLs to prevent duplicates
        if let Some(url) = product_url {
            self.processed_product_urls.insert(url);
        }

        // Update UI progress
        let total_items = self.detail_stage_total_product_count;
        let percentage = if total_items > 0 {
            (self.detail_stage_processed_count as f64 / total_items as f64) * 100.0
        } else {
            0.0
        };

        self.update_progress(ProgressUpdate {
            current: self.detail_stage_processed_count,
            total: total_items,
            total_items,
            processed_items: self.detail_stage_processed_count,
            percentage,
            current_stage: CrawlingStage::ProductDetail,
            current_step: "Stage 4: Product detail collection".to_string(),
            status: CrawlingStatus::Running,
            message: format!(
                "Stage 4: Product details {}/{} processing ({:.1}%)",
                self.detail_stage_processed_count,
                total_items,
                percentage
            ),
            new_items: self.detail_stage_new_count,
            updated_items: self.detail_stage_updated_count,
            ..Default::default()
        }).await?;

        Ok(())
    }
}
```

### 8.2 Configuration Management Pattern

```rust
// Dynamic configuration updates during crawling
impl ProductDetailCollector {
    pub async fn update_config(&mut self, updates: ConfigurationUpdates) -> Result<(), CrawlerError> {
        if let Some(concurrency) = updates.detail_concurrency {
            self.config.detail_concurrency = Some(concurrency);
            log::info!("Updated detail collection concurrency to {}", concurrency);
        }
        
        if let Some(timeout) = updates.page_timeout_ms {
            self.config.page_timeout_ms = Some(timeout);
            log::info!("Updated page timeout to {}ms", timeout);
        }
        
        if let Some(retry_count) = updates.retry_max {
            self.config.retry_max = Some(retry_count);
            log::info!("Updated max retry count to {}", retry_count);
        }
        
        // Emit configuration change event
        self.state.emit_configuration_changed(self.config.clone()).await?;
        
        Ok(())
    }
}
```

### 8.3 Resource Management Pattern

```rust
impl ProductDetailCollector {
    pub async fn cleanup_resources(&mut self) -> Result<(), CrawlerError> {
        log::info!("Cleaning up ProductDetailCollector resources");
        
        // Cancel any ongoing operations
        self.abort_controller.store(true, Ordering::Relaxed);
        
        // Wait for pending operations to complete
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Clean up browser resources if needed
        if let Some(browser_manager) = &self.browser_manager {
            browser_manager.cleanup().await?;
        }
        
        // Clear internal state
        self.processed_product_urls.clear();
        
        log::info!("ProductDetailCollector cleanup completed");
        Ok(())
    }
}

// RAII pattern for automatic cleanup
pub struct ProductDetailCollectorGuard {
    collector: ProductDetailCollector,
}

impl ProductDetailCollectorGuard {
    pub fn new(collector: ProductDetailCollector) -> Self {
        Self { collector }
    }
    
    pub async fn collect(&mut self, products: Vec<Product>) -> Result<Vec<MatterProduct>, CrawlerError> {
        self.collector.collect(products).await
    }
}

impl Drop for ProductDetailCollectorGuard {
    fn drop(&mut self) {
        // Ensure cleanup happens even if explicit cleanup is not called
        let collector = std::mem::take(&mut self.collector);
        tokio::spawn(async move {
            let _ = collector.cleanup_resources().await;
        });
    }
}
```

---

This Chapter 4 covers the complete implementation of Stage 4 (Product Detail Collection), missing data recovery mechanisms, and real-time UI progress reporting for the Rust+Tauri backend. The patterns shown here integrate with the previous chapters to provide a complete batch crawling solution with robust error handling and recovery capabilities.

The implementation emphasizes:
- **Parallel processing** with controlled concurrency
- **Comprehensive error handling** with exponential backoff
- **Real-time progress reporting** for UI integration
- **Missing data recovery** with intelligent analysis
- **Resource management** with proper cleanup
- **State synchronization** between backend and frontend

The next chapter would typically cover database integration, final result processing, and system optimization techniques.
