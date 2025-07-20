//! # Product Detail Parser
//!
//! Parses individual product detail pages to extract structured product data.

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;
use scraper::{Html, Selector};
use chrono::{DateTime, Utc, NaiveDateTime};
use regex::Regex;

use crate::crawling::{tasks::*, state::*};
use crate::domain::value_objects::ProductData;
use super::{Worker, WorkerError};

/// Worker that parses product detail HTML to extract structured data
pub struct ProductDetailParser {
    company_name_regex: Regex,
    product_name_regex: Regex,
    date_regex: Regex,
}

impl ProductDetailParser {
    /// Creates a new product detail parser
    pub fn new() -> Result<Self, WorkerError> {
        Ok(Self {
            company_name_regex: Regex::new(r"(주식회사|㈜|회사|기업|코퍼레이션|corp|inc|ltd)")
                .map_err(|e| WorkerError::InitializationError(e.to_string()))?,
            product_name_regex: Regex::new(r"[^\w\s\-\(\)가-힣]")
                .map_err(|e| WorkerError::InitializationError(e.to_string()))?,
            date_regex: Regex::new(r"(\d{4})[.\-/](\d{1,2})[.\-/](\d{1,2})")
                .map_err(|e| WorkerError::InitializationError(e.to_string()))?,
        })
    }

    /// 개발 용이성을 위한 간단한 생성자
    pub fn new_simple() -> Self {
        Self {
            company_name_regex: Regex::new(r"(주식회사|㈜|회사|기업|코퍼레이션|corp|inc|ltd)").unwrap(),
            product_name_regex: Regex::new(r"[^\w\s\-\(\)가-힣]").unwrap(),
            date_regex: Regex::new(r"(\d{4})[.\-/](\d{1,2})[.\-/](\d{1,2})").unwrap(),
        }
    }

    fn parse_product_data(&self, html: &str, product_id: &str, source_url: &str) -> Result<TaskProductData, WorkerError> {
        let document = Html::parse_document(html);
        
        // Extract basic product information
        let product_name = self.extract_product_name(&document)?;
        let company_name = self.extract_company_name(&document)?;
        let license_number = self.extract_license_number(&document);
        let registration_date = self.extract_registration_date(&document);
        let expiry_date = self.extract_expiry_date(&document);
        let product_type = self.extract_product_type(&document);
        let status = self.extract_status(&document);
        
        // Extract additional fields
        let manufacturer = self.extract_manufacturer(&document);
        let country_of_origin = self.extract_country_of_origin(&document);
        let ingredients = self.extract_ingredients(&document);
        let usage_instructions = self.extract_usage_instructions(&document);
        let warnings = self.extract_warnings(&document);
        let storage_conditions = self.extract_storage_conditions(&document);

        use crate::crawling::tasks::TaskProductData;
        use std::collections::HashMap;
        
        let mut details = HashMap::new();
        if !company_name.is_empty() {
            details.insert("company_name".to_string(), company_name.clone());
        }
        if let Some(license) = license_number.as_ref() {
            details.insert("license_number".to_string(), license.clone());
        }
        if let Some(manufacturer_val) = manufacturer.as_ref() {
            details.insert("manufacturer".to_string(), manufacturer_val.clone());
        }
        if let Some(country) = country_of_origin.as_ref() {
            details.insert("country_of_origin".to_string(), country.clone());
        }

        // 추가 필드들을 details 맵에 저장
        if let Some(registration_date) = registration_date {
            details.insert("registration_date".to_string(), registration_date.to_rfc3339());
        }
        if let Some(expiry_date) = expiry_date {
            details.insert("expiry_date".to_string(), expiry_date.to_rfc3339());
        }
        if let Some(ref product_type_value) = product_type {
            details.insert("product_type".to_string(), product_type_value.clone());
        }
        if let Some(status) = status {
            details.insert("status".to_string(), status);
        }
        if let Some(manufacturer) = manufacturer {
            details.insert("manufacturer".to_string(), manufacturer);
        }
        if let Some(country_of_origin) = country_of_origin {
            details.insert("country_of_origin".to_string(), country_of_origin);
        }

        Ok(TaskProductData {
            product_id: product_id.to_string(),
            name: product_name,
            category: product_type,
            manufacturer: Some(company_name),
            model: None,
            certification_number: license_number,
            certification_date: registration_date.map(|dt| dt.to_rfc3339()),
            details,
            extracted_at: chrono::Utc::now(),
            source_url: source_url.to_string(),
        })
    }

