//! # List Page Fetcher Worker
//!
//! Fetches HTML content from product list pages with rate limiting and error handling.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use reqwest::Client;
use tokio::time::timeout;

use crate::crawling::{tasks::*, state::*};
use super::{Worker, WorkerError};

/// Worker that fetches HTML content from list pages
pub struct ListPageFetcher {
    http_client: Client,
    request_timeout: Duration,
    max_retries: u32,
}

impl ListPageFetcher {
    /// Creates a new list page fetcher
    pub fn new(request_timeout: Duration, max_retries: u32) -> Result<Self, WorkerError> {
        let http_client = Client::builder()
            .timeout(request_timeout)
            .user_agent("Mozilla/5.0 (compatible; RMatterCertis/2.0)")
            .build()
            .map_err(|e| WorkerError::NetworkError(e.to_string()))?;

        Ok(Self {
            http_client,
            request_timeout,
            max_retries,
        })
    }

    /// 개발 용이성을 위한 간단한 생성자
    pub fn new_simple() -> Self {
        use crate::infrastructure::config::defaults;
        Self {
            http_client: reqwest::Client::new(),
            request_timeout: Duration::from_secs(defaults::REQUEST_TIMEOUT_SECONDS),
            max_retries: defaults::MAX_RETRIES,
        }
    }

    async fn fetch_with_retry(
        &self,
        url: &str,
        shared_state: Arc<SharedState>,
    ) -> Result<String, WorkerError> {
        let mut last_error = None;
        
        for attempt in 0..=self.max_retries {
            if shared_state.is_shutdown_requested() {
                return Err(WorkerError::Cancelled);
            }

            // Rate limiting: acquire semaphore permit
            let _permit = shared_state
                .http_semaphore
                .acquire()
                .await
                .map_err(|_| WorkerError::Cancelled)?;

            let start_time = Instant::now();
            
            match timeout(self.request_timeout, self.http_client.get(url).send()).await {
                Ok(Ok(response)) => {
                    let duration = start_time.elapsed();
                    
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(html) => {
                                // Update stats
                                let mut stats = shared_state.stats.write().await;
                                stats.record_task_completion("fetch_list_page", duration);
                                return Ok(html);
                            }
                            Err(e) => {
                                last_error = Some(WorkerError::NetworkError(format!(
                                    "Failed to read response body: {}", e
                                )));
                            }
                        }
                    } else {
                        last_error = Some(WorkerError::NetworkError(format!(
                            "HTTP error {}: {}", response.status(), url
                        )));
                    }
                }
                Ok(Err(e)) => {
                    last_error = Some(WorkerError::NetworkError(e.to_string()));
                }
                Err(_) => {
                    last_error = Some(WorkerError::Timeout { 
                        message: format!("Request timeout after {} seconds", self.request_timeout.as_secs())
                    });
                }
            }

            // Wait before retry (exponential backoff)
            if attempt < self.max_retries {
                let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                tokio::time::sleep(delay).await;
            }
        }

        // Update failure stats
        let mut stats = shared_state.stats.write().await;
        stats.record_task_failure("fetch_list_page");
        
        Err(last_error.unwrap_or_else(|| {
            WorkerError::NetworkError("Unknown error after all retries".to_string())
        }))
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
            },
        };

        let result = fetcher.process_task(invalid_task, shared_state).await;
        assert!(matches!(result, Err(WorkerError::ValidationError(_))));
    }
}
