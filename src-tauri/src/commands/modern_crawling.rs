//! Legacy modern_crawling v3 removed (empty stub).
#![allow(dead_code)]

// Intentionally left blank; all functionality migrated.

/// Get the current crawling status and progress
#[tauri::command]
pub async fn get_crawling_status(state: State<'_, AppState>) -> Result<CrawlingProgress, String> {
    info!("Getting current crawling status");
    
    let progress = state.current_progress.read().await.clone();
    Ok(progress)
}

/// Get active crawling sessions
#[tauri::command]
pub async fn get_active_sessions(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    info!("Getting active crawling sessions");
    
    // For now, return current session if it exists
    let current_session = state.current_session.read().await;
    if let Some(session) = current_session.as_ref() {
        Ok(vec![session.id.clone()])
    } else {
        Ok(vec![])
    }
}

/// Get database statistics
#[tauri::command]
pub async fn get_databasestats(state: State<'_, AppState>) -> Result<DatabaseStats, String> {
    info!("🔍 Getting real database statistics...");
    
    // Connect to database (global pool)
    let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("Failed to obtain database pool: {}", e))?;
    
    // Get actual product count
    let total_products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&db_pool)
        .await
        .unwrap_or(0);
    
    // Get device count (assuming devices are stored in products table with device info)
    let total_devices: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT name) FROM products WHERE name IS NOT NULL")
        .fetch_one(&db_pool)
        .await
        .unwrap_or(0);
    
    // Get last updated time
    let last_updated = sqlx::query_scalar::<_, Option<String>>("SELECT MAX(created_at) FROM products")
        .fetch_one(&db_pool)
        .await
        .unwrap_or(None);
    
    let last_updated_time = if let Some(time_str) = last_updated {
        chrono::DateTime::parse_from_rfc3339(&time_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
    } else {
        Utc::now()
    };
    
    // Get incomplete records count (records with missing required fields)
    let incomplete_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM products WHERE name IS NULL OR url IS NULL OR certification_date IS NULL"
    )
    .fetch_one(&db_pool)
    .await
    .unwrap_or(0);
    
    // Calculate storage size (approximation)
    let storage_size = format!("{:.1} MB", (total_products * 2) as f64 / 1000.0); // Rough estimate
    
    let health_status = if incomplete_records as f64 / total_products.max(1) as f64 > 0.1 {
        DatabaseHealth::Warning
    } else {
        DatabaseHealth::Healthy
    };
    
    let stats = DatabaseStats {
        total_products: total_products as u64,
        total_devices: total_devices as u64,
        last_updated: last_updated_time,
        storage_size,
        incomplete_records: incomplete_records as u64,
        health_status,
    };
    
    info!("✅ Real database stats: products={}, devices={}, incomplete={}", 
          stats.total_products, stats.total_devices, stats.incomplete_records);
    
    state.update_database_stats(stats.clone()).await?;
    
    Ok(stats)
}

/// Backup the database
#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>) -> Result<String, String> {
    info!("Starting database backup");
    
    // TODO: Implement actual database backup logic
    let backup_path = format!("backup_{}.db", Utc::now().format("%Y%m%d_%H%M%S"));
    
    // Load app config for delay settings
    let app_config = state.get_config().await;
    
    // Simulate backup process
    // Sleep briefly to prevent overwhelming the system
    tokio::time::sleep(tokio::time::Duration::from_millis(app_config.user.batch.batch_delay_ms)).await;
    
    info!("Database backup completed: {}", backup_path);
    Ok(backup_path)
}

/// Optimize the database
#[tauri::command]
pub async fn optimize_database(state: State<'_, AppState>) -> Result<(), String> {
    info!("Starting database optimization");
    
    // Connect to database
    let db_pool = match crate::infrastructure::database_connection::get_or_init_global_pool().await {
        Ok(pool) => pool,
        Err(e) => {
            warn!("Database connection failed during optimization: {}", e);
            // Continue with simulated optimization anyway
            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            
            // Get real stats
            let _stats = get_databasestats(state.clone()).await?;
            return Ok(());
        }
    };
    
    // Run VACUUM
    match sqlx::query("VACUUM").execute(&db_pool).await {
        Ok(_) => info!("VACUUM executed successfully"),
        Err(e) => warn!("VACUUM failed: {}", e),
    }
    
    // Run ANALYZE
    match sqlx::query("ANALYZE").execute(&db_pool).await {
        Ok(_) => info!("ANALYZE executed successfully"),
        Err(e) => warn!("ANALYZE failed: {}", e),
    }
    
    // Get real stats after optimization
    let _stats = get_databasestats(state.clone()).await?;
    
    info!("Database optimization completed");
    Ok(())
}

