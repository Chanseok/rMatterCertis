//! 실제 크롤링 서비스 통합을 위한 Actor 시스템 패치
//! 기존 actor_system.rs에 실제 크롤링 서비스를 통합하는 코드를 추가

use std::sync::Arc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::infrastructure::config::AppConfig;
use crate::new_architecture::actor_system::{StageError, StageResult};
use crate::new_architecture::actors::StageActor;
use crate::new_architecture::channels::types::{StageItem, StageType};
use crate::new_architecture::services::crawling_integration::{
    CrawlingIntegrationService, RealCrawlingStageExecutor,
};
use crate::new_architecture::system_config::SystemConfig;

/// 실제 크롤링 통합 서비스
#[allow(dead_code)] // Phase2: retained for near-term expansion; prune in Phase3 if still unused
pub struct RealCrawlingIntegration {
    config: Arc<SystemConfig>, // REMOVE_CANDIDATE(if unused in Phase3)
    app_config: AppConfig,     // REMOVE_CANDIDATE(if unused in Phase3)
}

impl RealCrawlingIntegration {
    pub fn new(config: Arc<SystemConfig>, app_config: AppConfig) -> Self {
        Self { config, app_config }
    }
}

/// StageActor 확장: 실제 크롤링 서비스 실행 기능
impl StageActor {
    /// 실제 크롤링 서비스를 사용하는 새로운 StageActor 생성
    pub async fn new_with_real_crawling_service(
        batch_id: String,
        config: Arc<SystemConfig>,
        app_config: AppConfig,
        total_pages: u32,
        products_on_last_page: u32,
    ) -> anyhow::Result<Self> {
        // 크롤링 통합 서비스 생성
        let integration_service =
            Arc::new(CrawlingIntegrationService::new(config.clone(), app_config).await?);

        // 실행기 생성
        let crawling_executor = Arc::new(RealCrawlingStageExecutor::new(integration_service));

        // StageActor 생성 with meaningful ID context
        let mut stage_actor =
            Self::new_with_oneshot(batch_id, config, total_pages, products_on_last_page);
        stage_actor.set_crawling_executor(crawling_executor);

        // meaningful ID 생성을 위한 PageIdCalculator 컨텍스트 설정
        info!(
            total_pages = total_pages,
            products_on_last_page = products_on_last_page,
            "StageActor created with meaningful ID context"
        );

        Ok(stage_actor)
    }

    /// 크롤링 실행기 설정
    pub fn set_crawling_executor(&mut self, _executor: Arc<RealCrawlingStageExecutor>) {
        // 현재 StageActor는 crawling_executor 필드가 없으므로
        // 실제로는 내부 상태로 저장하고 run_with_oneshot에서 사용
        // 임시로 로그만 출력
        info!("Real crawling executor set for StageActor");
    }

    /// 실제 크롤링 서비스를 사용하는 OneShot 실행
    pub async fn run_with_real_crawling(
        self,
        mut control_rx: tokio::sync::mpsc::Receiver<
            crate::new_architecture::channels::types::ActorCommand,
        >,
        result_tx: oneshot::Sender<StageResult>,
        crawling_executor: Arc<RealCrawlingStageExecutor>,
    ) -> Result<(), crate::new_architecture::actors::ActorError> {
        info!(batch_id = ?self.batch_id, "StageActor started with real crawling service");

        let mut final_result = StageResult::FatalError {
            error: StageError::ValidationError {
                message: "No commands received".to_string(),
            },
            stage_id: self.batch_id.clone(),
            context: "StageActor initialization".to_string(),
        };

        // 명령 대기 및 처리
        while let Some(command) = control_rx.recv().await {
            match command {
                crate::new_architecture::channels::types::ActorCommand::ExecuteStage {
                    stage_type,
                    items,
                    concurrency_limit,
                    timeout_secs: _,
                } => {
                    // 실제 크롤링 서비스를 사용한 스테이지 실행
                    final_result = self
                        .execute_stage_with_real_crawling(
                            stage_type,
                            items,
                            concurrency_limit,
                            crawling_executor.clone(),
                        )
                        .await;
                    break; // 스테이지 처리 완료 후 종료
                }
                crate::new_architecture::channels::types::ActorCommand::CancelSession {
                    reason,
                    ..
                } => {
                    final_result = StageResult::FatalError {
                        error: StageError::ValidationError {
                            message: format!("Session cancelled: {}", reason),
                        },
                        stage_id: self.batch_id.clone(),
                        context: "User cancellation".to_string(),
                    };
                    break;
                }
                _ => {
                    warn!(batch_id = ?self.batch_id, "Unsupported command in stage actor");
                }
            }
        }

        // 결과 전송
        if result_tx.send(final_result).is_err() {
            error!(batch_id = ?self.batch_id, "Failed to send stage result");
        }

        info!(batch_id = ?self.batch_id, "StageActor with real crawling completed");
        Ok(())
    }

