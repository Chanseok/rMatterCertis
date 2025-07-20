//! TaskActor - 개별 작업 실행 계층
//! Modern Rust 2024 준수: 4단계 Actor 계층의 최하위 실행 단위

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::new_architecture::{
    integrated_context::{IntegratedContext, ContextError},
    channel_types::{ActorCommand, AppEvent, StageType, StageItem},
    actor_system::{StageResult, StageSuccessResult, StageError},
    system_config::SystemConfig,
};

/// TaskActor - 개별 작업 실행을 담당하는 최하위 Actor
pub struct TaskActor {
    /// 태스크 식별자
    task_id: String,
    /// 태스크 타입
    task_type: TaskType,
    /// 통합 컨텍스트
    context: IntegratedContext,
    /// 실행 통계
    execution_stats: Arc<Mutex<TaskExecutionStats>>,
    /// 시작 시간
    start_time: Instant,
}

/// 태스크 타입 정의
#[derive(Debug, Clone)]
pub enum TaskType {
    /// 페이지 수집 태스크
    PageCollection { page_number: u32 },
    /// URL 처리 태스크
    UrlProcessing { url: String },
    /// 데이터 검증 태스크
    DataValidation { data_id: String },
    /// 데이터베이스 저장 태스크
    DatabaseSave { records_count: u32 },
}

/// 태스크 실행 통계
#[derive(Debug, Default)]
struct TaskExecutionStats {
    /// 처리된 아이템 수
    processed_items: u64,
    /// 성공한 아이템 수
    successful_items: u64,
    /// 실패한 아이템 수
    failed_items: u64,
    /// 평균 처리 시간
    avg_processing_time: Duration,
    /// 총 실행 시간
    total_execution_time: Duration,
}

/// 태스크 실행 결과
#[derive(Debug)]
pub enum TaskResult {
    Success {
        task_id: String,
        processed_items: u64,
        execution_time: Duration,
        data: TaskResultData,
    },
    Failure {
        task_id: String,
        error: TaskError,
        execution_time: Duration,
        retry_possible: bool,
    },
    Cancelled {
        task_id: String,
        reason: String,
        partial_results: Option<TaskResultData>,
    },
}

/// 태스크 결과 데이터
#[derive(Debug)]
pub enum TaskResultData {
    PageData { html_content: String, url: String },
    UrlData { processed_urls: Vec<String> },
    ValidationData { validated_records: u32 },
    SaveData { saved_records: u32 },
}

/// 태스크 에러
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Parsing error: {message}")]
    ParsingError { message: String },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("Task timeout after {duration:?}")]
    Timeout { duration: Duration },

    #[error("Context error: {source}")]
    ContextError {
        #[from]
        source: ContextError,
    },
}

impl TaskActor {
    /// 새로운 TaskActor 생성
    pub fn new(
        task_type: TaskType,
        context: IntegratedContext,
    ) -> Self {
        let task_id = Uuid::new_v4().to_string();
        let task_context = context.with_task(task_id.clone());

        Self {
            task_id,
            task_type,
            context: task_context,
            execution_stats: Arc::new(Mutex::new(TaskExecutionStats::default())),
            start_time: Instant::now(),
        }
    }

    /// 태스크 실행
    pub async fn execute(&mut self) -> Result<TaskResult, TaskError> {
        info!(
            task_id = %self.task_id,
            task_type = ?self.task_type,
            context_path = %self.context.context_path(),
            "Starting task execution"
        );

        // 취소 신호 확인
        if self.context.is_cancelled() {
            return Ok(TaskResult::Cancelled {
                task_id: self.task_id.clone(),
                reason: "Task cancelled before execution".to_string(),
                partial_results: None,
            });
        }

        // 시작 이벤트 발행
        self.emit_task_started_event().await?;

        let start_time = Instant::now();
        let result = match &self.task_type {
            TaskType::PageCollection { page_number } => {
                self.execute_page_collection(*page_number).await
            }
            TaskType::UrlProcessing { url } => {
                self.execute_url_processing(url.clone()).await
            }
            TaskType::DataValidation { data_id } => {
                self.execute_data_validation(data_id.clone()).await
            }
            TaskType::DatabaseSave { records_count } => {
                self.execute_database_save(*records_count).await
            }
        };

        let execution_time = start_time.elapsed();

        // 통계 업데이트
        self.update_execution_stats(execution_time, result.is_ok()).await;

        // 완료 이벤트 발행
        self.emit_task_completed_event(&result, execution_time).await?;

        result
    }

