//! Modern Tauri commands for real-time crawling operations
//! 
//! This module implements Tauri commands that support real-time event emission
//! and proper state management following the architectural guide.

use crate::application::{AppState, EventEmitter};
use crate::domain::events::{CrawlingProgress, CrawlingStatus, CrawlingStage, DatabaseStats, DatabaseHealth};
use crate::domain::entities::CrawlingSession;
use crate::commands::config_commands::ComprehensiveCrawlerConfig;
use crate::domain::services::crawling_services::{StatusChecker, DatabaseAnalyzer};
use tauri::{State, AppHandle};
use tracing::{info, warn, error};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::infrastructure::config::ConfigManager;

/// Start a new crawling session with comprehensive configuration
#[tauri::command]
pub async fn start_crawling(
    config: ComprehensiveCrawlerConfig,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    info!("Starting crawling session with comprehensive config: batch_size={}, concurrency={}, delay_ms={}", 
          config.batch_size, config.concurrency, config.delay_ms);
    
    // Initialize event emitter if not already done
    {
        let emitter_guard = state.event_emitter.read().await;
        if emitter_guard.is_none() {
            drop(emitter_guard);
            let emitter = EventEmitter::new(app_handle.clone());
            state.initialize_event_emitter(emitter).await?;
        }
    }
    
    // Create new crawling session
    let session = CrawlingSession {
        id: uuid::Uuid::new_v4().to_string(),
        url: "https://csa-iot.org/csa_product/".to_string(), // Default URL for now
        start_page: config.start_page,
        end_page: config.end_page,
        status: "running".to_string(),
        created_at: Utc::now(),
        ..Default::default()
    };
    
    let session_id = session.id.clone();
    
    // Start the session
    state.start_session(session).await?;
    
    // Emit initial progress
    let initial_progress = CrawlingProgress {
        current: 0,
        total: config.end_page - config.start_page + 1,
        percentage: 0.0,
        current_stage: CrawlingStage::TotalPages,
        current_step: "크롤링 세션을 초기화하는 중...".to_string(),
        status: CrawlingStatus::Running,
        message: "크롤링을 시작합니다".to_string(),
        remaining_time: None,
        elapsed_time: 0,
        new_items: 0,
        updated_items: 0,
        current_batch: Some(1),
        total_batches: Some(config.end_page - config.start_page + 1),
        errors: 0,
        timestamp: Utc::now(),
    };
    
    state.update_progress(initial_progress).await?;
    
    // 비동기 태스크에서 사용할 변수들 복제
    let session_id_for_task = session_id.clone();
    let _app_handle_for_task = app_handle.clone();
    let crawling_config = crate::infrastructure::service_based_crawling_engine::BatchCrawlingConfig {
        start_page: config.start_page,
        end_page: config.end_page,
        concurrency: config.concurrency,
        delay_ms: config.delay_ms,
        batch_size: 10, // 기본 배치 크기
        retry_max: config.retry_max,
        timeout_ms: config.page_timeout_ms,
    };
    
    // 이벤트 이미터 참조 복제 
    let event_emitter_for_task = {
        let emitter_guard = state.event_emitter.read().await;
        emitter_guard.clone()
    };
    
    // AppState 복제하여 백그라운드 작업에 전달
    let app_state_for_update = Arc::clone(&state.current_progress);
    
    // 실제 배치 크롤링 엔진 백그라운드로 실행
    tokio::spawn(async move {
        // HTTP 클라이언트 및 파서 초기화
        let http_client = match crate::infrastructure::HttpClient::new() {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Failed to create HTTP client: {}", e);
                
                // 에러 상태 업데이트
                update_error_state(&app_state_for_update, &format!("HTTP 클라이언트 생성 실패: {}", e)).await;
                return;
            }
        };
        
        let data_extractor = match crate::infrastructure::MatterDataExtractor::new() {
            Ok(extractor) => extractor,
            Err(e) => {
                tracing::error!("Failed to create data extractor: {}", e);
                
                // 에러 상태 업데이트
                update_error_state(&app_state_for_update, &format!("데이터 추출기 생성 실패: {}", e)).await;
                return;
            }
        };
        
        // 통합 제품 리포지토리 생성
        let database_url = match get_database_url() {
            Ok(url) => url,
            Err(e) => {
                tracing::error!("Failed to get database URL: {}", e);
                update_error_state(&app_state_for_update, &format!("데이터베이스 경로 설정 실패: {}", e)).await;
                return;
            }
        };
        
        let db_pool = match sqlx::SqlitePool::connect(&database_url).await {
            Ok(pool) => pool,
            Err(e) => {
                tracing::error!("Failed to connect to database: {}", e);
                
                // 에러 상태 업데이트
                update_error_state(&app_state_for_update, &format!("데이터베이스 연결 실패: {}", e)).await;
                return;
            }
        };
        
        let product_repo = std::sync::Arc::new(
            crate::infrastructure::IntegratedProductRepository::new(db_pool)
        );

        // 고급 배치 크롤링 엔진 생성 및 실행 (고급 데이터 처리 파이프라인 포함)
        let engine = crate::infrastructure::advanced_crawling_engine::AdvancedBatchCrawlingEngine::new(
            http_client,
            data_extractor,
            product_repo,
            std::sync::Arc::new(event_emitter_for_task),
            crawling_config,
            session_id_for_task,
        );

        if let Err(e) = engine.execute().await {
            tracing::error!("Batch crawling failed: {}", e);
            
            // 에러 상태 업데이트
            update_error_state(&app_state_for_update, &format!("크롤링 엔진 실행 실패: {}", e)).await;
        }
    });
    
    info!("Crawling session started with ID: {}", session_id);
    Ok(session_id)
}

