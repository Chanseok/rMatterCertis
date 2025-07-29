//! Advanced Crawling Engine 프론트엔드 연동 Commands (실제 구현 버전)
//! ts-rs로 생성된 TypeScript 타입과 연동되는 Tauri 명령어들

use tauri::{command, AppHandle, State, Emitter};
use tracing::{info, warn, error};
use uuid::Uuid;
use std::sync::Arc;

use crate::types::frontend_api::*;
use crate::commands::crawling_v4::CrawlingEngineState;
use crate::application::shared_state::SharedStateCache;
use crate::infrastructure::{
    AdvancedBatchCrawlingEngine, HttpClient, MatterDataExtractor, 
    IntegratedProductRepository, DatabaseConnection
};
use crate::infrastructure::service_based_crawling_engine::BatchCrawlingConfig;
use crate::application::EventEmitter;

/// Advanced Crawling Engine 사이트 상태 확인 (실제 구현)
#[command]
pub async fn check_advanced_site_status(
    app: AppHandle,
    _state: State<'_, CrawlingEngineState>,
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
        info!("🎯 Using cached site analysis - analyzed: {}, age: {} minutes", 
             cached_analysis.analyzed_at.format("%H:%M:%S"),
             chrono::Utc::now().signed_duration_since(cached_analysis.analyzed_at).num_minutes());
        
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
    let progress_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
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
    
    // 캐시가 없거나 만료된 경우, 실제 크롤링 엔진 인스턴스 생성
    let http_client = match HttpClient::create_from_global_config() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create HTTP client: {}", e);
            
            // 🔥 실패 이벤트 발송
            let failed_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
                is_standalone: true,
                status: crate::domain::events::SiteCheckStatus::Failed,
                message: format!("HTTP 클라이언트 생성 실패: {}", e),
                timestamp: chrono::Utc::now(),
            };
            let _ = app.emit("site-status-check", &failed_event);
            
            return Err(format!("HTTP client creation failed: {}", e));
        }
    };
    
    let data_extractor = match MatterDataExtractor::new() {
        Ok(extractor) => extractor,
        Err(e) => {
            error!("Failed to create data extractor: {}", e);
            return Err(format!("Data extractor creation failed: {}", e));
        }
    };
    
    // 데이터베이스 연결 생성
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/matter_certis.db".to_string());
    
    let db_connection = match DatabaseConnection::new(&database_url).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Database connection failed: {}", e);
            return Err(format!("Database connection error: {}", e));
        }
    };
    
    let product_repo = Arc::new(IntegratedProductRepository::new(db_connection.pool().clone()));
    
    // Advanced 크롤링 엔진 생성
    let config = BatchCrawlingConfig {
        start_page: 1,
        end_page: 1, // 상태 확인용으로 1페이지만
        batch_size: 10,
        concurrency: 1,
        list_page_concurrency: 1,
        product_detail_concurrency: 1,
        delay_ms: 1000,
        retry_max: 3,
        timeout_ms: 30000,
        cancellation_token: None,
    };
    
    let session_id = format!("status_check_{}", Uuid::new_v4().simple());
    let event_emitter = Arc::new(None::<EventEmitter>);
    
    let engine = AdvancedBatchCrawlingEngine::new(
        http_client,
        data_extractor,
        product_repo,
        event_emitter,
        config,
        session_id,
    );
    
    info!("🚀 Starting real site analysis...");
    
    // 실제 사이트 상태 확인
    match engine.stage0_check_site_status().await {
        Ok(site_status) => {
            info!("✅ Fresh site status check completed - {} pages found", site_status.total_pages);
            
            // 🔥 성공 이벤트 발송
            let success_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
                is_standalone: true,
                status: crate::domain::events::SiteCheckStatus::Success,
                message: format!("사이트 상태 확인 완료: {}개 페이지 발견", site_status.total_pages),
                timestamp: chrono::Utc::now(),
            };
            
            if let Err(e) = app.emit("site-status-check", &success_event) {
                warn!("Failed to emit site status success event: {}", e);
            }
            
            // 새로운 분석 결과를 캐시에 저장
            let site_analysis = crate::application::shared_state::SiteAnalysisResult::new(
                site_status.total_pages,
                site_status.products_on_last_page,
                site_status.estimated_products,
                "https://iotready.kr".to_string(), // site_url
                1.0, // health_score
            );
            shared_state.set_site_analysis(site_analysis).await;
            
            let site_status_info = SiteStatusInfo {
                is_accessible: true,
                response_time_ms: 500, // 기본값
                total_pages: site_status.total_pages,
                products_on_last_page: site_status.products_on_last_page,
                estimated_total_products: site_status.estimated_products,
                health_score: 1.0,
            };
            
            info!("✅ Fresh site status check completed and cached");
            Ok(ApiResponse::success(site_status_info))
        },
        Err(e) => {
            error!("Site status check failed: {}", e);
            
            // 🔥 실패 이벤트 발송
            let failed_event = crate::domain::events::CrawlingEvent::SiteStatusCheck {
                is_standalone: true,
                status: crate::domain::events::SiteCheckStatus::Failed,
                message: format!("사이트 상태 확인 실패: {}", e),
                timestamp: chrono::Utc::now(),
            };
            
            if let Err(emit_err) = app.emit("site-status-check", &failed_event) {
                warn!("Failed to emit site status failed event: {}", emit_err);
            }
            
            Err(format!("Site status check error: {}", e))
        }
    }
}

