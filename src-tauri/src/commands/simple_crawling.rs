use tauri::State;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use tracing::info;

use crate::infrastructure::config::ConfigManager;
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
pub async fn start_smart_crawling(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>
) -> Result<CrawlingSession, String> {
    let session_id = format!("session_{}", chrono::Utc::now().timestamp());
    let started_at = chrono::Utc::now().to_rfc3339();
    
    info!("🚀 Starting smart crawling session: {} (설정 파일 기반 자율 동작)", session_id);
    
    // 🎯 설계 문서 준수: 파라미터 없이 설정 파일만으로 동작
    // 1. 설정 파일 자동 로딩 (matter_certis_config.json)
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config from file: {}", e))?;

    info!("✅ Config loaded from files: max_pages={}, request_delay={}ms", 
          config.user.crawling.page_range_limit, config.user.request_delay_ms);

    // 2. Actor 시스템에 세션 시작 명령 전송 (설계 문서 준수)
    // TODO: SessionActor → BatchActor → StageActor 계층적 구조 사용
    // use crate::new_architecture::actor_system::SessionActor;
    // let session_command = ActorCommand::StartSession { session_id: session_id.clone() };
    // session_actor.send(session_command).await?;
    
    // 3. 임시: 직접 크롤링 실행 (나중에 Actor 시스템으로 교체)
    info!("⚠️ 임시 구현: Actor 시스템 대신 직접 실행 (추후 설계 문서 준수로 변경 필요)");
    
    // ServiceBasedBatchCrawlingEngine 사용 (임시)
    use crate::commands::crawling_v4::{CrawlingEngineState, execute_crawling_with_range, init_crawling_engine};
    use tauri::Manager;
    
    if let Some(engine_state) = app_handle.try_state::<CrawlingEngineState>() {
        // 엔진 초기화 확인
        {
            let engine_guard = engine_state.engine.read().await;
            if engine_guard.is_none() {
                drop(engine_guard);
                info!("🔧 Initializing crawling engine...");
                
                match init_crawling_engine(app_handle.clone(), engine_state.clone()).await {
                    Ok(response) => {
                        if !response.success {
                            return Err(format!("Engine initialization failed: {}", response.message));
                        }
                    }
                    Err(e) => return Err(format!("Engine initialization error: {}", e)),
                }
            }
        }
        
        // 🎯 guide/re-arch-plan-final2.md 설계 준수: 설정 파일 완전 의존
        // 범위 계산 없이 설정 파일의 page_range_limit 직접 사용
        let page_range_limit = config.user.crawling.page_range_limit;
        info!("📊 설정 파일 기반 크롤링 범위: {} 페이지 (설정: page_range_limit)", page_range_limit);
        
        // ServiceBasedBatchCrawlingEngine으로 직접 실행 (범위 재계산 방지)
        match execute_crawling_with_range(
            &app_handle,
            &engine_state,
            1, // 시작 페이지는 항상 1
            page_range_limit // 설정 파일에서 가져온 범위 한도
        ).await {
            Ok(response) => {
                info!("✅ 설정 파일 기반 크롤링 시작: {}", response.message);
            }
            Err(e) => {
                return Err(format!("Crawling execution failed: {}", e));
            }
        }
    } else {
        return Err("CrawlingEngineState not available".to_string());
    }
    
    Ok(CrawlingSession {
        session_id,
        started_at,
        status: "started".to_string(),
    })
}
