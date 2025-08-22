//! 새로운 아키텍처 테스트 명령어들
//! 앱 UI를 통해 새로운 Actor 시스템을 시험할 수 있는 명령어들

use crate::crawl_engine::{
    system_config::SystemConfig,
    channels::types::ChannelFactory,
    actor_system::{SessionActor, BatchActor, StageActor},
};
use std::{sync::Arc, time::Instant};
use tauri::command;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewArchTestResult {
    pub success: bool,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfigInfo {
    pub session_timeout_secs: u64,
    pub max_concurrent_sessions: u32,
    pub control_buffer_size: usize,
    pub event_buffer_size: usize,
    pub batch_initial_size: u32,
    pub max_concurrent_tasks: u32,
}

/// 새로운 아키텍처 시스템 설정 정보 조회
#[command]
pub async fn get_new_arch_config() -> Result<SystemConfigInfo, String> {
    let config = SystemConfig::default();
    
    match config.validate() {
        Ok(_) => {
            let info = SystemConfigInfo {
                session_timeout_secs: config.system.session_timeout_secs,
                max_concurrent_sessions: config.system.max_concurrent_sessions,
                control_buffer_size: config.channels.control_buffer_size,
                event_buffer_size: config.channels.event_buffer_size,
                batch_initial_size: config.performance.batch_sizes.initial_size,
                max_concurrent_tasks: config.performance.concurrency.max_concurrent_tasks,
            };
            
            tracing::info!("새로운 아키텍처 설정 정보 조회 성공");
            Ok(info)
        }
        Err(e) => {
            tracing::error!("설정 검증 실패: {}", e);
            Err(format!("설정 검증 실패: {}", e))
        }
    }
}

/// 새로운 아키텍처 채널 통신 테스트
#[command]
pub async fn test_new_arch_channels() -> Result<NewArchTestResult, String> {
    let start_time = Instant::now();
    
    tracing::info!("새로운 아키텍처 채널 통신 테스트 시작");
    
    let config = SystemConfig::default();
    match config.validate() {
        Ok(_) => {
            let _channels = ChannelFactory::create_triple_channel(&config);
            
            let execution_time = start_time.elapsed().as_millis() as u64;
            
            tracing::info!("채널 통신 테스트 성공 ({}ms)", execution_time);
            Ok(NewArchTestResult {
                success: true,
                message: "채널 생성 및 초기화 성공".to_string(),
                details: Some(serde_json::json!({
                    "control_buffer_size": config.channels.control_buffer_size,
                    "event_buffer_size": config.channels.event_buffer_size
                })),
                execution_time_ms: execution_time,
            })
        }
        Err(e) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            tracing::error!("설정 검증 실패: {}", e);
            Ok(NewArchTestResult {
                success: false,
                message: format!("설정 검증 실패: {}", e),
                details: None,
                execution_time_ms: execution_time,
            })
        }
    }
}

/// SessionActor 테스트
#[command]
pub async fn test_new_arch_session_actor() -> Result<NewArchTestResult, String> {
    let start_time = Instant::now();
    
    tracing::info!("SessionActor 테스트 시작");
    
    let config = SystemConfig::default();
    match config.validate() {
        Ok(_) => {
            let channels = ChannelFactory::create_triple_channel(&config);
            
            match SessionActor::new(Arc::new(config.clone()), channels).await {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    tracing::info!("SessionActor 테스트 성공 ({}ms)", execution_time);
                    Ok(NewArchTestResult {
                        success: true,
                        message: "SessionActor 생성 및 초기화 성공".to_string(),
                        details: Some(serde_json::json!({
                            "session_timeout_secs": config.system.session_timeout_secs,
                            "max_concurrent_sessions": config.system.max_concurrent_sessions
                        })),
                        execution_time_ms: execution_time,
                    })
                }
                Err(e) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    tracing::error!("SessionActor 테스트 실패: {}", e);
                    Ok(NewArchTestResult {
                        success: false,
                        message: format!("SessionActor 생성 실패: {}", e),
                        details: None,
                        execution_time_ms: execution_time,
                    })
                }
            }
        }
        Err(e) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            tracing::error!("설정 검증 실패: {}", e);
            Ok(NewArchTestResult {
                success: false,
                message: format!("설정 검증 실패: {}", e),
                details: None,
                execution_time_ms: execution_time,
            })
        }
    }
}