/// Advanced Crawling Engine 실제 크롤링 실행
#[command]
pub async fn start_advanced_crawling(
    request: StartCrawlingRequest,
    app: AppHandle,
    _state: State<'_, CrawlingEngineState>,
    _shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<CrawlingSession>, String> {
    let session_id = format!("advanced_{}", Uuid::new_v4().simple());
    info!("🚀 Starting real advanced crawling session: {}", session_id);
    
    // 실제 크롤링 엔진 인스턴스 생성
    let http_client = match HttpClient::create_from_global_config() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create HTTP client: {}", e);
            return Err(format!("HTTP client creation failed: {}", e));
        }
    };
    
    let data_extractor = match MatterDataExtractor::new() {
        Ok(extractor) => extractor,
        Err(e) => {
            error!("Failed to create data extractor: {}", e);
            return Err(format!("Data extractor creation failed: {}", e));
        }
    };
    
    // 데이터베이스 연결 생성
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/matter_certis.db".to_string());
    
    let db_connection = match DatabaseConnection::new(&database_url).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Database connection failed: {}", e);
            return Err(format!("Database connection error: {}", e));
        }
    };
    
    let product_repo = Arc::new(IntegratedProductRepository::new(db_connection.pool().clone()));
    
    // 프론트엔드 설정을 BatchCrawlingConfig로 변환
    let config = BatchCrawlingConfig {
        start_page: request.config.start_page,
        end_page: request.config.end_page,
        batch_size: request.config.batch_size,
        concurrency: request.config.concurrency,
        list_page_concurrency: request.config.concurrency,
        product_detail_concurrency: request.config.concurrency,
        delay_ms: request.config.delay_ms,
        retry_max: request.config.retry_max,
        timeout_ms: 30000,
        cancellation_token: None,
    };
    
    // 세션 정보 생성
    let session = CrawlingSession {
        session_id: session_id.clone(),
        started_at: chrono::Utc::now(),
        config: request.config,
        status: SessionStatus::Running,
        total_products_processed: 0,
        success_rate: 0.0,
    };
    
    // EventEmitter 설정 (앱 핸들을 사용하여 이벤트 발송)
    let event_emitter = Arc::new(Some(EventEmitter::new(app.clone())));
    
    // Advanced 크롤링 엔진 생성
    let engine = AdvancedBatchCrawlingEngine::new(
        http_client,
        data_extractor,
        product_repo,
        event_emitter,
        config,
        session_id.clone(),
    );
    
    // 백그라운드에서 실제 크롤링 실행
    let session_clone = session.clone();
    let app_clone = app.clone();
    let session_id_for_spawn = session_id.clone();
    
    tokio::spawn(async move {
        info!("🔄 Starting real advanced crawling execution for session: {}", session_id_for_spawn);
        
        // 실제 크롤링 엔진 실행
        match engine.execute().await {
            Ok(()) => {
                info!("✅ Real advanced crawling completed successfully for session: {}", session_id_for_spawn);
                
                // 성공 완료 이벤트 발송
                if let Err(e) = app_clone.emit("crawling-completed", &session_clone) {
                    warn!("Failed to emit crawling-completed event: {}", e);
                }
            },
            Err(e) => {
                error!("❌ Real advanced crawling failed for session {}: {}", session_id_for_spawn, e);
                
                // 실패 이벤트 발송
                let mut failed_session = session_clone;
                failed_session.status = SessionStatus::Failed;
                
                if let Err(emit_err) = app_clone.emit("crawling-failed", &failed_session) {
                    warn!("Failed to emit crawling-failed event: {}", emit_err);
                }
            }
        }
    });
    
    // 즉시 세션 정보 반환
    info!("✅ Real advanced crawling session started: {}", session_id);
    Ok(ApiResponse::success(session))
}