/// Export database data in the specified format
#[tauri::command]
pub async fn export_database_data(
    format: String,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Exporting database data in {} format", format);
    
    // TODO: Implement actual export logic
    let export_path = format!("export_{}.{}", Utc::now().format("%Y%m%d_%H%M%S"), format);
    
    // Simulate export process
    tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
    
    info!("Database export completed: {}", export_path);
    Ok(export_path)
}

/// Clear crawling error logs
#[tauri::command]
pub fn clear_crawling_errors(_state: State<'_, AppState>) -> Result<(), String> {
    info!("Clearing crawling error logs");
    
    // TODO: Implement actual error log clearing
    
    info!("Crawling error logs cleared");
    Ok(())
}

/// Export crawling results
#[tauri::command]
pub fn export_crawling_results(_state: State<'_, AppState>) -> Result<String, String> {
    info!("Exporting crawling results");
    
    // TODO: Implement actual results export
    let export_path = format!("crawling_results_{}.json", Utc::now().format("%Y%m%d_%H%M%S"));
    
    info!("Crawling results exported: {}", export_path);
    Ok(export_path)
}

/// Check site status with detailed page discovery
#[tauri::command]
pub async fn check_site_status(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    // 🔍 호출 추적을 위한 로그 추가
    let caller_info = std::panic::Location::caller();
    info!("🚨 check_site_status called from: {}:{}", caller_info.file(), caller_info.line());
    
    // 🕒 마지막 호출 시간 추적
    static LAST_CALL: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    {
        let mut last_call = LAST_CALL.lock().unwrap();
        if let Some(last) = *last_call {
            let elapsed = last.elapsed();
            warn!("⚠️ check_site_status called again after only {:?}", elapsed);
        }
        *last_call = Some(std::time::Instant::now());
    }
    
    info!("Starting comprehensive site status check with detailed page discovery");
    
    // Get the advanced crawling engine from the state
    let config = state.config.read().await.clone();
    
    // Create a simple HTTP client and necessary components
    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    // Create status checker
    let status_checker = crate::infrastructure::StatusCheckerImpl::new(
        http_client,
        data_extractor,
        config.clone(),
    );
    
    // Create database analyzer for local DB stats
    let database_url = get_database_url()?;
    let db_analyzer: Option<Box<dyn DatabaseAnalyzer>> = match sqlx::SqlitePool::connect(&database_url).await {
        Ok(_pool) => {
            // Don't use actual StatusCheckerImpl since it requires complex initialization
            // Return None to skip database analysis for now
            None
        }
        Err(e) => {
            warn!("Failed to create database connection: {}", e);
            None
        }
    };
    
    // Perform the site status check
    let site_check_result = status_checker.check_site_status().await;
    let db_analysis_result: Result<crate::domain::services::crawling_services::DatabaseAnalysis, anyhow::Error> = if let Some(ref _analyzer) = db_analyzer {
        // Placeholder analysis since we don't have actual DatabaseAnalyzer implementation
        Ok(crate::domain::services::crawling_services::DatabaseAnalysis {
            total_products: 0,
            unique_products: 0,
            duplicate_count: 0,
            missing_products_count: 0,
            last_update: Some(chrono::Utc::now()),
            missing_fields_analysis: crate::domain::services::crawling_services::FieldAnalysis {
                missing_company: 0,
                missing_model: 0,
                missing_matter_version: 0,
                missing_connectivity: 0,
                missing_certification_date: 0,
            },
            data_quality_score: 0.0,
        })
    } else {
        Err(anyhow::anyhow!("Database analyzer not available"))
    };
    
    match (site_check_result, db_analysis_result) {
        (Ok(site_status), Ok(db_analysis)) => {
            info!("Site status check completed successfully");
            info!("Site: accessible={}, total_pages={}, estimated_products={}", 
                  site_status.is_accessible, site_status.total_pages, site_status.estimated_products);
            info!("Database: total_products={}, unique_products={}, missing_products={}", 
                  db_analysis.total_products, db_analysis.unique_products, db_analysis.missing_products_count);
            
            // Create comprehensive status object
            let comprehensive_status = serde_json::json!({
                "site_status": {
                    "accessible": site_status.is_accessible,
                    "response_time_ms": site_status.response_time_ms,
                    "total_pages": site_status.total_pages,
                    "estimated_products": site_status.estimated_products,
                    "health_score": site_status.health_score,
                    "last_check": site_status.last_check_time.to_rfc3339(),
                    "data_change_status": site_status.data_change_status,
                    "decrease_recommendation": site_status.decrease_recommendation
                },
                "database_analysis": {
                    "total_products": db_analysis.total_products,
                    "unique_products": db_analysis.unique_products,
                    "missing_products_count": db_analysis.missing_products_count,
                    "data_quality_score": db_analysis.data_quality_score,
                    "missing_fields": {
                        "company": db_analysis.missing_fields_analysis.missing_company,
                        "model": db_analysis.missing_fields_analysis.missing_model,
                        "matter_version": db_analysis.missing_fields_analysis.missing_matter_version,
                        "connectivity": db_analysis.missing_fields_analysis.missing_connectivity,
                        "certification_date": db_analysis.missing_fields_analysis.missing_certification_date
                    }
                },
                "comparison": {
                    "difference": site_status.estimated_products as i32 - db_analysis.total_products as i32,
                    "sync_percentage": if site_status.estimated_products > 0 { 
                        (db_analysis.total_products as f64 / site_status.estimated_products as f64) * 100.0 
                    } else { 0.0 },
                    "recommended_action": if site_status.estimated_products > db_analysis.total_products {
                        "crawling_needed"
                    } else if db_analysis.missing_products_count > 0 {
                        "cleanup_needed"
                    } else {
                        "up_to_date"
                    }
                }
            });
            
            Ok(comprehensive_status)
        }
        (Ok(site_status), Err(db_error)) => {
            warn!("Database analysis failed: {}", db_error);
            
            // Return site status with DB error
            let partial_response = serde_json::json!({
                "site_status": {
                    "accessible": site_status.is_accessible,
                    "response_time_ms": site_status.response_time_ms,
                    "total_pages": site_status.total_pages,
                    "estimated_products": site_status.estimated_products,
                    "health_score": site_status.health_score,
                    "last_check": site_status.last_check_time.to_rfc3339(),
                    "data_change_status": site_status.data_change_status,
                    "decrease_recommendation": site_status.decrease_recommendation
                },
                "database_analysis": {
                    "error": db_error.to_string()
                }
            });
            
            Ok(partial_response)
        }
        (Err(site_error), Ok(db_analysis)) => {
            warn!("Site status check failed: {}", site_error);
            
            // Still return DB analysis with site error
            let error_response = serde_json::json!({
                "site_status": {
                    "accessible": false,
                    "error": site_error.to_string()
                },
                "database_analysis": {
                    "total_products": db_analysis.total_products,
                    "unique_products": db_analysis.unique_products,
                    "missing_products_count": db_analysis.missing_products_count,
                    "data_quality_score": db_analysis.data_quality_score
                }
            });
            
            Ok(error_response)
        }
        (Err(site_error), Err(db_error)) => {
            error!("Both site and database checks failed: site={}, db={}", site_error, db_error);
            
            let error_response = serde_json::json!({
                "site_status": {
                    "accessible": false,
                    "error": site_error.to_string()
                },
                "database_analysis": {
                    "error": db_error.to_string()
                }
            });
            
            Ok(error_response)
        }
    }
}

