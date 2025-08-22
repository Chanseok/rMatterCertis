//! 삼중 채널 시스템: 제어, 데이터, 이벤트의 완전한 분리
//! Modern Rust 2024 준수: 설정 기반 채널 관리

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tokio::sync::{mpsc, oneshot, broadcast};
use std::sync::Arc;
use serde::Serialize;
use crate::new_architecture::system_config::SystemConfig;

/// 제어 채널: 명령 하향 전달 (MPSC)
pub type ControlChannel<T> = mpsc::Sender<T>;
pub type ControlReceiver<T> = mpsc::Receiver<T>;

/// 데이터 채널: 결과 상향 보고 (OneShot)
pub type DataChannel<T> = oneshot::Sender<T>;
pub type DataReceiver<T> = oneshot::Receiver<T>;

/// 이벤트 채널: 독립적 상태 발행 (Broadcast)
pub type EventChannel<T> = broadcast::Sender<T>;
pub type EventReceiver<T> = broadcast::Receiver<T>;

/// Actor 명령 정의
#[derive(Debug, Clone)]
pub enum ActorCommand {
    ProcessBatch {
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
        total_pages: u32,
        products_on_last_page: u32,
    },
    ExecuteStage {
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
    },
    CancelSession {
        session_id: String,
        reason: String,
    },
}

/// 스테이지 타입 정의
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StageType {
    Collection,
    Processing,
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
}

/// 배치 설정
#[derive(Debug, Clone, Serialize)]
pub struct BatchConfig {
    pub target_url: String,
    pub max_pages: Option<u32>,
}

/// (Legacy) 앱 이벤트 정의 - new_architecture::actors::types::AppEvent 와 구분
#[derive(Debug, Clone, Serialize)]
pub enum LegacyAppEvent {
    SessionStarted {
        session_id: String,
        config: BatchConfig,
    },
    BatchStarted {
        batch_id: String,
    },
    BatchCompleted {
        batch_id: String,
        success_count: u32,
    },
    BatchFailed {
        batch_id: String,
        error: String,
        final_failure: bool,
    },
    StageCompleted {
        stage: StageType,
        result: crate::new_architecture::actor_system::StageResult,
    },
    SessionTimeout {
        session_id: String,
        elapsed: std::time::Duration,
    },
}

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
    
    /// 데이터 채널 생성
    pub fn create_data_channel<T>(&self) -> (DataChannel<T>, DataReceiver<T>) {
        oneshot::channel()
    }
    
    /// 백프레셔 임계값 확인
    pub fn check_backpressure(&self, current_load: f64) -> bool {
        current_load > self.config.channels.backpressure_threshold
    }

    /// 테스트용 통합 채널 생성
    pub fn create_triple_channel(config: &SystemConfig) -> TripleChannels {
        let (control_tx, control_rx) = mpsc::channel(config.channels.control_buffer_size);
        let (data_tx, data_rx) = oneshot::channel();
        let (event_tx, _) = mpsc::channel(config.channels.event_buffer_size);
        
        TripleChannels {
            control_tx,
            control_rx,
            data_tx,
            data_rx,
            event_tx,
        }
    }
}

/// 테스트용 채널 집합
pub struct TripleChannels {
    pub control_tx: ControlChannel<ActorCommand>,
    pub control_rx: ControlReceiver<ActorCommand>,
    pub data_tx: DataChannel<String>,
    pub data_rx: DataReceiver<String>,
    pub event_tx: mpsc::Sender<AppEvent>,
}
