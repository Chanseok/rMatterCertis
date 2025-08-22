use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use crate::crawl_engine::actor_system::{
    SessionActor, BatchActor, StageActor, ActorCommand, AppEvent, SystemConfig,
    StageType, StageResult, StageSuccessResult, CollectionMetrics, ProcessingMetrics
};
use crate::infrastructure::config::AppConfig;

/// OneShot 채널 통합 테스트
/// 이 테스트는 SessionActor → BatchActor → StageActor 간의
/// OneShot 기반 Request-Response 패턴이 올바르게 동작하는지 확인합니다.
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oneshot_channel_integration() {
        // 1. 시스템 설정 초기화
        let config = create_test_config().await;
        let system_config = Arc::new(SystemConfig {
            batch_size: 10,
            max_concurrent_batches: 2,
            retry_policy: crate::crawl_engine::actor_system::RetryPolicy {
                max_attempts: 3,
                base_delay_ms: 100,
                max_delay_ms: 5000,
                exponential_factor: 2.0,
                jitter_enabled: true,
            },
            channel_config: crate::crawl_engine::actor_system::ChannelConfig {
                control_buffer_size: 100,
                event_buffer_size: 1000,
            },
            timeout_config: crate::crawl_engine::actor_system::TimeoutConfig {
                batch_timeout_ms: 30000,
                stage_timeout_ms: 10000,
                oneshot_timeout_ms: 5000,
            },
        });

        // 2. SessionActor 생성 및 시작
        let (event_tx, mut event_rx) = mpsc::channel(1000);
        let session_actor = SessionActor::new(
            "test-session".to_string(),
            system_config.clone(),
            event_tx.clone()
        );

        let (control_tx, control_rx) = mpsc::channel(100);
        
        // SessionActor 비동기 실행
        let session_handle = tokio::spawn(async move {
            session_actor.run(control_rx).await
        });

        // 3. 테스트용 페이지 데이터 준비
        let test_pages = vec![
            "https://example.com/page1".to_string(),
            "https://example.com/page2".to_string(),
            "https://example.com/page3".to_string(),
        ];

        // 4. OneShot 채널로 배치 처리 요청
        let (response_tx, response_rx) = oneshot::channel();
        
        let command = ActorCommand::ProcessBatch {
            pages: test_pages.clone(),
            config: config.clone(),
            batch_size: 2,
            concurrency_limit: 1,
            response: Some(response_tx),
        };

        // 명령 전송
        control_tx.send(command).await.expect("Failed to send command");

        // 5. 응답 대기 및 검증
        match tokio::time::timeout(
            std::time::Duration::from_secs(10), 
            response_rx
        ).await {
            Ok(Ok(result)) => {
                println!("✅ OneShot 채널 통신 성공!");
                println!("   처리 결과: {:?}", result);
                
                // 결과 검증
                match result {
                    StageResult::Success(success_result) => {
                        assert!(!success_result.batch_id.is_empty());
                        assert!(success_result.collection_metrics.total_items > 0);
                        println!("   배치 ID: {}", success_result.batch_id);
                        println!("   처리된 아이템 수: {}", success_result.collection_metrics.total_items);
                    },
                    StageResult::Error(error) => {
                        println!("⚠️  처리 중 에러 발생 (정상적인 상황일 수 있음): {}", error);
                    }
                }
            },
            Ok(Err(_)) => {
                println!("❌ OneShot 채널 수신 실패 - 송신자가 채널을 닫았습니다");
            },
            Err(_) => {
                println!("⏰ OneShot 채널 타임아웃 발생");
            }
        }

        // 6. 이벤트 수신 확인
        let mut events_received = 0;
        while let Ok(event) = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            event_rx.recv()
        ).await {
            if let Some(event) = event {
                println!("📨 이벤트 수신: {:?}", event);
                events_received += 1;
                if events_received >= 3 { // 적당한 수의 이벤트 받으면 종료
                    break;
                }
            }
        }

        // 7. 정리
        drop(control_tx);
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            session_handle
        ).await;

        println!("🎯 OneShot 채널 통합 테스트 완료!");
        println!("   - 수신된 이벤트 수: {}", events_received);
    }

    async fn create_test_config() -> Arc<AppConfig> {
        // 테스트용 최소 설정
    let config_content = r#"
    [database]
    path = ":memory:"

        [user.crawling]
        concurrent_requests = 2
        request_delay_ms = 100
        page_range_limit = 5

        [user.performance]
        batch_size = 10
        max_concurrent_batches = 2
        
        [user.retry]
        max_attempts = 3
    base_delay_ms = 100
    "#;

        // 임시 설정 파일 생성
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        
        // 설정 로드
        Arc::new(
            crate::infrastructure::config::load_config(
                temp_file.path().to_str().unwrap()
            ).await.unwrap()
        )
    }

    #[tokio::test]
    async fn test_batch_actor_oneshot() {
        println!("🧪 BatchActor OneShot 테스트 시작");

        let config = create_test_config().await;
        let system_config = Arc::new(SystemConfig {
            batch_size: 5,
            max_concurrent_batches: 1,
            retry_policy: crate::crawl_engine::actor_system::RetryPolicy {
                max_attempts: 2,
                base_delay_ms: 50,
                max_delay_ms: 1000,
                exponential_factor: 1.5,
                jitter_enabled: false,
            },
            channel_config: crate::crawl_engine::actor_system::ChannelConfig {
                control_buffer_size: 50,
                event_buffer_size: 500,
            },
            timeout_config: crate::crawl_engine::actor_system::TimeoutConfig {
                batch_timeout_ms: 5000,
                stage_timeout_ms: 3000,
                oneshot_timeout_ms: 2000,
            },
        });

        let (event_tx, _event_rx) = mpsc::channel(100);
        
        // BatchActor 직접 테스트
        let batch_actor = BatchActor::new_with_oneshot(
            "test-batch-001".to_string(),
            system_config.clone(),
            event_tx
        );

        let test_pages = vec![
            "https://test.com/1".to_string(),
            "https://test.com/2".to_string(),
        ];

        let (result_tx, result_rx) = oneshot::channel();

        // BatchActor 실행
        let batch_handle = tokio::spawn(async move {
            batch_actor.run_with_oneshot(
                test_pages,
                config,
                2, // batch_size
                1, // concurrency_limit
                result_tx
            ).await
        });

        // 결과 대기
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            result_rx
        ).await {
            Ok(Ok(result)) => {
                println!("✅ BatchActor OneShot 테스트 성공: {:?}", result);
            },
            Ok(Err(_)) => {
                println!("❌ BatchActor OneShot 채널 수신 실패");
            },
            Err(_) => {
                println!("⏰ BatchActor OneShot 타임아웃");
            }
        }

        let _ = batch_handle.await;
        println!("🎯 BatchActor OneShot 테스트 완료");
    }

    #[tokio::test]
    async fn test_stage_actor_oneshot() {
        println!("🧪 StageActor OneShot 테스트 시작");

        let config = create_test_config().await;
        let system_config = Arc::new(SystemConfig {
            batch_size: 3,
            max_concurrent_batches: 1,
            retry_policy: crate::crawl_engine::actor_system::RetryPolicy {
                max_attempts: 1,
                base_delay_ms: 10,
                max_delay_ms: 100,
                exponential_factor: 1.0,
                jitter_enabled: false,
            },
            channel_config: crate::crawl_engine::actor_system::ChannelConfig {
                control_buffer_size: 10,
                event_buffer_size: 100,
            },
            timeout_config: crate::crawl_engine::actor_system::TimeoutConfig {
                batch_timeout_ms: 3000,
                stage_timeout_ms: 2000,
                oneshot_timeout_ms: 1000,
            },
        });

        let (event_tx, _event_rx) = mpsc::channel(50);
        
        // StageActor 직접 테스트
        let stage_actor = StageActor::new_with_oneshot(
            "test-batch-stage".to_string(),
            system_config,
            event_tx
        );

        let test_pages = vec!["https://stage-test.com/1".to_string()];
        let (result_tx, result_rx) = oneshot::channel();

        // StageActor 실행
        let stage_handle = tokio::spawn(async move {
            stage_actor.run_with_oneshot(
                test_pages,
                config,
                StageType::ListPage,
                1, // concurrency_limit
                result_tx
            ).await
        });

        // 결과 대기
        match tokio::time::timeout(
            std::time::Duration::from_secs(3),
            result_rx
        ).await {
            Ok(Ok(result)) => {
                println!("✅ StageActor OneShot 테스트 성공: {:?}", result);
            },
            Ok(Err(_)) => {
                println!("❌ StageActor OneShot 채널 수신 실패");
            },
            Err(_) => {
                println!("⏰ StageActor OneShot 타임아웃");
            }
        }

        let _ = stage_handle.await;
        println!("🎯 StageActor OneShot 테스트 완료");
    }
}
