//! # Product Detail Fetcher Worker
//!
//! Fetches individual product detail pages with optimized request handling.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_qualification)]
#![allow(unused_must_use)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use reqwest::Client;
use tokio::time::sleep;
use tokio::sync::Semaphore;

use crate::crawling::{tasks::*, state::*};
use super::{Worker, WorkerError};

/// Worker that fetches product detail pages
pub struct ProductDetailFetcher {
    client: Client,
    rate_limiter: Arc<Semaphore>,
    #[allow(dead_code)]
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

impl ProductDetailFetcher {
    /// Creates a new product detail fetcher
    pub fn new(
        concurrent_requests: usize,
        request_timeout: Duration,
        max_retries: u32,
        retry_delay: Duration,
    ) -> Result<Self, WorkerError> {
        let client = Client::builder()
            .timeout(request_timeout)
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .cookie_store(true)
            .gzip(true)
            .build()
            .map_err(|e| WorkerError::InitializationError(e.to_string()))?;

        Ok(Self {
            client,
            rate_limiter: Arc::new(Semaphore::new(concurrent_requests)),
            request_timeout,
            max_retries,
            retry_delay,
        })
    }

    /// 개발 용이성을 위한 간단한 생성자
    pub fn new_simple() -> Self {
        Self {
            client: Client::new(),
            rate_limiter: Arc::new(Semaphore::new(10)),
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    async fn fetch_with_retry(&self, url: &str) -> Result<String, WorkerError> {
        let _permit = self.rate_limiter.acquire().await
            .map_err(|e| WorkerError::RateLimitError(e.to_string()))?;

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match self.fetch_page(url).await {
                Ok(content) => {
                    if attempt > 0 {
                        tracing::info!("Successfully fetched {} on attempt {}", url, attempt + 1);
                    }
                    return Ok(content);
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    if attempt < self.max_retries {
                        let delay = self.retry_delay * (2_u32.pow(attempt)); // Exponential backoff
                        tracing::warn!(
                            "Failed to fetch {} (attempt {}), retrying in {:?}: {}",
                            url, attempt + 1, delay, last_error.as_ref().unwrap()
                        );
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| 
            WorkerError::NetworkError("Unknown error after retries".to_string())
        ))
    }

    async fn fetch_page(&self, url: &str) -> Result<String, WorkerError> {
        let response = self.client
            .get(url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ko-KR,ko;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate")
            .header("Connection", "keep-alive")
            .header("Upgrade-Insecure-Requests", "1")
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    WorkerError::TimeoutError(format!("Request timeout for {}", url))
                } else if e.is_connect() {
                    WorkerError::NetworkError(format!("Connection error for {}: {}", url, e))
                } else {
                    WorkerError::NetworkError(format!("Request failed for {}: {}", url, e))
                }
            })?;

        // Check for HTTP errors
        if !response.status().is_success() {
            return Err(WorkerError::HttpError(
                response.status().as_u16(),
                format!("HTTP {} for {}", response.status(), url)
            ));
        }

        // Check content type
        if let Some(content_type) = response.headers().get("content-type") {
            let content_type_str = content_type.to_str().unwrap_or("");
            if !content_type_str.contains("text/html") {
                return Err(WorkerError::ValidationError(
                    format!("Expected HTML content, got: {}", content_type_str)
                ));
            }
        }

        // Get content
        let content = response
            .text()
            .await
            .map_err(|e| WorkerError::NetworkError(format!("Failed to read response body: {}", e)))?;

        // Validate content is not empty
        if content.trim().is_empty() {
            return Err(WorkerError::ValidationError(
                "Received empty content".to_string()
            ));
        }

        // Basic HTML validation
        if !content.contains("<html") && !content.contains("<HTML") {
            return Err(WorkerError::ValidationError(
                "Response does not appear to be HTML".to_string()
            ));
        }

        Ok(content)
    }

    fn extract_product_id(&self, url: &str) -> Option<String> {
        // Extract product ID from URL patterns
        if let Some(id_start) = url.find("id=") {
            let id_part = &url[id_start + 3..];
            if let Some(id_end) = id_part.find('&') {
                Some(id_part[..id_end].to_string())
            } else {
                Some(id_part.to_string())
            }
        } else if let Some(id_start) = url.find("prdctNo=") {
            let id_part = &url[id_start + 8..];
            if let Some(id_end) = id_part.find('&') {
                Some(id_part[..id_end].to_string())
            } else {
                Some(id_part.to_string())
            }
        } else {
            None
        }
    }
}

#[async_trait]
impl Worker<CrawlingTask> for ProductDetailFetcher {
    type Task = CrawlingTask;