    /// 페이지 수집 실행
    async fn execute_page_collection(&self, page_number: u32) -> Result<TaskResult, TaskError> {
        debug!(
            task_id = %self.task_id,
            page_number = page_number,
            "Executing page collection"
        );

        // 실제 구현에서는 HTTP 클라이언트를 사용하여 페이지를 가져옴
        // 여기서는 모의 구현
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 취소 신호 재확인
        if self.context.is_cancelled() {
            return Ok(TaskResult::Cancelled {
                task_id: self.task_id.clone(),
                reason: "Task cancelled during page collection".to_string(),
                partial_results: None,
            });
        }

        let mock_html = format!("<html>Page {} content</html>", page_number);
        let mock_url = format!("https://example.com/page/{}", page_number);

        Ok(TaskResult::Success {
            task_id: self.task_id.clone(),
            processed_items: 1,
            execution_time: self.start_time.elapsed(),
            data: TaskResultData::PageData {
                html_content: mock_html,
                url: mock_url,
            },
        })
    }

    /// URL 처리 실행
    async fn execute_url_processing(&self, url: String) -> Result<TaskResult, TaskError> {
        debug!(
            task_id = %self.task_id,
            url = %url,
            "Executing URL processing"
        );

        tokio::time::sleep(Duration::from_millis(50)).await;

        if self.context.is_cancelled() {
            return Ok(TaskResult::Cancelled {
                task_id: self.task_id.clone(),
                reason: "Task cancelled during URL processing".to_string(),
                partial_results: None,
            });
        }

        Ok(TaskResult::Success {
            task_id: self.task_id.clone(),
            processed_items: 1,
            execution_time: self.start_time.elapsed(),
            data: TaskResultData::UrlData {
                processed_urls: vec![url],
            },
        })
    }

    /// 데이터 검증 실행
    async fn execute_data_validation(&self, data_id: String) -> Result<TaskResult, TaskError> {
        debug!(
            task_id = %self.task_id,
            data_id = %data_id,
            "Executing data validation"
        );

        tokio::time::sleep(Duration::from_millis(30)).await;

        if self.context.is_cancelled() {
            return Ok(TaskResult::Cancelled {
                task_id: self.task_id.clone(),
                reason: "Task cancelled during data validation".to_string(),
                partial_results: None,
            });
        }

        Ok(TaskResult::Success {
            task_id: self.task_id.clone(),
            processed_items: 1,
            execution_time: self.start_time.elapsed(),
            data: TaskResultData::ValidationData {
                validated_records: 1,
            },
        })
    }

    /// 데이터베이스 저장 실행
    async fn execute_database_save(&self, records_count: u32) -> Result<TaskResult, TaskError> {
        debug!(
            task_id = %self.task_id,
            records_count = records_count,
            "Executing database save"
        );

        tokio::time::sleep(Duration::from_millis(200)).await;

        if self.context.is_cancelled() {
            return Ok(TaskResult::Cancelled {
                task_id: self.task_id.clone(),
                reason: "Task cancelled during database save".to_string(),
                partial_results: Some(TaskResultData::SaveData {
                    saved_records: records_count / 2, // 부분 저장
                }),
            });
        }

        Ok(TaskResult::Success {
            task_id: self.task_id.clone(),
            processed_items: u64::from(records_count),
            execution_time: self.start_time.elapsed(),
            data: TaskResultData::SaveData {
                saved_records: records_count,
            },
        })
    }

    /// 태스크 시작 이벤트 발행
    async fn emit_task_started_event(&self) -> Result<(), TaskError> {
        let event = AppEvent::StageCompleted {
            stage: self.get_stage_type_for_task(),
            result: StageResult::Success(StageSuccessResult {
                processed_items: 0,
                stage_duration_ms: 0,
                collection_metrics: None,
                processing_metrics: None,
            }),
        };

        self.context.emit_event(event)?;
        Ok(())
    }

    /// 태스크 완료 이벤트 발행
    async fn emit_task_completed_event(
        &self,
        result: &Result<TaskResult, TaskError>,
        execution_time: Duration,
    ) -> Result<(), TaskError> {
        let stage_result = match result {
            Ok(task_result) => match task_result {
                TaskResult::Success { processed_items, .. } => {
                    StageResult::Success(StageSuccessResult {
                        processed_items: (*processed_items as u32),
                        stage_duration_ms: execution_time.as_millis() as u64,
                        collection_metrics: None,
                        processing_metrics: None,
                    })
                }
                TaskResult::Cancelled { .. } => {
                    StageResult::FatalError {
                        error: StageError::TaskCancelled {
                            task_id: self.task_id.clone(),
                        },
                        stage_id: self.task_id.clone(),
                        context: "Task was cancelled".to_string(),
                    }
                }
                TaskResult::Failure { error, .. } => {
                    StageResult::RecoverableError {
                        error: StageError::TaskExecutionFailed {
                            task_id: self.task_id.clone(),
                            message: error.to_string(),
                        },
                        attempts: 1,
                        stage_id: self.task_id.clone(),
                        suggested_retry_delay: Duration::from_secs(1),
                    }
                }
            },
            Err(error) => StageResult::FatalError {
                error: StageError::TaskExecutionFailed {
                    task_id: self.task_id.clone(),
                    message: error.to_string(),
                },
                stage_id: self.task_id.clone(),
                context: format!("Task execution failed: {}", error),
            },
        };

        let event = AppEvent::StageCompleted {
            stage: self.get_stage_type_for_task(),
            result: stage_result,
        };

        self.context.emit_event(event)?;
        Ok(())
    }

