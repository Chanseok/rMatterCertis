# Matter Certis v2.0 배치 크롤링 시스템: Chapter 5.3 - 배치 진행 상태 영속화 및 복구

## 목차
- [5.3.1 배치 진행 상태 추적 시스템](#531-배치-진행-상태-추적-시스템)
- [5.3.2 진행 상태 영속화 메커니즘](#532-진행-상태-영속화-메커니즘)
- [5.3.3 중단된 배치 복구 시스템](#533-중단된-배치-복구-시스템)
- [5.3.4 배치 상태 분석 및 최적화](#534-배치-상태-분석-및-최적화)

---

## 5.3.1 배치 진행 상태 추적 시스템

### 실시간 배치 상태 관리

```rust
// src/batch/progress/batch_tracker.rs
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct BatchProgressTracker {
    state: Arc<RwLock<BatchState>>,
    persistence: Arc<BatchPersistence>,
    event_sender: broadcast::Sender<BatchEvent>,
    config: TrackerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchState {
    pub session_id: String,
    pub total_batches: usize,
    pub completed_batches: Vec<CompletedBatch>,
    pub current_batch: Option<ActiveBatch>,
    pub failed_batches: Vec<FailedBatch>,
    pub session_start_time: DateTime<Utc>,
    pub last_checkpoint: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub total_items_processed: usize,
    pub session_statistics: SessionStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveBatch {
    pub batch_id: String,
    pub batch_number: usize,
    pub start_time: DateTime<Utc>,
    pub items_total: usize,
    pub items_processed: usize,
    pub items_failed: usize,
    pub current_phase: BatchPhase,
    pub retry_count: usize,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub detailed_progress: Vec<ItemProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedBatch {
    pub batch_id: String,
    pub batch_number: usize,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_ms: u64,
    pub items_processed: usize,
    pub items_failed: usize,
    pub success_rate: f64,
    pub performance_metrics: BatchMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedBatch {
    pub batch_id: String,
    pub batch_number: usize,
    pub failure_time: DateTime<Utc>,
    pub error_message: String,
    pub error_type: BatchErrorType,
    pub retry_count: usize,
    pub is_recoverable: bool,
    pub partial_progress: Option<ActiveBatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchPhase {
    Initializing,
    DataCollecting,
    DataProcessing,
    DataValidating,
    DataSaving,
    Finalizing,
    Retrying { phase: Box<BatchPhase> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchErrorType {
    NetworkError,
    DatabaseError,
    ValidationError,
    TimeoutError,
    ResourceExhaustion,
    UnknownError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProgress {
    pub item_id: String,
    pub status: ItemStatus,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub retry_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Skipped,
}

impl BatchProgressTracker {
    pub fn new(
        session_id: String,
        total_batches: usize,
        persistence: Arc<BatchPersistence>,
        config: TrackerConfig,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        let initial_state = BatchState::new(session_id, total_batches);

        Self {
            state: Arc::new(RwLock::new(initial_state)),
            persistence,
            event_sender,
            config,
        }
    }

    /// 새 배치 시작
    pub async fn start_batch(&self, batch_id: String, batch_number: usize, items_total: usize) -> Result<()> {
        let mut state = self.state.write().await;
        
        let active_batch = ActiveBatch {
            batch_id: batch_id.clone(),
            batch_number,
            start_time: Utc::now(),
            items_total,
            items_processed: 0,
            items_failed: 0,
            current_phase: BatchPhase::Initializing,
            retry_count: 0,
            estimated_completion: None,
            detailed_progress: Vec::new(),
        };

        state.current_batch = Some(active_batch);
        drop(state);

        // 이벤트 발송
        let _ = self.event_sender.send(BatchEvent::BatchStarted {
            batch_id,
            batch_number,
            items_total,
        });

        // 영속화
        self.save_checkpoint().await?;

        Ok(())
    }

    /// 배치 진행 상태 업데이트
    pub async fn update_batch_progress(
        &self,
        items_processed: usize,
        items_failed: usize,
        phase: BatchPhase,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        
        if let Some(ref mut current_batch) = state.current_batch {
            current_batch.items_processed = items_processed;
            current_batch.items_failed = items_failed;
            current_batch.current_phase = phase.clone();
            
            // 예상 완료 시간 계산
            current_batch.estimated_completion = self.calculate_estimated_completion(current_batch);
            
            // 전체 세션 통계 업데이트
            state.total_items_processed += items_processed.saturating_sub(state.total_items_processed);
            state.last_checkpoint = Utc::now();
            state.estimated_completion = self.calculate_session_completion(&state);
        }
        
        drop(state);

        // 이벤트 발송
        let _ = self.event_sender.send(BatchEvent::ProgressUpdated {
            items_processed,
            items_failed,
            phase,
        });

        // 주기적 영속화 (설정에 따라)
        if self.should_save_checkpoint().await {
            self.save_checkpoint().await?;
        }

        Ok(())
    }

    /// 개별 아이템 진행 상태 업데이트
    pub async fn update_item_progress(
        &self,
        item_id: String,
        status: ItemStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        
        if let Some(ref mut current_batch) = state.current_batch {
            // 기존 아이템 찾기 또는 새로 추가
            if let Some(item) = current_batch.detailed_progress
                .iter_mut()
                .find(|item| item.item_id == item_id) {
                
                item.status = status.clone();
                item.error_message = error_message.clone();
                
                match status {
                    ItemStatus::Processing => {
                        if item.start_time.is_none() {
                            item.start_time = Some(Utc::now());
                        }
                    }
                    ItemStatus::Completed | ItemStatus::Failed | ItemStatus::Skipped => {
                        item.end_time = Some(Utc::now());
                    }
                    ItemStatus::Failed => {
                        item.retry_count += 1;
                    }
                    _ => {}
                }
            } else {
                let item = ItemProgress {
                    item_id: item_id.clone(),
                    status: status.clone(),
                    start_time: if matches!(status, ItemStatus::Processing) {
                        Some(Utc::now())
                    } else {
                        None
                    },
                    end_time: if matches!(status, ItemStatus::Completed | ItemStatus::Failed | ItemStatus::Skipped) {
                        Some(Utc::now())
                    } else {
                        None
                    },
                    retry_count: 0,
                    error_message,
                };
                current_batch.detailed_progress.push(item);
            }
        }
        
        drop(state);

        // 이벤트 발송
        let _ = self.event_sender.send(BatchEvent::ItemUpdated {
            item_id,
            status,
        });

        Ok(())
    }

    /// 배치 완료 처리
    pub async fn complete_batch(&self, performance_metrics: BatchMetrics) -> Result<()> {
        let mut state = self.state.write().await;
        
        if let Some(current_batch) = state.current_batch.take() {
            let end_time = Utc::now();
            let duration_ms = end_time
                .signed_duration_since(current_batch.start_time)
                .num_milliseconds() as u64;
            
            let success_rate = if current_batch.items_total > 0 {
                (current_batch.items_processed as f64) / (current_batch.items_total as f64)
            } else {
                1.0
            };

            let completed_batch = CompletedBatch {
                batch_id: current_batch.batch_id.clone(),
                batch_number: current_batch.batch_number,
                start_time: current_batch.start_time,
                end_time,
                duration_ms,
                items_processed: current_batch.items_processed,
                items_failed: current_batch.items_failed,
                success_rate,
                performance_metrics,
            };

            state.completed_batches.push(completed_batch);
            
            // 세션 통계 업데이트
            state.session_statistics.update_with_completed_batch(&state.completed_batches.last().unwrap());
        }
        
        drop(state);

        // 이벤트 발송
        let _ = self.event_sender.send(BatchEvent::BatchCompleted);

        // 영속화
        self.save_checkpoint().await?;

        Ok(())
    }

    /// 배치 실패 처리
    pub async fn fail_batch(
        &self,
        error_message: String,
        error_type: BatchErrorType,
        is_recoverable: bool,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        
        if let Some(current_batch) = state.current_batch.take() {
            let failed_batch = FailedBatch {
                batch_id: current_batch.batch_id.clone(),
                batch_number: current_batch.batch_number,
                failure_time: Utc::now(),
                error_message,
                error_type,
                retry_count: current_batch.retry_count,
                is_recoverable,
                partial_progress: Some(current_batch),
            };

            state.failed_batches.push(failed_batch);
        }
        
        drop(state);

        // 이벤트 발송
        let _ = self.event_sender.send(BatchEvent::BatchFailed {
            error_type: error_type.clone(),
            is_recoverable,
        });

        // 영속화
        self.save_checkpoint().await?;

        Ok(())
    }

    /// 예상 완료 시간 계산
    fn calculate_estimated_completion(&self, batch: &ActiveBatch) -> Option<DateTime<Utc>> {
        if batch.items_processed == 0 {
            return None;
        }

        let elapsed = Utc::now().signed_duration_since(batch.start_time);
        let items_per_ms = (batch.items_processed as f64) / (elapsed.num_milliseconds() as f64);
        
        if items_per_ms > 0.0 {
            let remaining_items = batch.items_total.saturating_sub(batch.items_processed);
            let remaining_ms = (remaining_items as f64) / items_per_ms;
            Some(Utc::now() + chrono::Duration::milliseconds(remaining_ms as i64))
        } else {
            None
        }
    }

    /// 세션 완료 시간 계산
    fn calculate_session_completion(&self, state: &BatchState) -> Option<DateTime<Utc>> {
        if state.completed_batches.is_empty() {
            return None;
        }

        // 완료된 배치들의 평균 처리 시간 계산
        let total_duration: u64 = state.completed_batches
            .iter()
            .map(|b| b.duration_ms)
            .sum();
        
        let avg_duration = total_duration / (state.completed_batches.len() as u64);
        let remaining_batches = state.total_batches.saturating_sub(state.completed_batches.len());
        
        if remaining_batches > 0 {
            let estimated_remaining_ms = avg_duration * (remaining_batches as u64);
            Some(Utc::now() + chrono::Duration::milliseconds(estimated_remaining_ms as i64))
        } else {
            None
        }
    }

    /// 체크포인트 저장 주기 판단
    async fn should_save_checkpoint(&self) -> bool {
        let state = self.state.read().await;
        let time_since_last = Utc::now()
            .signed_duration_since(state.last_checkpoint)
            .num_seconds();
        
        time_since_last >= self.config.checkpoint_interval_seconds as i64
    }

    /// 체크포인트 저장
    pub async fn save_checkpoint(&self) -> Result<()> {
        let state = self.state.read().await;
        self.persistence.save_state(&state).await?;
        Ok(())
    }

    /// 상태 복원
    pub async fn restore_from_checkpoint(&self, session_id: &str) -> Result<bool> {
        if let Some(restored_state) = self.persistence.load_state(session_id).await? {
            let mut state = self.state.write().await;
            *state = restored_state;
            
            log::info!("Restored batch state for session: {}", session_id);
            return Ok(true);
        }
        
        Ok(false)
    }

    /// 이벤트 스트림 구독
    pub fn subscribe_events(&self) -> broadcast::Receiver<BatchEvent> {
        self.event_sender.subscribe()
    }

    /// 현재 상태 조회
    pub async fn get_current_state(&self) -> BatchState {
        self.state.read().await.clone()
    }

    /// 세션 통계 조회
    pub async fn get_session_statistics(&self) -> SessionStatistics {
        let state = self.state.read().await;
        state.session_statistics.clone()
    }
}

#[derive(Debug, Clone)]
pub struct TrackerConfig {
    pub checkpoint_interval_seconds: u64,
    pub max_detailed_progress_items: usize,
    pub enable_item_tracking: bool,
    pub enable_performance_metrics: bool,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval_seconds: 30,
            max_detailed_progress_items: 10000,
            enable_item_tracking: true,
            enable_performance_metrics: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchEvent {
    BatchStarted {
        batch_id: String,
        batch_number: usize,
        items_total: usize,
    },
    ProgressUpdated {
        items_processed: usize,
        items_failed: usize,
        phase: BatchPhase,
    },
    ItemUpdated {
        item_id: String,
        status: ItemStatus,
    },
    BatchCompleted,
    BatchFailed {
        error_type: BatchErrorType,
        is_recoverable: bool,
    },
    CheckpointSaved,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionStatistics {
    pub total_batches_completed: usize,
    pub total_batches_failed: usize,
    pub total_items_processed: usize,
    pub total_items_failed: usize,
    pub average_batch_duration_ms: u64,
    pub average_items_per_second: f64,
    pub success_rate: f64,
    pub last_updated: DateTime<Utc>,
}

impl SessionStatistics {
    fn update_with_completed_batch(&mut self, batch: &CompletedBatch) {
        self.total_batches_completed += 1;
        self.total_items_processed += batch.items_processed;
        self.total_items_failed += batch.items_failed;
        
        // 평균 배치 시간 업데이트
        let total_duration = (self.average_batch_duration_ms * (self.total_batches_completed - 1) as u64)
            + batch.duration_ms;
        self.average_batch_duration_ms = total_duration / self.total_batches_completed as u64;
        
        // 초당 아이템 처리량 업데이트
        if batch.duration_ms > 0 {
            let items_per_second = (batch.items_processed as f64) / (batch.duration_ms as f64 / 1000.0);
            self.average_items_per_second = (self.average_items_per_second * (self.total_batches_completed - 1) as f64
                + items_per_second) / self.total_batches_completed as f64;
        }
        
        // 성공률 업데이트
        if self.total_items_processed + self.total_items_failed > 0 {
            self.success_rate = self.total_items_processed as f64 
                / (self.total_items_processed + self.total_items_failed) as f64;
        }
        
        self.last_updated = Utc::now();
    }
}

impl BatchState {
    fn new(session_id: String, total_batches: usize) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            total_batches,
            completed_batches: Vec::new(),
            current_batch: None,
            failed_batches: Vec::new(),
            session_start_time: now,
            last_checkpoint: now,
            estimated_completion: None,
            total_items_processed: 0,
            session_statistics: SessionStatistics::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetrics {
    pub items_per_second: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub network_requests_count: usize,
    pub database_operations_count: usize,
    pub error_rate: f64,
}
```

---

## 5.3.2 진행 상태 영속화 메커니즘

### 내구성 있는 상태 저장소

```rust
// src/batch/progress/persistence.rs
use sqlx::{Pool, Sqlite, Transaction};
use std::path::PathBuf;
use serde_json;

#[derive(Clone)]
pub struct BatchPersistence {
    pool: Pool<Sqlite>,
    config: PersistenceConfig,
}

#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    pub enable_compression: bool,
    pub max_state_history: usize,
    pub cleanup_interval_hours: u64,
    pub backup_enabled: bool,
    pub backup_path: Option<PathBuf>,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            max_state_history: 10,
            cleanup_interval_hours: 24,
            backup_enabled: true,
            backup_path: None,
        }
    }
}

impl BatchPersistence {
    pub async fn new(pool: Pool<Sqlite>, config: PersistenceConfig) -> Result<Self> {
        let persistence = Self { pool, config };
        persistence.initialize_tables().await?;
        Ok(persistence)
    }

    /// 테이블 초기화
    async fn initialize_tables(&self) -> Result<()> {
        sqlx::query!(
            r#"
            CREATE TABLE IF NOT EXISTS batch_states (
                session_id TEXT NOT NULL,
                state_version INTEGER NOT NULL,
                state_data TEXT NOT NULL,
                compressed BOOLEAN NOT NULL DEFAULT FALSE,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (session_id, state_version)
            );

            CREATE INDEX IF NOT EXISTS idx_batch_states_session 
            ON batch_states(session_id);

            CREATE INDEX IF NOT EXISTS idx_batch_states_created 
            ON batch_states(created_at);

            CREATE TABLE IF NOT EXISTS batch_checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                checkpoint_type TEXT NOT NULL,
                checkpoint_data TEXT NOT NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES batch_states(session_id)
            );

            CREATE INDEX IF NOT EXISTS idx_batch_checkpoints_session 
            ON batch_checkpoints(session_id);
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 배치 상태 저장
    pub async fn save_state(&self, state: &BatchState) -> Result<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        // 현재 버전 조회
        let current_version = self.get_latest_version(&mut tx, &state.session_id).await?;
        let new_version = current_version + 1;

        // 상태 직렬화
        let state_json = serde_json::to_string(state)
            .map_err(|e| BatchError::SerializationError(e.to_string()))?;

        // 압축 여부 결정
        let (final_data, compressed) = if self.config.enable_compression {
            match self.compress_data(&state_json) {
                Ok(compressed_data) => (compressed_data, true),
                Err(_) => (state_json, false),
            }
        } else {
            (state_json, false)
        };

        // 새 상태 저장
        sqlx::query!(
            r#"
            INSERT INTO batch_states 
            (session_id, state_version, state_data, compressed, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
            state.session_id,
            new_version,
            final_data,
            compressed,
            Utc::now()
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        // 오래된 상태 정리
        self.cleanup_old_states(&mut tx, &state.session_id).await?;

        // 백업 생성 (설정된 경우)
        if self.config.backup_enabled {
            self.create_backup(&state.session_id, &final_data).await?;
        }

        tx.commit().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::debug!("Saved batch state for session: {} (version: {})", 
                   state.session_id, new_version);

        Ok(())
    }

    /// 배치 상태 로드
    pub async fn load_state(&self, session_id: &str) -> Result<Option<BatchState>> {
        let row = sqlx::query!(
            r#"
            SELECT state_data, compressed 
            FROM batch_states 
            WHERE session_id = ? 
            ORDER BY state_version DESC 
            LIMIT 1
            "#,
            session_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        // 압축 해제
        let state_json = if row.compressed {
            self.decompress_data(&row.state_data)?
        } else {
            row.state_data
        };

        // 역직렬화
        let state: BatchState = serde_json::from_str(&state_json)
            .map_err(|e| BatchError::SerializationError(e.to_string()))?;

        log::debug!("Loaded batch state for session: {}", session_id);
        Ok(Some(state))
    }

    /// 특정 버전의 상태 로드
    pub async fn load_state_version(&self, session_id: &str, version: i64) -> Result<Option<BatchState>> {
        let row = sqlx::query!(
            r#"
            SELECT state_data, compressed 
            FROM batch_states 
            WHERE session_id = ? AND state_version = ?
            "#,
            session_id,
            version
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let state_json = if row.compressed {
            self.decompress_data(&row.state_data)?
        } else {
            row.state_data
        };

        let state: BatchState = serde_json::from_str(&state_json)
            .map_err(|e| BatchError::SerializationError(e.to_string()))?;

        Ok(Some(state))
    }

    /// 체크포인트 저장
    pub async fn save_checkpoint(
        &self,
        session_id: &str,
        checkpoint_type: CheckpointType,
        data: &impl Serialize,
    ) -> Result<String> {
        let checkpoint_id = Uuid::new_v4().to_string();
        let checkpoint_json = serde_json::to_string(data)
            .map_err(|e| BatchError::SerializationError(e.to_string()))?;

        sqlx::query!(
            r#"
            INSERT INTO batch_checkpoints 
            (id, session_id, checkpoint_type, checkpoint_data, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
            checkpoint_id,
            session_id,
            checkpoint_type.to_string(),
            checkpoint_json,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::debug!("Saved checkpoint: {} for session: {}", checkpoint_id, session_id);
        Ok(checkpoint_id)
    }

    /// 체크포인트 로드
    pub async fn load_checkpoint<T: DeserializeOwned>(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<T>> {
        let row = sqlx::query!(
            "SELECT checkpoint_data FROM batch_checkpoints WHERE id = ?",
            checkpoint_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let data: T = serde_json::from_str(&row.checkpoint_data)
            .map_err(|e| BatchError::SerializationError(e.to_string()))?;

        Ok(Some(data))
    }

    /// 세션별 체크포인트 목록 조회
    pub async fn list_checkpoints(&self, session_id: &str) -> Result<Vec<CheckpointInfo>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, checkpoint_type, created_at 
            FROM batch_checkpoints 
            WHERE session_id = ? 
            ORDER BY created_at DESC
            "#,
            session_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let checkpoints = rows
            .into_iter()
            .map(|row| CheckpointInfo {
                id: row.id,
                checkpoint_type: CheckpointType::from_string(&row.checkpoint_type),
                created_at: row.created_at.and_utc(),
            })
            .collect();

        Ok(checkpoints)
    }

    /// 오래된 상태 정리
    async fn cleanup_old_states(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM batch_states 
            WHERE session_id = ? 
            AND state_version NOT IN (
                SELECT state_version 
                FROM batch_states 
                WHERE session_id = ? 
                ORDER BY state_version DESC 
                LIMIT ?
            )
            "#,
            session_id,
            session_id,
            self.config.max_state_history as i64
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 최신 버전 조회
    async fn get_latest_version(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> Result<i64> {
        let row = sqlx::query!(
            "SELECT COALESCE(MAX(state_version), 0) as max_version FROM batch_states WHERE session_id = ?",
            session_id
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        Ok(row.max_version)
    }

    /// 데이터 압축
    fn compress_data(&self, data: &str) -> Result<String> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        use base64::{Engine as _, engine::general_purpose};

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data.as_bytes())
            .map_err(|e| BatchError::CompressionError(e.to_string()))?;
        
        let compressed = encoder.finish()
            .map_err(|e| BatchError::CompressionError(e.to_string()))?;
        
        Ok(general_purpose::STANDARD.encode(&compressed))
    }

    /// 데이터 압축 해제
    fn decompress_data(&self, compressed_data: &str) -> Result<String> {
        use flate2::read::GzDecoder;
        use std::io::Read;
        use base64::{Engine as _, engine::general_purpose};

        let compressed = general_purpose::STANDARD.decode(compressed_data)
            .map_err(|e| BatchError::CompressionError(e.to_string()))?;
        
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)
            .map_err(|e| BatchError::CompressionError(e.to_string()))?;
        
        Ok(decompressed)
    }

    /// 백업 생성
    async fn create_backup(&self, session_id: &str, data: &str) -> Result<()> {
        if let Some(backup_path) = &self.config.backup_path {
            let backup_file = backup_path
                .join(format!("batch_state_{}_{}.json", session_id, Utc::now().timestamp()));
            
            tokio::fs::write(&backup_file, data).await
                .map_err(|e| BatchError::FileSystemError(e.to_string()))?;
            
            log::debug!("Created backup: {:?}", backup_file);
        }
        
        Ok(())
    }

    /// 오래된 데이터 정리 (백그라운드 작업)
    pub async fn cleanup_old_data(&self) -> Result<CleanupResult> {
        let cutoff_time = Utc::now() - chrono::Duration::hours(self.config.cleanup_interval_hours as i64);
        
        // 오래된 상태 삭제
        let deleted_states = sqlx::query!(
            "DELETE FROM batch_states WHERE created_at < ?",
            cutoff_time
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        // 오래된 체크포인트 삭제
        let deleted_checkpoints = sqlx::query!(
            "DELETE FROM batch_checkpoints WHERE created_at < ?",
            cutoff_time
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let result = CleanupResult {
            deleted_states: deleted_states.rows_affected() as usize,
            deleted_checkpoints: deleted_checkpoints.rows_affected() as usize,
        };

        log::info!("Cleanup completed: {:?}", result);
        Ok(result)
    }

    /// 모든 세션 목록 조회
    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                session_id,
                MAX(state_version) as latest_version,
                MAX(created_at) as last_updated,
                COUNT(*) as state_count
            FROM batch_states 
            GROUP BY session_id 
            ORDER BY last_updated DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let sessions = rows
            .into_iter()
            .map(|row| SessionInfo {
                session_id: row.session_id,
                latest_version: row.latest_version,
                last_updated: row.last_updated.and_utc(),
                state_count: row.state_count as usize,
            })
            .collect();

        Ok(sessions)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointType {
    BatchStart,
    BatchProgress,
    BatchComplete,
    BatchError,
    SessionStart,
    SessionComplete,
}

impl CheckpointType {
    fn to_string(&self) -> String {
        match self {
            CheckpointType::BatchStart => "batch_start".to_string(),
            CheckpointType::BatchProgress => "batch_progress".to_string(),
            CheckpointType::BatchComplete => "batch_complete".to_string(),
            CheckpointType::BatchError => "batch_error".to_string(),
            CheckpointType::SessionStart => "session_start".to_string(),
            CheckpointType::SessionComplete => "session_complete".to_string(),
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "batch_start" => CheckpointType::BatchStart,
            "batch_progress" => CheckpointType::BatchProgress,
            "batch_complete" => CheckpointType::BatchComplete,
            "batch_error" => CheckpointType::BatchError,
            "session_start" => CheckpointType::SessionStart,
            "session_complete" => CheckpointType::SessionComplete,
            _ => CheckpointType::BatchProgress, // 기본값
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckpointInfo {
    pub id: String,
    pub checkpoint_type: CheckpointType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub latest_version: i64,
    pub last_updated: DateTime<Utc>,
    pub state_count: usize,
}

#[derive(Debug)]
pub struct CleanupResult {
    pub deleted_states: usize,
    pub deleted_checkpoints: usize,
}
```

---

## 5.3.3 중단된 배치 복구 시스템

### 스마트 복구 메커니즘

```rust
// src/batch/recovery/recovery_service.rs
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct BatchRecoveryService {
    persistence: Arc<BatchPersistence>,
    tracker: Arc<BatchProgressTracker>,
    config: RecoveryConfig,
}

#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub auto_recovery_enabled: bool,
    pub max_recovery_attempts: usize,
    pub recovery_delay_seconds: u64,
    pub partial_recovery_threshold: f64,
    pub consistency_check_enabled: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            auto_recovery_enabled: true,
            max_recovery_attempts: 3,
            recovery_delay_seconds: 10,
            partial_recovery_threshold: 0.1, // 10% 이상 진행된 배치만 복구
            consistency_check_enabled: true,
        }
    }
}

impl BatchRecoveryService {
    pub fn new(
        persistence: Arc<BatchPersistence>,
        tracker: Arc<BatchProgressTracker>,
        config: RecoveryConfig,
    ) -> Self {
        Self {
            persistence,
            tracker,
            config,
        }
    }

    /// 중단된 세션 감지 및 복구
    pub async fn detect_and_recover_sessions(&self) -> Result<RecoveryReport> {
        let mut report = RecoveryReport::new();
        
        // 모든 세션 조회
        let sessions = self.persistence.list_sessions().await?;
        
        for session_info in sessions {
            // 세션 상태 로드
            if let Some(state) = self.persistence.load_state(&session_info.session_id).await? {
                let recovery_status = self.analyze_session_for_recovery(&state).await?;
                
                match recovery_status {
                    RecoveryStatus::NoRecoveryNeeded => {
                        report.healthy_sessions += 1;
                    }
                    RecoveryStatus::RequiresRecovery { reason, priority } => {
                        report.sessions_requiring_recovery += 1;
                        
                        if self.config.auto_recovery_enabled {
                            match self.attempt_session_recovery(&state, reason, priority).await {
                                Ok(recovery_result) => {
                                    report.successful_recoveries += 1;
                                    report.recovery_details.push(recovery_result);
                                }
                                Err(e) => {
                                    report.failed_recoveries += 1;
                                    log::error!("Failed to recover session {}: {}", 
                                              session_info.session_id, e);
                                }
                            }
                        }
                    }
                    RecoveryStatus::Unrecoverable { reason } => {
                        report.unrecoverable_sessions += 1;
                        log::warn!("Session {} is unrecoverable: {}", 
                                 session_info.session_id, reason);
                    }
                }
            }
        }
        
        Ok(report)
    }

    /// 세션 복구 필요성 분석
    async fn analyze_session_for_recovery(&self, state: &BatchState) -> Result<RecoveryStatus> {
        // 완료된 세션인지 확인
        if state.completed_batches.len() == state.total_batches {
            return Ok(RecoveryStatus::NoRecoveryNeeded);
        }

        // 현재 활성 배치가 있는지 확인
        if let Some(current_batch) = &state.current_batch {
            let progress_ratio = if current_batch.items_total > 0 {
                current_batch.items_processed as f64 / current_batch.items_total as f64
            } else {
                0.0
            };

            // 진행률이 임계값 이상인 경우만 복구
            if progress_ratio >= self.config.partial_recovery_threshold {
                return Ok(RecoveryStatus::RequiresRecovery {
                    reason: RecoveryReason::InterruptedBatch,
                    priority: if progress_ratio > 0.5 {
                        RecoveryPriority::High
                    } else {
                        RecoveryPriority::Medium
                    },
                });
            }
        }

        // 실패한 배치들이 복구 가능한지 확인
        let recoverable_failed = state.failed_batches
            .iter()
            .filter(|f| f.is_recoverable && f.retry_count < self.config.max_recovery_attempts)
            .count();

        if recoverable_failed > 0 {
            return Ok(RecoveryStatus::RequiresRecovery {
                reason: RecoveryReason::RecoverableFailures,
                priority: RecoveryPriority::Medium,
            });
        }

        // 세션이 비정상적으로 중단되었는지 확인
        let time_since_last_checkpoint = Utc::now()
            .signed_duration_since(state.last_checkpoint)
            .num_hours();

        if time_since_last_checkpoint > 1 && state.completed_batches.len() < state.total_batches {
            return Ok(RecoveryStatus::RequiresRecovery {
                reason: RecoveryReason::StaleSession,
                priority: RecoveryPriority::Low,
            });
        }

        // 복구할 수 없는 상태
        Ok(RecoveryStatus::Unrecoverable {
            reason: "No recoverable progress found".to_string(),
        })
    }

    /// 세션 복구 시도
    async fn attempt_session_recovery(
        &self,
        state: &BatchState,
        reason: RecoveryReason,
        priority: RecoveryPriority,
    ) -> Result<RecoveryDetail> {
        let recovery_start = Utc::now();
        log::info!("Starting recovery for session {} (reason: {:?}, priority: {:?})", 
                  state.session_id, reason, priority);

        // 복구 전 일관성 검사
        if self.config.consistency_check_enabled {
            self.verify_state_consistency(state).await?;
        }

        // 상태 복원
        let restored = self.tracker.restore_from_checkpoint(&state.session_id).await?;
        if !restored {
            return Err(BatchError::RecoveryError("Failed to restore state".to_string()));
        }

        // 복구 전략 결정
        let recovery_strategy = self.determine_recovery_strategy(state, reason).await?;
        
        // 복구 실행
        let recovery_result = match recovery_strategy {
            RecoveryStrategy::ResumeCurrentBatch => {
                self.resume_current_batch(state).await?
            }
            RecoveryStrategy::RestartFailedBatches => {
                self.restart_failed_batches(state).await?
            }
            RecoveryStrategy::PartialRecovery { from_batch } => {
                self.partial_recovery(state, from_batch).await?
            }
            RecoveryStrategy::FullRestart => {
                self.full_restart(state).await?
            }
        };

        let recovery_duration = Utc::now()
            .signed_duration_since(recovery_start)
            .num_milliseconds() as u64;

        Ok(RecoveryDetail {
            session_id: state.session_id.clone(),
            recovery_reason: reason,
            recovery_strategy,
            recovery_result,
            recovery_duration_ms: recovery_duration,
            recovered_at: Utc::now(),
        })
    }

    /// 복구 전략 결정
    async fn determine_recovery_strategy(
        &self,
        state: &BatchState,
        reason: RecoveryReason,
    ) -> Result<RecoveryStrategy> {
        match reason {
            RecoveryReason::InterruptedBatch => {
                // 현재 배치가 있고 상당한 진행이 있는 경우
                if let Some(current_batch) = &state.current_batch {
                    let progress = current_batch.items_processed as f64 / current_batch.items_total as f64;
                    if progress > 0.2 {
                        return Ok(RecoveryStrategy::ResumeCurrentBatch);
                    }
                }
                Ok(RecoveryStrategy::PartialRecovery { 
                    from_batch: state.completed_batches.len() 
                })
            }
            RecoveryReason::RecoverableFailures => {
                Ok(RecoveryStrategy::RestartFailedBatches)
            }
            RecoveryReason::StaleSession => {
                // 진행률에 따라 결정
                let progress = state.completed_batches.len() as f64 / state.total_batches as f64;
                if progress > 0.5 {
                    Ok(RecoveryStrategy::PartialRecovery { 
                        from_batch: state.completed_batches.len() 
                    })
                } else {
                    Ok(RecoveryStrategy::FullRestart)
                }
            }
        }
    }

    /// 현재 배치 재개
    async fn resume_current_batch(&self, state: &BatchState) -> Result<BatchRecoveryResult> {
        if let Some(current_batch) = &state.current_batch {
            log::info!("Resuming batch {} from item {}", 
                      current_batch.batch_number, current_batch.items_processed);

            // 이미 처리된 아이템들 건너뛰기
            let resume_from_item = current_batch.items_processed;
            
            // 배치 추적기에 현재 상태 설정
            self.tracker.start_batch(
                current_batch.batch_id.clone(),
                current_batch.batch_number,
                current_batch.items_total,
            ).await?;

            // 진행 상태 복원
            self.tracker.update_batch_progress(
                current_batch.items_processed,
                current_batch.items_failed,
                current_batch.current_phase.clone(),
            ).await?;

            Ok(BatchRecoveryResult::ResumedBatch {
                batch_id: current_batch.batch_id.clone(),
                resumed_from_item: resume_from_item,
            })
        } else {
            Err(BatchError::RecoveryError("No current batch to resume".to_string()))
        }
    }

    /// 실패한 배치들 재시작
    async fn restart_failed_batches(&self, state: &BatchState) -> Result<BatchRecoveryResult> {
        let recoverable_batches: Vec<_> = state.failed_batches
            .iter()
            .filter(|f| f.is_recoverable && f.retry_count < self.config.max_recovery_attempts)
            .collect();

        if recoverable_batches.is_empty() {
            return Err(BatchError::RecoveryError("No recoverable failed batches".to_string()));
        }

        log::info!("Restarting {} failed batches", recoverable_batches.len());

        let batch_numbers: Vec<_> = recoverable_batches
            .iter()
            .map(|b| b.batch_number)
            .collect();

        // 실제 재시작 로직은 배치 실행 엔진에서 처리
        // 여기서는 복구 가능한 배치 목록만 반환

        Ok(BatchRecoveryResult::RestartedFailedBatches {
            batch_numbers,
        })
    }

    /// 부분 복구
    async fn partial_recovery(&self, state: &BatchState, from_batch: usize) -> Result<BatchRecoveryResult> {
        log::info!("Starting partial recovery from batch {}", from_batch);

        // 완료된 배치들은 유지하고, 지정된 배치부터 재시작
        let remaining_batches = state.total_batches - from_batch;

        Ok(BatchRecoveryResult::PartialRecovery {
            completed_batches: state.completed_batches.len(),
            restart_from_batch: from_batch,
            remaining_batches,
        })
    }

    /// 전체 재시작
    async fn full_restart(&self, state: &BatchState) -> Result<BatchRecoveryResult> {
        log::info!("Starting full restart for session {}", state.session_id);

        // 모든 진행 상태 초기화
        // 실제 초기화 로직은 배치 실행 엔진에서 처리

        Ok(BatchRecoveryResult::FullRestart {
            total_batches: state.total_batches,
        })
    }

    /// 상태 일관성 검증
    async fn verify_state_consistency(&self, state: &BatchState) -> Result<()> {
        // 1. 배치 번호 연속성 확인
        let mut expected_batch = 0;
        for completed in &state.completed_batches {
            if completed.batch_number != expected_batch {
                return Err(BatchError::InconsistentState(format!(
                    "Expected batch {}, found {}", expected_batch, completed.batch_number
                )));
            }
            expected_batch += 1;
        }

        // 2. 현재 배치 유효성 확인
        if let Some(current) = &state.current_batch {
            if current.batch_number != expected_batch {
                return Err(BatchError::InconsistentState(format!(
                    "Current batch number {} doesn't match expected {}", 
                    current.batch_number, expected_batch
                )));
            }

            if current.items_processed > current.items_total {
                return Err(BatchError::InconsistentState(
                    "Items processed exceeds total items".to_string()
                ));
            }
        }

        // 3. 시간 일관성 확인
        let mut last_time = state.session_start_time;
        for completed in &state.completed_batches {
            if completed.start_time < last_time {
                return Err(BatchError::InconsistentState(
                    "Batch timestamps are out of order".to_string()
                ));
            }
            last_time = completed.end_time;
        }

        Ok(())
    }

    /// 복구 상태 모니터링
    pub async fn monitor_recovery_progress(&self, session_id: &str) -> Result<RecoveryMonitorResult> {
        // 복구 후 세션 상태 조회
        let current_state = self.tracker.get_current_state().await;
        
        if current_state.session_id != session_id {
            return Err(BatchError::RecoveryError("Session ID mismatch".to_string()));
        }

        let progress_since_recovery = if let Some(current_batch) = &current_state.current_batch {
            current_batch.items_processed as f64 / current_batch.items_total as f64
        } else {
            1.0 // 현재 배치가 없으면 완료된 것으로 간주
        };

        let overall_progress = (current_state.completed_batches.len() as f64 
            + progress_since_recovery) / current_state.total_batches as f64;

        Ok(RecoveryMonitorResult {
            session_id: session_id.to_string(),
            overall_progress,
            current_batch_progress: progress_since_recovery,
            recovery_successful: overall_progress > 0.0,
            estimated_completion: current_state.estimated_completion,
        })
    }
}

#[derive(Debug, Clone)]
pub enum RecoveryStatus {
    NoRecoveryNeeded,
    RequiresRecovery { reason: RecoveryReason, priority: RecoveryPriority },
    Unrecoverable { reason: String },
}

#[derive(Debug, Clone)]
pub enum RecoveryReason {
    InterruptedBatch,
    RecoverableFailures,
    StaleSession,
}

#[derive(Debug, Clone)]
pub enum RecoveryPriority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    ResumeCurrentBatch,
    RestartFailedBatches,
    PartialRecovery { from_batch: usize },
    FullRestart,
}

#[derive(Debug, Clone)]
pub enum BatchRecoveryResult {
    ResumedBatch {
        batch_id: String,
        resumed_from_item: usize,
    },
    RestartedFailedBatches {
        batch_numbers: Vec<usize>,
    },
    PartialRecovery {
        completed_batches: usize,
        restart_from_batch: usize,
        remaining_batches: usize,
    },
    FullRestart {
        total_batches: usize,
    },
}

#[derive(Debug)]
pub struct RecoveryDetail {
    pub session_id: String,
    pub recovery_reason: RecoveryReason,
    pub recovery_strategy: RecoveryStrategy,
    pub recovery_result: BatchRecoveryResult,
    pub recovery_duration_ms: u64,
    pub recovered_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub healthy_sessions: usize,
    pub sessions_requiring_recovery: usize,
    pub successful_recoveries: usize,
    pub failed_recoveries: usize,
    pub unrecoverable_sessions: usize,
    pub recovery_details: Vec<RecoveryDetail>,
}

impl RecoveryReport {
    fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug)]
pub struct RecoveryMonitorResult {
    pub session_id: String,
    pub overall_progress: f64,
    pub current_batch_progress: f64,
    pub recovery_successful: bool,
    pub estimated_completion: Option<DateTime<Utc>>,
}
```

이번 Chapter 5.3에서는 배치 진행 상태의 영속화와 복구 시스템을 다뤘습니다. 주요 구현 내용은:

1. **실시간 배치 상태 추적**: 세밀한 진행 상태 모니터링
2. **내구성 있는 영속화**: 압축과 백업을 포함한 안전한 저장
3. **스마트 복구 메커니즘**: 다양한 복구 전략과 일관성 검증
4. **복구 모니터링**: 복구 후 진행 상황 추적

다음 섹션에서는 트랜잭션 관리와 에러 복구에 대해 다루겠습니다.