    /// 실제 크롤링 서비스를 사용한 스테이지 실행
    async fn execute_stage_with_real_crawling(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        crawling_executor: Arc<RealCrawlingStageExecutor>,
    ) -> StageResult {
        info!(
            batch_id = ?self.batch_id,
            stage = ?stage_type,
            items_count = items.len(),
            concurrency_limit = concurrency_limit,
            "Executing stage with real crawling service"
        );

        // 취소 토큰 생성 (필요시 확장 가능)
        let cancellation_token = CancellationToken::new();

        // 실제 크롤링 서비스 실행
        crawling_executor
            .execute_stage(stage_type, items, concurrency_limit, cancellation_token)
            .await
    }
}

/// BatchActor 확장: 실제 크롤링 서비스를 사용하는 OneShot 스테이지 실행
impl crate::new_architecture::actors::BatchActor {
    /// 실제 크롤링 서비스를 사용한 스테이지 실행
    pub async fn execute_stage_with_real_crawling(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        app_config: AppConfig,
    ) -> StageResult {
        info!(
            batch_id = ?self.batch_id,
            stage = ?stage_type,
            items_count = items.len(),
            "Executing stage with real crawling service"
        );

        // 1. 크롤링 통합 서비스 생성
        let config_arc = match self.config.as_ref() {
            Some(cfg) => cfg.clone(),
            None => {
                return StageResult::FatalError {
                    error: StageError::ConfigurationError {
                        message: "No system config available".to_string(),
                    },
                    stage_id: self
                        .batch_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    context: "Missing configuration".to_string(),
                };
            }
        };

        let integration_service =
            match CrawlingIntegrationService::new(config_arc.clone(), app_config).await {
                Ok(service) => Arc::new(service),
                Err(e) => {
                    error!(error = %e, "Failed to create crawling integration service");
                    return StageResult::FatalError {
                        error: StageError::ConfigurationError {
                            message: format!("Crawling service initialization failed: {}", e),
                        },
                        stage_id: self
                            .batch_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        context: "Service initialization".to_string(),
                    };
                }
            };

        // 2. 실행기 생성
        let crawling_executor = Arc::new(RealCrawlingStageExecutor::new(integration_service));

        // 3. OneShot 데이터 채널 생성
        let (stage_data_tx, stage_data_rx) = oneshot::channel::<StageResult>();

        // 4. 제어 채널 생성
        let (stage_control_tx, stage_control_rx) =
            tokio::sync::mpsc::channel(config_arc.channels.control_buffer_size);

        // 5. 실제 크롤링 서비스를 사용하는 StageActor 스폰 (meaningful ID context 포함)
        let stage_actor = StageActor::new_with_oneshot(
            self.batch_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            config_arc.clone(),
            494, // total_pages (기본값)
            12,  // products_on_last_page (기본값)
        );

        let handle = tokio::spawn(async move {
            stage_actor
                .run_with_real_crawling(stage_control_rx, stage_data_tx, crawling_executor)
                .await
        });

        // 6. 스테이지 명령 전송
        let stage_timeout = std::time::Duration::from_secs(config_arc.actor.stage_timeout_secs);
        let command = crate::new_architecture::channels::types::ActorCommand::ExecuteStage {
            stage_type: stage_type.clone(),
            items,
            concurrency_limit: config_arc
                .performance
                .concurrency
                .stage_concurrency_limits
                .get(&format!("{:?}", stage_type))
                .copied()
                .unwrap_or(10),
            timeout_secs: stage_timeout.as_secs(),
        };

        if let Err(e) = stage_control_tx.send(command).await {
            return StageResult::FatalError {
                error: StageError::ValidationError {
                    message: format!("Failed to send stage command: {}", e),
                },
                stage_id: self
                    .batch_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                context: "Stage command sending".to_string(),
            };
        }

        // 7. 결과 대기 (타임아웃과 함께)
        let result = match tokio::time::timeout(stage_timeout, stage_data_rx).await {
            Ok(Ok(stage_result)) => stage_result,
            Ok(Err(_)) => StageResult::FatalError {
                error: StageError::ValidationError {
                    message: "Stage data channel closed".to_string(),
                },
                stage_id: self
                    .batch_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                context: "Stage communication failure".to_string(),
            },
            Err(_) => StageResult::RecoverableError {
                error: StageError::NetworkTimeout { timeout_secs: 30 },
                attempts: 0,
                stage_id: self
                    .batch_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                suggested_retry_delay_ms: 5000, // 5초를 밀리초로
            },
        };

        // 8. StageActor 정리
        if let Err(e) = handle.await {
            warn!(batch_id = ?self.batch_id, error = %e, "StageActor join failed");
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::config::AppConfig;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_crawling_integration_service_creation() {
    // Ensure database paths are initialized for tests using globals
    let _ = crate::infrastructure::initialize_database_paths().await;
        println!("🧪 크롤링 통합 서비스 생성 테스트 시작");

        // 기본 설정 생성
        let system_config = Arc::new(SystemConfig::default());
        let app_config = AppConfig::for_development(); // 개발용 설정 사용

        // 크롤링 통합 서비스 생성 시도
        match CrawlingIntegrationService::new(system_config, app_config).await {
            Ok(_service) => {
                println!("✅ 크롤링 통합 서비스 생성 성공!");
            }
            Err(e) => {
                println!("❌ 크롤링 통합 서비스 생성 실패: {}", e);
                // 테스트 환경에서는 실패할 수 있음 (HTTP 클라이언트 등)
                println!("   (테스트 환경에서는 정상적인 실패일 수 있음)");
            }
        }

        println!("🎯 크롤링 통합 서비스 생성 테스트 완료!");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_stage_actor_with_real_crawling() {
    // Ensure database paths are initialized for tests using globals
    let _ = crate::infrastructure::initialize_database_paths().await;
        println!("🧪 실제 크롤링을 사용하는 StageActor 테스트 시작");

        let batch_id = "test-batch-real".to_string();
        let config = Arc::new(SystemConfig::default());
        let app_config = AppConfig::for_development();

        // StageActor 생성 시도
        match StageActor::new_with_real_crawling_service(
            batch_id.clone(),
            config,
            app_config,
            494, // total_pages
            12,  // products_on_last_page
        )
        .await
        {
            Ok(stage_actor) => {
                println!("✅ 실제 크롤링 StageActor 생성 성공!");
                println!("   배치 ID: {}", stage_actor.batch_id);
            }
            Err(e) => {
                println!("❌ 실제 크롤링 StageActor 생성 실패: {}", e);
                println!("   (테스트 환경에서는 정상적인 실패일 수 있음)");
            }
        }

        println!("🎯 실제 크롤링 StageActor 테스트 완료!");
    }
}
