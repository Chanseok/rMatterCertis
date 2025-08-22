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
    let initial_event = crate::crawl_engine::events::task_lifecycle::ConcurrencyEvent::SessionEvent {
        session_id: session_id.clone(),
        event_type: crate::crawl_engine::events::task_lifecycle::SessionEventType::Started,
        timestamp: chrono::Utc::now(),
        metadata: Some(std::collections::HashMap::new()),
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
        match execute_crawling_without_state().await {
            Ok(_) => {
                info!("✅ Crawling completed successfully");
                
                // 세션 완료 이벤트
                let completion_event = crate::crawl_engine::events::task_lifecycle::ConcurrencyEvent::SessionEvent {
                    session_id: session_id_clone.clone(),
                    event_type: crate::crawl_engine::events::task_lifecycle::SessionEventType::Completed,
                    timestamp: chrono::Utc::now(),
                    metadata: Some(std::collections::HashMap::new()),
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
                let failure_event = crate::crawl_engine::events::task_lifecycle::ConcurrencyEvent::SessionEvent {
                    session_id: session_id_clone.clone(),
                    event_type: crate::crawl_engine::events::task_lifecycle::SessionEventType::Failed,
                    timestamp: chrono::Utc::now(),
                    metadata: Some([
                        ("error".to_string(), e.to_string())
                    ].iter().cloned().collect()),
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

/// State 없이 크롤링 실행하는 헬퍼 함수
async fn execute_crawling_without_state() -> Result<(), String> {
    // 기본 크롤링 서비스 직접 호출
    match crate::infrastructure::service_based_crawling_engine::start_complete_crawling().await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Crawling failed: {}", e))
    }
}