    fn worker_id(&self) -> &'static str {
        "ProductDetailFetcher"
    }

    fn worker_name(&self) -> &'static str {
        "ProductDetailFetcher"
    }

    fn max_concurrency(&self) -> usize {
        10 // Network I/O can handle good concurrency
    }

    async fn process_task(
        &self,
        task: CrawlingTask,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        let start_time = Instant::now();

        match task {
            CrawlingTask::FetchProductDetail { task_id, product_url, .. } => {
                if shared_state.is_shutdown_requested() {
                    return Err(WorkerError::Cancelled);
                }

                // Extract product ID for logging
                let product_id = self.extract_product_id(&product_url)
                    .unwrap_or_else(|| "unknown".to_string());

                tracing::info!("Fetching product detail: {} (ID: {})", product_url, product_id);

                // Fetch the page content
                let html_content = self.fetch_with_retry(&product_url).await?;

                // Update statistics
                let mut stats = shared_state.stats.write().await;
                stats.product_details_fetched += 1;
                
                let duration = start_time.elapsed();
                stats.record_task_completion("fetch_product_detail", duration);

                tracing::info!(
                    "Successfully fetched product detail: {} ({} bytes, {:?})",
                    product_id,
                    html_content.len(),
                    duration
                );

                Ok(TaskResult::Success {
                    task_id,
                    output: TaskOutput::ProductDetailHtml {
                        product_id,
                        html_content,
                        source_url: product_url,
                    },
                    duration,
                })
            }
            _ => Err(WorkerError::ValidationError(
                "ProductDetailFetcher can only process FetchProductDetail tasks".to_string()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetcher_creation() {
        let fetcher = ProductDetailFetcher::new(
            10,
            Duration::from_secs(30),
            3,
            Duration::from_millis(1000),
        );
        assert!(fetcher.is_ok());
        assert_eq!(fetcher.unwrap().worker_name(), "ProductDetailFetcher");
    }

    #[test]
    fn product_id_extraction() {
        let fetcher = ProductDetailFetcher::new(
            10,
            Duration::from_secs(30),
            3,
            Duration::from_millis(1000),
        ).unwrap();

        // Test ID extraction from different URL patterns
        assert_eq!(
            fetcher.extract_product_id("https://rra.go.kr/ko/license/A_01_01_view.do?id=123456"),
            Some("123456".to_string())
        );

        assert_eq!(
            fetcher.extract_product_id("https://rra.go.kr/product?prdctNo=789&other=value"),
            Some("789".to_string())
        );

        assert_eq!(
            fetcher.extract_product_id("https://rra.go.kr/product?id=456&page=1"),
            Some("456".to_string())
        );

        assert_eq!(
            fetcher.extract_product_id("https://rra.go.kr/no-id-here"),
            None
        );
    }

    #[tokio::test]
    async fn rate_limiting() {
        let fetcher = ProductDetailFetcher::new(
            2, // Only 2 concurrent requests
            Duration::from_secs(30),
            3,
            Duration::from_millis(100),
        ).unwrap();

        // Test that rate limiter allows acquiring permits
        let permit1 = fetcher.rate_limiter.acquire().await;
        let permit2 = fetcher.rate_limiter.acquire().await;
        
        assert!(permit1.is_ok());
        assert!(permit2.is_ok());

        // Third request should be able to acquire after releases
        drop(permit1);
        drop(permit2);

        let permit3 = fetcher.rate_limiter.acquire().await;
        assert!(permit3.is_ok());
    }

    #[tokio::test]
    async fn task_processing_validation() {
        let fetcher = ProductDetailFetcher::new(
            10,
            Duration::from_secs(30),
            3,
            Duration::from_millis(1000),
        ).unwrap();

        let config = CrawlingConfig::default();
        let shared_state = Arc::new(SharedState::new(config));

        // Test with wrong task type
        let wrong_task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            url: "https://example.com".to_string(),
        };

        let result = fetcher.process_task(wrong_task, shared_state).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WorkerError::ValidationError(_)));
    }
}
