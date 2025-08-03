//! ServiceMigrationBridge
//! 
//! 기존 ServiceBasedBatchCrawlingEngine 로직을 새로운 Actor 시스템으로 
//! 안전하게 브릿지하는 마이그레이션 레이어입니다.

use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use ts_rs::TS;

use crate::infrastructure::service_based_crawling_engine::ServiceBasedBatchCrawlingEngine;
use super::super::{
    context::AppContext,
    actors::types::{
        AppEvent, ActorError, StageResult, StageItemResult, 
        BatchConfig, StageType, StageItem, StageItemType
    }
};

/// 기존 ServiceBasedBatchCrawlingEngine 로직을 Actor 시스템으로 브릿지
/// 
/// 이 브릿지는 Phase 2에서 기존 검증된 로직을 새로운 Actor 시스템으로
/// 안전하게 마이그레이션하기 위해 사용됩니다.
#[derive(Debug, Clone)]
pub struct ServiceMigrationBridge {
    /// 기존 서비스 엔진에 대한 참조
    legacy_engine: Arc<ServiceBasedBatchCrawlingEngine>,
    
    /// 새로운 Actor 컨텍스트
    actor_context: AppContext,
    
    /// 브릿지 설정
    config: BridgeConfig,
}

impl ServiceMigrationBridge {
    /// 새로운 브릿지 인스턴스를 생성합니다.
    /// 
    /// # Arguments
    /// * `legacy_engine` - 기존 ServiceBased 엔진
    /// * `actor_context` - Actor 시스템 컨텍스트
    /// * `config` - 브릿지 설정 (옵션)
    #[must_use]
    pub fn new(
        legacy_engine: Arc<ServiceBasedBatchCrawlingEngine>,
        actor_context: AppContext,
        config: Option<BridgeConfig>,
    ) -> Self {
        Self {
            legacy_engine,
            actor_context,
            config: config.unwrap_or_default(),
        }
    }
    
    /// 기존 배치 크롤링 로직을 Actor 방식으로 래핑하여 실행합니다.
    /// 
    /// # Arguments
    /// * `pages` - 처리할 페이지 번호 목록
    /// * `config` - 배치 설정
    /// 
    /// # Returns
    /// * `Ok(StageResult)` - 처리 결과
    /// * `Err(ActorError)` - 처리 실패
    pub async fn execute_batch_crawling(
        &self,
        pages: Vec<u32>,
        config: BatchConfig,
    ) -> Result<StageResult, ActorError> {
        let start_time = Instant::now();
        let batch_id = format!("legacy_{}", Uuid::new_v4());
        
        // 1. 이벤트 발행: 배치 시작
        if self.config.enable_event_emission {
            self.actor_context.emit_event(AppEvent::BatchStarted {
                batch_id: batch_id.clone(),
                session_id: self.actor_context.session_id().to_string(),
                pages_count: pages.len() as u32,
                timestamp: chrono::Utc::now(),
            }).await?;
        }
        
        // 2. 취소 신호 확인
        if self.actor_context.is_cancelled() {
            return Err(ActorError::Cancelled("Batch cancelled before execution".to_string()));
        }
        
        // 3. 기존 ServiceBased 로직 실행 (임시로 mock 구현)
        let result = self.execute_legacy_batch_logic(pages, config).await
            .map_err(|e| ActorError::LegacyServiceError(e.to_string()))?;
        
        let duration = start_time.elapsed();
        
        // 4. 이벤트 발행: 배치 완료
        if self.config.enable_event_emission {
            self.actor_context.emit_event(AppEvent::BatchCompleted {
                batch_id,
                session_id: self.actor_context.session_id().to_string(),
                success_count: result.successful_items,
                failed_count: result.failed_items,
                duration: duration.as_millis() as u64,
                timestamp: chrono::Utc::now(),
            }).await?;
        }
        
        Ok(result)
    }
    
    /// 스테이지 실행을 브릿지합니다.
    /// 
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템 목록
    /// * `concurrency_limit` - 동시 실행 제한
    /// * `timeout_secs` - 타임아웃 (초)
    /// 
    /// # Returns
    /// * `Ok(StageResult)` - 스테이지 실행 결과
    /// * `Err(ActorError)` - 실행 실패
    pub async fn execute_stage(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
    ) -> Result<StageResult, ActorError> {
        let start_time = Instant::now();
        
        // 1. 이벤트 발행: 스테이지 시작
        if self.config.enable_event_emission {
            self.actor_context.emit_event(AppEvent::StageStarted {
                stage_type: stage_type.clone(),
                session_id: self.actor_context.session_id().to_string(),
                items_count: items.len() as u32,
                timestamp: chrono::Utc::now(),
            }).await?;
        }
        
        // 2. 취소 신호 확인
        if self.actor_context.is_cancelled() {
            return Err(ActorError::Cancelled("Stage cancelled before execution".to_string()));
        }
        
        // 3. 스테이지별 로직 실행
        let result = match stage_type {
            StageType::StatusCheck => self.execute_status_check_stage(items).await?,
            StageType::ListPageCrawling => self.execute_list_page_stage(items, concurrency_limit).await?,
            StageType::ProductDetailCrawling => self.execute_detail_stage(items, concurrency_limit).await?,
            StageType::DataValidation => self.execute_validation_stage(items).await?,
            StageType::DataSaving => self.execute_saving_stage(items).await?,
        };
        
        let duration = start_time.elapsed();
        
        // 4. 이벤트 발행: 스테이지 완료
        if self.config.enable_event_emission {
            self.actor_context.emit_event(AppEvent::StageCompleted {
                stage_type,
                session_id: self.actor_context.session_id().to_string(),
                result: result.clone(),
                timestamp: chrono::Utc::now(),
            }).await?;
        }
        
        Ok(result)
    }
    
