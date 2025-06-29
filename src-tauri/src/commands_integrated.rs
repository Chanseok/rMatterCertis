use std::sync::Arc;
use tauri::State;
use crate::infrastructure::{
    database_connection::DatabaseConnection,
    integrated_product_repository::IntegratedProductRepository,
};
use crate::application::integrated_use_cases::IntegratedProductUseCases;
use crate::domain::integrated_product::{ProductSearchCriteria, DatabaseStatistics};

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
) -> Result<crate::domain::integrated_product::ProductSearchResult, String> {
    let repo = IntegratedProductRepository::new(db.pool().clone());
    let use_cases = IntegratedProductUseCases::new(Arc::new(repo));
    
    let criteria = ProductSearchCriteria {
        manufacturer,
        device_type: None,
        certificate_id: None,
        specification_version: None,
        program_type: None,
        certification_date_from: None,
        certification_date_to: None,
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
) -> Result<Vec<crate::domain::integrated_product::Product>, String> {
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
