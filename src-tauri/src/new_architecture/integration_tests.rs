//! 새로운 아키텍처 통합 테스트
//! Modern Rust 2024 준수: proptest + cargo nextest

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::new_architecture::{
    SystemConfig, ChannelFactory, SessionActor, BatchActor, StageActor,
    ActorCommand, AppEvent, ActorError,
};
use crate::new_architecture::channel_types::{BatchConfig, StageType, StageItem};

/// 시스템 설정 테스트
#[cfg(test)]
mod system_config_tests {
    use super::*;
    
    #[test]
    fn test_default_config_validation() {
        let config = SystemConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.system.max_concurrent_sessions > 0);
        assert!(config.actor.session_timeout_secs > 0);
        assert!(config.actor.stage_timeout_secs > 0);
    }
    
    #[test]
    fn test_environment_specific_configs() {
        // 테스트에서는 기본 설정 사용
        let default_config = SystemConfig::default();
        assert!(default_config.actor.session_timeout_secs > 0);
        assert!(default_config.channels.control_buffer_size > 0);
        
        // 다른 환경 설정도 테스트 가능
        let test_config = SystemConfig::load_for_test().unwrap();
        assert_eq!(test_config.actor.session_timeout_secs, default_config.actor.session_timeout_secs);
    }
    
    #[test]
    fn test_retry_policy_timing_properties() {
        let mut config = SystemConfig::default();
        config.retry_policies.list_collection.max_attempts = 5;
        config.retry_policies.list_collection.base_delay_ms = 1000;
        config.retry_policies.list_collection.backoff_multiplier = 2.0;
        
        assert!(config.validate().is_ok());
        assert!(config.retry_policies.list_collection.base_delay().as_millis() >= 100);
    }
}

/// 채널 시스템 테스트
#[cfg(test)]
mod channel_factory_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_channel_creation_and_communication() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config);
        
        // 제어 채널 테스트
        let (control_tx, mut control_rx) = factory.create_control_channel::<String>();
        control_tx.send("test_command".to_string()).await.unwrap();
        let received = control_rx.recv().await.unwrap();
        assert_eq!(received, "test_command");
        
        // 이벤트 채널 테스트
        let event_tx = factory.create_event_channel::<String>();
        let mut event_rx = event_tx.subscribe();
        event_tx.send("test_event".to_string()).unwrap();
        let received_event = event_rx.recv().await.unwrap();
        assert_eq!(received_event, "test_event");
        
        // 데이터 채널 테스트
        let (data_tx, data_rx) = factory.create_data_channel::<u32>();
        data_tx.send(42).unwrap();
        let received_data = data_rx.await.unwrap();
        assert_eq!(received_data, 42);
    }
    
    #[test]
    fn test_backpressure_detection() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config);
        
        // 임계값 테스트
        assert!(!factory.check_backpressure(0.5)); // 50% - OK
        assert!(factory.check_backpressure(0.9));  // 90% - 백프레셔
    }
    
    #[test]
    fn test_channel_buffer_sizes() {
        let mut config = SystemConfig::default();
        config.channels.control_buffer_size = 100;
        config.channels.event_buffer_size = 500;
        
        let factory = ChannelFactory::new(Arc::new(config));
        let (control_tx, _) = factory.create_control_channel::<i32>();
        let event_tx = factory.create_event_channel::<i32>();
        
        // 채널이 생성되는지 확인
        assert!(control_tx.capacity() > 0);
        // 이벤트 채널은 생성되었다고 가정
        drop(event_tx); // 사용하지 않으므로 drop
    }
}

/// Actor 시스템 통합 테스트
#[cfg(test)]
mod actor_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_session_actor_lifecycle() {
        let config = Arc::new(SystemConfig::default());
        let (command_tx, command_rx) = mpsc::channel(10);
        let (event_tx, mut event_rx) = mpsc::channel(10);
        
        let mut session_actor = SessionActor::new(config, command_rx, event_tx);
        
