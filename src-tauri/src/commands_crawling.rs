//! Tauri commands for crawling operations
//! 
//! This module provides Tauri command handlers that expose crawling functionality
//! to the frontend application.

use std::sync::Arc;
use tauri::State;
use serde::{Deserialize, Serialize};

use crate::infrastructure::{DatabaseConnection, HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::application::{CrawlingUseCases, CrawlingConfig};
use crate::domain::session_manager::{SessionManager, CrawlingSessionState};

/// Request structure for starting a crawling session
#[derive(Debug, Deserialize)]
pub struct StartCrawlingRequest {
    pub start_url: Option<String>,
    pub max_pages: Option<u32>,
    pub delay_ms: Option<u64>,
    pub max_concurrent: Option<u32>,
}

/// Response structure for crawling operations
#[derive(Debug, Serialize)]
pub struct CrawlingResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
}

/// Response structure for crawling status
#[derive(Debug, Serialize)]
pub struct CrawlingStatusResponse {
    pub success: bool,
    pub session_found: bool,
    pub session_state: Option<CrawlingSessionState>,
    pub message: String,
}

/// Start a new crawling session
#[tauri::command]
pub async fn start_crawling_session(
    request: StartCrawlingRequest,
    db: State<'_, DatabaseConnection>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("Starting crawling session with request: {:?}", request);

    // Create crawling configuration
    let config = CrawlingConfig {
        start_url: request.start_url.unwrap_or_else(|| {
            use crate::infrastructure::config::csa_iot;
            csa_iot::PRODUCTS_PAGE_GENERAL.to_string()
        }),
        max_pages: request.max_pages.unwrap_or(10),
        delay_ms: request.delay_ms.unwrap_or(1000),
        max_concurrent: request.max_concurrent.unwrap_or(3),
        base_url: {
            use crate::infrastructure::config::csa_iot;
            csa_iot::BASE_URL.to_string()
        },
    };

    // Create components
    let http_client = HttpClient::new()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let product_repo = Arc::new(IntegratedProductRepository::new(db.inner().pool().clone()));
    let session_manager = Arc::new(SessionManager::new());

    // Create use cases
    let crawling_use_cases = CrawlingUseCases::new(
        http_client,
        data_extractor,
        product_repo,
        session_manager,
    );

    // Start crawling
    match crawling_use_cases.start_crawling_session(config).await {
        Ok(session_id) => {
            tracing::info!("Successfully started crawling session: {}", session_id);
            Ok(CrawlingResponse {
                success: true,
                message: "Crawling session started successfully".to_string(),
                session_id: Some(session_id),
            })
        }
        Err(e) => {
            tracing::error!("Failed to start crawling session: {}", e);
            Ok(CrawlingResponse {
                success: false,
                message: format!("Failed to start crawling: {}", e),
                session_id: None,
            })
        }
    }
}

/// Get the current status of a crawling session
#[tauri::command]
pub async fn get_crawling_status(
    session_id: String,
    _db: State<'_, DatabaseConnection>,
) -> Result<CrawlingStatusResponse, String> {
    tracing::debug!("Getting status for crawling session: {}", session_id);

    // Create session manager (in a real implementation, this would be a singleton)
    let session_manager = Arc::new(SessionManager::new());

    match session_manager.get_session_state(&session_id).await {
        Ok(Some(session_state)) => {
            Ok(CrawlingStatusResponse {
                success: true,
                session_found: true,
                session_state: Some(session_state),
                message: "Session status retrieved successfully".to_string(),
            })
        }
        Ok(None) => {
            Ok(CrawlingStatusResponse {
                success: true,
                session_found: false,
                session_state: None,
                message: "Session not found".to_string(),
            })
        }
        Err(e) => {
            tracing::error!("Failed to get session status: {}", e);
            Ok(CrawlingStatusResponse {
                success: false,
                session_found: false,
                session_state: None,
                message: format!("Failed to get session status: {}", e),
            })
        }
    }
}

/// Stop a running crawling session
#[tauri::command]
pub async fn stop_crawling_session(
    session_id: String,
    _db: State<'_, DatabaseConnection>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("Stopping crawling session: {}", session_id);

    // Create session manager
    let session_manager = Arc::new(SessionManager::new());

    match session_manager.set_status(&session_id, crate::domain::session_manager::SessionStatus::Stopped).await {
        Ok(()) => {
            Ok(CrawlingResponse {
                success: true,
                message: "Crawling session stopped successfully".to_string(),
                session_id: Some(session_id),
            })
        }
        Err(e) => {
            tracing::error!("Failed to stop crawling session: {}", e);
            Ok(CrawlingResponse {
                success: false,
                message: format!("Failed to stop crawling: {}", e),
                session_id: Some(session_id),
            })
        }
    }
}

/// Perform a health check on the crawling system
#[tauri::command]
pub async fn crawling_health_check(
    db: State<'_, DatabaseConnection>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("Performing crawling system health check");

    // Create components for health check
    let http_client = HttpClient::new()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let product_repo = Arc::new(IntegratedProductRepository::new(db.inner().pool().clone()));
    let session_manager = Arc::new(SessionManager::new());

    let crawling_use_cases = CrawlingUseCases::new(
        http_client,
        data_extractor,
        product_repo,
        session_manager,
    );

    match crawling_use_cases.health_check().await {
        Ok(()) => {
            Ok(CrawlingResponse {
                success: true,
                message: "All crawling components are healthy".to_string(),
                session_id: None,
            })
        }
        Err(e) => {
            tracing::error!("Crawling health check failed: {}", e);
            Ok(CrawlingResponse {
                success: false,
                message: format!("Health check failed: {}", e),
                session_id: None,
            })
        }
    }
}
