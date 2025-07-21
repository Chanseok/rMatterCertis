use tauri::State;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use tracing::info;
use sqlx::SqlitePool;

use crate::infrastructure::config::ConfigManager;
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::application::AppState;

/// 크롤링 세션 정보 (간소화)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingSession {
    pub session_id: String,
    pub started_at: String,
    pub status: String,
}

/// Smart Crawling 시작 - 설정 파일 기반 자동 실행
#[tauri::command]
pub async fn start_smart_crawling(_state: State<'_, AppState>) -> Result<CrawlingSession, String> {
    // 1. 설정 파일 자동 로딩
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config from file: {}", e))?;

    info!("🚀 Starting smart crawling with {} max pages, {}ms delay", 
          config.user.crawling.page_range_limit, config.user.request_delay_ms);

    info!("✅ Config loaded successfully: max_pages={}, request_delay={}ms", 
          config.user.crawling.page_range_limit, config.user.request_delay_ms);

        // 2. 데이터베이스 연결 및 레포지터리 생성
    let database_url = {
        let app_data_dir = std::env::var("HOME")
            .map_err(|_| "Failed to get HOME directory".to_string())?;
        let data_dir = format!("{}/Library/Application Support/matter-certis-v2", app_data_dir);
        format!("sqlite:{}/matter_certis.db", data_dir)
    };

    let pool = SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
    let product_repo = IntegratedProductRepository::new(pool);

    // 3. 크롤링 범위 계산
    let range_calculator = CrawlingRangeCalculator::new(
        std::sync::Arc::new(product_repo),
        config.clone(),
    );

    // 임시로 총 페이지 수와 마지막 페이지 제품 수를 하드코딩 (나중에 사이트 분석으로 대체)
    let total_pages = 485u32;
    let products_on_last_page = 11u32;

    let range_result = range_calculator.calculate_next_crawling_range(
        total_pages,
        products_on_last_page,
    ).await
    .map_err(|e| format!("Failed to calculate crawling range: {}", e))?;

    match range_result {
        Some((start_page, end_page)) => {
            let session_id = format!("session_{}", chrono::Utc::now().timestamp());
            let started_at = chrono::Utc::now().to_rfc3339();
            
            info!("✅ Smart crawling session created: {} (pages {} → {})", 
                  session_id, start_page, end_page);

            // TODO: 실제 크롤링 시작 (Actor 시스템 통합)
            
            Ok(CrawlingSession {
                session_id,
                started_at,
                status: "started".to_string(),
            })
        }
        None => {
            info!("🏁 No more pages to crawl");
            Err("모든 페이지가 이미 크롤링되었습니다.".to_string())
        }
    }
}
