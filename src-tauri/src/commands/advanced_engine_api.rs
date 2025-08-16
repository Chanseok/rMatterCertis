//! Advanced Crawling Engine 관련 status / 조회 전용 명령어 모듈
//! NOTE: start_advanced_crawling(실행 엔트리포인트)는 통합 Actor 진입점으로 완전히 이관되어 제거되었습니다.

use tauri::{AppHandle, Emitter, State, command};
use tracing::{error, info, warn};

use crate::application::shared_state::SharedStateCache;
use crate::application::state::AppState;
use crate::domain::services::crawling_services::StatusChecker;
use crate::infrastructure::IntegratedProductRepository;
use crate::types::frontend_api::*; // trait import for check_site_status

/// Advanced Crawling Engine 사이트 상태 확인 (실제 구현)
#[command]
pub async fn check_advanced_site_status(
    app: AppHandle,
    _app_state: State<'_, AppState>,
    shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<SiteStatusInfo>, String> {
    info!("🌐 Advanced site status check requested");

    // 🔥 독립적인 사이트 상태 체크 시작 이벤트 발송
    let start_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
        is_standalone: true,
        status: crate::domain::events::SiteCheckStatus::Started,
        message: "사이트 상태 확인을 시작합니다...".to_string(),
        timestamp: chrono::Utc::now(),
    };

    if let Err(e) = app.emit("site-status-check", &start_event) {
        warn!("Failed to emit site status check start event: {}", e);
    }

    // 먼저 캐시된 사이트 분석 결과 확인 (5분 TTL)
    if let Some(cached_analysis) = shared_state.get_valid_site_analysis_async(Some(5)).await {
        info!(
            "🎯 Using cached site analysis - analyzed: {}, age: {} minutes",
            cached_analysis.analyzed_at.format("%H:%M:%S"),
            chrono::Utc::now()
                .signed_duration_since(cached_analysis.analyzed_at)
                .num_minutes()
        );

        // 🔥 캐시 사용 완료 이벤트 발송
        let cache_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
            is_standalone: true,
            status: crate::domain::events::SiteCheckStatus::Success,
            message: "캐시된 사이트 분석 결과를 사용했습니다".to_string(),
            timestamp: chrono::Utc::now(),
        };

        if let Err(e) = app.emit("site-status-check", &cache_event) {
            warn!("Failed to emit cached site status event: {}", e);
        }

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

    // 🔥 실제 사이트 체크 진행 중 이벤트 발송
    let _progress_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
        is_standalone: true,
        status: crate::domain::events::SiteCheckStatus::InProgress,
        message: "사이트에 접속하여 상태를 확인 중입니다...".to_string(),
        timestamp: chrono::Utc::now(),
    };

    // 🔥 실제 사이트 체크 진행 중 이벤트 발송
    let progress_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
        is_standalone: true,
        status: crate::domain::events::SiteCheckStatus::InProgress,
        message: "사이트에 접속하여 상태를 확인 중입니다...".to_string(),
        timestamp: chrono::Utc::now(),
    };

    if let Err(e) = app.emit("site-status-check", &progress_event) {
        warn!("Failed to emit site status progress event: {}", e);
    }

    // 실제 사이트 상태 분석 (system_analysis 로직 재사용 경량 버전)
    use crate::application::shared_state::SiteAnalysisResult;
    use crate::domain::constants::site;
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
    let database_url = crate::infrastructure::get_main_database_url();
    let db_pool = sqlx::SqlitePool::connect(&database_url)
        .await
        .map_err(|e| format!("DB connect failed: {}", e))?;
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

    // 2. 사이트 상태 조회
    let site_status = status_checker
        .check_site_status()
        .await
        .map_err(|e| format!("Site status check failed: {}", e))?;

    // 3. 결과 캐시에 저장
    let analysis = SiteAnalysisResult::new(
        site_status.total_pages,
        site_status.products_on_last_page,
        site_status.estimated_products,
        site::BASE_URL.to_string(),
        1.0,
    );
    shared_state.set_site_analysis(analysis.clone()).await;

    // 4. 성공 이벤트 발송
    let success_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
        is_standalone: true,
        status: crate::domain::events::SiteCheckStatus::Success,
        message: format!(
            "사이트 분석 완료: pages={} last_page_products={} est_products={}",
            site_status.total_pages,
            site_status.products_on_last_page,
            site_status.estimated_products
        ),
        timestamp: chrono::Utc::now(),
    };
    if let Err(e) = app.emit("site-status-check", &success_event) {
        warn!("Failed to emit site status success event: {}", e);
    }

    // 5. 응답 변환
    let site_status_info = SiteStatusInfo {
        is_accessible: true,
        response_time_ms: site_status.response_time_ms,
        total_pages: site_status.total_pages,
        products_on_last_page: site_status.products_on_last_page,
        estimated_total_products: site_status.estimated_products,
        health_score: 1.0,
    };
    Ok(ApiResponse::success(site_status_info))
}

// (start_advanced_crawling 제거됨)

/// 최근 제품 목록 조회 (실제 데이터베이스)
#[command]
pub async fn get_recent_products(
    page: Option<u32>,
    limit: Option<u32>,
    app_state: State<'_, AppState>,
    _shared_state: State<'_, SharedStateCache>,
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
        match pool_guard.as_ref() {
            Some(pool) => pool.clone(),
            None => {
                error!("Database pool is not initialized");
                return Err("Database pool is not available".to_string());
            }
        }
    };
    let product_repo = IntegratedProductRepository::new(database_pool);

    // 실제 데이터베이스에서 제품 목록 조회
    match product_repo
        .get_products_paginated(page as i32, limit as i32)
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
                Ok(count) => count as u32,
                Err(e) => {
                    warn!("Failed to get total product count: {}", e);
                    0
                }
            };

            let total_pages = (total_items + limit - 1) / limit; // 올림 계산

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
pub async fn get_database_stats(
    app_state: State<'_, AppState>,
    _shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<DatabaseStats>, String> {
    info!("📊 Fetching real database statistics");

    // AppState에서 중앙화된 데이터베이스 풀 사용
    let database_pool = {
        let pool_guard = app_state.database_pool.read().await;
        match pool_guard.as_ref() {
            Some(pool) => pool.clone(),
            None => {
                error!("Database pool is not initialized");
                return Err("Database pool is not available".to_string());
            }
        }
    };
    let product_repo = IntegratedProductRepository::new(database_pool);

    // 실제 데이터베이스 통계 조회
    match product_repo.get_database_statistics().await {
        Ok(_db_stats) => {
            // 총 제품 수 조회
            let total_products = match product_repo.get_product_count().await {
                Ok(count) => count as u32,
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