/// 최근 제품 목록 조회 (실제 데이터베이스)
#[command]
pub async fn get_recent_products(
    page: Option<u32>,
    limit: Option<u32>,
    _state: State<'_, CrawlingEngineState>,
    _shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<ProductPage>, String> {
    let page = page.unwrap_or(1);
    let limit = limit.unwrap_or(20);
    
    info!("📋 Fetching recent products from real database - page: {}, limit: {}", page, limit);
    
    // 데이터베이스 연결 생성 (v4와 동일한 경로 사용)
    let database_url = match crate::commands::crawling_v4::get_database_url_v4() {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to get database URL: {}", e);
            return Err(format!("Database URL error: {}", e));
        }
    };
    
    let db_connection = match DatabaseConnection::new(&database_url).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Database connection failed: {}", e);
            return Err(format!("Database connection error: {}", e));
        }
    };
    
    let product_repo = IntegratedProductRepository::new(db_connection.pool().clone());
    
    // 실제 데이터베이스에서 제품 목록 조회
    match product_repo.get_products_paginated(page as i32, limit as i32).await {
        Ok(products) => {
            // Product를 ProductInfo로 변환
            let product_infos: Vec<ProductInfo> = products.into_iter().map(|product| {
                ProductInfo {
                    id: product.url.clone(), // URL을 ID로 사용
                    url: product.url,
                    name: product.model.unwrap_or_else(|| "Unknown Product".to_string()),
                    company: product.manufacturer.unwrap_or_else(|| "Unknown Company".to_string()),
                    certification_number: product.certificate_id.unwrap_or_else(|| "N/A".to_string()),
                    description: None, // Product 구조체에 description 필드가 없는 경우
                    created_at: product.created_at,
                    updated_at: Some(product.updated_at),
                }
            }).collect();
            
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
            
            info!("✅ Retrieved {} real products from database", product_page.products.len());
            Ok(ApiResponse::success(product_page))
        },
        Err(e) => {
            error!("Failed to fetch products from database: {}", e);
            Err(format!("Database query failed: {}", e))
        }
    }
}

/// 데이터베이스 통계 조회 (실제 데이터베이스)
#[command]
pub async fn get_database_stats(
    _state: State<'_, CrawlingEngineState>,
    _shared_state: State<'_, SharedStateCache>,
) -> Result<ApiResponse<DatabaseStats>, String> {
    info!("📊 Fetching real database statistics");
    
    // 데이터베이스 연결 생성 (v4와 동일한 경로 사용)
    let database_url = match crate::commands::crawling_v4::get_database_url_v4() {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to get database URL: {}", e);
            return Err(format!("Database URL error: {}", e));
        }
    };
    
    let db_connection = match DatabaseConnection::new(&database_url).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Database connection failed: {}", e);
            return Err(format!("Database connection error: {}", e));
        }
    };
    
    let product_repo = IntegratedProductRepository::new(db_connection.pool().clone());
    
    // 실제 데이터베이스 통계 조회
    match product_repo.get_database_statistics().await {
        Ok(db_stats) => {
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
            
            info!("✅ Retrieved real database statistics: {} products", total_products);
            Ok(ApiResponse::success(database_stats))
        },
        Err(e) => {
            error!("Failed to fetch database statistics: {}", e);
            Err(format!("Database statistics query failed: {}", e))
        }
    }
}