    /// 태스크 타입에 따른 스테이지 타입 반환
    fn get_stage_type_for_task(&self) -> StageType {
        match &self.task_type {
            TaskType::PageCollection { .. } => StageType::ListCollection,
            TaskType::UrlProcessing { .. } => StageType::DetailCollection,
            TaskType::DataValidation { .. } => StageType::DataValidation,
            TaskType::DatabaseSave { .. } => StageType::DatabaseSave,
        }
    }

    /// 실행 통계 업데이트
    async fn update_execution_stats(&self, execution_time: Duration, success: bool) {
        let mut stats = self.execution_stats.lock().await;
        stats.processed_items += 1;
        
        if success {
            stats.successful_items += 1;
        } else {
            stats.failed_items += 1;
        }

        stats.total_execution_time += execution_time;
        stats.avg_processing_time = stats.total_execution_time / stats.processed_items as u32;
    }

    /// 태스크 통계 조회
    pub async fn get_execution_stats(&self) -> TaskExecutionStats {
        self.execution_stats.lock().await.clone()
    }
}

impl Clone for TaskExecutionStats {
    fn clone(&self) -> Self {
        Self {
            processed_items: self.processed_items,
            successful_items: self.successful_items,
            failed_items: self.failed_items,
            avg_processing_time: self.avg_processing_time,
            total_execution_time: self.total_execution_time,
        }
    }
}

/// TaskActor 팩토리
pub struct TaskActorFactory {
    config: Arc<SystemConfig>,
}

impl TaskActorFactory {
    /// 새로운 팩토리 생성
    pub fn new(config: Arc<SystemConfig>) -> Self {
        Self { config }
    }

    /// TaskActor 생성
    pub fn create_task_actor(
        &self,
        task_type: TaskType,
        context: IntegratedContext,
    ) -> TaskActor {
        TaskActor::new(task_type, context)
    }

    /// 여러 TaskActor를 병렬로 실행
    pub async fn execute_tasks_parallel(
        &self,
        task_types: Vec<TaskType>,
        context: IntegratedContext,
    ) -> Vec<Result<TaskResult, TaskError>> {
        let tasks = task_types
            .into_iter()
            .map(|task_type| {
                let mut actor = self.create_task_actor(task_type, context.clone());
                tokio::spawn(async move { actor.execute().await })
            })
            .collect::<Vec<_>>();

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(join_error) => results.push(Err(TaskError::NetworkError {
                    message: format!("Task join error: {}", join_error),
                })),
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_architecture::{
        integrated_context::IntegratedContextFactory,
        system_config::SystemConfig,
    };

    #[tokio::test]
    async fn test_task_actor_creation() {
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config.clone());
        
        let (context, _channels) = factory
            .create_session_context("test-session".to_string())
            .expect("Should create context");

        let task_type = TaskType::PageCollection { page_number: 1 };
        let task_actor = TaskActor::new(task_type, context);

        assert!(!task_actor.task_id.is_empty());
    }

    #[tokio::test]
    async fn test_page_collection_task() {
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config.clone());
        
        let (context, _channels) = factory
            .create_session_context("test-session".to_string())
            .expect("Should create context");

        let task_type = TaskType::PageCollection { page_number: 1 };
        let mut task_actor = TaskActor::new(task_type, context);

        let result = task_actor.execute().await.expect("Task should succeed");

        match result {
            TaskResult::Success { processed_items, .. } => {
                assert_eq!(processed_items, 1);
            }
            _ => panic!("Expected success result"),
        }
    }

    #[tokio::test]
    async fn test_task_factory_parallel_execution() {
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config.clone());
        
        let (context, _channels) = factory
            .create_session_context("test-session".to_string())
            .expect("Should create context");

        let task_factory = TaskActorFactory::new(config);
        let task_types = vec![
            TaskType::PageCollection { page_number: 1 },
            TaskType::PageCollection { page_number: 2 },
            TaskType::UrlProcessing { url: "https://test.com".to_string() },
        ];

        let results = task_factory
            .execute_tasks_parallel(task_types, context)
            .await;

        assert_eq!(results.len(), 3);
        
        for result in results {
            assert!(result.is_ok());
        }
    }
}