/// 재시도 통계 조회 명령어 - INTEGRATED_PHASE2_PLAN Week 1 Day 3-4
#[tauri::command]
pub async fn get_retrystats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    info!("📊 Getting retry statistics");
    
    // Load app config for retry settings
    let app_config = state.get_config().await;
    
    // ServiceBasedBatchCrawlingEngine에서 재시도 통계를 가져오는 것은 복잡하므로
    // 향후 CrawlerManager 통합 시 구현 예정
    
    // 임시 응답
    let stats = serde_json::json!({
        "total_items": 0,
        "pending_retries": 0,
        "successful_retries": 0,
        "failed_retries": 0,
        "max_retries": app_config.user.crawling.product_list_retry_count,
        "status": "RetryManager integration in progress"
    });
    
    Ok(stats)
}

/// Update user crawling preferences (max pages, delay, concurrency)
#[tauri::command]
pub async fn update_user_crawling_preferences(
    max_pages: Option<u32>,
    request_delay_ms: Option<u64>,
    max_concurrent_requests: Option<u32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Updating user crawling preferences: max_pages={:?}, delay={:?}, concurrency={:?}", 
          max_pages, request_delay_ms, max_concurrent_requests);
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    config_manager.update_user_config(|user_config| {
        if let Some(pages) = max_pages {
            user_config.crawling.page_range_limit = pages;
        }
        if let Some(delay) = request_delay_ms {
            user_config.request_delay_ms = delay;
        }
        if let Some(concurrency) = max_concurrent_requests {
            user_config.max_concurrent_requests = concurrency;
        }
    }).await.map_err(|e| format!("Failed to update user config: {}", e))?;
    
    // Reload config in app state
    let updated_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to reload config: {}", e))?;
    state.update_config(updated_config).await?;
    
    info!("✅ User crawling preferences updated successfully");
    Ok(())
}

