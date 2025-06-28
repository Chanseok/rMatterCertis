//! Minimal dependencies test runner for specific features
//! 
//! This binary uses a modular approach to test only the features you're working on.
//! Compile time is optimized by excluding heavy dependencies.

use std::sync::Arc;
use anyhow::Result;

// Core domain and infrastructure only
use matter_certis_v2_lib::infrastructure::{DatabaseConnection, SqliteVendorRepository};
use matter_certis_v2_lib::application::{VendorUseCases, CreateVendorDto};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Minimal test setup
    let db = DatabaseConnection::new("sqlite::memory:").await?;
    db.migrate().await?;
    
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let vendor_use_cases = VendorUseCases::new(vendor_repo);
    
    // Single feature test
    let vendor = vendor_use_cases.create_vendor(CreateVendorDto {
        vendor_number: 1234,
        vendor_name: "Test Vendor".to_string(),
        company_legal_name: "Test Vendor Corp".to_string(),
    }).await?;
    
    println!("✅ Minimal test passed: Created vendor {}", vendor.vendor_name);
    Ok(())
}
