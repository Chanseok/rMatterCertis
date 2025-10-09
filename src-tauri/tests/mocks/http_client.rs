//! Mock HTTP Client for testing page discovery logic
//!
//! Simulates server responses for testing crawling algorithms without network calls.

use std::collections::HashMap;

/// Mock HTTP response
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub body: String,
}

/// Mock HTTP Client that returns predefined responses
#[derive(Debug, Clone)]
pub struct MockHttpClient {
    /// Map of URL -> Response
    responses: HashMap<String, MockResponse>,
    /// Default response for unmapped URLs
    default_response: MockResponse,
}

impl MockHttpClient {
    /// Create a new mock client with default 404 responses
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            default_response: MockResponse {
                status: 404,
                body: String::new(),
            },
        }
    }

    /// Set response for a specific URL
    pub fn with_response(mut self, url: impl Into<String>, response: MockResponse) -> Self {
        self.responses.insert(url.into(), response);
        self
    }

    /// Set multiple responses
    pub fn with_responses(mut self, responses: Vec<(String, MockResponse)>) -> Self {
        for (url, response) in responses {
            self.responses.insert(url, response);
        }
        self
    }

    /// Set default response for unmapped URLs
    pub fn with_default(mut self, response: MockResponse) -> Self {
        self.default_response = response;
        self
    }

    /// Fetch response for a URL (simulated)
    pub fn fetch(&self, url: &str) -> MockResponse {
        self.responses
            .get(url)
            .cloned()
            .unwrap_or_else(|| self.default_response.clone())
    }
}

impl Default for MockHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate empty page HTML (no products)
pub fn empty_page_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>Matter Devices</title></head>
<body>
    <div class="container">
        <h1>Certified Products</h1>
        <div class="product-list">
            <!-- No products -->
        </div>
        <div class="pagination">
            <span class="disabled">Previous</span>
            <span class="current">1</span>
            <span class="disabled">Next</span>
        </div>
    </div>
</body>
</html>"#
        .to_string()
}

/// Generate page HTML with products
pub fn page_with_products_html(page_num: u32, product_count: u32) -> String {
    let mut products_html = String::new();
    for i in 0..product_count {
        products_html.push_str(&format!(
            r#"<div class="product-item">
    <h3>Product {} on Page {}</h3>
    <a href="/products/p{:04}i{:02}">View Details</a>
</div>
"#,
            i + 1,
            page_num,
            page_num,
            i
        ));
    }

    let prev_link = if page_num > 1 {
        format!(r#"<a href="?page={}">Previous</a>"#, page_num - 1)
    } else {
        r#"<span class="disabled">Previous</span>"#.to_string()
    };

    let next_link = format!(r#"<a href="?page={}">Next</a>"#, page_num + 1);

    format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Matter Devices - Page {}</title></head>
<body>
    <div class="container">
        <h1>Certified Products - Page {}</h1>
        <div class="product-list">
{}
        </div>
        <div class="pagination">
            {}
            <span class="current">{}</span>
            {}
        </div>
    </div>
</body>
</html>"#,
        page_num, page_num, products_html, prev_link, page_num, next_link
    )
}

/// Generate pagination HTML for testing
pub fn pagination_html(total_pages: u32, current_page: u32) -> String {
    let mut pages_html = String::new();
    for i in 1..=total_pages {
        if i == current_page {
            pages_html.push_str(&format!(r#"<span class="current">{}</span> "#, i));
        } else {
            pages_html.push_str(&format!(r#"<a href="?page={}">{}</a> "#, i, i));
        }
    }

    format!(
        r#"<div class="pagination">
    <a href="?page={}">Previous</a>
    {}
    <a href="?page={}">Next</a>
</div>"#,
        current_page.saturating_sub(1),
        pages_html,
        current_page + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_page_generation() {
        let html = empty_page_html();
        assert!(html.contains("<div class=\"product-list\">"));
        assert!(html.contains("<!-- No products -->"));
    }

    #[test]
    fn test_page_with_products_generation() {
        let html = page_with_products_html(5, 12);
        assert!(html.contains("Page 5"));
        assert!(html.contains("Product 1"));
        assert!(html.contains("Product 12"));
        assert!(html.contains(r#"href="/products/p0005i00""#));
    }

    #[test]
    fn test_mock_client_default_404() {
        let client = MockHttpClient::new();
        let response = client.fetch("https://example.com/page1");
        assert_eq!(response.status, 404);
    }

    #[test]
    fn test_mock_client_custom_response() {
        let client = MockHttpClient::new().with_response(
            "https://example.com/page1",
            MockResponse {
                status: 200,
                body: "test".to_string(),
            },
        );
        let response = client.fetch("https://example.com/page1");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "test");
    }
}
