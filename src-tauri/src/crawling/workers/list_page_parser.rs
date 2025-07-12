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
        
        // Try to extract total pages/items information
        let mut total_items = None;
        let mut total_pages = None;

        // Common patterns for pagination info
        let pagination_selectors = [
            ".pagination .total",
            ".paging-info",
            ".total-count",
            "span[class*='total']",
        ];

        for selector_str in &pagination_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in document.select(&selector) {
                    let text = element.text().collect::<String>();
                    
                    // Try to extract numbers from text
                    if let Some(num) = self.extract_number_from_text(&text) {
                        if text.contains("총") || text.contains("전체") {
                            total_items = Some(num);
                        } else if text.contains("페이지") || text.contains("page") {
                            total_pages = Some(num);
                        }
                    }
                }
            }
        }

        PageMetadata {
            total_items,
            total_pages,
            has_next_page: self.detect_next_page(&document),
        }
    }

    fn extract_number_from_text(&self, text: &str) -> Option<u32> {
        // Extract numbers from Korean text like "총 1,234건" or "전체 567 페이지"
        let re = regex::Regex::new(r"[\d,]+").ok()?;
        let captures = re.find(text)?;
        let number_str = captures.as_str().replace(",", "");
        number_str.parse().ok()
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
    total_items: Option<u32>,
    total_pages: Option<u32>,
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
            "https://rra.go.kr".to_string(),
            50
        );
        assert_eq!(parser.worker_name(), "ListPageParser");
    }

    #[test]
    fn url_normalization() {
        let parser = ListPageParser::new(
            "https://rra.go.kr".to_string(),
            50
        );

        // Relative URL
        let result = parser.normalize_url("/ko/license/A_01_01_view.do?id=123");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("https://rra.go.kr"));

        // Absolute URL
        let result = parser.normalize_url("https://example.com/test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://example.com/test");
    }

    #[test]
    fn url_validation() {
        let parser = ListPageParser::new(
            "https://rra.go.kr".to_string(),
            50
        );

        assert!(parser.is_valid_product_url("https://rra.go.kr/ko/license/A_01_01_view.do?id=123"));
        assert!(parser.is_valid_product_url("/detail/product/456"));
        assert!(!parser.is_valid_product_url("https://example.com/other"));
    }

    #[test]
    fn number_extraction() {
        let parser = ListPageParser::new(
            "https://rra.go.kr".to_string(),
            50
        );

        assert_eq!(parser.extract_number_from_text("총 1,234건"), Some(1234));
        assert_eq!(parser.extract_number_from_text("전체 567 페이지"), Some(567));
        assert_eq!(parser.extract_number_from_text("no numbers here"), None);
    }

    #[tokio::test]
    async fn task_processing() {
        let parser = ListPageParser::new(
            "https://rra.go.kr".to_string(),
            50
        );

        let config = CrawlingConfig::default();
        let shared_state = Arc::new(SharedState::new(config));

        // Test with minimal HTML
        let html = r#"
            <html>
                <body>
                    <a href="/ko/license/A_01_01_view.do?id=123">Product 1</a>
                    <a href="/ko/license/A_01_01_view.do?id=456">Product 2</a>
                </body>
            </html>
        "#;

        let task = CrawlingTask::ParseListPage {
            task_id: TaskId::new(),
            page_number: 1,
            html_content: html.to_string(),
            source_url: "https://rra.go.kr/page1".to_string(),
        };

        let result = parser.process_task(task, shared_state).await;
        assert!(result.is_ok());

        if let Ok(TaskResult::Success { output: TaskOutput::ProductUrls(urls), .. }) = result {
            assert_eq!(urls.len(), 2);
            assert!(urls[0].contains("A_01_01_view.do"));
        }
    }
}