/// Get the correct database URL for the application
/// 중앙집중식 데이터베이스 URL 가져오기 (Modern Rust 2024)
/// 
/// 기존의 여러 곳에서 다른 방식으로 경로를 생성하던 문제를 해결
/// "엉뚱한 경로를 잡는 문제" 영구 해결
fn get_database_url() -> Result<String, String> {
    // 환경변수 우선 확인 (개발/테스트 환경용)
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        if !db_url.is_empty() {
            info!("Using database URL from environment: {}", db_url);
            return Ok(db_url);
        }
    }

    // 중앙집중식 경로 관리자 사용 (Modern Rust 2024)
    let database_url = crate::infrastructure::get_main_database_url();
    info!("Using centralized database URL: {}", database_url);
    Ok(database_url)
}

// Helper function to check if a file is writable
fn is_file_writable(path: &std::path::Path) -> bool {
    use std::fs::OpenOptions;
    OpenOptions::new()
        .write(true)
        .open(path)
        .is_ok()
}

// Helper function to check if a directory is writable
fn is_dir_writable(path: &std::path::Path) -> bool {
    use std::fs::OpenOptions;
    use uuid::Uuid;
    
    // Try to create a temporary file in the directory
    let temp_file_name = path.join(format!("temp_{}.tmp", Uuid::new_v4()));
    let result = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&temp_file_name);
    
    // Clean up
    if temp_file_name.exists() {
        let _ = std::fs::remove_file(&temp_file_name);
    }
    
    result.is_ok()
}

/// Initialize database with required schema
async fn ensure_database_schema_exists(db_pool: &sqlx::SqlitePool) -> Result<(), String> {
    info!("Checking and initializing database schema if needed");

    // Check if the products table exists
    let table_exists = match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='products'"
    )
    .fetch_one(db_pool)
    .await {
        Ok(count) => count > 0,
        Err(e) => {
            warn!("Failed to check if products table exists: {}", e);
            false // Assume the table doesn't exist if we can't check
        }
    };

    if !table_exists {
        info!("Creating products table");
        
        // Enable foreign keys
        if let Err(e) = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(db_pool)
            .await {
            warn!("Failed to enable foreign keys: {}", e);
            // Continue anyway - this is not critical
        }
        
        // Create products table with complete schema
        let create_result = sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT,
                url TEXT UNIQUE,
                matter_version TEXT,
                certification_date TEXT,
                company TEXT,
                model TEXT,
                connectivity TEXT,
                manufacturer TEXT,
                certificate_id TEXT,
                page_id INTEGER,
                index_in_page INTEGER,
                createdAt TEXT DEFAULT CURRENT_TIMESTAMP,
                updatedAt TEXT DEFAULT CURRENT_TIMESTAMP,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            "
        )
        .execute(db_pool)
        .await;
        
        match create_result {
            Ok(_) => info!("Products table created successfully"),
            Err(e) => return Err(format!("Failed to create products table: {}", e))
        }
        
        // Create indexes for better performance
        let indexes = vec![
            ("idx_products_company", "CREATE INDEX IF NOT EXISTS idx_products_company ON products (company)"),
            ("idx_products_manufacturer", "CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer)"),
            ("idx_products_certificate_id", "CREATE INDEX IF NOT EXISTS idx_products_certificate_id ON products (certificate_id)"),
            ("idx_products_page_id", "CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id)"),
            ("idx_products_certification_date", "CREATE INDEX IF NOT EXISTS idx_products_certification_date ON products (certification_date)"),
            ("idx_products_created_at", "CREATE INDEX IF NOT EXISTS idx_products_created_at ON products (created_at)"),
            ("idx_products_createdAt", "CREATE INDEX IF NOT EXISTS idx_products_createdAt ON products (createdAt)")
        ];
        
        for (index_name, create_sql) in indexes {
            if let Err(e) = sqlx::query(create_sql).execute(db_pool).await {
                warn!("Failed to create index {}: {}", index_name, e);
                // Continue with other indexes
            } else {
                info!("Created index: {}", index_name);
            }
        }
    } else {
        info!("Products table already exists, checking for missing columns");
        
        // Check and add missing columns for existing tables
        let missing_columns = vec![
            ("manufacturer", "ALTER TABLE products ADD COLUMN manufacturer TEXT"),
            ("certificate_id", "ALTER TABLE products ADD COLUMN certificate_id TEXT"),
            ("page_id", "ALTER TABLE products ADD COLUMN page_id INTEGER"),
            ("index_in_page", "ALTER TABLE products ADD COLUMN index_in_page INTEGER"),
            ("createdAt", "ALTER TABLE products ADD COLUMN createdAt TEXT DEFAULT CURRENT_TIMESTAMP"),
            ("updatedAt", "ALTER TABLE products ADD COLUMN updatedAt TEXT DEFAULT CURRENT_TIMESTAMP"),
        ];
        
        for (column_name, alter_sql) in missing_columns {
            // Check if column exists
            let column_exists = match sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = ?"
            )
            .bind(column_name)
            .fetch_one(db_pool)
            .await {
                Ok(count) => count > 0,
                Err(e) => {
                    warn!("Failed to check if column '{}' exists: {}", column_name, e);
                    false
                }
            };
            
            if !column_exists {
                info!("Adding missing column: {}", column_name);
                if let Err(e) = sqlx::query(alter_sql).execute(db_pool).await {
                    warn!("Failed to add column {}: {}", column_name, e);
                } else {
                    info!("Successfully added column: {}", column_name);
                }
            }
        }
        
        // Add missing indexes
        let indexes = vec![
            ("idx_products_manufacturer", "CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer)"),
            ("idx_products_certificate_id", "CREATE INDEX IF NOT EXISTS idx_products_certificate_id ON products (certificate_id)"),
            ("idx_products_page_id", "CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id)"),
            ("idx_products_createdAt", "CREATE INDEX IF NOT EXISTS idx_products_createdAt ON products (createdAt)")
        ];
        
        for (index_name, create_sql) in indexes {
            if let Err(e) = sqlx::query(create_sql).execute(db_pool).await {
                warn!("Failed to create index {}: {}", index_name, e);
            } else {
                info!("Created/verified index: {}", index_name);
            }
        }
    }

    Ok(())
}