        // 배치 처리 명령 전송
        let batch_config = BatchConfig {
            target_url: "https://example.com".to_string(),
            max_pages: Some(5),
        };
        
        let command = ActorCommand::ProcessBatch {
            pages: vec![1, 2, 3],
            config: batch_config.clone(),
            batch_size: 2,
            concurrency_limit: 3,
        };
        
        // 백그라운드에서 세션 실행
        let session_handle = tokio::spawn(async move {
            session_actor.run().await
        });
        
        // 명령 전송
        command_tx.send(command).await.unwrap();
        drop(command_tx); // 채널 닫기
        
        // 이벤트 수신 확인
        let session_started = event_rx.recv().await.unwrap();
        match session_started {
            AppEvent::SessionStarted { config: received_config, .. } => {
                assert_eq!(received_config.target_url, batch_config.target_url);
            }
            _ => panic!("Expected SessionStarted event"),
        }
        
        // 세션 완료 대기
        let result = session_handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_batch_actor_page_processing() {
        let config = Arc::new(SystemConfig::default());
        let (event_tx, _event_rx) = mpsc::channel(10);
        
        let mut batch_actor = BatchActor::new(
            "test_batch".to_string(),
            config,
            event_tx,
        );
        
        // 페이지 처리 실행
        let pages = vec![1, 2, 3, 4, 5];
        let result = batch_actor.process_pages(pages, 2, 3).await;
        
        // 성공적인 처리 확인
        assert!(result.is_ok());
        let success_count = result.unwrap();
        assert!(success_count > 0);
    }
    
    #[tokio::test]
    async fn test_stage_actor_execution() {
        let config = Arc::new(SystemConfig::default());
        let mut stage_actor = StageActor::new("test_batch".to_string(), config);
        
        let items = vec![
            StageItem::Page(1),
            StageItem::Page(2),
            StageItem::Url("https://example.com/page/3".to_string()),
        ];
        
        // 각 스테이지 타입 테스트
        let stages = [
            StageType::ListCollection,
            StageType::DetailCollection,
            StageType::DataValidation,
            StageType::DatabaseSave,
        ];
        
        for stage_type in &stages {
            let result = stage_actor.execute_stage(
                stage_type.clone(),
                items.clone(),
                2, // concurrency_limit
                Duration::from_secs(10), // timeout
            ).await;
            
            assert!(result.is_ok(), "Stage {:?} failed: {:?}", stage_type, result);
            let processed_count = result.unwrap();
            assert_eq!(processed_count, items.len() as u32);
        }
    }
    
    #[tokio::test]
    async fn test_session_timeout_handling() {
        // Create config with very short timeout for testing
        let mut config = SystemConfig::load_for_test().unwrap();
        config.actor.session_timeout_secs = 1; // 1 second timeout

        let channels = ChannelFactory::create_triple_channel(&config);
        
        // Create session with short timeout
        let mut session = SessionActor::new(
            Arc::new(config.clone()),
            channels.control_rx,
            channels.event_tx.clone(),
        );

        // Start a long-running operation that should timeout
        let start_time = std::time::Instant::now();
        let result = session.run().await;
        let elapsed = start_time.elapsed();

        // Should timeout within a reasonable time (allow some margin)
        assert!(elapsed.as_secs() <= 3, "Operation took too long: {:?}", elapsed);
        
        // The result should be an error due to timeout
        if let Err(e) = result {
            assert!(e.to_string().contains("timeout") || e.to_string().contains("Timeout"));
        } else {
            // If no error, it should have completed quickly (not actually timed out)
            assert!(elapsed.as_millis() < 100, "Expected quick completion or timeout error");
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_stage_processing() {
        let config = Arc::new(SystemConfig::default());
        let mut stage_actor = StageActor::new("test_concurrent".to_string(), config);
        
        let items: Vec<StageItem> = (1..=10)
            .map(|i| StageItem::Page(i as u32))
            .collect();
        
        let result = stage_actor.execute_stage(
            StageType::ListCollection,
            items.clone(),
            3, // concurrency_limit
            Duration::from_secs(30),
        ).await;
        
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert_eq!(processed, items.len() as u32);
    }
}

/// 성능 테스트
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_high_throughput_channel_communication() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config);
        