    /// 기존 배치 로직을 실행합니다 (임시 구현).
    /// 
    /// 실제로는 self.legacy_engine을 사용하지만, 
    /// 현재 컴파일 에러를 피하기 위해 mock으로 구현합니다.
    async fn execute_legacy_batch_logic(
        &self,
        pages: Vec<u32>,
        _config: BatchConfig,
    ) -> Result<StageResult, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: 실제 legacy_engine 호출로 대체
        // let result = self.legacy_engine.execute_batch_with_pages(pages, config).await?;
        
        // Mock 구현 (임시)
        let processed_items = pages.len() as u32;
        let successful_items = (processed_items as f32 * 0.95) as u32; // 95% 성공률
        let failed_items = processed_items - successful_items;
        
        let details = pages.into_iter().enumerate().map(|(i, page)| {
            let success = i % 20 != 0; // 5% 실패율
            StageItemResult {
                item_id: format!("page_{page}"),
                item_type: crate::new_architecture::actors::types::StageItemType::Page { page_number: page },
                success,
                error: if success { None } else { Some("Mock error".to_string()) },
                duration_ms: 100 + (i % 500) as u64,
                retry_count: 0,
            }
        }).collect();
        
        Ok(StageResult {
            processed_items,
            successful_items,
            failed_items,
            duration_ms: 5000,
            details,
        })
    }
    
    /// 상태 확인 스테이지를 실행합니다.
    async fn execute_status_check_stage(&self, items: Vec<StageItem>) -> Result<StageResult, ActorError> {
        // Mock 구현
        let processed_items = items.len() as u32;
        let details = items.into_iter().map(|item| {
            StageItemResult {
                item_id: item.id,
                item_type: crate::new_architecture::actors::types::StageItemType::Url { url_type: "mock".to_string() },
                retry_count: 0,
                success: true,
                error: None,
                duration_ms: 50,
            }
        }).collect();
        
        Ok(StageResult {
            processed_items,
            successful_items: processed_items,
            failed_items: 0,
            duration_ms: 1 * 1000,
            details,
        })
    }
    
    /// 리스트 페이지 크롤링 스테이지를 실행합니다.
    async fn execute_list_page_stage(&self, items: Vec<StageItem>, _concurrency: u32) -> Result<StageResult, ActorError> {
        // Mock 구현
        let processed_items = items.len() as u32;
        let successful_items = (processed_items as f32 * 0.90) as u32;
        let failed_items = processed_items - successful_items;
        
        let details = items.into_iter().enumerate().map(|(i, item)| {
            let success = i % 10 != 0; // 10% 실패율
            StageItemResult {
                item_id: item.id,
                item_type: crate::new_architecture::actors::types::StageItemType::Url { url_type: "mock".to_string() },
                retry_count: 0,
                success,
                error: if success { None } else { Some("Page load failed".to_string()) },
                duration_ms: 200 + ((i % 1000) as u64),
            }
        }).collect();
        
        Ok(StageResult {
            processed_items,
            successful_items,
            failed_items,
            duration_ms: 3 * 1000,
            details,
        })
    }
    
    /// 상품 상세 크롤링 스테이지를 실행합니다.
    async fn execute_detail_stage(&self, items: Vec<StageItem>, _concurrency: u32) -> Result<StageResult, ActorError> {
        // Mock 구현
        let processed_items = items.len() as u32;
        let successful_items = (processed_items as f32 * 0.85) as u32;
        let failed_items = processed_items - successful_items;
        
        let details = items.into_iter().enumerate().map(|(i, item)| {
            let success = i % 7 != 0; // ~15% 실패율
            StageItemResult {
                item_id: item.id,
                item_type: crate::new_architecture::actors::types::StageItemType::Url { url_type: "mock".to_string() },
                retry_count: 0,
                success,
                error: if success { None } else { Some("Detail extraction failed".to_string()) },
                duration_ms: 500 + ((i % 2000) as u64),
            }
        }).collect();
        
        Ok(StageResult {
            processed_items,
            successful_items,
            failed_items,
            duration_ms: 10 * 1000,
            details,
        })
    }
    
    /// 데이터 검증 스테이지를 실행합니다.
    async fn execute_validation_stage(&self, items: Vec<StageItem>) -> Result<StageResult, ActorError> {
        // Mock 구현
        let processed_items = items.len() as u32;
        let successful_items = (processed_items as f32 * 0.98) as u32;
        let failed_items = processed_items - successful_items;
        
        let details = items.into_iter().enumerate().map(|(i, item)| {
            let success = i % 50 != 0; // 2% 실패율
            StageItemResult {
                item_id: item.id,
                item_type: crate::new_architecture::actors::types::StageItemType::Url { url_type: "mock".to_string() },
                retry_count: 0,
                success,
                error: if success { None } else { Some("Validation failed".to_string()) },
                duration_ms: 10 + ((i % 50) as u64),
            }
        }).collect();
        
        Ok(StageResult {
            processed_items,
            successful_items,
            failed_items,
            duration_ms: 500,
            details,
        })
    }
    
    /// 데이터 저장 스테이지를 실행합니다.
    async fn execute_saving_stage(&self, items: Vec<StageItem>) -> Result<StageResult, ActorError> {
        // Mock 구현
        let processed_items = items.len() as u32;
        let successful_items = (processed_items as f32 * 0.99) as u32;
        let failed_items = processed_items - successful_items;
        
        let details = items.into_iter().enumerate().map(|(i, item)| {
            let success = i % 100 != 0; // 1% 실패율
            StageItemResult {
                item_id: item.id,
                item_type: crate::new_architecture::actors::types::StageItemType::Url { url_type: "mock".to_string() },
                retry_count: 0,
                success,
                error: if success { None } else { Some("Database save failed".to_string()) },
                duration_ms: 20 + ((i % 100) as u64),
            }
        }).collect();
        
        Ok(StageResult {
            processed_items,
            successful_items,
            failed_items,
            duration_ms: 1 * 1000,
            details,
        })
    }
}

