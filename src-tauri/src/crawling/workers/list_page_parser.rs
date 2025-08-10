//! # List Page Parser
//!
//! Parses HTML content from list pages to extract product URLs and metadata.

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;
use scraper::{Html, Selector};
use url::Url;

use crate::crawling::{tasks::*, state::*};
use super::{Worker, WorkerError};

/// Worker that parses list page HTML to extract product URLs
pub struct ListPageParser {
    base_url: String,
    max_products_per_page: usize,
}

impl ListPageParser {
    /// Creates a new list page parser
    pub fn new(base_url: String, max_products_per_page: usize) -> Self {
        Self {
            base_url,
            max_products_per_page,
        }
    }

    /// 개발 용이성을 위한 간단한 생성자
    pub fn new_simple() -> Self {
        Self {
            base_url: "https://csa-iot.org".to_string(),
            max_products_per_page: 50,
        }
    }

    fn parse_product_urls(&self, html: &str, page_number: u32) -> Result<Vec<String>, WorkerError> {
        let document = Html::parse_document(html);
        
        // CSS selectors for product links (Matter Certis CSA-IoT website)
        let product_container_selector = "div.post-feed article.type-product";
        let link_selector = "a";

        let mut product_urls = Vec::new();

        // Parse article containers first
        if let Ok(container_selector) = Selector::parse(product_container_selector) {
            if let Ok(link_sel) = Selector::parse(link_selector) {
                for article in document.select(&container_selector) {
                    if let Some(link) = article.select(&link_sel).next() {
                        if let Some(href) = link.value().attr("href") {
                            match self.normalize_url(href) {
                                Ok(url) => {
                                    if self.is_valid_product_url(&url) && !product_urls.contains(&url) {
                                        product_urls.push(url);
                                        
                                        // Limit products per page
                                        if product_urls.len() >= self.max_products_per_page {
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to normalize URL '{}': {}", href, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        if product_urls.is_empty() {
            return Err(WorkerError::ParseError(format!(
                "No product URLs found on page {}", page_number
            )));
        }

        tracing::info!(
            "Extracted {} product URLs from page {}",
            product_urls.len(),
            page_number
        );

        Ok(product_urls)
    }

    fn normalize_url(&self, href: &str) -> Result<String, url::ParseError> {
        if href.starts_with("http://") || href.starts_with("https://") {
            Ok(href.to_string())
        } else {
            let base = Url::parse(&self.base_url)?;
            let joined = base.join(href)?;
            Ok(joined.to_string())
        }
    }

    fn is_valid_product_url(&self, url: &str) -> bool {
        // Validate that this looks like a product detail URL for Matter Certis (CSA-IoT)
        url.contains("/csa_product/") || 
        url.contains("/product/") ||
        (url.contains("csa-iot.org") && url.contains("csa_product"))
    }

    fn extract_page_metadata(&self, html: &str) -> PageMetadata {
        let document = Html::parse_document(html);
        
    // (Removed total_items/total_pages extraction to simplify parser)

        // Common patterns for pagination info
        let pagination_selectors = [
            ".pagination .total",
            ".paging-info",
            ".total-count",
            "span[class*='total']",
        ];

    for _selector_str in &pagination_selectors { /* simplified */ }

    PageMetadata { has_next_page: self.detect_next_page(&document) }
    }

    fn detect_next_page(&self, document: &Html) -> bool {
        let next_selectors = [
            "a[class*='next']",
            "a[href*='page=']",
            ".pagination .next",
            "a:contains('다음')",
            "a:contains('>')",
        ];

        for selector_str in &next_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if document.select(&selector).next().is_some() {
                    return true;
                }
            }
        }

        false
    }
}

#[derive(Debug, Clone)]
struct PageMetadata {
    has_next_page: bool,
}

#[async_trait]
impl Worker<CrawlingTask> for ListPageParser {
    type Task = CrawlingTask;

    fn worker_id(&self) -> &'static str {
        "ListPageParser"
    }

    fn worker_name(&self) -> &'static str {
        "ListPageParser"
    }

    fn max_concurrency(&self) -> usize {
        8 // CPU-bound parsing can handle good concurrency
    }

    async fn process_task(
        &self,
        task: CrawlingTask,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        let start_time = Instant::now();

        match task {
            CrawlingTask::ParseListPage { task_id, page_number, html_content, .. } => {
                if shared_state.is_shutdown_requested() {
                    return Err(WorkerError::Cancelled);
                }

                // Parse product URLs from HTML
                let product_urls = self.parse_product_urls(&html_content, page_number)?;
                
                // Extract metadata
                let metadata = self.extract_page_metadata(&html_content);
                
                // Update statistics
                let mut stats = shared_state.stats.write().await;
                stats.list_pages_processed += 1;
                stats.product_urls_discovered += product_urls.len() as u64;
                
                let duration = start_time.elapsed();
                stats.record_task_completion("parse_list_page", duration);

                tracing::info!(
                    "Parsed page {}: found {} product URLs, has_next: {}",
                    page_number,
                    product_urls.len(),
                    metadata.has_next_page
                );

                Ok(TaskResult::Success {
                    task_id,
                    output: TaskOutput::ProductUrls(product_urls),
                    duration,
                })
            }
            _ => Err(WorkerError::ValidationError(
                "ListPageParser can only process ParseListPage tasks".to_string()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_creation() {
        let parser = ListPageParser::new(
            "https://csa-iot.org".to_string(),
            50
        );
        assert_eq!(parser.worker_name(), "ListPageParser");
    }

    #[test]
    fn url_normalization() {
        let parser = ListPageParser::new(
            "https://csa-iot.org".to_string(),
            50
        );

        // Relative URL - Matter Certis product path
        let result = parser.normalize_url("/csa_product/test-device-123");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("https://csa-iot.org"));

        // Absolute URL
        let result = parser.normalize_url("https://csa-iot.org/csa_product/wifi-plug-27");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://csa-iot.org/csa_product/wifi-plug-27");
    }

    #[test]
    fn url_validation() {
        let parser = ListPageParser::new(
            "https://csa-iot.org".to_string(),
            50
        );

        assert!(parser.is_valid_product_url("https://csa-iot.org/csa_product/wifi-plug-27"));
        assert!(parser.is_valid_product_url("/csa_product/matter-device-456"));
        assert!(!parser.is_valid_product_url("https://example.com/other"));
    }

    // number_extraction test removed: extract_number_from_text method deprecated/removed.

    #[tokio::test]
    async fn task_processing() {
        let parser = ListPageParser::new(
            "https://csa-iot.org".to_string(),
            50
        );

        let config = CrawlingConfig::default();
        let shared_state = Arc::new(SharedState::new(config));

        // Test with Matter Certis HTML structure
        let html = r#"
            <html>
                <body>
                    <div class="post-feed">
                        <article class="type-product">
                            <a href="/csa_product/wifi-plug-27/">Matter WiFi Plug 27</a>
                        </article>
                        <article class="type-product">
                            <a href="/csa_product/matter-switch-456/">Matter Light Switch 456</a>
                        </article>
                    </div>
                </body>
            </html>
        "#;

        let task = CrawlingTask::ParseListPage {
            task_id: TaskId::new(),
            page_number: 1,
            html_content: html.to_string(),
            source_url: "https://csa-iot.org/csa-iot_products/?page=1".to_string(),
        };

        let result = parser.process_task(task, shared_state).await;
        assert!(result.is_ok());

        if let Ok(TaskResult::Success { output: TaskOutput::ProductUrls(urls), .. }) = result {
            assert_eq!(urls.len(), 2);
            assert!(urls[0].contains("csa_product"));
        }
    }
}