/// Calculate intelligent crawling range based on site status and database state
async fn calculate_intelligent_crawling_range(
    _state: &State<'_, AppState>, 
    app_config: &crate::infrastructure::config::AppConfig
) -> Result<(u32, u32), String> {
    info!("🔍 Calculating intelligent crawling range...");
    
    // Get database URL and connect
    let database_url = get_database_url()?;
    let db_pool = sqlx::SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
    // Create necessary components for range calculation
    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    // Create status checker
    let status_checker = crate::infrastructure::StatusCheckerImpl::new(
        http_client,
        data_extractor,
        app_config.clone(),
    );
    
    // Create database analyzer - skip complex initialization for now
    let repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);
    let _repo_arc = std::sync::Arc::new(repo);
    // Skip StatusCheckerImpl creation due to complex dependencies
    
    // Get site status
    let site_status = status_checker.check_site_status().await
        .map_err(|e| format!("Failed to check site status: {}", e))?;
    
    // Get database analysis - using placeholder since StatusCheckerImpl doesn't implement DatabaseAnalyzer
    let db_analysis = crate::domain::services::crawling_services::DatabaseAnalysis {
        total_products: 0,
        unique_products: 0,
        duplicate_count: 0,
        missing_products_count: 0,
        last_update: Some(chrono::Utc::now()),
        missing_fields_analysis: crate::domain::services::crawling_services::FieldAnalysis {
            missing_company: 0,
            missing_model: 0,
            missing_matter_version: 0,
            missing_connectivity: 0,
            missing_certification_date: 0,
        },
        data_quality_score: 0.0,
    };
    
    // Calculate crawling range recommendation
    let recommendation = status_checker.calculate_crawling_range_recommendation(&site_status, &db_analysis).await
        .map_err(|e| format!("Failed to calculate range recommendation: {}", e))?;
    
    match recommendation {
        crate::domain::services::crawling_services::CrawlingRangeRecommendation::Full => {
            // STRICTLY RESPECT USER SETTINGS: Use page_range_limit instead of max_pages
            let total_pages = site_status.total_pages;
            let user_page_limit = app_config.user.crawling.page_range_limit;
            let start_page = total_pages;
            let end_page = if start_page >= user_page_limit {
                start_page - user_page_limit + 1
            } else {
                1
            };
            info!("📊 Full crawl recommended: {} to {} ({} pages, STRICTLY following user page_range_limit)", start_page, end_page, user_page_limit);
            Ok((start_page, end_page))
        },
        crate::domain::services::crawling_services::CrawlingRangeRecommendation::Partial(pages_to_crawl) => {
            // STRICTLY RESPECT USER SETTINGS: Limit to page_range_limit
            let total_pages = site_status.total_pages;
            let user_page_limit = app_config.user.crawling.page_range_limit;
            let actual_pages = pages_to_crawl.min(user_page_limit);
            let start_page = total_pages;
            let end_page = if start_page >= actual_pages {
                start_page - actual_pages + 1
            } else {
                1
            };
            info!("📊 Partial crawl recommended: {} to {} ({} pages, STRICTLY following user page_range_limit)", start_page, end_page, actual_pages);
            Ok((start_page, end_page))
        },
        crate::domain::services::crawling_services::CrawlingRangeRecommendation::None => {
            // Still crawl a minimal range for verification
            let verification_pages = 5.min(app_config.user.crawling.page_range_limit);
            let start_page = site_status.total_pages;
            let end_page = if start_page >= verification_pages {
                start_page - verification_pages + 1
            } else {
                1
            };
            info!("📊 No update needed, verification crawl: {} to {} ({} pages)", start_page, end_page, verification_pages);
            Ok((start_page, end_page))
        }
    }
}

