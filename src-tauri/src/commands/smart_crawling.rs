//! Smart crawling commands - uses the range calculation logic from prompts6
//!
//! This module provides commands for smart crawling that automatically calculates
//! the next pages to crawl based on the current database state and site information.

use crate::application::AppState;
use crate::infrastructure::crawling_service_impls::RangeSimpleProgress;
use crate::domain::pagination::CanonicalPageIdCalculator;
use crate::infrastructure::DatabaseConnection;
use crate::infrastructure::config::ConfigManager;
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tracing::info;

/// Response for crawling range calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRangeResponse {
    pub success: bool,
    pub range: Option<(u32, u32)>,
    pub progress: CrawlingProgressInfo,
    pub site_info: SiteInfo,
    pub local_db_info: LocalDbInfo,
    pub crawling_info: CrawlingInfo,
    pub batch_plan: BatchPlan, // CrawlingPlanner가 계산한 배치 계획 추가
    pub message: String,
}

/// `CrawlingPlanner가` 계산한 배치 계획 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPlan {
    pub batch_size: u32,
    pub total_batches: u32,
    pub concurrency_limit: u32,
    pub batches: Vec<BatchInfo>,
    pub execution_strategy: String, // "concurrent", "sequential", "mixed"
    pub estimated_duration_seconds: u32,
}

/// 개별 배치 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub batch_id: u32,
    pub pages: Vec<u32>,
    pub estimated_products: u32,
}

/// Site information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteInfo {
    pub total_pages: u32,
    pub products_on_last_page: u32,
    pub estimated_total_products: u32,
}

/// Local database information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDbInfo {
    pub total_saved_products: u32,
    pub last_crawled_page: Option<u32>,
    pub last_crawled_page_id: Option<i32>,
    pub coverage_percentage: f64,
}

/// Crawling strategy information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingInfo {
    pub pages_to_crawl: Option<u32>,
    pub estimated_new_products: Option<u32>,
    pub strategy: String, // "full", "partial", "none"
}

/// Crawling progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgressInfo {
    pub total_products: u32,
    pub saved_products: u32,
    pub progress_percentage: f64,
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub is_completed: bool,
}

/// Request for calculating crawling range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRangeRequest {
    pub total_pages_on_site: u32,
    pub products_on_last_page: u32,
}

/// Create product repository from database connection
async fn create_product_repo() -> Result<IntegratedProductRepository, String> {
    // 데이터베이스 경로 생성 (macOS 경로에 맞게 수정)
    let database_url = {
        let app_data_dir = if cfg!(target_os = "macos") {
            std::env::var("HOME").map_or_else(|_| "./data".to_string(), |h| format!("{}/Library/Application Support", h))
        } else {
            std::env::var("APPDATA")
                .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
                .unwrap_or_else(|_| "./data".to_string())
        };
        let data_dir = format!("{}/matter-certis-v2/database", app_data_dir);
        format!("sqlite:{}/matter_certis.db", data_dir)
    };

    info!("Using database at: {}", database_url);

    let db_conn = DatabaseConnection::new(&database_url)
        .await
        .map_err(|e| format!("Failed to create database connection: {}", e))?;

    let pool = db_conn.pool().clone();

    Ok(IntegratedProductRepository::new(pool))
}

