// 얕은 크롤링 및 진단 기반 보완 크롤링 Commands
//
// 목적: 전체 페이지 URL + 좌표 동기화 후 누락 제품 감지 및 선택적 크롤링

use crate::application::AppState;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::domain::services::crawling_services::StatusChecker;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::{Manager, State};
use tracing::info;
use ts_rs::TS;

/// 얕은 크롤링 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ShallowCrawlResult {
    pub session_id: String,
    pub pages_scanned: u32,
    pub urls_synced: u32,
    pub duration_ms: u64,
    pub status: String,
}

/// 누락 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MissingAnalysisResult {
    pub total_products: u32,
    pub complete_products: u32,
    pub missing_details: Vec<MissingProductInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MissingProductInfo {
    pub url: String,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
}

/// 스마트 동기화 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SmartSyncResult {
    pub shallow_crawl: ShallowCrawlResult,
    pub missing_analysis: MissingAnalysisResult,
    #[serde(rename = "補完_crawl")]
    pub complement_crawl: Option<ComplementCrawlResult>,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ComplementCrawlResult {
    pub urls_targeted: u32,
    pub urls_completed: u32,
    pub urls_failed: u32,
    pub duration_ms: u64,
}

/// 얕은 동기화: 전체 페이지 리스팅만 크롤링하여 좌표 갱신
///
/// # Errors
/// Returns error if site check or crawling fails
#[tauri::command]
pub async fn start_shallow_sync(
    app: tauri::AppHandle,
) -> Result<ShallowCrawlResult, String> {
    use std::time::Instant;
    
    info!("🏃 Starting shallow sync (coordinate update only)");
    let start_time = Instant::now();
    
    // 전체 페이지 크롤링을 위한 ExecutionPlan 생성
    use crate::crawl_engine::services::planning_service::{ManualPlanningStrategy, PlanningStrategy, PlanOverrides};
    
    // 사이트 상태 확인 (총 페이지 수)
    let app_state = app.state::<AppState>();
    let config_guard = app_state.config.read().await;
    let app_config = config_guard.clone();
    drop(config_guard);
    
    let pool = app_state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);
    
    // 사이트 분석으로 총 페이지 수 확인
    use crate::infrastructure::HttpClient;
    use crate::infrastructure::MatterDataExtractor;
    use crate::infrastructure::crawling_service_impls::StatusCheckerImpl;
    
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let status_checker = StatusCheckerImpl::with_product_repo(
        http_client,
        data_extractor,
        app_config.clone(),
        std::sync::Arc::new(repo.clone()),
    );
    
    let site_status = status_checker.check_site_status().await
        .map_err(|e| format!("Failed to check site status: {}", e))?;
    
    let total_pages = site_status.total_pages;
    info!("📊 Shallow sync: scanning {} pages (list-only mode)", total_pages);
    
    // 전체 페이지 크롤링 계획 (역순)
    let overrides = PlanOverrides {
        batch_size: Some(20), // 빠른 스캔을 위해 배치 크기 증가
        concurrency: Some(app_config.user.max_concurrent_requests),
        delay_ms: None,
        start_page: Some(total_pages),
        end_page: Some(1),
        page_count: None,
    };
    
    let strategist = ManualPlanningStrategy;
    let (mut execution_plan, app_config, _site_status) = strategist.plan(&app, Some(&overrides)).await?;
    
    // 얕은 크롤링 모드 설정: ListPageCrawling만 수행
    execution_plan.list_only = true;
    
    info!("📋 Execution plan created: {} ranges (SHALLOW MODE: list_only=true)", execution_plan.crawling_ranges.len());
    
    // SessionActor 실행
    let (session_id, _exec_clone) = crate::commands::crawling::actor_system::bootstrap_and_spawn_session(
        &app,
        execution_plan.clone(),
        app_config.clone()
    ).await?;
    
    // 크롤링 완료 대기 (간단한 폴링)
    // Note: 실제 환경에서는 이벤트 리스너를 사용하는 것이 좋지만,
    // 여기서는 간단하게 구현
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let duration = start_time.elapsed().as_millis() as u64;
    
    // 최종 결과 조회
    let final_product_count = repo.get_product_count().await
        .map_err(|e| format!("Failed to get final product count: {}", e))? as u32;
    
    info!("✅ Shallow sync initiated: session={}, {} pages queued", session_id, total_pages);
    info!("💡 Crawling will complete in background. Check session status for progress.");
    
    Ok(ShallowCrawlResult {
        session_id,
        pages_scanned: total_pages,
        urls_synced: final_product_count,
        duration_ms: duration,
        status: "started".to_string(),
    })
}

