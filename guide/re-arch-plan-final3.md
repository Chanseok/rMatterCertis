# 최종 통합 설계 계획 v8: Actor 핵심 인프라 우선 구축

> **문서 목적:** `.local/analysis_gap.md` 분석 결과를 반영하여, Actor 시스템의 **핵심 인프라 부재** 문제를 해결하고 검증된 `ServiceBasedBatchCrawlingEngine` 로직을 안정적으로 마이그레이션하기 위한 **구조적 기반 우선 구축** 전략을 수립합니다.

> **🔄 v8 업데이트 (2025.01.03)**: 설계-구현 간극 분석을 통해 확인된 **Actor 핵심 인프라 부재** 문제 해결을 최우선 과제로 설정하고, 점진적 마이그레이션 전략을 수립했습니다.

> **📌 2025.08.09 추가 동기화 메모**
> - v8 문서의 “핵심 인프라 구축” 단계는 현재 Phase Roadmap 상 Phase 1 완료 상태로 전환됨
> - Phase abstraction / Graceful shutdown 최소 기능이 main 브랜치에 반영됨 (ListPages / Finalize)
> - 다음 집중 대상: UI 이중 모드(Advanced Engine / Live Production) 구축 (Phase 2)
> - 본 문서는 인프라 중심, 실행 로드맵 세부는 `re-arch-plan-final2.md` 최신 표 참조
> - Contract Freeze v1: AppEvent / ActorCommand 확정, PhaseStarted/Completed/Aborted & ShutdownRequested/Completed 포함. 변경은 additive-only 정책 적용.


**🦀 Modern Rust 2024 & Clean Code 필수 준수**: 
- `mod.rs` 사용 금지 - 모듈은 `lib.rs` 또는 `directory/file.rs` 사용
- Clippy 권고사항 100% 준수 (`cargo clippy --all-targets --all-features`)
- `#![warn(clippy::all, clippy::pedantic, clippy::nursery)]` 적용
- ts-rs 8.0 기반 자동 타입 생성으로 타입 안전성 보장
- **함수형 프로그래밍 원칙**: 가급적 stateless 메서드, 순수 함수 우선, 불변성 추구
- **명시적 의존성**: 메서드 파라미터로 필요한 모든 데이터를 명시적으로 전달
- **상태 의존성 최소화**: 내부 캐시나 상태에 의존하는 대신 명시적 파라미터 사용

## 1. 핵심 문제점 및 해결 전략

### 1.1 🚨 현재 확인된 핵심 문제점

**Actor 시스템 핵심 인프라 부재**:
- **강한 결합**: Actor들이 서로를 직접 참조하여 확장성 제약
- **중앙화된 제어 불가**: 세션 일시정지, 재개, 취소 등 시스템 전반 제어 신호 전파 방법 없음
- **독립적 이벤트 발행 불가**: Actor가 상태 변화를 시스템 전체에 알릴 수 없음
- **타입 동기화 부재**: 백엔드-프론트엔드 간 데이터 모델 불일치

### 1.2 🎯 해결 전략: 구조적 기반 우선 구축

**Phase 1: Actor 핵심 인프라 구축 (최우선)**
```
src-tauri/src/new_architecture/
├── context.rs               // AppContext, EventEmitter trait
├── channels/
│   ├── types.rs            // ControlChannel, DataChannel, EventChannel  
│   └── channels.rs         // 채널 팩토리 및 유틸리티
├── actors/
│   ├── types.rs            // ActorCommand enum 통합
│   ├── traits.rs           // Actor trait 정의
│   └── message_router.rs   // Actor 간 메시지 라우팅
└── migration/
    └── service_bridge.rs   // 기존 ServiceBased 로직 브릿지
```

**Phase 2: 점진적 마이그레이션**
- `ServiceBasedBatchCrawlingEngine`의 검증된 로직을 새로운 Actor 인프라로 단계별 이식
- 기존 동작 유지하면서 새로운 아키텍처로 안전한 전환

**Phase 3: 타입 동기화 및 UI 연동**
- `ts-rs` 기반 자동 타입 생성
- 프론트엔드 상태 관리 재설계

