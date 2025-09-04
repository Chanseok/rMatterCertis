#![allow(clippy::used_underscore_binding)]
//! Advanced Crawling Engine 관련 status / 조회 전용 명령어 모듈
//! NOTE: `start_advanced_crawling(실행` 엔트리포인트)는 통합 Actor 진입점으로 완전히 이관되어 제거되었습니다.

use tauri::{AppHandle, State, command};
use tracing::{error, info, warn};

use crate::application::shared_state::SharedStateCache;
use crate::application::state::AppState;
use crate::infrastructure::IntegratedProductRepository;
use crate::api::frontend_api::{ApiResponse, SiteStatusInfo, ProductPage, ProductInfo, DatabaseStats}; // trait import for check_site_status
use crate::application::shared_state::SiteAnalysisResult;
use crate::domain::constants::site;

/// Advanced Crawling Engine 사이트 상태 확인 (실제 구현)
#[command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if configuration, HTTP, or database operations fail during site status check.
pub async fn check_advanced_site_status(
    app: AppHandle,
    #[allow(clippy::used_underscore_binding)] _app_state: State<'_, AppState>,
    shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<SiteStatusInfo>, String> {
    info!("🌐 Advanced site status check requested");

    // 시작: unified actor-event를 사용하므로 별도 레거시 이벤트 발신은 생략합니다 (로그만 남김)

    // 먼저 캐시된 사이트 분석 결과 확인 (5분 TTL)
    if let Some(cached_analysis) = shared_state.get_valid_site_analysis_async(Some(5)).await {
        info!(
            "🎯 Using cached site analysis - analyzed: {}, age: {} minutes",
            cached_analysis.analyzed_at.format("%H:%M:%S"),
            chrono::Utc::now()
                .signed_duration_since(cached_analysis.analyzed_at)
                .num_minutes()
        );

    // 캐시 히트: 이벤트 발신 대신 로그만 남깁니다

        let site_status_info = SiteStatusInfo {
            is_accessible: true,
            response_time_ms: 500, // 기본값 - 캐시된 데이터이므로
            total_pages: cached_analysis.total_pages,
            products_on_last_page: cached_analysis.products_on_last_page,
            estimated_total_products: cached_analysis.estimated_products,
            health_score: cached_analysis.health_score,
        };
        return Ok(ApiResponse::success(site_status_info));
    }

    info!("⏰ No valid cached site analysis found - performing fresh site check");
    info!("🔄 Starting real site status check...");

    // 진행 로그만 남깁니다 (actor-event 브릿지가 진짜 진행 이벤트를 처리합니다)

    // 실제 사이트 상태 분석 (system_analysis 로직 재사용 경량 버전)
    // 1. 구성 로드 및 의존성 생성
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Config init failed: {}", e))?;
    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Config load failed: {}", e))?;
    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("HTTP client create failed: {}", e))?;
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Data extractor create failed: {}", e))?;
    let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("DB pool failed: {}", e))?;
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

    // 2. 사이트 상태 조회 (SharedStateCache single-flight 사용)
    let site_analysis_cached = shared_state
        .get_or_refresh_site_analysis_singleflight(Some(5), std::sync::Arc::new(status_checker))
        .await
        .map_err(|e| format!("Site status refresh failed: {}", e))?;
    let site_status = crate::domain::services::SiteStatus {
        is_accessible: true,
        response_time_ms: 0,
        total_pages: site_analysis_cached.total_pages,
        estimated_products: site_analysis_cached.estimated_products,
        products_on_last_page: site_analysis_cached.products_on_last_page,
        last_check_time: site_analysis_cached.analyzed_at,
        health_score: site_analysis_cached.health_score,
        data_change_status:
            crate::domain::services::crawling_services::SiteDataChangeStatus::Stable {
                count: site_analysis_cached.estimated_products,
            },
        decrease_recommendation: None,
        crawling_range_recommendation:
            crate::domain::services::crawling_services::CrawlingRangeRecommendation::Full,
    };

    // 3. 결과 캐시에 저장
    // 이미 single-flight에서 캐시에 저장됨. 필요 시 그대로 사용
    let analysis = SiteAnalysisResult::new(
        site_status.total_pages,
        site_status.products_on_last_page,
        site_status.estimated_products,
        site::BASE_URL.to_string(),
        site_status.health_score,
    );
    shared_state.set_site_analysis(analysis.clone()).await;

    // 성공: unified 경로 사용으로 레거시 이벤트는 발신하지 않습니다

    // 5. 응답 변환
    let site_status_info = SiteStatusInfo {
        is_accessible: true,
        response_time_ms: site_status.response_time_ms,
        total_pages: site_status.total_pages,
        products_on_last_page: site_status.products_on_last_page,
        estimated_total_products: site_status.estimated_products,
        health_score: site_status.health_score,
    };
    Ok(ApiResponse::success(site_status_info))
}