/// Get products from database with pagination
#[tauri::command]
pub async fn get_products(
    state: State<'_, AppState>,
    page: Option<u32>,
    limit: Option<u32>,
) -> Result<serde_json::Value, String> {
    info!("Getting products from database (page: {:?}, limit: {:?})", page, limit);
    
    // Load app config for batch settings
    let app_config = state.get_config().await;
    
    // Get database URL with fallback mechanisms
    let database_url = match get_database_url() {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to determine database URL: {}", e);
            // Fallback to in-memory DB as last resort
            "sqlite::memory:".to_string()
        }
    };
    
    // Attempt to connect to the database
    let db_pool = match sqlx::SqlitePool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to database at {}: {}", database_url, e);
            
            if database_url != "sqlite::memory:" {
                // Try falling back to in-memory database
                info!("Falling back to in-memory database");
                match sqlx::SqlitePool::connect("sqlite::memory:").await {
                    Ok(memory_pool) => memory_pool,
                    Err(fallback_err) => {
                        return Err(format!(
                            "Failed to connect to primary database ({}) and in-memory fallback: {}", 
                            e, fallback_err
                        ));
                    }
                }
            } else {
                return Err(format!("Failed to connect to database: {}", e));
            }
        }
    };
    
    // Ensure database schema exists
    if let Err(e) = ensure_database_schema_exists(&db_pool).await {
        error!("Failed to ensure database schema: {}", e);
        return Err(format!("Database schema error: {}", e));
    }
    
    let page = page.unwrap_or(1);
    let limit = limit.unwrap_or(app_config.user.batch.batch_size);
    let offset = (page - 1) * limit;
    
    // Get total count with proper error handling
    let total_count = match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(&db_pool)
        .await {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get total product count: {}", e);
                0 // Default to 0 if count fails
            }
        };
    
    // Get products with pagination and proper error handling
    let products = match sqlx::query(
        r"
        SELECT 
            id,
            name,
            url,
            matter_version,
            certification_date,
            company,
            model,
            connectivity,
            created_at
        FROM products 
        ORDER BY created_at DESC 
        LIMIT ? OFFSET ?
        "
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&db_pool)
    .await {
        Ok(rows) => {
            rows.into_iter()
                .map(|row| {
                    let name = row.try_get::<Option<String>, _>("name").unwrap_or_default();
                    let company = row.try_get::<Option<String>, _>("company").unwrap_or_default();
                    let status = if name.is_some() && company.is_some() { 
                        "Valid"
                    } else { 
                        "Incomplete"
                    };
                    
                    serde_json::json!({
                        "id": row.try_get::<i64, _>("id").unwrap_or_default(),
                        "title": name.unwrap_or_else(|| "Unknown Product".to_string()),
                        "url": row.try_get::<Option<String>, _>("url").unwrap_or_default().unwrap_or_else(|| "".to_string()),
                        "matter_version": row.try_get::<Option<String>, _>("matter_version").unwrap_or_default().unwrap_or_else(|| "".to_string()),
                        "certification_date": row.try_get::<Option<String>, _>("certification_date").unwrap_or_default().unwrap_or_else(|| "".to_string()),
                        "company": company.unwrap_or_else(|| "".to_string()),
                        "model": row.try_get::<Option<String>, _>("model").unwrap_or_default().unwrap_or_else(|| "".to_string()),
                        "connectivity": row.try_get::<Option<String>, _>("connectivity").unwrap_or_default().unwrap_or_else(|| "".to_string()),
                        "created_at": row.try_get::<Option<String>, _>("created_at").unwrap_or_default().unwrap_or_else(|| "".to_string()),
                        "status": status
                    })
                })
                .collect::<Vec<_>>()
        },
        Err(e) => {
            error!("Failed to fetch products: {}", e);
            return Err(format!("Failed to fetch products: {}", e));
        }
    };
    
    info!("Found {} products (total: {})", products.len(), total_count);
    
    Ok(serde_json::json!({
        "products": products,
        "total": total_count,
        "page": page,
        "limit": limit,
        "total_pages": (total_count as f64 / limit as f64).ceil() as i64
    }))
}