/// 브릿지 설정
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BridgeConfig {
    /// 이벤트 발행 활성화 여부
    pub enable_event_emission: bool,
    
    /// 성능 메트릭 수집 여부
    pub enable_performance_metrics: bool,
    
    /// 상세 로깅 활성화 여부
    pub enable_detailed_logging: bool,
    
    /// 에러 시 재시도 여부
    pub enable_retry_on_error: bool,
    
    /// 최대 재시도 횟수
    pub max_retry_attempts: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enable_event_emission: true,
            enable_performance_metrics: true,
            enable_detailed_logging: false,
            enable_retry_on_error: true,
            max_retry_attempts: 3,
        }
    }
}

/// 브릿지 유틸리티 함수들
pub mod utils {
    use super::*;
    
    /// 페이지 번호 목록을 StageItem으로 변환합니다.
    #[must_use]
    pub fn pages_to_stage_items(pages: Vec<u32>) -> Vec<StageItem> {
        pages.into_iter().map(|page| {
            StageItem {
                id: format!("page_{page}"),
                item_type: StageItemType::Page { page_number: page },
                url: format!("https://example.com/page/{page}"),
                metadata: serde_json::json!({"page_number": page}).to_string(),
            }
        }).collect()
    }
    
    /// BatchConfig를 레거시 형식으로 변환합니다.
    /// 
    /// 실제 마이그레이션 시 필요한 설정 변환 로직입니다.
    #[must_use]
    pub fn convert_batch_config_to_legacy(_config: &BatchConfig) -> serde_json::Value {
        // TODO: 실제 레거시 설정 구조체로 변환
        serde_json::json!({
            "batch_size": _config.batch_size,
            "concurrency_limit": _config.concurrency_limit,
            "retry_on_failure": _config.retry_on_failure
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;
    use std::sync::Arc;
    
    // Mock ServiceBasedBatchCrawlingEngine for testing
    struct MockServiceEngine;
    
    #[tokio::test]
    async fn test_bridge_creation() {
        let mock_engine = Arc::new(MockServiceEngine);
        let (event_tx, _) = broadcast::channel(100);
        let context = AppContext::new(
            "test-session".to_string(),
            Arc::new(crate::new_architecture::context::SystemConfig::default()),
            event_tx,
            CancellationToken::new(),
        );
        
        let bridge = ServiceMigrationBridge::new(
            mock_engine as Arc<dyn std::any::Any + Send + Sync>, // Type workaround for test
            context,
            None,
        );
        
        assert!(bridge.config.enable_event_emission);
    }
    
    #[tokio::test]
    async fn test_pages_to_stage_items_conversion() {
        let pages = vec![1, 2, 3, 4, 5];
        let items = utils::pages_to_stage_items(pages);
        
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].id, "page_1");
        
        match &items[0].item_type {
            StageItemType::Page { page_number } => assert_eq!(*page_number, 1),
            _ => panic!("Expected Page item type"),
        }
    }
    
    #[test]
    fn test_bridge_config_default() {
        let config = BridgeConfig::default();
        
        assert!(config.enable_event_emission);
        assert!(config.enable_performance_metrics);
        assert!(!config.enable_detailed_logging);
        assert!(config.enable_retry_on_error);
        assert_eq!(config.max_retry_attempts, 3);
    }
}