// (start_advanced_crawling 제거됨)

/// 최근 제품 목록 조회 (실제 데이터베이스)
#[command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if database queries fail.
pub async fn get_recent_products(
    page: Option<u32>,
    limit: Option<u32>,
    app_state: State<'_, AppState>,
    #[allow(clippy::used_underscore_binding)] _shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<ProductPage>, String> {
    let page = page.unwrap_or(1);
    let limit = limit.unwrap_or(20);

    info!(
        "📋 Fetching recent products from real database - page: {}, limit: {}",
        page, limit
    );

    // AppState에서 중앙화된 데이터베이스 풀 사용
    let database_pool = {
        let pool_guard = app_state.database_pool.read().await;
        if let Some(pool) = pool_guard.as_ref() {
            pool.clone()
        } else {
            error!("Database pool is not initialized");
            return Err("Database pool is not available".to_string());
        }
    };
    let product_repo = IntegratedProductRepository::new(database_pool);

    // 실제 데이터베이스에서 제품 목록 조회
    let page_i32 = i32::try_from(page).unwrap_or(i32::MAX);
    let limit_i32 = i32::try_from(limit).unwrap_or(i32::MAX);
    match product_repo
        .get_products_paginated(page_i32, limit_i32)
        .await
    {
        Ok(products) => {
            // Product를 ProductInfo로 변환
            let product_infos: Vec<ProductInfo> = products
                .into_iter()
                .map(|product| {
                    ProductInfo {
                        id: product.url.clone(), // URL을 ID로 사용
                        url: product.url,
                        name: product
                            .model
                            .unwrap_or_else(|| "Unknown Product".to_string()),
                        company: product
                            .manufacturer
                            .unwrap_or_else(|| "Unknown Company".to_string()),
                        certification_number: product
                            .certificate_id
                            .unwrap_or_else(|| "N/A".to_string()),
                        description: None, // Product 구조체에 description 필드가 없는 경우
                        created_at: product.created_at,
                        updated_at: Some(product.updated_at),
                    }
                })
                .collect();

            // 총 제품 수 조회
            let total_items = match product_repo.get_product_count().await {
                Ok(count) => u32::try_from(count).unwrap_or(0),
                Err(e) => {
                    warn!("Failed to get total product count: {}", e);
                    0
                }
            };

            let total_pages = total_items.div_ceil(limit); // 올림 계산

            let product_page = ProductPage {
                products: product_infos,
                current_page: page,
                page_size: limit,
                total_items,
                total_pages,
            };

            info!(
                "✅ Retrieved {} real products from database",
                product_page.products.len()
            );
            Ok(ApiResponse::success(product_page))
        }
        Err(e) => {
            error!("Failed to fetch products from database: {}", e);
            Err(format!("Database query failed: {}", e))
        }
    }
}

/// 데이터베이스 통계 조회 (실제 데이터베이스)
#[command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if database queries fail.
pub async fn get_database_stats(
    app_state: State<'_, AppState>,
    #[allow(clippy::used_underscore_binding)] _shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<DatabaseStats>, String> {
    info!("📊 Fetching real database statistics");

    // AppState에서 중앙화된 데이터베이스 풀 사용
    let database_pool = {
        let pool_guard = app_state.database_pool.read().await;
        if let Some(pool) = pool_guard.as_ref() {
            pool.clone()
        } else {
            error!("Database pool is not initialized");
            return Err("Database pool is not available".to_string());
        }
    };
    let product_repo = IntegratedProductRepository::new(database_pool);

    // 실제 데이터베이스 통계 조회
    match product_repo.get_database_statistics().await {
        Ok(_db_stats) => {
            // 총 제품 수 조회
            let total_products = match product_repo.get_product_count().await {
                Ok(count) => u32::try_from(count).unwrap_or(0),
                Err(e) => {
                    warn!("Failed to get product count: {}", e);
                    0
                }
            };

            // 오늘 추가된 제품 수 조회 (최근 24시간 내)
            let products_added_today = 0; // IntegratedProductRepository에 해당 메서드가 없으므로 0으로 설정

            // 마지막 업데이트 시간 조회
            let last_updated = match product_repo.get_latest_updated_product().await {
                Ok(Some(product)) => Some(product.updated_at),
                Ok(None) => None,
                Err(e) => {
                    warn!("Failed to get last updated time: {}", e);
                    None
                }
            };

            let database_stats = DatabaseStats {
                total_products,
                products_added_today,
                last_updated,
                database_size_bytes: 0, // 계산이 복잡하므로 기본값
            };

            info!(
                "✅ Retrieved real database statistics: {} products",
                total_products
            );
            Ok(ApiResponse::success(database_stats))
        }
        Err(e) => {
            error!("Failed to fetch database statistics: {}", e);
            Err(format!("Database statistics query failed: {}", e))
        }
    }
}
