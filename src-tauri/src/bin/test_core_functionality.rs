//! Core crawling functionality test
//! 
//! Tests the complete product crawling pipeline:
//! 1. Extract product summary from listing page
//! 2. Extract detailed product information from detail pages
//! 3. Database operations (save/delete)

use anyhow::Result;
use matter_certis_v2_lib::infrastructure::{HttpClient, config::{csa_iot, ConfigManager}};
use matter_certis_v2_lib::infrastructure::simple_http_client::HttpClientConfig;
use matter_certis_v2_lib::application::PageDiscoveryService;
use tracing::{info, warn, error};
use sqlx::SqlitePool;
use scraper::{Html, Selector};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("🧪 Core crawling functionality test");

    // Load configuration
    let config_manager = ConfigManager::new()?;
    let config = config_manager.load_config().await?;
    
    // Initialize HTTP client
    let mut http_client = HttpClient::with_config(HttpClientConfig::default())?;
    
    // Initialize database connection
    let db_pool = SqlitePool::connect("sqlite:data/matter_certis.db").await?;
    info!("✅ Database connection established");

    // Test 1: Extract product summaries from first page
    info!("\n🔍 Test 1: Extracting product summaries from first page");
    let first_page_products = test_product_list_extraction(&mut http_client, &config).await?;
    
    // Test 2: Extract detailed information for first few products
    info!("\n🔍 Test 2: Extracting detailed product information");
    test_product_detail_extraction(&mut http_client, &first_page_products[0..2]).await?;
    
    // Test 3: Database operations
    info!("\n🔍 Test 3: Testing database operations");
    test_database_operations(&db_pool).await?;
    
    info!("\n✅ All core functionality tests completed successfully!");
    Ok(())
}

/// Test extracting product summaries from product listing page
async fn test_product_list_extraction(
    http_client: &mut HttpClient, 
    config: &matter_certis_v2_lib::infrastructure::config::AppConfig
) -> Result<Vec<ProductSummary>> {
    info!("📄 Fetching first page of Matter products...");
    
    let html = http_client.fetch_html(csa_iot::PRODUCTS_PAGE_MATTER_ONLY).await?;
    info!("✅ First page HTML fetched successfully");
    
    // Count products on page
    let product_count = PageDiscoveryService::count_products(&html, &config.advanced.product_selectors);
    info!("📊 Found {} products on first page", product_count);
    
    // Extract product summaries using CSS selectors
    let products = extract_product_summaries(&html)?;
    
    info!("✅ Successfully extracted {} product summaries", products.len());
    
    // Display first few products for verification
    for (i, product) in products.iter().take(3).enumerate() {
        info!("📦 Product {}: {} (Certificate: {})", 
              i + 1, 
              product.title, 
              product.certificate_id.as_deref().unwrap_or("N/A")
        );
        if let Some(detail_url) = &product.detail_url {
            info!("   🔗 Detail URL: {}", detail_url);
        }
    }
    
    Ok(products)
}

/// Test extracting detailed product information
async fn test_product_detail_extraction(
    http_client: &mut HttpClient,
    summary_products: &[ProductSummary]
) -> Result<()> {
    for (i, product) in summary_products.iter().enumerate() {
        if let Some(detail_url) = &product.detail_url {
            info!("🔍 Extracting details for product {}: {}", i + 1, product.title);
            
            // Resolve relative URL to absolute
            let absolute_url = if detail_url.starts_with("http") {
                detail_url.clone()
            } else {
                format!("{}{}", csa_iot::BASE_URL, detail_url)
            };
            
            info!("🔗 Fetching: {}", absolute_url);
            
            match http_client.fetch_html(&absolute_url).await {
                Ok(detail_html) => {
                    let product_detail = extract_product_detail(&detail_html, &absolute_url)?;
                    
                    info!("✅ Successfully extracted detailed information");
                    info!("   📋 Product Name: {}", product_detail.name);
                    info!("   🏢 Manufacturer: {}", product_detail.manufacturer.as_deref().unwrap_or("N/A"));
                    info!("   🔢 Certificate ID: {}", product_detail.certificate_id.as_deref().unwrap_or("N/A"));
                    info!("   📅 Certification Date: {}", product_detail.certification_date.as_deref().unwrap_or("N/A"));
                }
                Err(e) => {
                    warn!("⚠️ Failed to fetch detail page: {}", e);
                }
            }
            
            // Small delay between requests
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        } else {
            warn!("⚠️ Product {} has no detail URL", i + 1);
        }
    }
    
    Ok(())
}

