//! Test crawler functionality with real websites
//! 
//! This CLI tool allows testing the crawler implementation
//! against real websites to verify functionality.

use std::sync::Arc;
use anyhow::Result;
use matter_certis_v2_lib::{
    infrastructure::{WebCrawler, HttpClientConfig, CrawlingConfig},
    domain::session_manager::SessionManager,
    application::dto::StartCrawlingDto,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🚀 Testing rMatterCertis Web Crawler");
    println!("=====================================");

    // Create session manager
    let session_manager = Arc::new(SessionManager::new());

    // Create HTTP client config
    let http_config = HttpClientConfig {
        user_agent: "rMatterCertis-Test/1.0".to_string(),
        timeout_seconds: 30,
        max_requests_per_second: 1, // Very conservative for testing
        respect_robots_txt: true,
        follow_redirects: true,
    };

    // Create crawler
    let crawler = WebCrawler::new(http_config, session_manager.clone())?;

    // Test crawling configuration
    let test_dto = StartCrawlingDto {
        start_url: "https://certification.csa-iot.org".to_string(),
        target_domains: vec!["csa-iot.org".to_string()],
        max_pages: Some(5), // Limited for testing
        concurrent_requests: Some(1),
        delay_ms: Some(2000), // 2 second delay between requests
    };

    let config = CrawlingConfig::from(test_dto);

    println!("📋 Test Configuration:");
    println!("  Start URL: {}", config.start_url);
    println!("  Target Domains: {:?}", config.target_domains);
    println!("  Max Pages: {}", config.max_pages);
    println!("  Delay: {}ms", config.delay_ms);
    println!();

    // Start crawling
    println!("🕷️  Starting crawler test...");
    let session_id = crawler.start_crawling(config).await?;
    println!("✅ Crawling session started: {}", session_id);

    // Monitor progress
    println!("📊 Monitoring progress (for 60 seconds)...");
    let mut last_status = None;
    
    for i in 0..60 {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        if let Ok(Some(status)) = session_manager.get_session_state(&session_id).await {
            // Always print status updates for debugging
            println!("  [{}s] Status: {:?}, Progress: {}/{}, Current: {}", 
                i + 1,
                status.status,
                status.current_page,
                status.total_pages,
                status.current_url.as_deref().unwrap_or("N/A")
            );
            last_status = Some(status.clone());

            // Check if completed
            match status.status {
                matter_certis_v2_lib::domain::session_manager::SessionStatus::Completed => {
                    println!("🎉 Crawling completed successfully!");
                    break;
                }
                matter_certis_v2_lib::domain::session_manager::SessionStatus::Failed => {
                    println!("❌ Crawling failed!");
                    break;
                }
                _ => {}
            }
        }
        
        if i % 10 == 9 {
            println!("  ... still running ({}s elapsed)", i + 1);
        }
    }

    // Final status
    if let Ok(Some(final_status)) = session_manager.get_session_state(&session_id).await {
        println!();
        println!("📈 Final Results:");
        println!("  Status: {:?}", final_status.status);
        println!("  Pages Crawled: {}", final_status.current_page);
        println!("  Products Found: {}", final_status.products_found);
        println!("  Errors: {}", final_status.errors_count);
        println!("  Duration: {:?}", 
            final_status.last_updated_at.signed_duration_since(final_status.started_at));
    }

    // Test individual page crawling
    println!();
    println!("🔍 Testing individual page crawling...");
    
    match crawler.crawl_page("https://www.google.com").await {
        Ok(page) => {
            println!("✅ Successfully crawled test page:");
            println!("  URL: {}", page.url);
            println!("  Status Code: {}", page.status_code);
            println!("  Title: {}", page.title.as_deref().unwrap_or("No title"));
            println!("  Content Length: {} chars", page.content.len());
            println!("  Links Found: {}", page.links.len());
        }
        Err(e) => {
            println!("❌ Failed to crawl test page: {}", e);
        }
    }

    println!();
    println!("🏁 Crawler test completed!");
    
    Ok(())
}