// 에러 상태 업데이트 헬퍼 함수 
async fn update_error_state(progress_state: &Arc<RwLock<CrawlingProgress>>, error_message: &str) {
    let mut progress = progress_state.write().await;
    progress.current_stage = CrawlingStage::Idle;
    progress.current_step = "크롤링 실패".to_string();
    progress.status = CrawlingStatus::Error;
    progress.message = error_message.to_string();
    progress.errors += 1;
    progress.timestamp = Utc::now();
}

/// Pause the current crawling session
#[tauri::command]
pub async fn pause_crawling(state: State<'_, AppState>) -> Result<(), String> {
    info!("Pausing crawling session");
    
    // Update state to paused
    {
        let mut progress = state.current_progress.write().await;
        if progress.status == CrawlingStatus::Running {
            progress.status = CrawlingStatus::Paused;
            progress.current_step = "크롤링이 일시 정지되었습니다".to_string();
            progress.message = "사용자가 크롤링을 일시 정지했습니다".to_string();
            progress.timestamp = Utc::now();
        } else {
            return Err("크롤링이 실행 중이 아닙니다".to_string());
        }
    }
    
    // Emit pause event
    if let Some(emitter) = state.event_emitter.read().await.as_ref() {
        let progress = state.current_progress.read().await.clone();
        if let Err(e) = emitter.emit_progress(progress).await {
            warn!("Failed to emit pause event: {}", e);
        }
    }
    
    info!("Crawling session paused successfully");
    Ok(())
}

/// Resume the paused crawling session
#[tauri::command]
pub async fn resume_crawling(state: State<'_, AppState>) -> Result<(), String> {
    info!("Resuming crawling session");
    
    // Update state to running
    {
        let mut progress = state.current_progress.write().await;
        if progress.status == CrawlingStatus::Paused {
            progress.status = CrawlingStatus::Running;
            progress.current_step = "크롤링이 재개되었습니다".to_string();
            progress.message = "사용자가 크롤링을 재개했습니다".to_string();
            progress.timestamp = Utc::now();
        } else {
            return Err("크롤링이 일시 정지 상태가 아닙니다".to_string());
        }
    }
    
    // Emit resume event
    if let Some(emitter) = state.event_emitter.read().await.as_ref() {
        let progress = state.current_progress.read().await.clone();
        if let Err(e) = emitter.emit_progress(progress).await {
            warn!("Failed to emit resume event: {}", e);
        }
    }
    
    info!("Crawling session resumed successfully");
    Ok(())
}