## 2. Actor 핵심 인프라 설계

### 2.1 AppContext: Actor 간 공유 컨텍스트

```rust
// src-tauri/src/new_architecture/context.rs
use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use tokio_util::sync::CancellationToken;
use serde::{Serialize, Deserialize};
use ts_rs::TS;

/// 모든 Actor가 공유하는 애플리케이션 컨텍스트
#[derive(Clone)]
pub struct AppContext {
    /// 세션 식별자
    pub session_id: String,
    /// 시스템 설정
    pub config: Arc<SystemConfig>,
    /// 이벤트 발행용 채널
    pub event_tx: broadcast::Sender<AppEvent>,
    /// 취소 신호 수신용 토큰
    pub cancellation_token: CancellationToken,
}

impl AppContext {
    pub fn new(
        session_id: String,
        config: Arc<SystemConfig>,
        event_tx: broadcast::Sender<AppEvent>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            session_id,
            config,
            event_tx,
            cancellation_token,
        }
    }
}

/// 이벤트 발행 능력을 가진 Actor를 위한 trait
#[async_trait::async_trait]
pub trait EventEmitter {
    /// 이벤트 발행
    async fn emit_event(&self, event: AppEvent) -> Result<(), ActorError>;
    
    /// 취소 신호 확인
    fn is_cancelled(&self) -> bool;
}

#[async_trait::async_trait]
impl EventEmitter for AppContext {
    async fn emit_event(&self, event: AppEvent) -> Result<(), ActorError> {
        self.event_tx.send(event)
            .map_err(|e| ActorError::EventBroadcastFailed(e.to_string()))?;
        Ok(())
    }
    
    fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}
```

### 2.2 삼중 채널 시스템

```rust
// src-tauri/src/new_architecture/channels/types.rs
use tokio::sync::{mpsc, oneshot, broadcast};
use serde::{Serialize, Deserialize};
use ts_rs::TS;

/// 제어 채널: Actor 간 명령 전달
pub type ControlChannel<T> = mpsc::Sender<T>;
pub type ControlReceiver<T> = mpsc::Receiver<T>;

/// 데이터 채널: 일회성 결과 전달
pub type DataChannel<T> = oneshot::Sender<T>;
pub type DataReceiver<T> = oneshot::Receiver<T>;

/// 이벤트 채널: 상태 변화 브로드캐스트
pub type EventChannel<T> = broadcast::Sender<T>;
pub type EventReceiver<T> = broadcast::Receiver<T>;

/// 채널 팩토리
pub struct ChannelFactory;

impl ChannelFactory {
    /// 제어 채널 생성
    pub fn create_control_channel<T>(buffer_size: usize) -> (ControlChannel<T>, ControlReceiver<T>) {
        mpsc::channel(buffer_size)
    }
    
    /// 데이터 채널 생성
    pub fn create_data_channel<T>() -> (DataChannel<T>, DataReceiver<T>) {
        oneshot::channel()
    }
    
    /// 이벤트 채널 생성
    pub fn create_event_channel<T>(buffer_size: usize) -> EventChannel<T> {
        broadcast::channel(buffer_size).0
    }
}
```

### 2.3 통합 ActorCommand 타입