/// 누락된 product_details 분석
///
/// products 테이블에는 있지만 product_details에는 없는 항목 추출
///
/// # Errors
/// Returns error if database query fails
#[tauri::command]
pub async fn analyze_missing_details(
    app_state: State<'_, AppState>,
) -> Result<MissingAnalysisResult, String> {
    info!("📊 Analyzing missing product details");
    
    let pool = app_state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);
    
    // Query: products에는 있지만 product_details에는 없는 URL
    let query = r"
        SELECT 
            p.url,
            p.page_id,
            p.index_in_page,
            p.manufacturer,
            p.model
        FROM products p
        LEFT JOIN product_details pd ON p.url = pd.url
        WHERE pd.url IS NULL
        ORDER BY p.page_id DESC, p.index_in_page ASC
    ";
    
    let rows = sqlx::query(query)
        .fetch_all(repo.pool())
        .await
        .map_err(|e| format!("Failed to query missing details: {}", e))?;
    
    let missing_details: Vec<MissingProductInfo> = rows
        .into_iter()
        .map(|row| MissingProductInfo {
            url: row.get("url"),
            page_id: row.get("page_id"),
            index_in_page: row.get("index_in_page"),
            manufacturer: row.get("manufacturer"),
            model: row.get("model"),
        })
        .collect();
    
    let total_products = repo.get_product_count().await
        .map_err(|e| format!("Failed to get product count: {}", e))? as u32;
    
    let missing_count = missing_details.len() as u32;
    let complete_products = total_products.saturating_sub(missing_count);
    
    info!(
        "📊 Missing analysis: total={}, complete={}, missing={}",
        total_products, complete_products, missing_count
    );
    
    Ok(MissingAnalysisResult {
        total_products,
        complete_products,
        missing_details,
    })
}

/// 스마트 동기화: 얕은 크롤링 → 진단 → 누락 제품만 보완 크롤링
///
/// # Errors
/// Returns error if any stage fails
#[tauri::command]
pub async fn start_smart_sync(
    app: tauri::AppHandle,
) -> Result<SmartSyncResult, String> {
    let start_time = std::time::Instant::now();
    info!("🧠 Starting smart sync with diagnostics");
    
    // Phase 1: 얕은 크롤링
    info!("📝 Phase 1: Shallow sync");
    let shallow_result = start_shallow_sync(app.clone()).await?;
    
    // Phase 2: 누락 분석
    info!("📝 Phase 2: Missing details analysis");
    let app_state = app.state::<AppState>();
    let missing_analysis = analyze_missing_details(app_state.clone()).await?;
    
    // Phase 3: 보완 크롤링 (누락된 URL만)
    let complement_result = if missing_analysis.missing_details.is_empty() {
        info!("✨ No missing details found - skipping complement crawl");
        None
    } else {
        info!(
            "📝 Phase 3: Complement crawl for {} missing products",
            missing_analysis.missing_details.len()
        );
        
        // TODO: 실제 보완 크롤링 구현
        // crawl_specific_urls(missing_analysis.missing_details)
        
        Some(ComplementCrawlResult {
            urls_targeted: missing_analysis.missing_details.len() as u32,
            urls_completed: 0,
            urls_failed: 0,
            duration_ms: 0,
        })
    };
    
    let total_duration = start_time.elapsed().as_millis() as u64;
    
    info!("🎉 Smart sync completed in {}ms", total_duration);
    
    Ok(SmartSyncResult {
        shallow_crawl: shallow_result,
        missing_analysis,
        complement_crawl: complement_result,
        total_duration_ms: total_duration,
    })
}