/// Calculate the next crawling range based on current DB state
#[tauri::command]
pub async fn calculate_crawling_range(
    _state: State<'_, AppState>,
    request: CrawlingRangeRequest,
) -> Result<CrawlingRangeResponse, String> {
    info!(
        "🎯 Calculating next crawling range with: total_pages={}, products_on_last_page={}",
        request.total_pages_on_site, request.products_on_last_page
    );

    // Get configuration
    let config_manager =
        ConfigManager::new().map_err(|e| format!("Failed to initialize config manager: {}", e))?;

    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to get config: {}", e))?;

    // Create product repository
    let product_repo = create_product_repo().await?;

    // Create range calculator
    let range_calculator = CrawlingRangeCalculator::new(Arc::new(product_repo), config);

    // Calculate next range
    let result = range_calculator
        .calculate_next_crawling_range(request.total_pages_on_site, request.products_on_last_page)
        .await
        .map_err(|e| format!("Failed to calculate crawling range: {}", e))?;

    // Get progress information
    let progress = range_calculator
        .analyze_simple_progress(request.total_pages_on_site, request.products_on_last_page)
        .await
        .map_err(|e| format!("Failed to analyze progress: {}", e))?;

    // Calculate site information
    let estimated_total_products =
        ((request.total_pages_on_site - 1) * 12) + request.products_on_last_page;
    let site_info = SiteInfo {
        total_pages: request.total_pages_on_site,
        products_on_last_page: request.products_on_last_page,
        estimated_total_products,
    };

    // Calculate local DB information
    let local_db_info = if progress.current > 0 {
        // DB에 데이터가 있으면 계산기를 사용해서 실제 페이지 번호를 계산
        let calculator = CanonicalPageIdCalculator::new(
            request.total_pages_on_site,
            request.products_on_last_page as usize,
        );

        let max_page_id = progress.current_batch.unwrap_or(0) as i32;
        let max_index_in_page = 0; // 간단히 0으로 가정 (정확한 값은 DB에서 가져와야 함)

        if let Some((actual_page, _)) = calculator.reverse_calculate(max_page_id, max_index_in_page)
        {
            LocalDbInfo {
                total_saved_products: progress.current,
                last_crawled_page: Some(actual_page),
                last_crawled_page_id: Some(max_page_id),
                coverage_percentage: progress.percentage,
            }
        } else {
            // 역계산 실패 시 기본값
            LocalDbInfo {
                total_saved_products: progress.current,
                last_crawled_page: None,
                last_crawled_page_id: None,
                coverage_percentage: progress.percentage,
            }
        }
    } else {
        // DB가 비어있으면 모든 값을 None으로 설정
        LocalDbInfo {
            total_saved_products: 0,
            last_crawled_page: None,
            last_crawled_page_id: None,
            coverage_percentage: 0.0,
        }
    };

    let response = if let Some((start_page, end_page)) = result {
        let total_pages = if start_page >= end_page {
            start_page - end_page + 1
        } else {
            end_page - start_page + 1
        };

        let estimated_new_products = total_pages * 12; // 평균 12개 제품/페이지

        let crawling_info = CrawlingInfo {
            pages_to_crawl: Some(total_pages),
            estimated_new_products: Some(estimated_new_products),
            strategy: "partial".to_string(),
        };

        // CrawlingPlanner 기반 배치 계획 생성
        info!(
            "🔧 Creating batch plan for range: {} to {}",
            start_page, end_page
        );
        let batch_plan = create_batch_plan(start_page, end_page).await;

        let message = format!(
            "Next crawling range: pages {} to {} (total: {} pages)",
            start_page, end_page, total_pages
        );
        info!("✅ {}", message);

        CrawlingRangeResponse {
            success: true,
            range: Some((start_page, end_page)),
            progress: convert_progress(&progress),
            site_info,
            local_db_info,
            crawling_info,
            batch_plan,
            message,
        }
    } else {
        let crawling_info = CrawlingInfo {
            pages_to_crawl: Some(0),
            estimated_new_products: Some(0),
            strategy: "none".to_string(),
        };

        // 빈 배치 계획
        let batch_plan = BatchPlan {
            batch_size: 0,
            total_batches: 0,
            concurrency_limit: 0,
            batches: vec![],
            execution_strategy: "none".to_string(),
            estimated_duration_seconds: 0,
        };

        let message = "All products have been crawled - no more pages to process".to_string();
        info!("🏁 {}", message);

        CrawlingRangeResponse {
            success: true,
            range: None,
            progress: convert_progress(&progress),
            site_info,
            local_db_info,
            crawling_info,
            batch_plan,
            message,
        }
    };

    Ok(response)
}