```rust
// src-tauri/src/new_architecture/actors/types.rs
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use tokio_util::sync::CancellationToken;

/// Actor 간 통신을 위한 통합 명령 타입
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActorCommand {
    // === 세션 레벨 명령 ===
    /// 크롤링 세션 시작
    StartCrawling {
        session_id: String,
        config: CrawlingConfig,
    },
    
    /// 세션 일시정지
    PauseSession {
        session_id: String,
        reason: String,
    },
    
    /// 세션 재개
    ResumeSession {
        session_id: String,
    },
    
    /// 세션 취소
    CancelSession {
        session_id: String,
        reason: String,
    },
    
    // === 배치 레벨 명령 ===
    /// 배치 처리
    ProcessBatch {
        batch_id: String,
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
        total_pages: u32,
        products_on_last_page: u32,
    },
    
    // === 스테이지 레벨 명령 ===
    /// 스테이지 실행
    ExecuteStage {
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
    },
    
    // === 시스템 레벨 명령 ===
    /// 시스템 종료
    Shutdown,
    
    /// 헬스 체크
    HealthCheck,
}

/// Actor 간 전달되는 이벤트
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AppEvent {
    // === 세션 이벤트 ===
    SessionStarted {
        session_id: String,
        config: CrawlingConfig,
    },
    
    SessionPaused {
        session_id: String,
        reason: String,
    },
    
    SessionResumed {
        session_id: String,
    },
    
    SessionCompleted {
        session_id: String,
        summary: SessionSummary,
    },
    
    SessionFailed {
        session_id: String,
        error: String,
        final_failure: bool,
    },
    
    SessionTimeout {
        session_id: String,
        elapsed: std::time::Duration,
    },
    
    // === 배치 이벤트 ===
    BatchStarted {
        batch_id: String,
        pages_count: u32,
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
    
    // === 스테이지 이벤트 ===
    StageStarted {
        stage_type: StageType,
        items_count: u32,
    },
    
    StageCompleted {
        stage_type: StageType,
        result: StageResult,
    },
    
    // === 진행 상황 이벤트 ===
    Progress {
        session_id: String,
        current_step: u32,
        total_steps: u32,
        message: String,
    },
}
```

### 2.4 Actor Trait 정의

```rust
// src-tauri/src/new_architecture/actors/traits.rs
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 모든 Actor가 구현해야 하는 기본 trait
#[async_trait]
pub trait Actor: Send + Sync + 'static {
    type Command: Send + Sync + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// Actor 고유 식별자
    fn actor_id(&self) -> &str;
    
    /// Actor 실행 루프
    async fn run(
        &mut self,
        context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error>;
    
    /// 헬스 체크
    async fn health_check(&self) -> Result<ActorHealth, Self::Error>;
    
    /// 우아한 종료
    async fn shutdown(&mut self) -> Result<(), Self::Error>;
}

/// Actor 헬스 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActorHealth {
    pub actor_id: String,
    pub status: ActorStatus,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub memory_usage_mb: u64,
    pub active_tasks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActorStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { error: String },
}
```

## 3. 점진적 마이그레이션 전략

### 3.1 ServiceBased 로직 브릿지

```rust
// src-tauri/src/new_architecture/migration/service_bridge.rs

/// 기존 ServiceBasedBatchCrawlingEngine 로직을 Actor 시스템으로 브릿지
pub struct ServiceMigrationBridge {
    /// 기존 서비스 엔진
    legacy_engine: Arc<ServiceBasedBatchCrawlingEngine>,
    /// 새로운 Actor 컨텍스트
    actor_context: AppContext,
}

impl ServiceMigrationBridge {
    pub fn new(
        legacy_engine: Arc<ServiceBasedBatchCrawlingEngine>,
        actor_context: AppContext,
    ) -> Self {
        Self {
            legacy_engine,
            actor_context,
        }
    }
    
    /// 기존 배치 크롤링 로직을 Actor 방식으로 래핑
    pub async fn execute_batch_crawling(
        &self,
        pages: Vec<u32>,
        config: BatchConfig,
    ) -> Result<StageResult, ActorError> {
        // 1. 이벤트 발행: 배치 시작
        self.actor_context.emit_event(AppEvent::BatchStarted {
            batch_id: format!("legacy_{}", uuid::Uuid::new_v4()),
            pages_count: pages.len() as u32,
        }).await?;
        
        // 2. 기존 ServiceBased 로직 실행
        let result = self.legacy_engine
            .execute_batch_with_pages(pages, config)
            .await
            .map_err(|e| ActorError::LegacyServiceError(e.to_string()))?;
        
        // 3. 이벤트 발행: 배치 완료
        self.actor_context.emit_event(AppEvent::BatchCompleted {
            batch_id: "legacy_batch".to_string(),
            success_count: result.processed_items,
        }).await?;
        
        Ok(result)
    }
}
```

