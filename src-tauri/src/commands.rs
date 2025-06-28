//! Tauri commands for Matter Certification domain
//! 
//! This module contains all Tauri commands that expose
//! backend functionality to the frontend. Each command uses
//! appropriate Use Cases and DTOs for clean separation of concerns.

use std::sync::Arc;
use tauri::State;
use crate::infrastructure::{
    database_connection::DatabaseConnection,
    repositories::{SqliteVendorRepository, SqliteProductRepository},
    http_client::HttpClientConfig,
    crawler::WebCrawler,
};
use crate::domain::session_manager::SessionManager;
use crate::application::{
    use_cases::{VendorUseCases, MatterProductUseCases, CrawlingUseCases},
    dto::{
        CreateVendorDto, UpdateVendorDto, VendorResponseDto,
        CreateProductDto, CreateMatterProductDto, ProductResponseDto, MatterProductResponseDto,
        ProductSearchDto, MatterProductFilterDto, ProductSearchResultDto,
        DatabaseSummaryDto, StartCrawlingDto, SessionStatusDto
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
// Web Crawling Commands
// ============================================================================

/// Start a new crawling session
#[tauri::command]
pub async fn start_crawling(
    dto: StartCrawlingDto,
    db: State<'_, DatabaseConnection>
) -> Result<String, String> {
    println!("🕷️  Starting crawling session: {}", dto.start_url);
    
    // Initialize repositories
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    
    // Initialize session manager
    let session_manager = Arc::new(SessionManager::new());
    
    // Initialize crawler
    let http_config = HttpClientConfig {
        max_requests_per_second: 2,
        ..Default::default()
    };
    
    let crawler = WebCrawler::new(http_config, session_manager.clone())
        .map_err(|e| format!("Failed to create crawler: {}", e))?;
    
    // Start crawling
    match crawler.start_crawling(dto.into()).await {
        Ok(session_id) => {
            println!("✅ Crawling session started: {}", session_id);
            Ok(session_id)
        },
        Err(e) => {
            println!("❌ Failed to start crawling: {}", e);
            Err(format!("Failed to start crawling: {}", e))
        }
    }
}

/// Get the status of a crawling session
#[tauri::command]
pub async fn get_crawling_status(
    session_id: String,
    db: State<'_, DatabaseConnection>
) -> Result<Option<SessionStatusDto>, String> {
    println!("📊 Getting status for session: {}", session_id);
    
    // Initialize repositories
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    
    // Initialize use cases
    let crawling_use_cases = CrawlingUseCases::new(
        product_repo,
        vendor_repo,
        Arc::new(SessionManager::new()),
    );
    
    match crawling_use_cases.get_session_status(&session_id).await {
        Ok(Some(status)) => {
            let dto = SessionStatusDto {
                session_id: session_id.clone(),
                status: format!("{:?}", status.status),
                progress: status.current_page,
                current_step: status.current_url.unwrap_or("N/A".to_string()),
                started_at: status.started_at.to_rfc3339(),
                last_updated: status.last_updated_at.to_rfc3339(),
            };
            Ok(Some(dto))
        },
        Ok(None) => {
            println!("⚠️  Session not found: {}", session_id);
            Ok(None)
        },
        Err(e) => {
            println!("❌ Failed to get session status: {}", e);
            Err(format!("Failed to get session status: {}", e))
        }
    }
}

/// Stop a running crawling session
#[tauri::command]
pub async fn stop_crawling(
    session_id: String,
    db: State<'_, DatabaseConnection>
) -> Result<String, String> {
    println!("⏹️  Stopping crawling session: {}", session_id);
    
    // Initialize repositories
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    
    // Initialize use cases
    let crawling_use_cases = CrawlingUseCases::new(
        product_repo,
        vendor_repo,
        Arc::new(SessionManager::new()),
    );
    
    match crawling_use_cases.complete_crawling(&session_id).await {
        Ok(_) => {
            println!("✅ Crawling session stopped: {}", session_id);
            Ok("Crawling session stopped successfully".to_string())
        },
        Err(e) => {
            println!("❌ Failed to stop crawling: {}", e);
            Err(format!("Failed to stop crawling: {}", e))
        }
    }
}

/// Pause a running crawling session
#[tauri::command]
pub async fn pause_crawling(
    session_id: String,
    _db: State<'_, DatabaseConnection>
) -> Result<String, String> {
    println!("⏸️  Pausing crawling session: {}", session_id);
    
    // Note: Session pausing would need to be implemented in SessionManager
    // For now, we'll return a placeholder response
    Ok(format!("Pause functionality not yet implemented for session: {}", session_id))
}

/// Resume a paused crawling session
#[tauri::command]
pub async fn resume_crawling(
    session_id: String,
    _db: State<'_, DatabaseConnection>
) -> Result<String, String> {
    println!("▶️  Resuming crawling session: {}", session_id);
    
    // Note: Session resuming would need to be implemented in SessionManager
    // For now, we'll return a placeholder response
    Ok(format!("Resume functionality not yet implemented for session: {}", session_id))
}

/// Get crawling session statistics
#[tauri::command]
pub async fn get_crawling_stats(
    _db: State<'_, DatabaseConnection>
) -> Result<serde_json::Value, String> {
    println!("📈 Getting crawling statistics");
    
    // Initialize session manager
    let session_manager = Arc::new(SessionManager::new());
    
    let stats = session_manager.get_session_stats().await;
    
    let stats_json = serde_json::json!({
        "active_sessions": stats.total_active_sessions,
        "sessions_by_status": stats.sessions_by_status
    });
    
    Ok(stats_json)
}

/// Get all active crawling sessions
#[tauri::command]
pub async fn get_active_crawling_sessions(
    _db: State<'_, DatabaseConnection>
) -> Result<Vec<serde_json::Value>, String> {
    println!("📋 Getting active crawling sessions");
    
    // Initialize session manager
    let session_manager = Arc::new(SessionManager::new());
    
    let sessions = session_manager.get_all_sessions().await;
    let active_sessions: Vec<_> = sessions.into_iter()
        .filter(|session| matches!(session.status, crate::domain::session_manager::SessionStatus::Running | crate::domain::session_manager::SessionStatus::Paused))
        .map(|session| serde_json::json!({
            "session_id": session.session_id,
            "status": format!("{:?}", session.status),
            "stage": format!("{:?}", session.stage),
            "pages_crawled": session.current_page,
            "max_pages": session.total_pages,
            "current_url": session.current_url,
            "start_time": session.started_at.to_rfc3339(),
            "errors": session.error_details
        }))
        .collect();
    
    Ok(active_sessions)
}

/// Get crawling session history
#[tauri::command]
pub async fn get_crawling_session_history(
    _db: State<'_, DatabaseConnection>
) -> Result<Vec<serde_json::Value>, String> {
    println!("📚 Getting crawling session history");
    
    // Initialize session manager
    let session_manager = Arc::new(SessionManager::new());
    
    let sessions = session_manager.get_all_sessions().await;
    let history: Vec<_> = sessions.into_iter()
        .map(|session| serde_json::json!({
            "session_id": session.session_id,
            "status": format!("{:?}", session.status),
            "stage": format!("{:?}", session.stage),
            "pages_crawled": session.current_page,
            "max_pages": session.total_pages,
            "current_url": session.current_url,
            "start_time": session.started_at.to_rfc3339(),
            "errors": session.error_details
        }))
        .collect();
    
    Ok(history)
}

/// Get enhanced crawling statistics for dashboard
#[tauri::command]
pub async fn get_enhanced_crawling_stats(
    _db: State<'_, DatabaseConnection>
) -> Result<serde_json::Value, String> {
    println!("📊 Getting enhanced crawling statistics");
    
    // Initialize session manager
    let session_manager = Arc::new(SessionManager::new());
    
    let sessions = session_manager.get_all_sessions().await;
    
    let total_sessions = sessions.len();
    let active_sessions = sessions.iter()
        .filter(|s| matches!(s.status, crate::domain::session_manager::SessionStatus::Running | crate::domain::session_manager::SessionStatus::Paused))
        .count();
    let completed_sessions = sessions.iter()
        .filter(|s| matches!(s.status, crate::domain::session_manager::SessionStatus::Completed))
        .count();
    let total_pages_crawled: u32 = sessions.iter()
        .map(|s| s.current_page)
        .sum();
    
    let success_rate = if total_sessions > 0 {
        completed_sessions as f64 / total_sessions as f64
    } else {
        0.0
    };
    
    let stats = serde_json::json!({
        "total_sessions": total_sessions,
        "active_sessions": active_sessions,
        "completed_sessions": completed_sessions,
        "total_pages_crawled": total_pages_crawled,
        "average_success_rate": success_rate
    });
    
    Ok(stats)
}

// ============================================================================
// Legacy/Example Commands (to be removed)
// ============================================================================

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