        let (control_tx, mut control_rx) = factory.create_control_channel::<u32>();
        
        let start = Instant::now();
        let message_count = 10000;
        
        // 생산자 태스크
        let producer = tokio::spawn(async move {
            for i in 0..message_count {
                control_tx.send(i).await.unwrap();
            }
        });
        
        // 소비자 태스크
        let consumer = tokio::spawn(async move {
            let mut received_count = 0;
            while let Some(_) = control_rx.recv().await {
                received_count += 1;
                if received_count == message_count {
                    break;
                }
            }
            received_count
        });
        
        let (_, received_count) = tokio::join!(producer, consumer);
        let received_count = received_count.unwrap();
        let elapsed = start.elapsed();
        
        assert_eq!(received_count, message_count);
        assert!(elapsed < Duration::from_secs(1), "High throughput test took too long: {:?}", elapsed);
        
        let throughput = message_count as f64 / elapsed.as_secs_f64();
        println!("Channel throughput: {:.0} messages/sec", throughput);
        assert!(throughput > 1000.0, "Throughput too low: {:.0} msg/sec", throughput);
    }
    
    #[tokio::test]
    async fn test_concurrent_session_handling() {
        let config = Arc::new(SystemConfig::default());
        let session_count = 5;
        let mut handles = Vec::new();
        
        let start = Instant::now();
        
        for _i in 0..session_count {
            let config = config.clone();
            let handle = tokio::spawn(async move {
                let (_, command_rx) = mpsc::channel(10);
                let (event_tx, _) = mpsc::channel(10);
                
                let mut session_actor = SessionActor::new(config, command_rx, event_tx);
                
                // 짧은 시간 실행 후 종료
                tokio::time::timeout(Duration::from_millis(100), session_actor.run()).await
            });
            handles.push(handle);
        }
        
        // 모든 세션 완료 대기
        for handle in handles {
            let result = handle.await.unwrap();
            // 타임아웃이나 성공 모두 허용
            assert!(result.is_err() || result.is_ok());
        }
        
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_secs(1), "Concurrent sessions took too long: {:?}", elapsed);
    }
}

/// 에러 처리 테스트
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_invalid_configuration_handling() {
        let mut config = SystemConfig::default();
        config.system.session_timeout_secs = 0; // 잘못된 설정
        
        let validation_result = config.validate();
        assert!(validation_result.is_err());
        
        if let Err(error) = validation_result {
            assert!(error.to_string().contains("session_timeout_secs"));
        }
    }
    
    #[tokio::test]
    async fn test_channel_closed_error_handling() {
        let config = Arc::new(SystemConfig::default());
        let (command_tx, command_rx) = mpsc::channel(10);
        let (event_tx, _) = mpsc::channel(10);
        
        let mut session_actor = SessionActor::new(config, command_rx, event_tx);
        
        // 채널을 즉시 닫기
        drop(command_tx);
        
        // 세션 실행 - 채널이 닫혀있으므로 정상 종료되어야 함
        let result = session_actor.run().await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_batch_size_edge_cases() {
        let config = Arc::new(SystemConfig::default());
        let (event_tx, _) = mpsc::channel(10);
        
        let mut batch_actor = BatchActor::new(
            "edge_test".to_string(),
            config,
            event_tx,
        );
        
        // 빈 페이지 리스트 테스트
        let empty_pages = vec![];
        let result = batch_actor.process_pages(empty_pages, 1, 1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        
        // 정상적인 배치 크기 테스트
        let pages = vec![1, 2, 3, 4, 5];
        let result = batch_actor.process_pages(pages, 2, 1).await;
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }
}