/// BatchActor 테스트
#[command]
pub async fn test_new_arch_batch_actor() -> Result<NewArchTestResult, String> {
    let start_time = Instant::now();
    
    tracing::info!("BatchActor 테스트 시작");
    
    let config = SystemConfig::default();
    match config.validate() {
        Ok(_) => {
            let channels = ChannelFactory::create_triple_channel(&config);
            
            match BatchActor::new(Arc::new(config.clone()), channels).await {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    tracing::info!("BatchActor 테스트 성공 ({}ms)", execution_time);
                    Ok(NewArchTestResult {
                        success: true,
                        message: "BatchActor 생성 및 초기화 성공".to_string(),
                        details: Some(serde_json::json!({
                            "batch_initial_size": config.performance.batch_sizes.initial_size,
                            "max_batch_size": config.performance.batch_sizes.max_size,
                            "min_batch_size": config.performance.batch_sizes.min_size
                        })),
                        execution_time_ms: execution_time,
                    })
                }
                Err(e) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    tracing::error!("BatchActor 테스트 실패: {}", e);
                    Ok(NewArchTestResult {
                        success: false,
                        message: format!("BatchActor 생성 실패: {}", e),
                        details: None,
                        execution_time_ms: execution_time,
                    })
                }
            }
        }
        Err(e) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            tracing::error!("설정 검증 실패: {}", e);
            Ok(NewArchTestResult {
                success: false,
                message: format!("설정 검증 실패: {}", e),
                details: None,
                execution_time_ms: execution_time,
            })
        }
    }
}

/// 통합 테스트
#[command]
pub async fn test_new_arch_integration() -> Result<NewArchTestResult, String> {
    let start_time = Instant::now();
    
    tracing::info!("새로운 아키텍처 통합 테스트 시작");
    
    let config = SystemConfig::default();
    match config.validate() {
        Ok(_) => {
            // 모든 컴포넌트 생성 테스트
            let channels1 = ChannelFactory::create_triple_channel(&config);
            let channels2 = ChannelFactory::create_triple_channel(&config);
            let channels3 = ChannelFactory::create_triple_channel(&config);
            
            match tokio::try_join!(
                SessionActor::new(Arc::new(config.clone()), channels1),
                BatchActor::new(Arc::new(config.clone()), channels2),
                StageActor::new(Arc::new(config.clone()), channels3)
            ) {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    tracing::info!("통합 테스트 성공 ({}ms)", execution_time);
                    Ok(NewArchTestResult {
                        success: true,
                        message: "전체 Actor 시스템 통합 테스트 성공".to_string(),
                        details: Some(serde_json::json!({
                            "components_tested": ["SystemConfig", "ChannelFactory", "SessionActor", "BatchActor", "StageActor"],
                            "total_test_time_ms": execution_time
                        })),
                        execution_time_ms: execution_time,
                    })
                }
                Err(e) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    tracing::error!("통합 테스트 실패: {}", e);
                    Ok(NewArchTestResult {
                        success: false,
                        message: format!("통합 테스트 실패: {}", e),
                        details: None,
                        execution_time_ms: execution_time,
                    })
                }
            }
        }
        Err(e) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            tracing::error!("설정 검증 실패: {}", e);
            Ok(NewArchTestResult {
                success: false,
                message: format!("설정 검증 실패: {}", e),
                details: None,
                execution_time_ms: execution_time,
            })
        }
    }
}

/// 성능 테스트
#[command]
pub async fn test_new_arch_performance() -> Result<NewArchTestResult, String> {
    let start_time = Instant::now();
    
    tracing::info!("새로운 아키텍처 성능 테스트 시작");
    
    let config = SystemConfig::default();
    match config.validate() {
        Ok(_) => {
            let mut performance_metrics = Vec::new();
            
            // 설정 생성 성능 테스트
            let config_start = Instant::now();
            for _ in 0..100 {
                let _config = SystemConfig::default();
            }
            let config_time = config_start.elapsed().as_micros();
            performance_metrics.push(("config_creation_100x", config_time));
            
            // 채널 생성 성능 테스트
            let channel_start = Instant::now();
            for _ in 0..10 {
                let _channels = ChannelFactory::create_triple_channel(&config);
            }
            let channel_time = channel_start.elapsed().as_micros();
            performance_metrics.push(("channel_creation_10x", channel_time));
            
            let execution_time = start_time.elapsed().as_millis() as u64;
            
            tracing::info!("성능 테스트 성공 ({}ms)", execution_time);
            
            let details = serde_json::json!({
                "performance_metrics": performance_metrics.iter().map(|(name, time)| {
                    serde_json::json!({
                        "test": name,
                        "time_microseconds": time
                    })
                }).collect::<Vec<_>>(),
                "total_test_time_ms": execution_time
            });
            
            Ok(NewArchTestResult {
                success: true,
                message: format!("성능 테스트 성공: {} 항목 측정", performance_metrics.len()),
                details: Some(details),
                execution_time_ms: execution_time,
            })
        }
        Err(e) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            tracing::error!("설정 검증 실패: {}", e);
            Ok(NewArchTestResult {
                success: false,
                message: format!("설정 검증 실패: {}", e),
                details: None,
                execution_time_ms: execution_time,
            })
        }
    }
}