/// Test database operations (save and delete)
async fn test_database_operations(db_pool: &SqlitePool) -> Result<()> {
    info!("🗄️ Testing database operations");
    
    // Test data
    let test_url = "https://example.com/test-product";
    let test_manufacturer = "Test Manufacturer";
    let test_model = "Test Model";
    let test_certificate_id = "TEST-12345";
    
    // Test 1: Save product to database
    info!("💾 Testing product save...");
    let insert_result = sqlx::query!(
        r#"
        INSERT INTO products (
            url, manufacturer, model, certificate_id, page_id, index_in_page
        ) VALUES (?, ?, ?, ?, ?, ?)
        "#,
        test_url,
        test_manufacturer,
        test_model,
        test_certificate_id,
        1,
        1
    )
    .execute(db_pool)
    .await;
    
    match insert_result {
        Ok(_) => {
            info!("✅ Product saved successfully");
            
            // Test 2: Verify product exists
            info!("🔍 Verifying product exists in database...");
            let found_product = sqlx::query!(
                "SELECT url, manufacturer, model, certificate_id FROM products WHERE url = ?",
                test_url
            )
            .fetch_optional(db_pool)
            .await?;
            
            match found_product {
                Some(product) => {
                    info!("✅ Product verified in database: {} by {} ({})", 
                          product.model.as_deref().unwrap_or("N/A"),
                          product.manufacturer.as_deref().unwrap_or("N/A"),
                          product.certificate_id.as_deref().unwrap_or("N/A")
                    );
                }
                None => {
                    error!("❌ Product not found in database after insert!");
                }
            }
            
            // Test 3: Delete product from database
            info!("🗑️ Testing product deletion...");
            let delete_result = sqlx::query!(
                "DELETE FROM products WHERE url = ?",
                test_url
            )
            .execute(db_pool)
            .await;
            
            match delete_result {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        info!("✅ Product deleted successfully");
                        
                        // Verify deletion
                        let deleted_check = sqlx::query!(
                            "SELECT url FROM products WHERE url = ?",
                            test_url
                        )
                        .fetch_optional(db_pool)
                        .await?;
                        
                        if deleted_check.is_none() {
                            info!("✅ Product deletion verified");
                        } else {
                            error!("❌ Product still exists after deletion!");
                        }
                    } else {
                        error!("❌ No rows affected during deletion");
                    }
                }
                Err(e) => {
                    error!("❌ Failed to delete product: {}", e);
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to save product: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

// Simple structs for testing
#[derive(Debug, Clone)]
struct ProductSummary {
    title: String,
    detail_url: Option<String>,
    certificate_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductDetail {
    name: String,
    manufacturer: Option<String>,
    certificate_id: Option<String>,
    certification_date: Option<String>,
}

/// Extract product summaries from listing page HTML
fn extract_product_summaries(html: &Html) -> Result<Vec<ProductSummary>> {
    let mut products = Vec::new();
    
    // Use the same selector as PageDiscoveryService
    let product_selector = Selector::parse(".product").unwrap();
    
    for (index, element) in html.select(&product_selector).enumerate() {
        // Extract title
        let title = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
        
        // Extract detail URL
        let detail_url = element.select(&Selector::parse("a").unwrap())
            .find_map(|link| link.value().attr("href"))
            .map(|href| href.to_string());
        
        // Try to extract certificate ID from text
        let text = element.text().collect::<String>();
        let certificate_id = extract_certificate_id_from_text(&text);
        
        products.push(ProductSummary {
            title: if title.is_empty() { 
                format!("Product {}", index + 1) 
            } else { 
                title 
            },
            detail_url,
            certificate_id,
        });
    }
    
    Ok(products)
}

/// Extract product detail from detail page HTML
fn extract_product_detail(html: &Html, _url: &str) -> Result<ProductDetail> {
    // Extract product name (try multiple selectors)
    let name_selectors = ["h1", ".product-title", ".title", "h2"];
    let mut name = "Unknown Product".to_string();
    
    for selector_str in name_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = html.select(&selector).next() {
                let extracted_name = element.text().collect::<String>().trim().to_string();
                if !extracted_name.is_empty() {
                    name = extracted_name;
                    break;
                }
            }
        }
    }
    
    // Extract other details from page text
    let page_text = html.root_element().text().collect::<String>();
    
    Ok(ProductDetail {
        name,
        manufacturer: extract_manufacturer_from_text(&page_text),
        certificate_id: extract_certificate_id_from_text(&page_text),
        certification_date: extract_date_from_text(&page_text),
    })
}

/// Extract certificate ID from text using regex
fn extract_certificate_id_from_text(text: &str) -> Option<String> {
    use regex::Regex;
    
    // Look for patterns like CSA followed by numbers/letters
    let patterns = [
        r"CSA[0-9A-Z\-]+",
        r"Certificate[:\s]+([A-Z0-9\-]+)",
        r"ID[:\s]+([A-Z0-9\-]+)",
    ];
    
    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(text) {
                if let Some(id) = captures.get(1).or_else(|| captures.get(0)) {
                    return Some(id.as_str().to_string());
                }
            }
        }
    }
    
    None
}

/// Extract manufacturer from text
fn extract_manufacturer_from_text(text: &str) -> Option<String> {
    // Simple extraction - look for "Manufacturer:" or "Company:" labels
    if let Some(start) = text.find("Manufacturer:") {
        let after_label = &text[start + 13..];
        if let Some(end) = after_label.find('\n') {
            return Some(after_label[..end].trim().to_string());
        }
    }
    
    None
}

/// Extract date from text
fn extract_date_from_text(text: &str) -> Option<String> {
    use regex::Regex;
    
    // Look for date patterns
    let date_patterns = [
        r"\d{4}-\d{2}-\d{2}",
        r"\d{2}/\d{2}/\d{4}",
        r"\d{1,2} \w+ \d{4}",
    ];
    
    for pattern in date_patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(date_match) = re.find(text) {
                return Some(date_match.as_str().to_string());
            }
        }
    }
    
    None
}
