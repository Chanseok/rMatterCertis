//! 삼중 채널 시스템: 제어, 데이터, 이벤트의 완전한 분리
//! Modern Rust 2024 준수: mod.rs 사용 금지, 명확한 파일 단위 분리
//! 모든 채널 크기는 설정 기반으로 동적 조정 가능

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tokio::sync::{mpsc, oneshot, broadcast, watch};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::new_architecture::SystemConfig;

/// 제어 채널: 명령 하향 전달 (MPSC)
pub type ControlChannel<T> = mpsc::Sender<T>;
pub type ControlReceiver<T> = mpsc::Receiver<T>;

/// 데이터 채널: 결과 상향 보고 (OneShot)
pub type DataChannel<T> = oneshot::Sender<T>;
pub type DataReceiver<T> = oneshot::Receiver<T>;

/// 이벤트 채널: 독립적 상태 발행 (Broadcast)
pub type EventChannel<T> = broadcast::Sender<T>;
pub type EventReceiver<T> = broadcast::Receiver<T>;

/// 취소 신호 채널 (Watch)
pub type CancellationChannel = watch::Sender<bool>;
pub type CancellationReceiver = watch::Receiver<bool>;

/// Actor 명령 정의
#[derive(Debug, Clone)]
pub enum ActorCommand {
    /// 배치 처리 명령
    ProcessBatch {
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
        total_pages: u32,
        products_on_last_page: u32,
    },
    
    /// 스테이지 실행 명령
    ExecuteStage {
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
    },
    
    /// 세션 취소 명령
    CancelSession {
        session_id: String,
        reason: String,
    },
    
    /// 일시 정지 명령
    PauseSession {
        session_id: String,
    },
    
    /// 재개 명령
    ResumeSession {
        session_id: String,
    },
}

/// 스테이지 타입 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageType {
    ListCollection,
    DetailCollection,
    DataValidation,
    DatabaseSave,
}

/// 스테이지 아이템 정의
#[derive(Debug, Clone)]
pub enum StageItem {
    Page(u32),
    Url(String),
    Product(ProductInfo),
    ValidationTarget(String),
    // 새로운 파이프라인 타입들 추가
    ProductList(ProductList),
    ProductUrls(ProductUrls),
    ProductDetails(ProductDetails),
    ValidatedProducts(ValidatedProducts),
}

/// 제품 목록 (Stage 2 ListPageCrawling 결과)
#[derive(Debug, Clone)]
pub struct ProductList {
    pub products: Vec<ProductInfo>,
    pub page_number: u32,
    pub total_count: Option<u32>,
}

/// 제품 URL 목록 (Stage 2 ProductUrlExtraction 결과)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductUrls {
    pub urls: Vec<String>,
    pub batch_id: Option<String>,
}

/// 제품 상세 정보 목록 (Stage 3 ProductDetailCrawling 결과)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetails {
    pub products: Vec<crate::domain::product::ProductDetail>,
    pub source_urls: Vec<String>,
    pub extraction_stats: ExtractionStats,
}

/// 검증된 제품 목록 (Stage 4 DataValidation 결과)  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedProducts {
    pub products: Vec<crate::domain::product::ProductDetail>,
    pub validation_report: Option<crate::new_architecture::services::data_quality_analyzer::DataQualityReport>,
    pub storage_recommendation: StorageRecommendation,
}

/// 데이터 추출 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionStats {
    pub attempted: u32,
    pub successful: u32,
    pub failed: u32,
    pub empty_responses: u32,
}

/// 저장 권장도
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRecommendation {
    HighlyRecommended,
    ConditionallyRecommended,
    ReviewRequired,
    NotRecommended,
}

/// 배치 설정
#[derive(Debug, Clone, Serialize)]
pub struct BatchConfig {
    pub target_url: String,
    pub max_pages: Option<u32>,
    pub filters: Vec<String>,
    pub batch_size: u32,
    pub concurrency_limit: u32,
    pub timeout_secs: u64,
    pub retry_attempts: u32,
}

// AppEvent는 actors/types.rs에서 import해서 사용
pub use crate::new_architecture::actors::types::AppEvent;

// 타입들을 frontend_api에서 import
pub use crate::types::frontend_api::ProductInfo;
pub use crate::new_architecture::actors::types::{
    StageSuccessResult, CollectionMetrics, ProcessingMetrics
};

/// 채널 팩토리 - 설정 기반 채널 생성
pub struct ChannelFactory {
    config: Arc<SystemConfig>,
}

impl ChannelFactory {
    pub fn new(config: Arc<SystemConfig>) -> Self {
        Self { config }
    }
    
    /// 설정 기반 제어 채널 생성
    pub fn create_control_channel<T>(&self) -> (ControlChannel<T>, ControlReceiver<T>) {
        mpsc::channel(self.config.channels.control_buffer_size)
    }
    
    /// 설정 기반 이벤트 채널 생성
    pub fn create_event_channel<T: Clone>(&self) -> EventChannel<T> {
        let (tx, _) = broadcast::channel(self.config.channels.event_buffer_size);
        tx
    }
    
    /// 데이터 채널 생성 (OneShot은 크기 설정 불필요)
    pub fn create_data_channel<T>(&self) -> (DataChannel<T>, DataReceiver<T>) {
        oneshot::channel()
    }
    
    /// 취소 채널 생성
    pub fn create_cancellation_channel(&self) -> (CancellationChannel, CancellationReceiver) {
        watch::channel(false)
    }
    
    /// 백프레셔 임계값 확인
    pub fn check_backpressure(&self, current_load: f64) -> bool {
        current_load > self.config.channels.backpressure_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_architecture::config::system_config::SystemConfig;

    #[test]
    fn test_channel_factory_creation() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config.clone());
        
        // 제어 채널 생성 테스트
        let (tx, rx) = factory.create_control_channel::<ActorCommand>();
        assert!(tx.capacity() > 0);
        
        // 이벤트 채널 생성 테스트
        let event_tx = factory.create_event_channel::<AppEvent>();
        assert!(event_tx.receiver_count() == 0); // 아직 구독자 없음
        
        // 데이터 채널 생성 테스트
        let (data_tx, data_rx) = factory.create_data_channel::<String>();
        // OneShot 채널은 사용 전까지는 특별한 검증이 어려움
    }
    
    #[test]
    fn test_backpressure_check() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config);
        
        // 정상 부하 (80% 미만)
        assert!(!factory.check_backpressure(0.7));
        
        // 백프레셔 발생 (80% 이상)
        assert!(factory.check_backpressure(0.9));
    }
    
    #[tokio::test]
    async fn test_control_channel_communication() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config);
        
        let (tx, mut rx) = factory.create_control_channel::<ActorCommand>();
        
        let test_command = ActorCommand::CancelSession {
            session_id: "test-session".to_string(),
            reason: "test".to_string(),
        };
        
        // 명령 전송
        tx.send(test_command).await.expect("Failed to send command");
        
        // 명령 수신
        let received = rx.recv().await.expect("Failed to receive command");
        
        match received {
            ActorCommand::CancelSession { session_id, reason } => {
                assert_eq!(session_id, "test-session");
                assert_eq!(reason, "test");
            }
            _ => panic!("Wrong command type received"),
        }
    }
}
