use std::sync::Arc;
use tauri::State;
use crate::infrastructure::{
    database_connection::DatabaseConnection,
    integrated_product_repository::IntegratedProductRepository,
};
use crate::application::integrated_use_cases::IntegratedProductUseCases;
use crate::domain::product::ProductSearchCriteria;
use crate::domain::integrated_product::DatabaseStatistics;
use tracing::info;

/// Get comprehensive database statistics from the integrated schema
#[tauri::command(async)]
pub async fn get_integrated_database_statistics(
    db: State<'_, DatabaseConnection>
) -> Result<DatabaseStatistics, String> {
    let repo = IntegratedProductRepository::new(db.pool().clone());
    let use_cases = IntegratedProductUseCases::new(Arc::new(repo));
    
    match use_cases.get_database_statistics().await {
        Ok(stats) => {
            println!("✅ Retrieved database statistics");
            Ok(stats)
        },
        Err(e) => {
            println!("❌ Failed to get database statistics: {e}");
            Err(format!("Failed to get database statistics: {e}"))
        }
    }
}

/// Search products using the integrated schema
#[tauri::command(async)]
pub async fn search_integrated_products_simple(
    db: State<'_, DatabaseConnection>,
    manufacturer: Option<String>,
    limit: Option<i32>
) -> Result<crate::domain::product::ProductSearchResult, String> {
    let repo = IntegratedProductRepository::new(db.pool().clone());
    let use_cases = IntegratedProductUseCases::new(Arc::new(repo));
    
    let criteria = ProductSearchCriteria {
        manufacturer,
        device_type: None,
    certificate_id: None,
        specification_version: None,
        program_type: None,
        page: Some(1),
        limit,
    };
    
    match use_cases.search_products(criteria).await {
        Ok(result) => {
            println!("✅ Found {} products", result.products.len());
            Ok(result)
        },
        Err(e) => {
            println!("❌ Failed to search products: {e}");
            Err(format!("Failed to search products: {e}"))
        }
    }
}

/// Get products without details for crawling prioritization
#[tauri::command(async)]
pub async fn get_integrated_products_without_details(
    db: State<'_, DatabaseConnection>,
    limit: Option<i32>
) -> Result<Vec<crate::domain::product::Product>, String> {
    let repo = IntegratedProductRepository::new(db.pool().clone());
    let use_cases = IntegratedProductUseCases::new(Arc::new(repo));
    
    let limit = limit.unwrap_or(100);
    
    match use_cases.get_products_without_details(limit).await {
        Ok(products) => {
            println!("✅ Found {} products without details", products.len());
            Ok(products)
        },
        Err(e) => {
            println!("❌ Failed to get products without details: {e}");
            Err(format!("Failed to get products without details: {e}"))
        }
    }
}

/// Validate database integrity
#[tauri::command(async)]
pub async fn validate_integrated_database_integrity(
    db: State<'_, DatabaseConnection>
) -> Result<DatabaseStatistics, String> {
    let repo = IntegratedProductRepository::new(db.pool().clone());
    let use_cases = IntegratedProductUseCases::new(Arc::new(repo));
    
    match use_cases.validate_database_integrity().await {
        Ok(stats) => {
            println!("✅ Database integrity validation completed");
            Ok(stats)
        },
        Err(e) => {
            println!("❌ Failed to validate database integrity: {e}");
            Err(format!("Failed to validate database integrity: {e}"))
        }
    }
}

/// Hard reset: delete all products & details so we can rebuild with new indexing semantics (Plan B)
#[tauri::command(async)]
pub async fn reset_product_storage(
    db: State<'_, DatabaseConnection>
) -> Result<(u64, u64), String> {
    let repo = IntegratedProductRepository::new(db.pool().clone());
    match repo.clear_all_products_and_details().await {
        Ok((p, d)) => { info!("✅ Product storage reset: products={}, details={}", p, d); Ok((p, d)) },
        Err(e) => Err(format!("Failed to reset product storage: {e}"))
    }
}