    fn extract_product_name(&self, document: &Html) -> Result<String, WorkerError> {
        let selectors = [
            "h1.product-name",
            "h2.product-title",
            ".product-info .name",
            "td:contains('제품명') + td",
            "th:contains('제품명') + td",
            "label:contains('제품명') + span",
            ".field-product-name .value",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 2 {
                        return Ok(self.clean_product_name(&text));
                    }
                }
            }
        }

        Err(WorkerError::ParseError("Product name not found".to_string()))
    }

    fn extract_company_name(&self, document: &Html) -> Result<String, WorkerError> {
        let selectors = [
            "td:contains('업체명') + td",
            "th:contains('업체명') + td",
            "td:contains('회사명') + td",
            "th:contains('회사명') + td",
            "label:contains('업체명') + span",
            ".field-company-name .value",
            ".company-info .name",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 2 {
                        return Ok(self.clean_company_name(&text));
                    }
                }
            }
        }

        Err(WorkerError::ParseError("Company name not found".to_string()))
    }

    fn extract_license_number(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('허가번호') + td",
            "th:contains('허가번호') + td",
            "td:contains('신고번호') + td",
            "th:contains('신고번호') + td",
            "label:contains('허가번호') + span",
            ".field-license-number .value",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 3 {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_registration_date(&self, document: &Html) -> Option<DateTime<Utc>> {
        let selectors = [
            "td:contains('등록일') + td",
            "th:contains('등록일') + td",
            "td:contains('신고일') + td",
            "th:contains('신고일') + td",
            "label:contains('등록일') + span",
            ".field-registration-date .value",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if let Some(date) = self.parse_date(&text) {
                        return Some(date);
                    }
                }
            }
        }

        None
    }

    fn extract_expiry_date(&self, document: &Html) -> Option<DateTime<Utc>> {
        let selectors = [
            "td:contains('유효기간') + td",
            "th:contains('유효기간') + td",
            "td:contains('만료일') + td",
            "th:contains('만료일') + td",
            "label:contains('유효기간') + span",
            ".field-expiry-date .value",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if let Some(date) = self.parse_date(&text) {
                        return Some(date);
                    }
                }
            }
        }

        None
    }

    fn extract_product_type(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('제품유형') + td",
            "th:contains('제품유형') + td",
            "td:contains('품목') + td",
            "th:contains('품목') + td",
            "label:contains('제품유형') + span",
            ".field-product-type .value",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_status(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('상태') + td",
            "th:contains('상태') + td",
            "td:contains('허가상태') + td",
            "th:contains('허가상태') + td",
            "label:contains('상태') + span",
            ".field-status .value",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_manufacturer(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('제조업체') + td",
            "th:contains('제조업체') + td",
            "td:contains('제조사') + td",
            "th:contains('제조사') + td",
            "label:contains('제조업체') + span",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_country_of_origin(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('제조국') + td",
            "th:contains('제조국') + td",
            "td:contains('원산지') + td",
            "th:contains('원산지') + td",
            "label:contains('제조국') + span",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_ingredients(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('성분') + td",
            "th:contains('성분') + td",
            "td:contains('원료') + td",
            "th:contains('원료') + td",
            "label:contains('성분') + span",
            ".ingredients-list",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 10 {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_usage_instructions(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('사용법') + td",
            "th:contains('사용법') + td",
            "td:contains('사용방법') + td",
            "th:contains('사용방법') + td",
            "label:contains('사용법') + span",
            ".usage-instructions",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 10 {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_warnings(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('주의사항') + td",
            "th:contains('주의사항') + td",
            "td:contains('경고') + td",
            "th:contains('경고') + td",
            "label:contains('주의사항') + span",
            ".warnings",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 10 {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn extract_storage_conditions(&self, document: &Html) -> Option<String> {
        let selectors = [
            "td:contains('보관방법') + td",
            "th:contains('보관방법') + td",
            "td:contains('저장조건') + td",
            "th:contains('저장조건') + td",
            "label:contains('보관방법') + span",
            ".storage-conditions",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() > 5 {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn clean_product_name(&self, name: &str) -> String {
        // Remove special characters but keep Korean, letters, numbers, and common symbols
        let cleaned = self.product_name_regex.replace_all(name, "");
        cleaned.trim().to_string()
    }

    fn clean_company_name(&self, name: &str) -> String {
        // Keep company name as is, just trim whitespace
        name.trim().to_string()
    }

    fn parse_date(&self, text: &str) -> Option<DateTime<Utc>> {
        if let Some(captures) = self.date_regex.captures(text) {
            let year: i32 = captures.get(1)?.as_str().parse().ok()?;
            let month: u32 = captures.get(2)?.as_str().parse().ok()?;
            let day: u32 = captures.get(3)?.as_str().parse().ok()?;
            
            let naive_date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
            let naive_datetime = naive_date.and_hms_opt(0, 0, 0)?;
            Some(DateTime::from_naive_utc_and_offset(naive_datetime, Utc))
        } else {
            None
        }
    }
}

#[async_trait]
impl Worker<CrawlingTask> for ProductDetailParser {
    type Task = CrawlingTask;

    fn worker_id(&self) -> &'static str {
        "ProductDetailParser"
    }

    fn worker_name(&self) -> &'static str {
        "ProductDetailParser"
    }

    fn max_concurrency(&self) -> usize {
        4 // CPU-bound parsing, more conservative
    }

    async fn process_task(
        &self,
        task: CrawlingTask,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        let start_time = Instant::now();

        match task {
            CrawlingTask::ParseProductDetail { task_id, product_url, html_content } => {
                if shared_state.is_shutdown_requested() {
                    return Err(WorkerError::Cancelled);
                }

                tracing::info!("Parsing product detail: {}", product_url);

                // Parse the HTML content
                let product_data = self.parse_product_data(&html_content, &product_url, &product_url)?;

                // Update statistics
                let mut stats = shared_state.stats.write().await;
                stats.product_details_fetched += 1;
                
                let duration = start_time.elapsed();
                stats.record_task_completion("parse_product_detail", duration);

                tracing::info!(
                    "Successfully parsed product: {} - {} ({})",
                    product_data.name,
                    product_data.manufacturer.as_deref().unwrap_or("Unknown"),
                    product_data.product_id
                );

                Ok(TaskResult::Success {
                    task_id,
                    output: TaskOutput::ProductData(product_data),
                    duration,
                })
            }
            _ => Err(WorkerError::ValidationError(
                "ProductDetailParser can only process ParseProductDetail tasks".to_string()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_creation() {
        let parser = ProductDetailParser::new();
        assert!(parser.is_ok());
        assert_eq!(parser.unwrap().worker_name(), "ProductDetailParser");
    }

    #[test]
    fn date_parsing() {
        let parser = ProductDetailParser::new().unwrap();
        
        assert!(parser.parse_date("2023.12.25").is_some());
        assert!(parser.parse_date("2023-12-25").is_some());
        assert!(parser.parse_date("2023/12/25").is_some());
        assert!(parser.parse_date("invalid date").is_none());
    }

    #[test]
    fn text_cleaning() {
        let parser = ProductDetailParser::new().unwrap();
        
        assert_eq!(parser.clean_product_name("테스트 제품™"), "테스트 제품");
        assert_eq!(parser.clean_company_name("  (주)테스트회사  "), "(주)테스트회사");
    }

    #[tokio::test]
    async fn task_processing() {
        let parser = ProductDetailParser::new().unwrap();
        let config = CrawlingConfig::default();
        let shared_state = Arc::new(SharedState::new(config));

        // Test with minimal HTML
        let html = r"
            <html>
                <body>
                    <table>
                        <tr>
                            <td>제품명</td>
                            <td>테스트 제품</td>
                        </tr>
                        <tr>
                            <td>업체명</td>
                            <td>(주)테스트회사</td>
                        </tr>
                    </table>
                </body>
            </html>
        ";

        let task = CrawlingTask::ParseProductDetail {
            task_id: TaskId::new(),
            product_url: "https://example.com/product/test123".to_string(),
            html_content: html.to_string(),
        };

        let result = parser.process_task(task, shared_state).await;
        assert!(result.is_ok());

        if let Ok(TaskResult::Success { output: TaskOutput::ProductData(data), .. }) = result {
            assert_eq!(data.name, "테스트 제품");
            assert_eq!(data.manufacturer, Some("(주)테스트회사".to_string()));
            assert_eq!(data.product_id, "test123");
        }
    }
}
