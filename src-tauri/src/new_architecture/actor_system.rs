// 🔄 Phase 2 호환성: actor_system 브릿지 
// 기존 코드가 찾는 SessionActor, StageActor, ActorError 등을 임시 제공

use serde::{Serialize, Deserialize};
use std::error::Error;
use std::fmt;

// SessionActor - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct SessionActor {
    pub session_id: String,
}

impl SessionActor {
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

// StageActor - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct StageActor {
    pub stage_id: String,
}

impl StageActor {
    pub fn new(stage_id: String) -> Self {
        Self { stage_id }
    }
}

// BatchActor - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct BatchActor {
    pub batch_id: String,
}

impl BatchActor {
    pub fn new(batch_id: String) -> Self {
        Self { batch_id }
    }
}

// ActorSystem - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct ActorSystem {
    pub system_id: String,
}

impl ActorSystem {
    pub fn new(system_id: String) -> Self {
        Self { system_id }
    }
}

// ActorError - 기존 코드 호환성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorError {
    InitializationError(String),
    CommunicationError(String),
    ProcessingError(String),
    TimeoutError(String),
    LegacyServiceError(String),
    EventBroadcastFailed(String),
}

impl fmt::Display for ActorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorError::InitializationError(msg) => write!(f, "Initialization error: {}", msg),
            ActorError::CommunicationError(msg) => write!(f, "Communication error: {}", msg),
            ActorError::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
            ActorError::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
            ActorError::LegacyServiceError(msg) => write!(f, "Legacy service error: {}", msg),
            ActorError::EventBroadcastFailed(msg) => write!(f, "Event broadcast failed: {}", msg),
        }
    }
}

impl Error for ActorError {}

// StageError - 기존 코드 호환성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageError {
    ValidationError { message: String },
    ProcessingError { message: String },
    NetworkError { message: String },
    TimeoutError { duration: std::time::Duration },
    ConfigurationError { message: String },
    NetworkTimeout { timeout_secs: u64 },
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StageError::ValidationError { message } => write!(f, "Validation error: {}", message),
            StageError::ProcessingError { message } => write!(f, "Processing error: {}", message),
            StageError::NetworkError { message } => write!(f, "Network error: {}", message),
            StageError::TimeoutError { duration } => write!(f, "Timeout error: {:?}", duration),
            StageError::ConfigurationError { message } => write!(f, "Configuration error: {}", message),
            StageError::NetworkTimeout { timeout_secs } => write!(f, "Network timeout: {}s", timeout_secs),
        }
    }
}

impl Error for StageError {}

// StageResult - 기존 코드 호환성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageResult {
    Success {
        processed_items: u32,
        duration_ms: u64,
    },
    Failure {
        error: StageError,
        partial_results: u32,
    },
    RecoverableError {
        error: StageError,
        attempts: u32,
        stage_id: String,
        suggested_retry_delay_ms: u64,
    },
    FatalError {
        error: StageError,
        stage_id: String,
        context: String,
    },
}

// 추가 타입들 (필요에 따라 확장)
pub type SessionResult = StageResult;
pub type BatchResult = StageResult;
