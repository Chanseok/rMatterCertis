use std::time::{Duration, Instant};
use tokio::time;
use tauri::{AppHandle, Manager, Emitter};
use crate::events::{SystemStatePayload, AtomicTaskEvent, LiveSystemState, BatchInfo, StageInfo, DbCursor};
use crate::application::shared_state::SharedStateCache;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::database_connection::DatabaseConnection;

/// 시스템 상태 브로드캐스터
pub struct SystemStateBroadcaster {
    app_handle: AppHandle,
    last_broadcast: Option<Instant>,
    broadcast_interval: Duration,
}

impl SystemStateBroadcaster {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            last_broadcast: None,
            broadcast_interval: Duration::from_secs(2), // 2초마다 브로드캐스트
        }
    }

    /// 시스템 상태 스냅샷 생성
    pub async fn create_system_state_snapshot(&self) -> anyhow::Result<SystemStatePayload> {
        let state_cache = self.app_handle.state::<SharedStateCache>();
        let site_analysis = state_cache.site_analysis.read().await;
        let db_analysis = state_cache.db_analysis.read().await;
        let runtime_state = state_cache.runtime_state.read().await;
        
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
            is_running: runtime_state.is_crawling_active,
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
        println!("📡 System state broadcasted: running={}, total_pages={}, db_products={}", 
                 system_state.is_running, system_state.total_pages, system_state.db_total_products);
        
        Ok(())
    }

    /// 원자적 작업 이벤트 발송
    pub fn emit_atomic_task_event(&self, event: AtomicTaskEvent) -> anyhow::Result<()> {
        self.app_handle.emit("atomic-task-update", &event)?;
        println!("⚡ Atomic task event: {} - {} - {:?}", event.task_id, event.stage_name, event.status);
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
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.broadcast_system_state().await {
                eprintln!("❌ Failed to broadcast system state: {}", e);
            }
        }
    }
}

/// 전역 브로드캐스터 인스턴스 생성 및 시작
pub fn start_system_broadcaster(app_handle: AppHandle) {
    let broadcaster = SystemStateBroadcaster::new(app_handle);
    
    tokio::spawn(async move {
        broadcaster.start_background_broadcast().await;
    });
}
