use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use tauri::Emitter;
use tauri::State;
use crate::application::AppState;

/// Actor 시스템을 통한 크롤링 세션 시작
#[tauri::command]
pub async fn start_crawling_session(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    
    info!("🚀 Starting actor-based crawling session: {}", session_id);
    
    // 초기 세션 시작 이벤트 발송
    let initial_event = crate::new_architecture::events::task_lifecycle::ConcurrencyEvent::SessionEvent {
        session_id: session_id.clone(),
        event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Started,
        timestamp: chrono::Utc::now(),
        metadata: std::collections::HashMap::new(),
    };
    
    if let Err(e) = app_handle.emit("concurrency-event", &initial_event) {
        error!("Failed to emit initial session event: {}", e);
    }
    
    // 백그라운드에서 실제 크롤링 실행
    let session_id_clone = session_id.clone();
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        // 잠시 대기 후 크롤링 시작
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        // 실제 크롤링 실행 (State 없이 직접 호출)
        match execute_crawling_with_state(&app_handle_clone).await {
            Ok(_) => {
                info!("✅ Crawling completed successfully");
                
                // 세션 완료 이벤트
                let completion_event = crate::new_architecture::events::task_lifecycle::ConcurrencyEvent::SessionEvent {
                    session_id: session_id_clone.clone(),
                    event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Completed,
                    timestamp: chrono::Utc::now(),
                    metadata: std::collections::HashMap::new(),
                };
                
                if let Err(e) = app_handle_clone.emit("concurrency-event", &completion_event) {
                    error!("❌ Failed to emit session completion event: {}", e);
                } else {
                    info!("🔔 Emitted SessionEvent::Completed");
                }
            },
            Err(e) => {
                error!("❌ Crawling failed: {}", e);
                
                // 세션 실패 이벤트
                let failure_event = crate::new_architecture::events::task_lifecycle::ConcurrencyEvent::SessionEvent {
                    session_id: session_id_clone.clone(),
                    event_type: crate::new_architecture::events::task_lifecycle::SessionEventType::Failed,
                    timestamp: chrono::Utc::now(),
                    metadata: [
                        ("error".to_string(), e.to_string())
                    ].iter().cloned().collect(),
                };
                
                if let Err(e) = app_handle_clone.emit("concurrency-event", &failure_event) {
                    error!("❌ Failed to emit session failure event: {}", e);
                } else {
                    info!("🔔 Emitted SessionEvent::Failed");
                }
            }
        }
    });
    
    Ok(session_id)
}

/// AppHandle을 사용하여 실제 Actor 크롤링 실행
async fn execute_crawling_with_state(app_handle: &tauri::AppHandle) -> Result<(), String> {
    info!("🔄 Starting real Actor-based crawling via monitoring");
    
    // 실제 Actor 크롤링 실행 (설정 기반)
    match crate::commands::real_actor_commands::start_real_actor_crawling(
        app_handle.clone(),
        crate::commands::real_actor_commands::RealActorCrawlingRequest {
            // CrawlingPlanner가 모든 설정을 자동 계산하므로 파라미터 불필요
            force_full_crawl: None,
            override_strategy: None,
        },
    ).await {
        Ok(response) => {
            info!("✅ Real Actor crawling completed: {}", response.message);
            Ok(())
        },
        Err(e) => {
            error!("❌ Real Actor crawling failed: {}", e);
            Err(e)
        }
    }
}