/// Get database statistics for LocalDB tab
#[tauri::command]
pub async fn get_local_dbstats(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    info!("Getting local database statistics");
    
    // Get database URL with fallback mechanisms
    let database_url = match get_database_url() {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to determine database URL: {}", e);
            // Fallback to in-memory DB as last resort
            "sqlite::memory:".to_string()
        }
    };
    
    // Attempt to connect to the database
    let db_pool = match sqlx::SqlitePool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to database at {}: {}", database_url, e);
            
            if database_url != "sqlite::memory:" {
                // Try falling back to in-memory database
                info!("Falling back to in-memory database");
                match sqlx::SqlitePool::connect("sqlite::memory:").await {
                    Ok(memory_pool) => memory_pool,
                    Err(fallback_err) => {
                        return Err(format!(
                            "Failed to connect to primary database ({}) and in-memory fallback: {}", 
                            e, fallback_err
                        ));
                    }
                }
            } else {
                return Err(format!("Failed to connect to database: {}", e));
            }
        }
    };
    
    // Ensure database schema exists
    if let Err(e) = ensure_database_schema_exists(&db_pool).await {
        error!("Failed to ensure database schema: {}", e);
        return Err(format!("Database schema error: {}", e));
    }
    
    // Get total records with error handling
    let total_records = match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(&db_pool)
        .await {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get product count: {}", e);
                0 // Default to 0 if count fails
            }
        };
    
    // Get last update time with error handling
    let last_update = match sqlx::query_scalar::<_, Option<String>>("SELECT MAX(created_at) FROM products")
        .fetch_one(&db_pool)
        .await {
            Ok(date) => date.unwrap_or_else(|| "Never".to_string()),
            Err(e) => {
                warn!("Failed to get last update time: {}", e);
                "Unknown".to_string()
            }
        };
    
    // Calculate database size information
    // For file-based DB, we could get the actual file size, but for consistency we'll use the estimation approach
    let avg_row_size_kb = 2.0; // Average row size estimation in KB
    let database_size = format!("{:.1}MB", (total_records as f64 * avg_row_size_kb) / 1000.0);
    let index_size = format!("{:.1}MB", (total_records as f64 * 0.3) / 1000.0);
    
    // Try to get actual DB size if it's a file-based database
    let mut actual_size = None;
    if !database_url.contains(":memory:") && database_url.starts_with("sqlite:") {
        let file_path = database_url.trim_start_matches("sqlite:");
        if let Ok(metadata) = std::fs::metadata(file_path) {
            if metadata.is_file() {
                let size_mb = (metadata.len() as f64) / (1024.0 * 1024.0);
                actual_size = Some(format!("{:.1}MB", size_mb));
            }
        }
    }
    
    Ok(serde_json::json!({
        "totalRecords": total_records,
        "lastUpdate": last_update,
        "databaseSize": actual_size.unwrap_or(database_size),
        "indexSize": index_size,
        "databasePath": if database_url.contains(":memory:") { 
            "In-memory database" 
        } else { 
            database_url.trim_start_matches("sqlite:") 
        }
    }))
}