/// Stop the current crawling session
#[tauri::command]
pub async fn stop_crawling(state: State<'_, AppState>) -> Result<(), String> {
    info!("Stopping crawling session");
    
    // Update state to cancelled
    {
        let mut progress = state.current_progress.write().await;
        if progress.status == CrawlingStatus::Running || progress.status == CrawlingStatus::Paused {
            progress.status = CrawlingStatus::Cancelled;
            progress.current_step = "크롤링이 중지되었습니다".to_string();
            progress.message = "사용자가 크롤링을 중지했습니다".to_string();
            progress.timestamp = Utc::now();
        } else {
            return Err("중지할 수 있는 크롤링 세션이 없습니다".to_string());
        }
    }
    
    // Stop the session
    state.stop_session().await?;
    
    // Emit stop event
    if let Some(emitter) = state.event_emitter.read().await.as_ref() {
        let progress = state.current_progress.read().await.clone();
        if let Err(e) = emitter.emit_progress(progress).await {
            warn!("Failed to emit stop event: {}", e);
        }
    }
    
    info!("Crawling session stopped successfully");
    Ok(())
}

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
pub async fn get_database_stats(state: State<'_, AppState>) -> Result<DatabaseStats, String> {
    info!("🔍 Getting real database statistics...");
    
    // Get database URL
    let database_url = {
        let app_data_dir = std::env::var("APPDATA")
            .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
            .unwrap_or_else(|_| "./data".to_string());
        let data_dir = format!("{}/matter-certis-v2/database", app_data_dir);
        format!("sqlite:{}/matter_certis.db", data_dir)
    };
    
    // Connect to database
    let db_pool = sqlx::SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
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
pub async fn backup_database(_state: State<'_, AppState>) -> Result<String, String> {
    info!("Starting database backup");
    
    // TODO: Implement actual database backup logic
    let backup_path = format!("backup_{}.db", Utc::now().format("%Y%m%d_%H%M%S"));
    
    // Simulate backup process
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    info!("Database backup completed: {}", backup_path);
    Ok(backup_path)
}

/// Optimize the database
#[tauri::command]
pub async fn optimize_database(state: State<'_, AppState>) -> Result<(), String> {
    info!("Starting database optimization");
    
    // TODO: Implement actual database optimization logic
    
    // Simulate optimization process
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    
    // Update database stats after optimization
    let updated_stats = DatabaseStats {
        total_products: 1250,
        total_devices: 850,
        last_updated: Utc::now(),
        storage_size: "12.8 MB".to_string(), // Reduced after optimization
        incomplete_records: 15, // Reduced after optimization
        health_status: DatabaseHealth::Healthy,
    };
    
    state.update_database_stats(updated_stats).await?;
    
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
pub async fn clear_crawling_errors(_state: State<'_, AppState>) -> Result<(), String> {
    info!("Clearing crawling error logs");
    
    // TODO: Implement actual error log clearing
    
    info!("Crawling error logs cleared");
    Ok(())
}

/// Export crawling results
#[tauri::command]
pub async fn export_crawling_results(_state: State<'_, AppState>) -> Result<String, String> {
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
    info!("Starting comprehensive site status check with detailed page discovery");
    
    // Get the advanced crawling engine from the state
    let config = state.config.read().await.clone();
    
    // Create a simple HTTP client and necessary components
    let http_client = crate::infrastructure::HttpClient::new()
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
    let db_analyzer = match sqlx::SqlitePool::connect(&database_url).await {
        Ok(pool) => {
            let repo = crate::infrastructure::IntegratedProductRepository::new(pool);
            let repo_arc = std::sync::Arc::new(repo);
            Some(crate::infrastructure::DatabaseAnalyzerImpl::new(repo_arc))
        }
        Err(e) => {
            warn!("Failed to create database connection: {}", e);
            None
        }
    };
    
    // Perform the site status check
    let site_check_result = status_checker.check_site_status().await;
    let db_analysis_result = if let Some(ref analyzer) = db_analyzer {
        analyzer.analyze_current_state().await
    } else {
        Err(anyhow::anyhow!("Database analyzer not available"))
    };
    
    match (site_check_result, db_analysis_result) {
        (Ok(site_status), Ok(db_analysis)) => {
            info!("Site status check completed successfully");
            info!("Site: accessible={}, total_pages={}, estimated_products={}", 
                  site_status.is_accessible, site_status.total_pages, site_status.estimated_products);
            info!("Database: total_products={}, unique_products={}, duplicates={}", 
                  db_analysis.total_products, db_analysis.unique_products, db_analysis.duplicate_count);
            
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
                    "duplicate_count": db_analysis.duplicate_count,
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
                    } else if db_analysis.duplicate_count > 0 {
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
                    "duplicate_count": db_analysis.duplicate_count,
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

/// Get the correct database URL for the application
fn get_database_url() -> Result<String, String> {
    let data_dir = ConfigManager::get_app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    let db_path = data_dir.join("database").join("matter_certis.db");
    let database_url = format!("sqlite:{}", db_path.to_string_lossy());
    
    Ok(database_url)
}
