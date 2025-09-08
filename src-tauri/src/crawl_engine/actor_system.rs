// 🔄 Phase 2 호환성: actor_system 브릿지
// 기존 코드가 찾는 SessionActor, StageActor, ActorError 등을 임시 제공

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

// SessionActor - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct SessionActor {
    pub session_id: String,
}

impl SessionActor {
    #[must_use]
    pub const fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

// StageActor - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct StageActor {
    pub stage_id: String,
}

impl StageActor {
    #[must_use]
    pub const fn new(stage_id: String) -> Self {
        Self { stage_id }
    }
}

// BatchActor compatibility removed

// ActorSystem - 기존 코드 호환성
#[derive(Debug, Clone)]
pub struct ActorSystem {
    pub system_id: String,
}

impl ActorSystem {
    #[must_use]
    pub const fn new(system_id: String) -> Self {
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
            Self::InitializationError(msg) => write!(f, "Initialization error: {}", msg),
            Self::CommunicationError(msg) => write!(f, "Communication error: {}", msg),
            Self::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
            Self::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
            Self::LegacyServiceError(msg) => write!(f, "Legacy service error: {}", msg),
            Self::EventBroadcastFailed(msg) => write!(f, "Event broadcast failed: {}", msg),
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
            Self::ValidationError { message } => write!(f, "Validation error: {}", message),
            Self::ProcessingError { message } => write!(f, "Processing error: {}", message),
            Self::NetworkError { message } => write!(f, "Network error: {}", message),
            Self::TimeoutError { duration } => write!(f, "Timeout error: {:?}", duration),
            Self::ConfigurationError { message } => {
                write!(f, "Configuration error: {}", message)
            }
            Self::NetworkTimeout { timeout_secs } => {
                write!(f, "Network timeout: {}s", timeout_secs)
            }
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
