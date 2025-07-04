//! Test site analysis functionality
//! 
//! This binary tests the actual site crawling and analysis capabilities
//! by connecting to the CSA-IoT website and analyzing page structure.

use anyhow::Result;
use matter_certis_v2_lib::infrastructure::{HttpClient, MatterDataExtractor, csa_iot};
use matter_certis_v2_lib::infrastructure::simple_http_client::HttpClientConfig;
use scraper::{Html, Selector};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting CSA-IoT site analysis test");

    // Create HTTP client
    let mut http_client = HttpClient::with_config(HttpClientConfig::default())?;
    
    // Create data extractor
    let data_extractor = MatterDataExtractor::new()?;

    // Test basic connectivity
    info!("🔗 Testing connectivity to CSA-IoT website...");
    
    match http_client.fetch_html(csa_iot::PRODUCTS_PAGE_MATTER_ONLY).await {
        Ok(html_content) => {
            info!("✅ Successfully connected to {}", csa_iot::PRODUCTS_PAGE_MATTER_ONLY);
            
            // Get the raw HTML string to check length
            let html_string = html_content.html();
            info!("📄 HTML content length: {} bytes", html_string.len());
            
            // Use the html_content directly since it's already parsed HTML
            let html = html_content;
            
            // Analyze page structure
            analyze_page_structure(&html).await?;
            
            // Try to find pagination info
            analyze_pagination(&html).await?;
            
            // Try to extract product information
            analyze_products(&html, &data_extractor).await?;
            
        },
        Err(e) => {
            error!("❌ Failed to connect to {}: {}", csa_iot::PRODUCTS_PAGE_MATTER_ONLY, e);
            return Err(e);
        }
    }

    info!("🎉 Site analysis test completed successfully!");
    Ok(())
}

async fn analyze_page_structure(html: &Html) -> Result<()> {
    info!("🔍 Analyzing page structure...");
    
    // Look for common pagination patterns
    let pagination_selectors = vec![
        ".pagination",
        ".page-numbers",
        ".pager",
        "[class*='page']",
        "[class*='pagination']",
        "nav[aria-label*='page']",
        ".wp-pagenavi", // WordPress pagination
    ];
    
    for selector_str in pagination_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            let elements = html.select(&selector).collect::<Vec<_>>();
            if !elements.is_empty() {
                info!("📄 Found {} pagination elements with selector: {}", elements.len(), selector_str);
                
                // Print some details about the first element
                if let Some(element) = elements.first() {
                    info!("   First element: <{}>", element.value().name());
                    if let Some(class) = element.value().attr("class") {
                        info!("   Classes: {}", class);
                    }
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !text.is_empty() && text.len() < 200 {
                        info!("   Text: {}", text);
                    }
                }
            }
        }
    }
    
    // Look for product containers
    let product_selectors = vec![
        ".product",
        ".product-item",
        ".product-card",
        "[class*='product']",
        ".entry",
        ".post",
        "article",
    ];
    
    for selector_str in product_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            let elements = html.select(&selector).collect::<Vec<_>>();
            if !elements.is_empty() {
                info!("🛍️ Found {} product-like elements with selector: {}", elements.len(), selector_str);
            }
        }
    }
    
    Ok(())
}

async fn analyze_pagination(html: &Html) -> Result<()> {
    info!("📖 Analyzing pagination...");
    
    // Try to find "last" or "next" links
    let link_selectors = vec![
        "a[href*='page']",
        "a[class*='next']",
        "a[class*='last']",
        "a[class*='page']",
    ];
    
    let mut max_page = 0;
    let mut found_links = Vec::new();
    
    for selector_str in link_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in html.select(&selector) {
                if let Some(href) = element.value().attr("href") {
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    found_links.push((href.to_string(), text));
                    
                    // Try to extract page number from href
                    if let Some(page_num) = extract_page_number(href) {
                        max_page = max_page.max(page_num);
                    }
                }
            }
        }
    }
    
    if !found_links.is_empty() {
        info!("🔗 Found {} pagination links:", found_links.len());
        for (href, text) in found_links.iter().take(10) { // Show first 10 links
            info!("   '{}' -> {}", text, href);
        }
        
        if max_page > 0 {
            info!("📊 Estimated maximum page number: {}", max_page);
        }
    } else {
        warn!("⚠️ No pagination links found");
    }
    
    Ok(())
}

async fn analyze_products(html: &Html, data_extractor: &MatterDataExtractor) -> Result<()> {
    info!("🧪 Analyzing products using data extractor...");
    
    match data_extractor.extract_products_from_list(html, 1) {
        Ok(products) => {
            info!("✅ Successfully extracted {} products from page", products.len());
            
            // Show details of first few products
            for (i, product) in products.iter().take(3).enumerate() {
                info!("   Product {}: {}", i + 1, product.manufacturer.as_deref().unwrap_or("Unknown"));
                if let Some(model) = &product.model {
                    info!("      Model: {}", model);
                }
                if let Some(cert_id) = &product.certificate_id {
                    info!("      Cert ID: {}", cert_id);
                }
            }
        },
        Err(e) => {
            warn!("⚠️ Failed to extract products using data extractor: {}", e);
            
            // Try manual extraction as fallback
            info!("🔧 Attempting manual product extraction...");
            manual_product_extraction(html).await?;
        }
    }
    
    Ok(())
}

async fn manual_product_extraction(html: &Html) -> Result<()> {
    // Try different selectors that might contain product information
    let potential_selectors = vec![
        "h1, h2, h3, h4", // Headings might be product titles
        "a[href*='product']",
        "a[href*='certification']",
        "[class*='title']",
        "[class*='name']",
    ];
    
    for selector_str in potential_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            let elements = html.select(&selector).collect::<Vec<_>>();
            if !elements.is_empty() {
                info!("🔍 Found {} elements with selector: {}", elements.len(), selector_str);
                
                // Show first few elements
                for (i, element) in elements.iter().take(5).enumerate() {
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !text.is_empty() && text.len() < 100 {
                        info!("   {}: {}", i + 1, text);
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn extract_page_number(url: &str) -> Option<u32> {
    // Try to extract page number from URL patterns like:
    // ?page=5, /page/5, ?paged=5, etc.
    
    if let Some(captures) = regex::Regex::new(r"[?&]page[d]?=(\d+)")
        .ok()
        .and_then(|re| re.captures(url)) 
    {
        if let Some(num_match) = captures.get(1) {
            return num_match.as_str().parse().ok();
        }
    }
    
    if let Some(captures) = regex::Regex::new(r"/page/(\d+)")
        .ok()
        .and_then(|re| re.captures(url))
    {
        if let Some(num_match) = captures.get(1) {
            return num_match.as_str().parse().ok();
        }
    }
    
    None
}
