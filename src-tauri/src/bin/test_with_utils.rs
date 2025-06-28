//! Example test using new test utilities
//! 
//! This demonstrates how to use the new TestContext and TestDatabase
//! utilities for clean, isolated testing.
//! 
//! Run with: cargo run --bin test_with_utils --features test-utils

use anyhow::Result;
use matter_certis_v2_lib::test_utils::TestContext;
use matter_certis_v2_lib::application::CreateVendorDto;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing with new test utilities");
    println!("═══════════════════════════════════");
    println!();

    // Create isolated test context with fresh in-memory database
    let ctx = TestContext::new().await?;
    println!("✅ Test context created with fresh database");

    // Test vendor creation
    let vendor = ctx.vendor_use_cases.create_vendor(CreateVendorDto {
        vendor_number: 12345,
        vendor_name: "Test Vendor Corp".to_string(),
        company_legal_name: "Test Vendor Corporation Ltd.".to_string(),
    }).await?;
    
    println!("✅ Created vendor: {} (ID: {})", vendor.vendor_name, vendor.vendor_id);

    // Test session manager (in-memory only)
    let session_id = "test-session-001";
    let start_url = "https://example.com";
    
    ctx.session_manager.start_session_simple(session_id, start_url, vec!["example.com".to_string()]).await?;
    println!("✅ Started crawling session: {}", session_id);

    // Simulate crawling progress
    ctx.session_manager.update_session_progress(session_id, 50, "Processing...".to_string()).await?;
    println!("✅ Updated session progress to 50%");

    // Get session status
    if let Some(status) = ctx.session_manager.get_session_state(session_id).await? {
        println!("✅ Session status: {:?} - Progress: {}", 
                 status.status, status.products_found);
    }

    // Complete session
    ctx.session_manager.complete_session_simple(session_id).await?;
    println!("✅ Completed session");

    // Verify database stats
    let summary = ctx.product_use_cases.get_database_summary().await?;
    println!("✅ Database summary: {} vendors, {} products", 
             summary.total_vendors, summary.total_products);

    println!();
    println!("🎉 All tests passed! The new test utilities work perfectly.");
    println!("💡 Key benefits:");
    println!("   • Each test gets a fresh, isolated in-memory database");
    println!("   • No file-based state issues or cleanup needed");
    println!("   • Fast test execution and reliable results");
    println!("   • Easy to use TestContext provides all dependencies");
    println!("   • Session management is completely in-memory");
    println!("   • Only final results would be saved to database (when needed)");

    Ok(())
}