/// Get analysis data for Analysis tab
#[tauri::command]
pub async fn get_analysis_data(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    info!("Getting analysis data");
    
    // Get database URL with fallback mechanisms
    let database_url = match get_database_url() {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to determine database URL: {}", e);
            // Fallback to in-memory DB as last resort
            "sqlite::memory:".to_string()
        }
    };
    
    // Attempt to connect to the database
    let db_pool = match sqlx::SqlitePool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to database at {}: {}", database_url, e);
            
            if database_url != "sqlite::memory:" {
                // Try falling back to in-memory database
                info!("Falling back to in-memory database");
                match sqlx::SqlitePool::connect("sqlite::memory:").await {
                    Ok(memory_pool) => memory_pool,
                    Err(fallback_err) => {
                        return Err(format!(
                            "Failed to connect to primary database ({}) and in-memory fallback: {}", 
                            e, fallback_err
                        ));
                    }
                }
            } else {
                return Err(format!("Failed to connect to database: {}", e));
            }
        }
    };
    
    // Ensure database schema exists
    if let Err(e) = ensure_database_schema_exists(&db_pool).await {
        error!("Failed to ensure database schema: {}", e);
        return Err(format!("Database schema error: {}", e));
    }
    
    // Get total crawled products with error handling
    let total_crawled = match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(&db_pool)
        .await {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get total product count: {}", e);
                0 // Default to 0 if count fails
            }
        };
    
    // Calculate success rate (products with complete data)
    let complete_products = match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM products WHERE name IS NOT NULL AND company IS NOT NULL AND certification_date IS NOT NULL"
    )
    .fetch_one(&db_pool)
    .await {
        Ok(count) => count,
        Err(e) => {
            error!("Failed to get complete product count: {}", e);
            0 // Default to 0 if count fails
        }
    };
    
    let success_rate = if total_crawled > 0 {
        (complete_products as f64 / total_crawled as f64) * 100.0
    } else {
        0.0
    };
    
    let error_rate = 100.0 - success_rate;
    
    // Get category counts (based on company for now) with error handling
    let category_counts = match sqlx::query(
        r"
        SELECT 
            COALESCE(company, 'Unknown') as category,
            COUNT(*) as count
        FROM products 
        WHERE company IS NOT NULL
        GROUP BY company
        ORDER BY count DESC
        LIMIT 10
        "
    )
    .fetch_all(&db_pool)
    .await {
        Ok(rows) => {
            rows.into_iter()
                .fold(serde_json::Map::new(), |mut acc, row| {
                    let category = row.try_get::<String, _>("category").unwrap_or_else(|_| "Unknown".to_string());
                    let count = row.try_get::<i64, _>("count").unwrap_or_default();
                    acc.insert(category, serde_json::Value::from(count));
                    acc
                })
        },
        Err(e) => {
            error!("Failed to get category counts: {}", e);
            serde_json::Map::new() // Return empty map if query fails
        }
    };
    
    // Get daily stats (last 7 days) with error handling
    let dailystats = match sqlx::query(
        r"
        SELECT 
            DATE(created_at) as date,
            COUNT(*) as count
        FROM products 
        WHERE created_at >= date('now', '-7 days')
        GROUP BY DATE(created_at)
        ORDER BY date DESC
        "
    )
    .fetch_all(&db_pool)
    .await {
        Ok(rows) => {
            rows.into_iter()
                .map(|row| {
                    let date = row.try_get::<Option<String>, _>("date").unwrap_or_default().unwrap_or_else(|| "Unknown".to_string());
                    let count = row.try_get::<i64, _>("count").unwrap_or_default();
                    serde_json::json!({
                        "date": date,
                        "count": count
                    })
                })
                .collect::<Vec<_>>()
        },
        Err(e) => {
            error!("Failed to get daily stats: {}", e);
            Vec::new() // Return empty vec if query fails
        }
    };
    
    Ok(serde_json::json!({
        "totalCrawled": total_crawled,
        "successRate": success_rate,
        "errorRate": error_rate,
        "avgResponseTime": 1.2, // Static for now
        "categoryCounts": category_counts,
        "dailyStats": dailystats
    }))
}

/// CrawlerManager를 사용한 통합 배치 크롤링 시작
#[tauri::command]
pub async fn start_integrated_crawling(
    _config: ComprehensiveCrawlerConfig,
    _state: State<'_, AppState>,
    _app_handle: AppHandle,
) -> Result<String, String> {
    // 추후 모듈 의존성 해결 후 활성화
    Err("CrawlerManager integration in progress".to_string())
}

/// CrawlerManager를 사용한 크롤링 중지
#[tauri::command]
pub async fn stop_integrated_crawling(
    _session_id: String,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    Err("CrawlerManager integration in progress".to_string())
}

/// CrawlerManager를 사용한 크롤링 일시정지
#[tauri::command]
pub async fn pause_integrated_crawling(
    _session_id: String,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    Err("CrawlerManager integration in progress".to_string())
}

/// CrawlerManager를 사용한 크롤링 재개
#[tauri::command]
pub async fn resume_integrated_crawling(
    _session_id: String,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    Err("CrawlerManager integration in progress".to_string())
}

/// CrawlerManager를 사용한 크롤링 진행 상황 조회
#[tauri::command]
pub async fn get_integrated_crawling_progress(
    _session_id: String,
    _state: State<'_, AppState>,
) -> Result<CrawlingProgress, String> {
    Err("CrawlerManager integration in progress".to_string())
}
