use tauri::State;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use tracing::{info, error};
use chrono::{DateTime, Utc};

use crate::application::AppState;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::domain::product::Product; // 올바른 Product 타입 사용

/// 제품 페이지 응답
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProductPage {
    pub products: Vec<Product>,
    pub total_count: u32,
    pub page: u32,
    pub size: u32,
    pub has_next: bool,
}

/// 크롤링 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingStatusInfo {
    pub is_running: bool,
    pub current_page: Option<u32>,
    pub total_pages: Option<u32>,
    pub last_updated: Option<String>,
    pub session_id: Option<String>,
}

/// 시스템 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemStatus {
    pub database_connected: bool,
    pub total_products: u32,
    pub last_crawl_time: Option<DateTime<chrono::Utc>>,
    pub config_loaded: bool,
}

/// 제품 데이터 페이지별 조회 (Backend-Only CRUD)
#[tauri::command]
pub async fn get_products_page(
    state: State<'_, AppState>,
    page: u32,
    size: u32,
) -> Result<ProductPage, String> {
    let pool = state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);
    
    match repo.get_products_paginated(page as i32, size as i32).await {
        Ok(products) => {
            // 전체 개수 조회 (향후 최적화 가능)
            let total_count = match repo.count_products().await {
                Ok(count) => count as u32,
                Err(e) => {
                    error!("Failed to count products: {}", e);
                    0
                }
            };
            
            let has_next = (page + 1) * size < total_count;
            
            info!("✅ Retrieved {} products for page {} (size: {})", products.len(), page, size);
            
            Ok(ProductPage {
                products,
                total_count,
                page,
                size,
                has_next,
            })
        }
        Err(e) => {
            error!("Failed to get products page: {}", e);
            Err(format!("Failed to retrieve products: {}", e))
        }
    }
}

/// 최근 업데이트된 제품 조회 (Backend-Only CRUD)
#[tauri::command]
pub async fn get_latest_products(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<Product>, String> {
    let pool = state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);
    
    match repo.get_latest_updated_products(limit).await {
        Ok(products) => {
            info!("✅ Retrieved {} latest updated products", products.len());
            Ok(products)
        }
        Err(e) => {
            error!("Failed to get latest products: {}", e);
            Err(format!("Failed to retrieve latest products: {}", e))
        }
    }
}

/// 크롤링 상태 조회 (Backend-Only CRUD)
#[tauri::command]
pub async fn get_crawling_status_v2(state: State<'_, AppState>) -> Result<CrawlingStatusInfo, String> {
    let current_session = state.current_session.read().await;
    let current_progress = state.current_progress.read().await;
    
    let status = CrawlingStatusInfo {
        is_running: current_session.is_some(),
        current_page: Some(current_progress.current),
        total_pages: Some(current_progress.total),
        last_updated: None, // CrawlingProgress doesn't have last_updated field
        session_id: current_session.as_ref().map(|s| s.id.clone()),
    };
    
    info!("✅ Retrieved crawling status: running={}", status.is_running);
    Ok(status)
}

/// 시스템 전체 상태 조회 (Backend-Only CRUD)
#[tauri::command]
pub async fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    // 데이터베이스 연결 확인
    let database_connected = state.get_database_pool().await.is_ok();
    
    let (total_products, last_crawl_time) = if database_connected {
        let pool = state.get_database_pool().await?;
        let repo = IntegratedProductRepository::new(pool);
        
        let total = match repo.count_products().await {
            Ok(count) => count as u32,
            Err(_) => 0,
        };
        
        let last_updated = match repo.get_latest_updated_product().await {
            Ok(Some(product)) => Some(product.updated_at),
            _ => None,
        };
        
        (total, last_updated)
    } else {
        (0, None)
    };
    
    // 설정 로딩 상태 확인
    let config_guard = state.config.read().await;
    let config_loaded = true; // config가 항상 로드되어 있음
    drop(config_guard);
    
    let status = SystemStatus {
        database_connected,
        total_products,
        last_crawl_time,
        config_loaded,
    };
    
    info!("✅ System status: db_connected={}, total_products={}, config_loaded={}", 
          status.database_connected, status.total_products, status.config_loaded);
    
    Ok(status)
}