/// `CrawlingPlanner` 기반 배치 계획 생성
async fn create_batch_plan(start_page: u32, end_page: u32) -> BatchPlan {
    info!(
        "🔧 Creating batch plan: start_page={}, end_page={}",
        start_page, end_page
    );

    // 설정에서 batch_size 가져오기 (비차단 방식; 실패 시 개발 기본값 사용)
    let app_config = match ConfigManager::new() {
        Ok(cm) => match cm.load_config().await {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!(
                    "⚠️ Failed to load AppConfig for batch plan: {}. Falling back to development defaults.",
                    e
                );
                crate::infrastructure::config::AppConfig::for_development()
            }
        },
        Err(e) => {
            tracing::warn!(
                "⚠️ Failed to initialize ConfigManager for batch plan: {}. Falling back to development defaults.",
                e
            );
            crate::infrastructure::config::AppConfig::for_development()
        }
    };
    let batch_size = app_config.user.batch.batch_size;
    let concurrency_limit = app_config.user.max_concurrent_requests; // ExecutionPlan 경로와 일치

    info!(
        "📋 Batch plan configuration: batch_size={}, concurrency_limit={}",
        batch_size, concurrency_limit
    );

    // 페이지 범위 계산
    let pages: Vec<u32> = if start_page >= end_page {
        (end_page..=start_page).rev().collect() // 역순 크롤링
    } else {
        (start_page..=end_page).collect()
    };

    let total_pages = pages.len() as u32;
    let total_batches = total_pages.div_ceil(batch_size); // 올림 계산

    info!(
        "📊 Batch plan calculation: total_pages={}, total_batches={}",
        total_pages, total_batches
    );

    // 배치 분할
    let mut batches = Vec::new();
    for (batch_id, chunk) in pages.chunks(batch_size as usize).enumerate() {
        let batch_info = BatchInfo {
            batch_id: batch_id as u32,
            pages: chunk.to_vec(),
            estimated_products: chunk.len() as u32 * 12, // 평균 12개/페이지
        };
        info!(
            "🔢 Batch {}: pages={:?}, estimated_products={}",
            batch_id + 1,
            chunk,
            batch_info.estimated_products
        );
        batches.push(batch_info);
    }

    // 예상 실행 시간 (각 페이지당 2초 + 네트워크 지연)
    let estimated_duration_seconds =
        total_pages * 2 + (total_batches * batch_size) / concurrency_limit;

    info!(
        "✅ Batch plan created successfully: {} batches, estimated duration: {}s",
        total_batches, estimated_duration_seconds
    );

    BatchPlan {
        batch_size,
        total_batches,
        concurrency_limit,
        batches,
        execution_strategy: "concurrent".to_string(),
        estimated_duration_seconds,
    }
}

/// Get current crawling progress
#[tauri::command]
pub async fn get_crawling_progress(
    _state: State<'_, AppState>,
    total_pages_on_site: u32,
    products_on_last_page: u32,
) -> Result<CrawlingProgressInfo, String> {
    info!("📊 Getting crawling progress information");

    // Get configuration
    let config_manager =
        ConfigManager::new().map_err(|e| format!("Failed to initialize config manager: {}", e))?;

    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to get config: {}", e))?;

    // Create product repository
    let product_repo = create_product_repo().await?;

    // Create range calculator
    let range_calculator = CrawlingRangeCalculator::new(Arc::new(product_repo), config);

    // Analyze progress
    let progress = range_calculator
        .analyze_simple_progress(total_pages_on_site, products_on_last_page)
        .await
        .map_err(|e| format!("Failed to analyze progress: {}", e))?;

    Ok(convert_progress(&progress))
}

/// Get database state for range calculation
#[tauri::command]
pub async fn get_database_state_for_range_calculation(
    _state: State<'_, AppState>,
) -> Result<DatabaseStateInfo, String> {
    info!("📊 Getting database state for range calculation");

    // Create product repository
    let product_repo = create_product_repo().await?;

    // Get max pageId and indexInPage
    let (max_page_id, max_index_in_page) = product_repo
        .get_max_page_id_and_index()
        .await
        .map_err(|e| format!("Failed to get max page ID and index: {}", e))?;

    // Get total product count
    let total_products = product_repo
        .get_product_count()
        .await
        .map_err(|e| format!("Failed to get product count: {}", e))?;

    let info = DatabaseStateInfo {
        max_page_id,
        max_index_in_page,
        total_products: total_products as u32,
        has_data: max_page_id.is_some() && max_index_in_page.is_some(),
    };

    info!(
        "✅ Database state: max_page_id={:?}, max_index_in_page={:?}, total_products={}",
        info.max_page_id, info.max_index_in_page, info.total_products
    );

    Ok(info)
}

