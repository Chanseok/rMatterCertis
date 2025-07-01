//! Modern Tauri commands for real-time crawling operations
//! 
//! This module implements Tauri commands that support real-time event emission
//! and proper state management following the architectural guide.

use crate::application::{AppState, EventEmitter};
use crate::domain::events::{CrawlingProgress, CrawlingStatus, CrawlingStage, DatabaseStats, DatabaseHealth};
use crate::domain::entities::CrawlingSession;
use crate::commands::config_commands::ComprehensiveCrawlerConfig;
use tauri::{State, AppHandle};
use tracing::{info, warn};
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
    let crawling_config = crate::infrastructure::BatchCrawlingConfig {
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

        // 배치 크롤링 엔진 생성 및 실행
        let engine = crate::infrastructure::BatchCrawlingEngine::new(
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
    // For now, return mock statistics
    // TODO: Implement actual database statistics collection
    let stats = DatabaseStats {
        total_products: 1250,
        total_devices: 850,
        last_updated: Utc::now(),
        storage_size: "15.3 MB".to_string(),
        incomplete_records: 23,
        health_status: DatabaseHealth::Healthy,
    };
    
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

/// Get the correct database URL for the application
fn get_database_url() -> Result<String, String> {
    let data_dir = ConfigManager::get_app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    let db_path = data_dir.join("database").join("matter_certis.db");
    let database_url = format!("sqlite:{}", db_path.to_string_lossy());
    
    Ok(database_url)
}
