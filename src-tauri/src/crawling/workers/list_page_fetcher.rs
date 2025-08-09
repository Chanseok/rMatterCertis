//! # List Page Fetcher Worker
//!
//! Fetches HTML content from product list pages with rate limiting and error handling.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use tokio::time::sleep;
use url::Url;

use crate::crawling::{tasks::*, state::*};
use crate::infrastructure::HttpClient;
use super::{Worker, WorkerError};

/// Worker that fetches HTML content from list pages
pub struct ListPageFetcher {
    max_retries: u32,
}

impl ListPageFetcher {
    /// Creates a new list page fetcher
    pub fn new(_request_timeout: Duration, max_retries: u32) -> Result<Self, WorkerError> {
        Ok(Self { max_retries })
    }

    /// 개발 용이성을 위한 간단한 생성자
    pub fn new_simple() -> Self {
        use crate::infrastructure::config::defaults;
    Self { max_retries: defaults::MAX_RETRIES }
    }

    pub async fn fetch_page(&self, url: &str) -> Result<String, WorkerError> {
        let parsed_url = Url::parse(url)
            .map_err(|e| WorkerError::InvalidInput(format!("Invalid URL '{}': {}", url, e)))?;
        
        let http_client = HttpClient::create_from_global_config()
            .map_err(|e| WorkerError::NetworkError(e.to_string()))?;
        
        let response = http_client
            .fetch_response(&parsed_url.to_string())
            .await
            .map_err(|e| WorkerError::NetworkError(e.to_string()))?;

        if response.status().is_success() {
            let text = response.text().await
                .map_err(|e| WorkerError::NetworkError(format!("Failed to read response body: {}", e)))?;
            Ok(text)
        } else {
            Err(WorkerError::NetworkError(
                format!("HTTP request failed with status: {}", response.status())
            ))
        }
    }

    /// Fetch page with retry logic and rate limiting
    async fn fetch_with_retry(&self, url: &str, _shared_state: Arc<SharedState>) -> Result<String, WorkerError> {
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
                        let delay = Duration::from_millis(1000) * (2_u32.pow(attempt)); // Exponential backoff
                        tracing::warn!(
                            "Failed to fetch {} (attempt {}), retrying in {:?}: {}",
                            url, attempt + 1, delay, last_error.as_ref().unwrap()
                        );
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| WorkerError::NetworkError("Unknown error during fetch".to_string())))
    }

    fn build_list_page_url(&self, page: u32) -> String {
        format!("https://csa-iot.org/csa-iot_products/?page={}", page)
    }
}

#[async_trait]
impl Worker<CrawlingTask> for ListPageFetcher {
    type Task = CrawlingTask;

    fn worker_id(&self) -> &'static str {
        "ListPageFetcher"
    }

    fn worker_name(&self) -> &'static str {
        "ListPageFetcher"
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
            CrawlingTask::FetchListPage { task_id, page_number, url } => {
                // Build URL if not provided
                let target_url = if url.is_empty() {
                    self.build_list_page_url(page_number)
                } else {
                    url
                };

                // Fetch HTML content
                let html_content = self.fetch_with_retry(&target_url, shared_state).await?;

                let duration = start_time.elapsed();
                
                Ok(TaskResult::Success {
                    task_id,
                    output: TaskOutput::HtmlContent(html_content),
                    duration,
                })
            }
            _ => Err(WorkerError::ValidationError(
                "ListPageFetcher can only process FetchListPage tasks".to_string()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawling::state::CrawlingConfig;

    #[test]
    fn list_page_fetcher_creation() {
        let fetcher = ListPageFetcher::new(
            Duration::from_secs(30),
            3
        );
        assert!(fetcher.is_ok());
    }

    #[test]
    fn url_building() {
        let fetcher = ListPageFetcher::new(
            Duration::from_secs(30),
            3
        ).unwrap();
        
        let url = fetcher.build_list_page_url(1);
        assert!(url.contains("page=1"));
        assert!(url.contains("csa-iot.org"));
    }

    #[tokio::test]
    async fn task_validation() {
        let fetcher = ListPageFetcher::new(
            Duration::from_secs(30),
            3
        ).unwrap();

        let config = CrawlingConfig::default();
        let shared_state = Arc::new(SharedState::new(config));

        // Valid task
        let valid_task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            url: "".to_string(),
        };

        // Should not immediately fail on validation
        // (actual network call would fail in test environment)
        
        // Invalid task
        let invalid_task = CrawlingTask::SaveProduct {
            task_id: TaskId::new(),
            product_data: crate::crawling::tasks::TaskProductData {
                product_id: "test".to_string(),
                name: "test".to_string(),
                category: None,
                manufacturer: None,
                model: None,
                certification_number: None,
                certification_date: None,
                details: std::collections::HashMap::new(),
                extracted_at: chrono::Utc::now(),
                source_url: "test".to_string(),
                page_id: None,
                index_in_page: None,
            },
        };

        let result = fetcher.process_task(invalid_task, shared_state).await;
        assert!(matches!(result, Err(WorkerError::ValidationError(_))));
    }
}
