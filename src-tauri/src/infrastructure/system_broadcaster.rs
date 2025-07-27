use std::time::{Duration, Instant};
use tokio::time;
use tauri::{AppHandle, Manager, Emitter};
use crate::events::{SystemStatePayload, AtomicTaskEvent, LiveSystemState, BatchInfo, StageInfo, DbCursor};
use crate::application::shared_state::SharedStateCache;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::database_connection::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// UI 이벤트 페이로드 구조체들
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreatedPayload {
    pub batch_id: String,
    pub page_range: (u32, u32),
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCrawledPayload {
    pub batch_id: String,
    pub page_id: u32,
    pub url: String,
    pub product_count: u32,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCollectedPayload {
    pub batch_id: String,
    pub page_id: u32,
    pub product_id: String,
    pub url: String,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCompletedPayload {
    pub batch_id: String,
    pub pages_processed: u32,
    pub products_collected: u32,
    pub success_rate: f64,
    pub timestamp: String,
}

// 🔥 새로운 이벤트 페이로드들 추가
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttemptPayload {
    pub batch_id: String,
    pub item_id: String,
    pub item_type: String, // "page" or "product"
    pub url: String,
    pub attempt_number: u32,
    pub max_attempts: u32,
    pub reason: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrySuccessPayload {
    pub batch_id: String,
    pub item_id: String,
    pub item_type: String,
    pub url: String,
    pub final_attempt: u32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryFailedPayload {
    pub batch_id: String,
    pub item_id: String,
    pub item_type: String,
    pub url: String,
    pub total_attempts: u32,
    pub final_error: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSaveAttemptPayload {
    pub batch_id: String,
    pub item_id: String,
    pub item_type: String, // "product" or "product_detail"
    pub url: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSaveSuccessPayload {
    pub batch_id: String,
    pub item_id: String,
    pub item_type: String,
    pub url: String,
    pub was_update: bool, // true if updated, false if newly created
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSaveFailedPayload {
    pub batch_id: String,
    pub item_id: String,
    pub item_type: String,
    pub url: String,
    pub error: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgressPayload {
    pub batch_id: String,
    pub stage: String,
    pub progress: f64, // 0.0 to 1.0
    pub items_total: u32,
    pub items_completed: u32,
    pub items_active: u32,
    pub items_failed: u32,
    pub timestamp: String,
}

/// 시스템 상태 브로드캐스터
pub struct SystemStateBroadcaster {
    app_handle: AppHandle,
    last_broadcast: Option<Instant>,
    broadcast_interval: Duration,
    current_batch_id: Option<String>,
}

impl SystemStateBroadcaster {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            last_broadcast: None,
            broadcast_interval: Duration::from_secs(2), // 2초마다 브로드캐스트
            current_batch_id: None,
        }
    }

    /// 시스템 상태 스냅샷 생성
    pub async fn create_system_state_snapshot(&self) -> anyhow::Result<SystemStatePayload> {
        let state_cache = self.app_handle.state::<SharedStateCache>();
        let site_analysis = state_cache.site_analysis.read().await;
        let _db_analysis = state_cache.db_analysis.read().await;
        let runtime_state = state_cache.runtime_state.read().await;
        
        // 🔥 애플리케이션 시작 시 강제로 false로 설정하여 잘못된 상태 방지
        let is_running = runtime_state.is_crawling_active && 
                        runtime_state.session_target_items.is_some() &&
                        runtime_state.current_stage.is_some();
        
        // 기존 데이터베이스 연결 풀을 애플리케이션 상태에서 가져오기
        let db_connection = self.app_handle.state::<DatabaseConnection>();
        let db_repo = IntegratedProductRepository::new(db_connection.pool().clone());
        
        // 최근 제품의 page_id, index_in_page 정보 가져오기
        let last_cursor = if let Some(last_product) = db_repo.get_latest_updated_product().await? {
            Some(DbCursor {
                page: last_product.page_id.unwrap_or(0) as u32,
                index: last_product.index_in_page.unwrap_or(0) as u32,
            })
        } else {
            None
        };

        // 총 제품 수 가져오기
        let total_products = db_repo.get_product_count().await?;

        Ok(SystemStatePayload {
            is_running, // 🔥 더 엄격한 조건으로 수정됨
            total_pages: site_analysis.as_ref().map(|s| s.total_pages).unwrap_or(0),
            db_total_products: total_products as u64,
            last_db_cursor: last_cursor,
            session_target_items: runtime_state.session_target_items.unwrap_or(0),
            session_collected_items: runtime_state.session_collected_items.unwrap_or(0),
            session_eta_seconds: runtime_state.session_eta_seconds.unwrap_or(0),
            items_per_minute: runtime_state.items_per_minute.unwrap_or(0.0),
            current_stage: runtime_state.current_stage.clone().unwrap_or_default(),
            analyzed_at: runtime_state.analyzed_at,
        })
    }

    /// 시스템 상태 브로드캐스트
    pub async fn broadcast_system_state(&mut self) -> anyhow::Result<()> {
        let now = Instant::now();
        
        // 브로드캐스트 간격 체크
        if let Some(last) = self.last_broadcast {
            if now.duration_since(last) < self.broadcast_interval {
                return Ok(());
            }
        }

        let system_state = self.create_system_state_snapshot().await?;
        
        // 프론트엔드로 이벤트 발송
        self.app_handle.emit("system-state-update", &system_state)?;
        
        self.last_broadcast = Some(now);
        
        Ok(())
    }

    /// 원자적 작업 이벤트 발송
    pub fn emit_atomic_task_event(&self, event: AtomicTaskEvent) -> anyhow::Result<()> {
        self.app_handle.emit("atomic-task-update", &event)?;
        Ok(())
    }

    /// Live Production Line 상태 브로드캐스트
    pub async fn broadcast_live_state(&mut self) -> anyhow::Result<()> {
        let basic_state = self.create_system_state_snapshot().await?;
        
        // 현재 배치 정보 생성 (예시)
        let current_batch = if basic_state.is_running {
            Some(BatchInfo {
                id: 1,
                status: "active".to_string(),
                progress: 0.75,
                items_total: 100,
                items_completed: 75,
                current_page: 480,
                pages_range: (482, 473),
            })
        } else {
            None
        };

        // 스테이지 정보 생성
        let stages = vec![
            StageInfo {
                name: "ListPageCollection".to_string(),
                status: "completed".to_string(),
                items_total: 10,
                items_completed: 10,
                items_active: 0,
                items_failed: 0,
            },
            StageInfo {
                name: "DetailPageCollection".to_string(),
                status: "active".to_string(),
                items_total: 75,
                items_completed: 65,
                items_active: 8,
                items_failed: 2,
            },
            StageInfo {
                name: "DatabaseSave".to_string(),
                status: "pending".to_string(),
                items_total: 0,
                items_completed: 0,
                items_active: 0,
                items_failed: 0,
            },
        ];

        let live_state = LiveSystemState {
            basic_state,
            current_batch,
            stages,
            recent_completions: vec![], // 최근 완료된 작업들
        };

        self.app_handle.emit("live-state-update", &live_state)?;
        Ok(())
    }

    /// 백그라운드 브로드캐스트 태스크 시작
    pub async fn start_background_broadcast(mut self) {
        let mut interval = time::interval(Duration::from_secs(2));
        
        // 🔥 애플리케이션 시작 시 즉시 브로드캐스트하지 않고 약간의 지연 추가
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.broadcast_system_state().await {
                eprintln!("❌ Failed to broadcast system state: {}", e);
            }
        }
    }

    /// 새로운 배치 시작 이벤트 발송
    pub fn emit_batch_created(&mut self, page_start: u32, page_end: u32) -> anyhow::Result<()> {
        let batch_id = Uuid::new_v4().to_string();
        self.current_batch_id = Some(batch_id.clone());
        
        let payload = BatchCreatedPayload {
            batch_id: batch_id.clone(),
            page_range: (page_start, page_end),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("batch-created", &payload)?;
        Ok(())
    }

    /// 페이지 크롤링 완료 이벤트 발송
    pub fn emit_page_crawled(&self, page_id: u32, url: String, product_count: u32, success: bool) -> anyhow::Result<()> {
        let payload = PageCrawledPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            page_id,
            url,
            product_count,
            status: if success { "completed" } else { "failed" }.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("page-crawled", &payload)?;
        Ok(())
    }

    /// 제품 수집 완료 이벤트 발송
    pub fn emit_product_collected(&self, page_id: u32, product_id: String, url: String, success: bool) -> anyhow::Result<()> {
        let payload = ProductCollectedPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            page_id,
            product_id,
            url,
            status: if success { "completed" } else { "failed" }.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("product-collected", &payload)?;
        Ok(())
    }

    /// 배치 완료 이벤트 발송
    pub fn emit_batch_completed(&mut self, pages_processed: u32, products_collected: u32, success_rate: f64) -> anyhow::Result<()> {
        if let Some(batch_id) = &self.current_batch_id {
            let payload = BatchCompletedPayload {
                batch_id: batch_id.clone(),
                pages_processed,
                products_collected,
                success_rate,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            
            self.app_handle.emit("batch-completed", &payload)?;
            
            // 배치 완료 후 ID 초기화
            self.current_batch_id = None;
        }
        Ok(())
    }

    /// 크롤링 시작 이벤트 발송
    pub fn emit_crawling_started(&self) -> anyhow::Result<()> {
        self.app_handle.emit("crawling-started", &{})?;
        Ok(())
    }

    /// 크롤링 완료 이벤트 발송
    pub fn emit_crawling_completed(&self) -> anyhow::Result<()> {
        self.app_handle.emit("crawling-completed", &{})?;
        Ok(())
    }

    /// 크롤링 에러 이벤트 발송
    pub fn emit_crawling_error(&self, error_message: String) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "error": error_message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        self.app_handle.emit("crawling-error", &payload)?;
        Ok(())
    }

    // 🔥 재시도 관련 이벤트 발송 메서드들 추가
    /// 재시도 시도 이벤트 발송
    pub fn emit_retry_attempt(&self, item_id: String, item_type: String, url: String, attempt_number: u32, max_attempts: u32, reason: String) -> anyhow::Result<()> {
        let payload = RetryAttemptPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            item_id,
            item_type,
            url,
            attempt_number,
            max_attempts,
            reason,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("retry-attempt", &payload)?;
        Ok(())
    }

    /// 재시도 성공 이벤트 발송
    pub fn emit_retry_success(&self, item_id: String, item_type: String, url: String, final_attempt: u32) -> anyhow::Result<()> {
        let payload = RetrySuccessPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            item_id,
            item_type,
            url,
            final_attempt,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("retry-success", &payload)?;
        Ok(())
    }

    /// 재시도 최종 실패 이벤트 발송
    pub fn emit_retry_failed(&self, item_id: String, item_type: String, url: String, total_attempts: u32, final_error: String) -> anyhow::Result<()> {
        let payload = RetryFailedPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            item_id,
            item_type,
            url,
            total_attempts,
            final_error,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("retry-failed", &payload)?;
        Ok(())
    }

    // 🔥 DB 저장 관련 이벤트 발송 메서드들 추가
    /// DB 저장 시도 이벤트 발송
    pub fn emit_database_save_attempt(&self, item_id: String, item_type: String, url: String) -> anyhow::Result<()> {
        let payload = DatabaseSaveAttemptPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            item_id,
            item_type,
            url,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("database-save-attempt", &payload)?;
        Ok(())
    }

    /// DB 저장 성공 이벤트 발송
    pub fn emit_database_save_success(&self, item_id: String, item_type: String, url: String, was_update: bool) -> anyhow::Result<()> {
        let payload = DatabaseSaveSuccessPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            item_id,
            item_type,
            url,
            was_update,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("database-save-success", &payload)?;
        Ok(())
    }

    /// DB 저장 실패 이벤트 발송
    pub fn emit_database_save_failed(&self, item_id: String, item_type: String, url: String, error: String) -> anyhow::Result<()> {
        let payload = DatabaseSaveFailedPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            item_id,
            item_type,
            url,
            error,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("database-save-failed", &payload)?;
        Ok(())
    }

    /// 배치 진행 상황 업데이트 이벤트 발송
    pub fn emit_batch_progress(&self, stage: String, progress: f64, items_total: u32, items_completed: u32, items_active: u32, items_failed: u32) -> anyhow::Result<()> {
        let payload = BatchProgressPayload {
            batch_id: self.current_batch_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            stage,
            progress,
            items_total,
            items_completed,
            items_active,
            items_failed,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        self.app_handle.emit("batch-progress", &payload)?;
        Ok(())
    }
    
    /// 🔥 새로운 CrawlingEvent 기반 발송 메서드 추가
    pub fn emit_site_status_check(&self, event: &crate::domain::events::CrawlingEvent) -> anyhow::Result<()> {
        self.app_handle.emit(event.event_name(), event)?;
        Ok(())
    }

    /// 🔥 세션 이벤트 발송
    pub fn emit_session_event(&self, session_id: String, event_type: crate::domain::events::SessionEventType, message: String) -> anyhow::Result<()> {
        let event = crate::domain::events::CrawlingEvent::SessionEvent {
            session_id,
            event_type,
            message,
            timestamp: chrono::Utc::now(),
        };
        self.app_handle.emit(event.event_name(), &event)?;
        Ok(())
    }

    /// 🔥 배치 이벤트 발송
    pub fn emit_batch_event(&self, session_id: String, batch_id: String, stage: crate::domain::events::CrawlingStage, event_type: crate::domain::events::BatchEventType, message: String, metadata: Option<crate::domain::events::BatchMetadata>) -> anyhow::Result<()> {
        let event = crate::domain::events::CrawlingEvent::BatchEvent {
            session_id,
            batch_id,
            stage,
            event_type,
            message,
            timestamp: chrono::Utc::now(),
            metadata,
        };
        self.app_handle.emit(event.event_name(), &event)?;
        Ok(())
    }

    /// 🔥 ProductList 페이지별 이벤트 발송
    pub fn emit_product_list_page_event(&self, session_id: String, batch_id: String, page_number: u32, event_type: crate::domain::events::PageEventType, message: String, metadata: Option<crate::domain::events::PageMetadata>) -> anyhow::Result<()> {
        let event = crate::domain::events::CrawlingEvent::ProductListPageEvent {
            session_id,
            batch_id,
            page_number,
            event_type,
            message,
            timestamp: chrono::Utc::now(),
            metadata,
        };
        self.app_handle.emit(event.event_name(), &event)?;
        Ok(())
    }

    /// 🔥 제품 상세정보 이벤트 발송
    pub fn emit_product_detail_event(&self, session_id: String, batch_id: String, product_id: String, product_url: String, event_type: crate::domain::events::ProductEventType, message: String, metadata: Option<crate::domain::events::ProductMetadata>) -> anyhow::Result<()> {
        let event = crate::domain::events::CrawlingEvent::ProductDetailEvent {
            session_id,
            batch_id,
            product_id,
            product_url,
            event_type,
            message,
            timestamp: chrono::Utc::now(),
            metadata,
        };
        self.app_handle.emit(event.event_name(), &event)?;
        Ok(())
    }
}

/// 전역 브로드캐스터 인스턴스 생성 및 시작
pub fn start_system_broadcaster(app_handle: AppHandle) {
    let broadcaster = SystemStateBroadcaster::new(app_handle);
    
    tokio::spawn(async move {
        broadcaster.start_background_broadcast().await;
    });
}