### 3.2 마이그레이션 단계별 계획

**Step 1: 인프라 구축 (1-2일)**
- `AppContext`, 채널 시스템, `ActorCommand` 타입 구현
- 기본 `Actor` trait 정의

**Step 2: 브릿지 구현 (1일)**  
- `ServiceMigrationBridge`로 기존 로직을 Actor 방식으로 래핑
- 이벤트 발행 기능 추가

**Step 3: 점진적 Actor 구현 (3-4일)**
- `SessionActor` → `BatchActor` → `StageActor` 순서로 구현
- 각 단계마다 기존 동작과 비교 검증

**Step 4: 타입 동기화 (1-2일)**
- `ts-rs` 기반 자동 타입 생성
- 프론트엔드 스토어 업데이트

## 4. 현대적 Rust 모듈 구조 (mod.rs 미사용)

```
src-tauri/src/new_architecture/
├── context.rs
├── channels/
│   ├── types.rs
│   └── channels.rs        // mod.rs 대신 같은 이름의 파일
├── actors/
│   ├── types.rs
│   ├── traits.rs
│   ├── session_actor.rs
│   ├── batch_actor.rs
│   ├── stage_actor.rs
│   ├── message_router.rs
│   └── actors.rs          // mod.rs 대신 같은 이름의 파일
├── migration/
│   ├── service_bridge.rs
│   └── migration.rs       // mod.rs 대신 같은 이름의 파일
└── new_architecture.rs    // mod.rs 대신 같은 이름의 파일
```

**모듈 선언 방식**:
```rust
// src-tauri/src/lib.rs
pub mod new_architecture;

// src-tauri/src/new_architecture/new_architecture.rs 
pub mod context;
pub mod channels;
pub mod actors;
pub mod migration;

// src-tauri/src/new_architecture/channels/channels.rs
pub mod types;
pub use types::*;

// src-tauri/src/new_architecture/actors/actors.rs
pub mod types;
pub mod traits;
pub mod session_actor;
pub mod batch_actor;
pub mod stage_actor;
pub mod message_router;

pub use types::*;
pub use traits::*;
```

## 5. 우선순위 구현 계획

### Phase 1: 핵심 인프라 (즉시 시작)
1. **AppContext 및 EventEmitter trait** (`context.rs`)
2. **삼중 채널 시스템** (`channels/types.rs`, `channels/channels.rs`)
3. **통합 ActorCommand** (`actors/types.rs`)
4. **Actor trait 정의** (`actors/traits.rs`)

### Phase 2: 브릿지 및 검증 (인프라 완료 후)
1. **ServiceMigrationBridge** (`migration/service_bridge.rs`)
2. **기존 동작 검증** (브릿지를 통한 동일 결과 확인)

### Phase 3: Actor 구현 (브릿지 검증 후)
1. **SessionActor** (`actors/session_actor.rs`)
2. **BatchActor** (`actors/batch_actor.rs`)  
3. **StageActor** (`actors/stage_actor.rs`)

### Phase 4: 타입 동기화 (Actor 완료 후)
1. **ts-rs 타입 생성**
2. **프론트엔드 스토어 재설계**

## 결론

이번 v8 계획은 **.local/analysis_gap.md**에서 확인된 **Actor 핵심 인프라 부재** 문제를 해결하기 위해 **구조적 기반을 먼저 구축**하는 것을 최우선으로 합니다. 

**핵심 성공 요인**:
1. **인프라 우선 구축**: 개별 Actor 구현보다 공통 인프라를 먼저 완성
2. **점진적 마이그레이션**: 검증된 ServiceBased 로직을 안전하게 이식
3. **Modern Rust 2024 준수**: `mod.rs` 미사용, Clippy pedantic, 함수형 원칙
4. **명확한 단계별 검증**: 각 Phase마다 동작 확인 후 다음 단계 진행

이 계획을 통해 Actor 시스템의 **독립성**, **확장성**, **재사용성**을 확보하고, 설계 문서에 명시된 목표를 안정적으로 달성할 수 있습니다.
