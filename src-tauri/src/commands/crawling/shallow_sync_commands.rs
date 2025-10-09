// 얕은 크롤링 및 진단 기반 보완 크롤링 Commands
//
// 목적: 전체 페이지 URL + 좌표 동기화 후 누락 제품 감지 및 선택적 크롤링

use crate::application::AppState;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::domain::services::crawling_services::StatusChecker;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
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
    let repo = Arc::new(IntegratedProductRepository::new(pool));
    
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
        Arc::clone(&repo),
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
/// 1. products 테이블에는 있지만 product_details에는 없는 항목
/// 2. product_details는 있지만 certification_date가 NULL인 항목
///
/// # Errors
/// Returns error if database query fails
#[tauri::command]
pub async fn analyze_missing_details(
    app_state: State<'_, AppState>,
) -> Result<MissingAnalysisResult, String> {
    info!("📊 Analyzing missing product details (including NULL certification_date)");
    
    let pool = app_state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);
    
    // Query: 
    // 1. products에는 있지만 product_details에는 없는 URL
    // 2. product_details는 있지만 다음 필드 중 하나라도 NULL인 경우:
    //    - certification_date
    //    - transport_interface
    //    - primary_device_type_ids
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
           OR pd.certification_date IS NULL
           OR pd.transport_interface IS NULL
           OR pd.primary_device_type_ids IS NULL
           OR pd.primary_device_type_ids = ''
           OR pd.primary_device_type_ids = '[]'
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