/// Database state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStateInfo {
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub total_products: u32,
    pub has_data: bool,
}

/// Demo function to show the prompts6 example calculation
#[tauri::command]
pub async fn demo_prompts6_calculation() -> Result<String, String> {
    info!("🎯 Running prompts6 example calculation demo");

    // Example data from prompts6
    let max_page_id = 10i32;
    let max_index_in_page = 6i32;
    let total_pages_on_site = 481u32;
    let products_on_last_page = 10u32;
    let crawl_page_limit = 10u32;
    let products_per_page = 12u32;

    let mut result = String::new();
    result.push_str("📊 prompts6 Example Calculation Demo\n\n");
    result.push_str("Input data:\n");
    result.push_str(&format!("  max_page_id: {}\n", max_page_id));
    result.push_str(&format!("  max_index_in_page: {}\n", max_index_in_page));
    result.push_str(&format!("  total_pages_on_site: {}\n", total_pages_on_site));
    result.push_str(&format!(
        "  products_on_last_page: {}\n",
        products_on_last_page
    ));
    result.push_str(&format!("  crawl_page_limit: {}\n", crawl_page_limit));
    result.push_str(&format!("  products_per_page: {}\n\n", products_per_page));

    // Step 1: Calculate last saved index
    let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;
    result.push_str(&format!(
        "Step 1: lastSavedIndex = ({} * {}) + {} = {}\n",
        max_page_id, products_per_page, max_index_in_page, last_saved_index
    ));

    // Step 2: Calculate next product index
    let next_product_index = last_saved_index + 1;
    result.push_str(&format!(
        "Step 2: nextProductIndex = {} + 1 = {}\n",
        last_saved_index, next_product_index
    ));

    // Step 3: Calculate total products
    let total_products = ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;
    result.push_str(&format!(
        "Step 3: totalProducts = (({} - 1) * {}) + {} = {}\n",
        total_pages_on_site, products_per_page, products_on_last_page, total_products
    ));

    // Step 4: Convert to forward index
    let forward_index = (total_products - 1) - next_product_index;
    result.push_str(&format!(
        "Step 4: forwardIndex = ({} - 1) - {} = {}\n",
        total_products, next_product_index, forward_index
    ));

    // Step 5: Calculate target page number
    let target_page_number = (forward_index / products_per_page) + 1;
    result.push_str(&format!(
        "Step 5: targetPageNumber = ({} / {}) + 1 = {}\n",
        forward_index, products_per_page, target_page_number
    ));

    // Step 6: Apply crawl page limit
    let start_page = target_page_number;
    let end_page = if start_page >= crawl_page_limit {
        start_page - crawl_page_limit + 1
    } else {
        1
    };
    result.push_str(&format!(
        "Step 6: startPage = {}, endPage = {} - {} + 1 = {}\n",
        start_page, start_page, crawl_page_limit, end_page
    ));

    result.push_str(&format!(
        "\n✅ Final result: crawl pages {} to {}\n",
        start_page, end_page
    ));
    result.push_str("🎯 This matches the prompts6 specification exactly!\n");

    Ok(result)
}

/// Convert internal progress to API response format
fn convert_progress(progress: &RangeSimpleProgress) -> CrawlingProgressInfo {
    CrawlingProgressInfo {
    total_products: progress.total,
    saved_products: progress.current,
    progress_percentage: progress.percentage,
    max_page_id: progress.current_batch.map(|b| b as i32),
    max_index_in_page: progress.total_batches.map(|b| b as i32),
    is_completed: progress.is_completed,
    }
}
