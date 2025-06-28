//! Tauri commands for Matter Certification domain
//! 
//! This module contains all Tauri commands that expose
//! backend functionality to the frontend. Each command uses
//! appropriate Use Cases and DTOs for clean separation of concerns.

use std::sync::Arc;
use tauri::State;
use crate::infrastructure::{
    database_connection::DatabaseConnection,
    repositories::{SqliteVendorRepository, SqliteProductRepository}
};
use crate::application::{
    use_cases::{VendorUseCases, MatterProductUseCases},
    dto::{
        CreateVendorDto, UpdateVendorDto, VendorResponseDto,
        CreateProductDto, CreateMatterProductDto, ProductResponseDto, MatterProductResponseDto,
        ProductSearchDto, MatterProductFilterDto, ProductSearchResultDto,
        DatabaseSummaryDto
    }
};

// ============================================================================
// Database Management Commands
// ============================================================================

#[tauri::command]
pub async fn test_database_connection() -> Result<String, String> {
    println!("🔄 Starting database connection test...");
    
    // Create data directory if it doesn't exist
    let data_dir = std::path::Path::new("./data");
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;
    }
    
    // Use relative path for database
    let database_url = "sqlite:./data/matter_certis.db";
    println!("📊 Database URL: {}", database_url);
    
    match DatabaseConnection::new(database_url).await {
        Ok(db) => {
            println!("✅ Database connection successful!");
            match db.migrate().await {
                Ok(_) => {
                    println!("✅ Migration successful!");
                    Ok("Database connection and migration successful!".to_string())
                },
                Err(e) => {
                    println!("❌ Migration failed: {}", e);
                    Err(format!("Migration failed: {}", e))
                }
            }
        },
        Err(e) => {
            println!("❌ Database connection failed: {}", e);
            Err(format!("Database connection failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_database_info() -> Result<String, String> {
    // Static database info since we're not using the managed state here
    let info = format!(
        "Database: SQLite\nLocation: ./data/matter_certis.db\nStatus: Available"
    );
    
    Ok(info)
}

#[tauri::command]
pub async fn get_database_summary(
    db: State<'_, DatabaseConnection>
) -> Result<DatabaseSummaryDto, String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.get_database_summary().await {
        Ok(summary) => Ok(summary),
        Err(e) => Err(format!("Failed to get database summary: {}", e)),
    }
}

// ============================================================================
// Vendor Management Commands
// ============================================================================

#[tauri::command]
pub async fn create_vendor(
    db: State<'_, DatabaseConnection>,
    dto: CreateVendorDto
) -> Result<VendorResponseDto, String> {
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let use_cases = VendorUseCases::new(vendor_repo);
    
    match use_cases.create_vendor(dto).await {
        Ok(vendor) => {
            println!("✅ Vendor created: {}", vendor.vendor_name);
            Ok(vendor)
        },
        Err(e) => {
            println!("❌ Failed to create vendor: {}", e);
            Err(format!("Failed to create vendor: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_all_vendors(
    db: State<'_, DatabaseConnection>
) -> Result<Vec<VendorResponseDto>, String> {
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let use_cases = VendorUseCases::new(vendor_repo);
    
    match use_cases.get_all_vendors().await {
        Ok(vendors) => {
            println!("✅ Retrieved {} vendors", vendors.len());
            Ok(vendors)
        },
        Err(e) => {
            println!("❌ Failed to get vendors: {}", e);
            Err(format!("Failed to get vendors: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_vendor_by_id(
    db: State<'_, DatabaseConnection>,
    vendor_id: String
) -> Result<Option<VendorResponseDto>, String> {
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let use_cases = VendorUseCases::new(vendor_repo);
    
    match use_cases.get_vendor_by_id(&vendor_id).await {
        Ok(vendor) => Ok(vendor),
        Err(e) => Err(format!("Failed to get vendor: {}", e)),
    }
}

#[tauri::command]
pub async fn search_vendors_by_name(
    db: State<'_, DatabaseConnection>,
    name: String
) -> Result<Vec<VendorResponseDto>, String> {
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let use_cases = VendorUseCases::new(vendor_repo);
    
    match use_cases.search_vendors_by_name(&name).await {
        Ok(vendors) => {
            println!("✅ Found {} vendors for query: '{}'", vendors.len(), name);
            Ok(vendors)
        },
        Err(e) => Err(format!("Failed to search vendors: {}", e)),
    }
}

#[tauri::command]
pub async fn update_vendor(
    db: State<'_, DatabaseConnection>,
    vendor_id: String,
    dto: UpdateVendorDto
) -> Result<VendorResponseDto, String> {
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let use_cases = VendorUseCases::new(vendor_repo);
    
    match use_cases.update_vendor(&vendor_id, dto).await {
        Ok(vendor) => {
            println!("✅ Vendor updated: {}", vendor.vendor_name);
            Ok(vendor)
        },
        Err(e) => {
            println!("❌ Failed to update vendor: {}", e);
            Err(format!("Failed to update vendor: {}", e))
        }
    }
}

#[tauri::command]
pub async fn delete_vendor(
    db: State<'_, DatabaseConnection>,
    vendor_id: String
) -> Result<(), String> {
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let use_cases = VendorUseCases::new(vendor_repo);
    
    match use_cases.delete_vendor(&vendor_id).await {
        Ok(_) => {
            println!("✅ Vendor deleted: {}", vendor_id);
            Ok(())
        },
        Err(e) => {
            println!("❌ Failed to delete vendor: {}", e);
            Err(format!("Failed to delete vendor: {}", e))
        }
    }
}

// ============================================================================
// Matter Product Management Commands
// ============================================================================

#[tauri::command]
pub async fn create_product(
    db: State<'_, DatabaseConnection>,
    dto: CreateProductDto
) -> Result<ProductResponseDto, String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.create_product(dto).await {
        Ok(product) => {
            println!("✅ Product created: {}", product.url);
            Ok(product)
        },
        Err(e) => {
            println!("❌ Failed to create product: {}", e);
            Err(format!("Failed to create product: {}", e))
        }
    }
}

#[tauri::command]
pub async fn create_matter_product(
    db: State<'_, DatabaseConnection>,
    dto: CreateMatterProductDto
) -> Result<MatterProductResponseDto, String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.create_matter_product(dto).await {
        Ok(product) => {
            println!("✅ Matter product created: {}", product.url);
            Ok(product)
        },
        Err(e) => {
            println!("❌ Failed to create Matter product: {}", e);
            Err(format!("Failed to create Matter product: {}", e))
        }
    }
}

#[tauri::command]
pub async fn search_matter_products(
    db: State<'_, DatabaseConnection>,
    dto: ProductSearchDto
) -> Result<ProductSearchResultDto, String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.search_matter_products(dto).await {
        Ok(result) => {
            println!("✅ Found {} Matter products", result.total_count);
            Ok(result)
        },
        Err(e) => {
            println!("❌ Failed to search Matter products: {}", e);
            Err(format!("Failed to search Matter products: {}", e))
        }
    }
}

#[tauri::command]
pub async fn filter_matter_products(
    db: State<'_, DatabaseConnection>,
    filter: MatterProductFilterDto
) -> Result<ProductSearchResultDto, String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.filter_matter_products(filter).await {
        Ok(result) => {
            println!("✅ Filtered {} Matter products", result.total_count);
            Ok(result)
        },
        Err(e) => {
            println!("❌ Failed to filter Matter products: {}", e);
            Err(format!("Failed to filter Matter products: {}", e))
        }
    }
}

#[tauri::command]
pub async fn delete_product(
    db: State<'_, DatabaseConnection>,
    url: String
) -> Result<(), String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.delete_product(&url).await {
        Ok(_) => {
            println!("✅ Product deleted: {}", url);
            Ok(())
        },
        Err(e) => {
            println!("❌ Failed to delete product: {}", e);
            Err(format!("Failed to delete product: {}", e))
        }
    }
}

// ============================================================================
// Legacy/Example Commands (to be removed)
// ============================================================================

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
