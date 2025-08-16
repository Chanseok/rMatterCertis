//! 시스템 상태 분석 커맨드
//!
//! proposal6.md의 워크플로우 재정의에 따라 StatusTab에서 사용하는
//! 사이트 종합 분석 기능을 제공합니다.

use serde_json;
use tauri::{AppHandle, State};
use tracing::info;

// Legacy CrawlingEngineState/CrawlingResponse removed – define minimal local response
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CrawlingResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
use crate::application::shared_state::{DbAnalysisResult, SharedStateCache, SiteAnalysisResult};
use crate::domain::constants::{crawling::ttl, site};
use crate::domain::services::crawling_services::StatusChecker;

/// 시스템 종합 분석 커맨드 (StatusTab용)
///
/// proposal6.md Section 3.1: StatusTab의 역할 - 분석 및 캐시 업데이트
///
/// 이 커맨드는:
/// 1. 사이트 분석과 DB 분석을 수행합니다
/// 2. 결과를 SharedStateCache에 업데이트합니다  
/// 3. 분석 결과를 UI에 전송하여 화면에 표시합니다
/// 4. 백엔드가 total_pages, DB 커서 위치를 "기억"하게 됩니다
#[tauri::command]
pub async fn analyze_system_status(
    _app: AppHandle,
    shared_state: State<'_, SharedStateCache>,
) -> Result<CrawlingResponse, String> {
    info!("🔍 Starting comprehensive system analysis...");

    // Phase 1 & 2: Perform Site and Database Analysis in Parallel
    info!("📊 Phase 1 & 2: Performing site and database analysis in parallel...");
    let (site_analysis, db_analysis) =
        tokio::try_join!(perform_site_analysis(), perform_database_analysis())
            .map_err(|e| format!("System analysis failed: {}", e))?;

    // Phase 3: Update SharedStateCache
    info!("💾 Phase 3: Updating SharedStateCache with analysis results");
    {
        shared_state.set_site_analysis(site_analysis.clone()).await;
        shared_state.set_db_analysis(db_analysis.clone()).await;
    }

    // Phase 4: Calculate intelligent range preview (optional)
    let range_preview = if !db_analysis.is_empty {
        calculate_range_preview(&site_analysis, &db_analysis).await
    } else {
        None
    };

    // Phase 5: Prepare comprehensive response for UI
    let analysis_data = serde_json::json!({
        "site_analysis": {
            "total_pages": site_analysis.total_pages,
            "products_on_last_page": site_analysis.products_on_last_page,
            "estimated_products": site_analysis.estimated_products,
            "health_score": site_analysis.health_score,
            "analyzed_at": site_analysis.analyzed_at,
            "site_url": site_analysis.site_url,
        },
        "database_analysis": {
            "total_products": db_analysis.total_products,
            "max_page_id": db_analysis.max_page_id,
            "max_index_in_page": db_analysis.max_index_in_page,
            "quality_score": db_analysis.quality_score,
            "is_empty": db_analysis.is_empty,
            "analyzed_at": db_analysis.analyzed_at,
        },
        "range_preview": range_preview,
        "cache_status": {
            "site_analysis_ttl_minutes": ttl::SITE_ANALYSIS_TTL_MINUTES,
            "db_analysis_ttl_minutes": ttl::DB_ANALYSIS_TTL_MINUTES,
            "products_per_page_constant": site::PRODUCTS_PER_PAGE,
        },
        "analysis_summary": format!(
            "Site: {} pages ({} products), DB: {} products saved, Position: {}:{}",
            site_analysis.total_pages,
            site_analysis.estimated_products,
            db_analysis.total_products,
            db_analysis.max_page_id.unwrap_or(-1),
            db_analysis.max_index_in_page.unwrap_or(-1)
        )
    });

    info!("✅ System analysis completed successfully");
    info!(
        "🧠 Backend now remembers: {} total pages, DB cursor at {}:{}",
        site_analysis.total_pages,
        db_analysis.max_page_id.unwrap_or(-1),
        db_analysis.max_index_in_page.unwrap_or(-1)
    );

    Ok(CrawlingResponse {
        success: true,
        message: "System analysis completed successfully".to_string(),
        data: Some(analysis_data),
    })
}

