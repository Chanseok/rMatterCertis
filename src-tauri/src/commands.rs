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
    
    match use_cases.get_vendor(&vendor_id).await {
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
    
    match use_cases.search_vendors(&name).await {
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
    dto: CreateMatterProductDto
) -> Result<MatterProductResponseDto, String> {
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    let use_cases = MatterProductUseCases::new(product_repo);
    
    match use_cases.create_matter_product(dto).await {
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
    
    let crawler = WebCrawler::new(http_config, session_manager.clone(), product_repo, vendor_repo)
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
// Product Results Management Commands  
// ============================================================================

/// Get all basic products from database
#[tauri::command]
pub async fn get_products(
    db: State<'_, DatabaseConnection>
) -> Result<Vec<ProductResponseDto>, String> {
    println!("📊 Getting all products");
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.get_all_products().await {
        Ok(products) => {
            println!("✅ Retrieved {} products", products.len());
            Ok(products)
        }
        Err(e) => {
            println!("❌ Failed to get products: {}", e);
            Err(format!("Failed to get products: {}", e))
        }
    }
}

/// Get all Matter products from database
#[tauri::command]
pub async fn get_matter_products(
    db: State<'_, DatabaseConnection>
) -> Result<Vec<MatterProductResponseDto>, String> {
    println!("📊 Getting all Matter products");
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.get_all_matter_products().await {
        Ok(products) => {
            println!("✅ Retrieved {} Matter products", products.len());
            Ok(products)
        }
        Err(e) => {
            println!("❌ Failed to get Matter products: {}", e);
            Err(format!("Failed to get Matter products: {}", e))
        }
    }
}

/// Search products with pagination and filters
#[tauri::command]
pub async fn search_products(
    search_dto: ProductSearchDto,
    db: State<'_, DatabaseConnection>
) -> Result<ProductSearchResultDto, String> {
    println!("🔍 Searching products with query: {:?}", search_dto.query);
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.search_products(search_dto).await {
        Ok(results) => {
            println!("✅ Found {} products", results.products.len());
            Ok(results)
        }
        Err(e) => {
            println!("❌ Failed to search products: {}", e);
            Err(format!("Failed to search products: {}", e))
        }
    }
}

/// Get products filtered by manufacturer
#[tauri::command]
pub async fn get_products_by_manufacturer(
    manufacturer: String,
    db: State<'_, DatabaseConnection>
) -> Result<Vec<MatterProductResponseDto>, String> {
    println!("🔍 Getting products by manufacturer: {}", manufacturer);
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.get_products_by_manufacturer(&manufacturer).await {
        Ok(products) => {
            println!("✅ Found {} products for manufacturer: {}", products.len(), manufacturer);
            Ok(products)
        }
        Err(e) => {
            println!("❌ Failed to get products by manufacturer: {}", e);
            Err(format!("Failed to get products by manufacturer: {}", e))
        }
    }
}

/// Get Matter products with advanced filtering
#[tauri::command]
pub async fn filter_matter_products(
    filter_dto: MatterProductFilterDto,
    db: State<'_, DatabaseConnection>
) -> Result<Vec<MatterProductResponseDto>, String> {
    println!("🔍 Filtering Matter products: {:?}", filter_dto);
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.get_all_matter_products().await {
        Ok(products) => {
            // Apply filtering if any filters are provided
            let filtered_products = if filter_dto.manufacturer.is_some() || 
                                     filter_dto.device_type.is_some() || 
                                     filter_dto.vid.is_some() {
                products.into_iter().filter(|p| {
                    let manufacturer_match = filter_dto.manufacturer.as_ref()
                        .map_or(true, |m| p.manufacturer.as_ref().map_or(false, |pm| pm.contains(m)));
                    let device_type_match = filter_dto.device_type.as_ref()
                        .map_or(true, |d| p.device_type.as_ref().map_or(false, |pd| pd.contains(d)));
                    let vid_match = filter_dto.vid.as_ref()
                        .map_or(true, |v| p.vid.as_ref().map_or(false, |pv| pv.contains(v)));
                    
                    manufacturer_match && device_type_match && vid_match
                }).collect()
            } else {
                products
            };
            
            println!("✅ Found {} filtered Matter products", filtered_products.len());
            Ok(filtered_products)
        }
        Err(e) => {
            println!("❌ Failed to filter Matter products: {}", e);
            Err(format!("Failed to filter Matter products: {}", e))
        }
    }
}

/// Get database summary with product counts
#[tauri::command]
pub async fn get_database_summary(
    db: State<'_, DatabaseConnection>
) -> Result<DatabaseSummaryDto, String> {
    println!("📊 Getting database summary");
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.get_database_summary().await {
        Ok(summary) => {
            println!("✅ Database summary - Products: {}, Matter Products: {}, Vendors: {}", 
                summary.total_products, summary.total_matter_products, summary.total_vendors);
            Ok(summary)
        }
        Err(e) => {
            println!("❌ Failed to get database summary: {}", e);
            Err(format!("Failed to get database summary: {}", e))
        }
    }
}

/// Get recently added products
#[tauri::command]
pub async fn get_recent_products(
    limit: Option<u32>,
    db: State<'_, DatabaseConnection>
) -> Result<Vec<MatterProductResponseDto>, String> {
    let limit = limit.unwrap_or(10);
    println!("📊 Getting {} recent products", limit);
    
    let pool = db.pool();
    let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
    let product_use_cases = MatterProductUseCases::new(product_repo);
    
    match product_use_cases.get_recent_products(limit).await {
        Ok(products) => {
            println!("✅ Retrieved {} recent products", products.len());
            Ok(products)
        }
        Err(e) => {
            println!("❌ Failed to get recent products: {}", e);
            Err(format!("Failed to get recent products: {}", e))
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