/// 보완 크롤링: certification_date가 NULL인 제품들의 상세 정보를 실제로 재크롤링
///
/// # Errors
/// Returns error if crawling fails
#[tauri::command]
pub async fn start_complement_crawl(
    app: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<ComplementCrawlResult, String> {
    let start_time = std::time::Instant::now();
    info!("🔧 Starting complement crawl for incomplete products (missing cert_date, transport, or device_type_ids)");
    
    // Phase 1: 핵심 필드가 누락된 제품 분석
    let pool = app_state.get_database_pool().await?;
    let repo = crate::infrastructure::integrated_product_repository::IntegratedProductRepository::new(pool.clone());
    
    // certification_date, transport_interface, primary_device_type_ids 중 하나라도 NULL인 제품 조회
    let query = r"
        SELECT 
            pd.url,
            pd.page_id,
            pd.index_in_page,
            p.manufacturer,
            p.model
        FROM product_details pd
        INNER JOIN products p ON pd.url = p.url
        WHERE pd.certification_date IS NULL
           OR pd.transport_interface IS NULL
           OR pd.primary_device_type_ids IS NULL
           OR pd.primary_device_type_ids = ''
           OR pd.primary_device_type_ids = '[]'
        ORDER BY pd.page_id DESC, pd.index_in_page ASC
    ";
    
    let rows = sqlx::query(query)
        .fetch_all(repo.pool())
        .await
        .map_err(|e| format!("Failed to query incomplete products: {}", e))?;
    
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
    
    if missing_details.is_empty() {
        info!("✨ No incomplete products found (all have cert_date, transport, and device_type_ids)");
        return Ok(ComplementCrawlResult {
            urls_targeted: 0,
            urls_completed: 0,
            urls_failed: 0,
            duration_ms: start_time.elapsed().as_millis() as u64,
        });
    }
    
    info!("📝 Found {} incomplete products (missing cert_date/transport/device_type_ids), starting concurrent re-crawl", missing_details.len());
    
    // Phase 2: 크롤링 인프라 준비
    // HTTP 클라이언트 및 데이터 추출기 생성
    use crate::infrastructure::{HttpClient, MatterDataExtractor};
    
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    // ProductDetailCollector 설정
    use crate::infrastructure::crawling_service_impls::{ProductDetailCollectorImpl, CollectorConfig};
    use std::sync::Arc;
    use std::time::Duration;
    
    let config_guard = app_state.config.read().await;
    let app_config = config_guard.clone();
    drop(config_guard);
    
    // 동시성 설정: 최대 18개 동시 요청 (성능 최적화)
    let concurrency = 18.min(missing_details.len()); // 최대 18개 또는 총 제품 수
    let detail_config = CollectorConfig {
        batch_size: concurrency as u32,
        max_concurrent: concurrency as u32,
        concurrency: concurrency as u32,
        delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
        delay_ms: app_config.user.request_delay_ms,
        retry_attempts: 3,
        retry_max: 3,
    };
    
    let collector = ProductDetailCollectorImpl::new(
        Arc::new(http_client),
        Arc::new(data_extractor),
        detail_config,
    );
    
    let urls_targeted = missing_details.len() as u32;
    let mut completed = 0;
    let mut failed = 0;
    
    // Phase 3: 배치로 나누어 병렬 크롤링 수행
    use crate::domain::product_url::ProductUrl;
    use tokio_util::sync::CancellationToken;
    
    let batch_size = concurrency;
    let total_batches = (missing_details.len() + batch_size - 1) / batch_size;
    
    // 🎯 SessionStarted 이벤트 발행
    let session_id = format!("complement-crawl-{}", chrono::Utc::now().timestamp_millis());
    let _ = app.emit("actor-event", serde_json::json!({
        "variant": "SessionStarted",
        "session_id": &session_id,
        "total_products": urls_targeted,
        "total_batches": total_batches,
        "concurrency": concurrency,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }));
    
    for (batch_idx, chunk) in missing_details.chunks(batch_size).enumerate() {
        info!("🔄 Processing batch {}/{} ({} products)", batch_idx + 1, total_batches, chunk.len());
        
        // ProductUrl 배열 생성
        let product_urls: Vec<ProductUrl> = chunk.iter().map(|product_info| {
            ProductUrl {
                url: product_info.url.clone(),
                page_id: product_info.page_id.unwrap_or(0),
                index_in_page: product_info.index_in_page.unwrap_or(0),
            }
        }).collect();
        
        // 배치 전체를 한 번에 크롤링 (병렬 처리)
        let batch_session_id = format!("{}-batch-{}", session_id, batch_idx);
        let batch_id = format!("batch-{}", batch_idx);
        
        match collector.collect_details_with_async_events(
            &product_urls,
            Some(CancellationToken::new()),
            batch_session_id,
            batch_id,
        ).await {
            Ok(details) => {
                let details_count = details.len();
                info!("✅ Batch {}/{} collected {} details", batch_idx + 1, total_batches, details_count);
                
                // 수집된 각 제품을 DB에 저장
                for detail in details {
                    match repo.create_or_update_product_detail(&detail).await {
                        Ok(_) => {
                            completed += 1;
                            info!("💾 Saved: {}", detail.url);
                        }
                        Err(e) => {
                            failed += 1;
                            tracing::warn!("❌ Failed to save: {} - {}", detail.url, e);
                        }
                    }
                }
                
                // 수집 실패한 제품 계산
                let batch_failed = chunk.len() - details_count;
                if batch_failed > 0 {
                    failed += batch_failed as u32;
                    tracing::warn!("⚠️ Batch {}/{}: {} products failed to crawl", 
                                   batch_idx + 1, total_batches, batch_failed);
                }
            }
            Err(e) => {
                failed += chunk.len() as u32;
                tracing::warn!("❌ Batch {}/{} failed entirely: {}", batch_idx + 1, total_batches, e);
            }
        }
        
        // 🎯 StageProgress 이벤트 발행 (배치 완료 시마다)
        let _ = app.emit("actor-event", serde_json::json!({
            "variant": "StageProgress",
            "session_id": &session_id,
            "current_batch": batch_idx + 1,
            "total_batches": total_batches,
            "completed_products": completed,
            "failed_products": failed,
            "progress_percentage": ((batch_idx + 1) as f64 / total_batches as f64 * 100.0) as u32,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));
        
        // 배치 간 짧은 대기 (rate limiting)
        if batch_idx < total_batches - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    
    let duration_ms = start_time.elapsed().as_millis() as u64;
    
    info!(
        "🎉 Complement crawl completed: {}/{} successful, {} failed ({}ms, {} batches with concurrency={})",
        completed, urls_targeted, failed, duration_ms, total_batches, concurrency
    );
    
    // 🎯 SessionCompleted 이벤트 발행
    let _ = app.emit("actor-event", serde_json::json!({
        "variant": "SessionCompleted",
        "session_id": &session_id,
        "total_products": urls_targeted,
        "completed_products": completed,
        "failed_products": failed,
        "duration_ms": duration_ms,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }));
    
    Ok(ComplementCrawlResult {
        urls_targeted,
        urls_completed: completed,
        urls_failed: failed,
        duration_ms,
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