/// 사이트 분석 수행
async fn perform_site_analysis() -> Result<SiteAnalysisResult, String> {
    info!("🌐 Analyzing site status...");

    // Create necessary components for site analysis
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to load configuration: {}", e))?;

    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;

    // Create database connection and repository for StatusChecker
    let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("Failed to obtain database pool: {}", e))?;
    let product_repo = std::sync::Arc::new(
        crate::infrastructure::IntegratedProductRepository::new(db_pool),
    );

    let status_checker =
        crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
            http_client,
            data_extractor,
            config,
            product_repo,
        );

    // Perform site status check
    let site_status = status_checker
        .check_site_status()
        .await
        .map_err(|e| format!("Failed to check site status: {}", e))?;

    info!(
        "📊 Site analysis completed: {} pages, {} products on last page, {} estimated total",
        site_status.total_pages, site_status.products_on_last_page, site_status.estimated_products
    );

    Ok(SiteAnalysisResult::new(
        site_status.total_pages,
        site_status.products_on_last_page,
        site_status.estimated_products,
        site::BASE_URL.to_string(),
        1.0, // health_score - TODO: Calculate actual health score
    ))
}

/// 데이터베이스 분석 수행
async fn perform_database_analysis() -> Result<DbAnalysisResult, String> {
    info!("🗄️ Analyzing database state...");

    // Create database connection and repository
    let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("Failed to obtain database pool: {}", e))?;
    let product_repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);

    // Perform actual database analysis
    let analysis = product_repo
        .analyze_database_state()
        .await
        .map_err(|e| format!("Failed to analyze database: {}", e))?;

    info!(
        "📊 Database analysis completed: {} products, cursor at {}:{}",
        analysis.total_products,
        analysis.max_page_id.unwrap_or(-1),
        analysis.max_index_in_page.unwrap_or(-1)
    );

    Ok(analysis)
}

/// 범위 계산 미리보기 (UI 표시용)
async fn calculate_range_preview(
    site_analysis: &SiteAnalysisResult,
    db_analysis: &DbAnalysisResult,
) -> Option<serde_json::Value> {
    // This is just a preview calculation, not the actual range used for crawling
    if db_analysis.is_empty {
        return Some(serde_json::json!({
            "type": "full_crawl",
            "reason": "Empty database - full site crawl recommended",
            "estimated_start_page": site_analysis.total_pages,
            "estimated_end_page": 1,
            "estimated_total_pages": site_analysis.total_pages,
        }));
    }

    // Calculate incremental crawl preview
    let max_page_id = db_analysis.max_page_id?;
    let max_index_in_page = db_analysis.max_index_in_page?;

    // Use site constants for calculation
    let products_per_page = site::PRODUCTS_PER_PAGE as u32;
    let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;
    let next_product_index = last_saved_index + 1;
    let next_page = (next_product_index / products_per_page) + 1;

    Some(serde_json::json!({
        "type": "incremental_crawl",
        "reason": format!("Continue from last saved position: page {} index {}", max_page_id, max_index_in_page),
        "last_saved_absolute_index": last_saved_index,
        "next_product_index": next_product_index,
        "next_page_to_crawl": next_page,
        "database_products": db_analysis.total_products,
    }))
}

/// 캐시 상태 조회 (디버그/관리용)
#[tauri::command]
pub async fn get_analysis_cache_status(
    shared_state: State<'_, SharedStateCache>,
) -> Result<serde_json::Value, String> {
    let site_status = shared_state
        .get_valid_site_analysis_async(Some(ttl::SITE_ANALYSIS_TTL_MINUTES))
        .await;
    let db_status = shared_state
        .get_valid_db_analysis_async(Some(ttl::DB_ANALYSIS_TTL_MINUTES))
        .await;
    let range_status = shared_state
        .get_valid_calculated_range_async(ttl::CALCULATED_RANGE_TTL_MINUTES)
        .await;

    Ok(serde_json::json!({
        "cache_status": {
            "has_valid_site_analysis": site_status.is_some(),
            "has_valid_db_analysis": db_status.is_some(),
            "has_valid_calculated_range": range_status.is_some(),
        },
        "ttl_settings": {
            "site_analysis_ttl_minutes": ttl::SITE_ANALYSIS_TTL_MINUTES,
            "db_analysis_ttl_minutes": ttl::DB_ANALYSIS_TTL_MINUTES,
            "calculated_range_ttl_minutes": ttl::CALCULATED_RANGE_TTL_MINUTES,
        },
        "site_constants": {
            "products_per_page": site::PRODUCTS_PER_PAGE,
            "base_url": site::BASE_URL,
            "page_numbering_base": site::PAGE_NUMBERING_BASE,
        }
    }))
}

/// 분석 캐시 수동 클리어 (디버그/관리용)
#[tauri::command]
pub async fn clear_analysis_cache(
    shared_state: State<'_, SharedStateCache>,
) -> Result<String, String> {
    shared_state.clear_all_caches().await;

    info!("🧹 Analysis cache cleared manually");
    Ok("Analysis cache cleared successfully".to_string())
}
