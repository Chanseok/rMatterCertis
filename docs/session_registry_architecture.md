# SessionRegistry + watch 채널 아키텍처 상세 설명

> 작성일: 2025-10-09  
> 위치: `src-tauri/src/crawl_engine/runtime/session_registry.rs`

## 📋 목차

1. [개요](#개요)
2. [핵심 개념](#핵심-개념)
3. [SessionRegistry 구조](#sessionregistry-구조)
4. [watch 채널 메커니즘](#watch-채널-메커니즘)
5. [동작 흐름](#동작-흐름)
6. [코드 예시](#코드-예시)
7. [Phase trait과의 비교](#phase-trait과의-비교)

---

## 개요

**SessionRegistry + watch 채널**은 Matter Certis v2의 크롤링 세션 제어 시스템입니다. 원래 계획된 "Phase trait" 패턴 대신, 더 간단하고 효율적인 방식으로 구현되었습니다.

### 🎯 핵심 목적

1. **중앙화된 세션 관리**: 모든 크롤링 세션을 하나의 레지스트리에서 관리
2. **실시간 제어**: pause/resume/shutdown 신호를 즉시 전파
3. **상태 추적**: 진행률, 에러, 메트릭을 실시간으로 업데이트
4. **비동기 통신**: tokio watch 채널을 통한 논블로킹 신호 전달

---

## 핵심 개념

### 1. SessionRegistry (세션 레지스트리)

**전역 싱글톤 HashMap**으로 모든 활성 세션을 관리:

```rust
static SESSION_REGISTRY: OnceCell<Arc<RwLock<HashMap<String, SessionEntry>>>> = OnceCell::new();

pub fn session_registry() -> Arc<RwLock<HashMap<String, SessionEntry>>> {
    SESSION_REGISTRY
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}
```

**특징**:
- `OnceCell`: 지연 초기화 (첫 호출 시 생성)
- `Arc<RwLock<HashMap>>`: 멀티스레드 안전 + 읽기/쓰기 락
- Key: `session_id` (String)
- Value: `SessionEntry` (세션 상태 + 제어 채널)

### 2. SessionStatus (세션 상태)

5가지 상태를 가지는 enum:

```rust
pub enum SessionStatus {
    Running,        // 실행 중
    Paused,         // 일시 정지
    Completed,      // 완료
    Failed,         // 실패
    ShuttingDown,   // 종료 중
}
```

**상태 전환**:
```
Running ──pause──> Paused
Paused ──resume──> Running
Running ──shutdown──> ShuttingDown ──완료──> Completed
Running ──에러 누적──> Failed
```

### 3. SessionEntry (세션 엔트리)

각 세션의 완전한 상태를 담는 구조체:

```rust
pub struct SessionEntry {
    // === 상태 & 제어 ===
    pub status: SessionStatus,
    pub pause_tx: watch::Sender<bool>,  // 🔑 핵심! pause 신호 송신기
    
    // === 타임스탬프 ===
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    
    // === 진행률 메트릭 ===
    pub total_pages_planned: u64,
    pub processed_pages: u64,
    pub total_batches_planned: u64,
    pub completed_batches: u64,
    pub batch_size: u32,
    pub concurrency_limit: u32,
    
    // === 에러 추적 ===
    pub last_error: Option<String>,
    pub error_count: u32,
    pub retries_per_page: HashMap<u32, u32>,
    pub failed_pages: Vec<u32>,
    pub error_type_stats: HashMap<String, (u32, DateTime<Utc>, DateTime<Utc>)>,
    
    // === Resume 토큰 ===
    pub resume_token: Option<String>,
    pub remaining_page_slots: Option<Vec<u32>>,
    pub plan_hash: Option<String>,
    
    // === 상품 상세 메트릭 ===
    pub detail_tasks_total: u64,
    pub detail_tasks_completed: u64,
    pub detail_tasks_failed: u64,
    pub detail_retry_counts: HashMap<String, u32>,
    pub remaining_detail_ids: Option<Vec<String>>,
    
    // === 실패 정책 ===
    pub page_failure_threshold: u32,
    pub detail_failure_threshold: u32,
    pub removal_deadline: Option<DateTime<Utc>>,
    
    // === Downshift 메타데이터 ===
    pub detail_downshifted: bool,
    pub detail_downshift_timestamp: Option<DateTime<Utc>>,
    pub detail_downshift_old_limit: Option<u32>,
    pub detail_downshift_new_limit: Option<u32>,
}
```

---

## watch 채널 메커니즘

### tokio::sync::watch란?

**단방향 broadcast 채널**로, 최신 값을 여러 수신자에게 전파:

```rust
use tokio::sync::watch;

// 채널 생성 (초기값: false)
let (tx, rx) = watch::channel(false);

// 송신: 값을 업데이트하면 모든 수신자가 통지받음
tx.send(true);

// 수신: 최신 값을 읽음 (논블로킹)
let paused = *rx.borrow();

// 수신: 값 변경 대기 (비동기)
rx.changed().await;
```

### SessionEntry의 pause_tx

**각 세션마다 독립적인 pause 채널**을 가짐:

```rust
pub struct SessionEntry {
    pub pause_tx: watch::Sender<bool>,  // pause 신호 송신기
    // ...
}
```

**사용 흐름**:

1. **세션 시작 시**: `watch::channel(false)` 생성, tx를 SessionEntry에 저장
2. **Pause 요청 시**: `pause_tx.send(true)` → 모든 worker에게 일시정지 신호 전파
3. **Resume 요청 시**: `pause_tx.send(false)` → worker 재개
4. **Worker 측**: `pause_rx.changed().await` 또는 `pause_rx.borrow()` 체크

### Shutdown 채널

세션별 pause 외에, **전역 shutdown 채널**도 존재:

```rust
// AppState에 저장
let (shutdown_tx, shutdown_rx) = watch::channel(false);

// Shutdown 요청 시
shutdown_tx.send(true);  // 모든 세션에게 종료 신호
```

---

## 동작 흐름

### 1️⃣ 세션 시작 (start_crawling)

```rust
// 1. pause 채널 생성
let (pause_tx, pause_rx) = watch::channel(false);

// 2. SessionEntry 생성 & 레지스트리 등록
let entry = SessionEntry {
    status: SessionStatus::Running,
    pause_tx: pause_tx.clone(),  // 송신기 저장
    started_at: Utc::now(),
    // ... 기타 필드
};

let registry = session_registry();
registry.write().await.insert(session_id.clone(), entry);

// 3. Worker에게 pause_rx 전달 (수신기)
spawn_worker(session_id, pause_rx);
```

### 2️⃣ Pause 요청 (pause_session)

```rust
pub async fn pause_session(session_id: String) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let mut g = registry.write().await;  // 쓰기 락 획득
    
    if let Some(entry) = g.get_mut(&session_id) {
        if entry.status == SessionStatus::Running {
            // 🔑 핵심: pause 신호 전송
            let _ = entry.pause_tx.send(true);
            
            // 상태 업데이트
            entry.status = SessionStatus::Paused;
        }
        Ok(ActorSystemResponse { success: true, ... })
    } else {
        Err(format!("Unknown session_id={}", session_id))
    }
}
```

**전파 흐름**:
```
UI (pause 버튼 클릭)
  → Tauri Command: pause_session
    → SessionRegistry: pause_tx.send(true)
      → watch 채널: 모든 수신자에게 통지
        → Worker 1: pause_rx.changed() 감지 → 작업 일시정지
        → Worker 2: pause_rx.changed() 감지 → 작업 일시정지
        → Worker N: pause_rx.changed() 감지 → 작업 일시정지
```

### 3️⃣ Resume 요청 (resume_session)

```rust
pub async fn resume_session(session_id: String) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let mut g = registry.write().await;
    
    if let Some(entry) = g.get_mut(&session_id) {
        if entry.status == SessionStatus::Paused {
            // 🔑 핵심: resume 신호 전송 (false)
            let _ = entry.pause_tx.send(false);
            
            // 상태 업데이트
            entry.status = SessionStatus::Running;
        }
        Ok(ActorSystemResponse { success: true, ... })
    } else {
        Err(format!("Unknown session_id={}", session_id))
    }
}
```

### 4️⃣ Graceful Shutdown (request_graceful_shutdown)

```rust
pub async fn request_graceful_shutdown(app: AppHandle) -> Result<ActorSystemResponse, String> {
    let app_state = app.state::<AppState>();
    
    // 1. 전역 shutdown 신호 전송
    if let Some(tx) = app_state.get_shutdown_tx().await {
        if tx.send(true).is_err() {
            return Err("Failed to send shutdown signal".into());
        }
        info!("📩 Graceful shutdown signal sent");
        
        // 2. 모든 세션 상태를 ShuttingDown으로 전환
        let registry = session_registry();
        let mut g = registry.write().await;
        for (_id, entry) in g.iter_mut() {
            if entry.status == SessionStatus::Running || entry.status == SessionStatus::Paused {
                entry.status = SessionStatus::ShuttingDown;
            }
        }
        
        Ok(ActorSystemResponse { success: true, ... })
    } else {
        Err("No active session to shutdown".into())
    }
}
```

### 5️⃣ Worker 측 pause 체크

Worker는 두 가지 방식으로 pause 신호를 체크:

**방식 1: 비동기 대기** (권장)
```rust
async fn worker_loop(pause_rx: watch::Receiver<bool>) {
    loop {
        // pause 상태 체크
        if *pause_rx.borrow() {
            // pause 상태라면 대기
            let _ = pause_rx.changed().await;  // resume될 때까지 대기
            continue;
        }
        
        // 실제 작업 수행
        do_work().await;
    }
}
```

**방식 2: 폴링** (레거시)
```rust
async fn worker_loop(pause_rx: watch::Receiver<bool>) {
    loop {
        // 주기적으로 pause 상태 확인
        if *pause_rx.borrow() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }
        
        // 작업 수행
        do_work().await;
    }
}
```

---

## 코드 예시

### 완전한 pause/resume 예제

```rust
use tokio::sync::watch;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // 1. 채널 생성
    let (pause_tx, mut pause_rx) = watch::channel(false);
    
    // 2. Worker 스폰
    let worker_handle = tokio::spawn(async move {
        let mut counter = 0;
        loop {
            // Pause 체크
            if *pause_rx.borrow() {
                println!("⏸️  Worker paused, waiting for resume...");
                let _ = pause_rx.changed().await;  // resume 대기
                println!("▶️  Worker resumed");
                continue;
            }
            
            // 작업 수행
            counter += 1;
            println!("🔄 Processing item {}", counter);
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            if counter >= 20 {
                break;
            }
        }
        println!("✅ Worker finished");
    });
    
    // 3. 메인 스레드: 제어 신호 전송
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("🛑 Sending pause signal...");
    let _ = pause_tx.send(true);
    
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("▶️  Sending resume signal...");
    let _ = pause_tx.send(false);
    
    // 4. Worker 완료 대기
    worker_handle.await.unwrap();
}
```

**출력 예시**:
```
🔄 Processing item 1
🔄 Processing item 2
🔄 Processing item 3
🛑 Sending pause signal...
⏸️  Worker paused, waiting for resume...
▶️  Sending resume signal...
▶️  Worker resumed
🔄 Processing item 4
🔄 Processing item 5
...
✅ Worker finished
```

---

## Phase trait과의 비교

### 원래 계획: Phase trait 패턴

**아키텍처 문서에서 언급**된 방식:

```rust
// ❌ 실제로는 구현되지 않음
trait Phase {
    async fn run(&mut self, ctx: &AppContext) -> Result<PhaseResult>;
    async fn pause(&mut self);
    async fn resume(&mut self);
    async fn shutdown(&mut self);
}

struct ListPagePhase { ... }
impl Phase for ListPagePhase { ... }

struct ProductDetailPhase { ... }
impl Phase for ProductDetailPhase { ... }
```

**장점**:
- ✅ 객체지향적 추상화
- ✅ Phase별 독립적인 로직 구현
- ✅ 타입 안전성

**단점**:
- ❌ 과도한 추상화 (간단한 작업에 과한 복잡도)
- ❌ 각 Phase마다 pause/resume/shutdown 중복 구현
- ❌ Phase 간 상태 공유 복잡
- ❌ Trait object 사용 시 성능 오버헤드

### 실제 구현: SessionRegistry + watch 채널

**현재 코드베이스**의 방식:

```rust
// ✅ 실제 구현
pub struct SessionEntry {
    pub status: SessionStatus,
    pub pause_tx: watch::Sender<bool>,
    // ... 모든 메트릭과 상태가 하나의 구조체에
}

static SESSION_REGISTRY: OnceCell<Arc<RwLock<HashMap<String, SessionEntry>>>> = OnceCell::new();
```

**장점**:
- ✅ **단순함**: 하나의 레지스트리로 모든 세션 관리
- ✅ **효율성**: watch 채널은 zero-copy broadcast
- ✅ **중앙화**: 상태 조회가 `registry.read().await` 한 번으로 해결
- ✅ **확장성**: SessionEntry 필드 추가만으로 기능 확장
- ✅ **디버깅**: 모든 세션 상태를 한 곳에서 조회 가능

**단점**:
- ⚠️ Phase별 추상화 부족 (모든 로직이 함수로 분산)
- ⚠️ SessionEntry가 비대해질 가능성 (현재 60+ 필드)

### 왜 Phase trait을 버렸는가?

1. **실용주의**: CSA-IoT 크롤링은 단순한 2단계 프로세스 (리스트 → 상세)
   - Phase trait의 복잡한 추상화 불필요
   
2. **Rust 생태계 관행**: tokio 생태계는 watch/mpsc 채널 기반 제어를 선호
   - trait 기반 Actor보다 채널 기반이 더 자연스러움
   
3. **개발 속도**: watch 채널은 10줄로 pause/resume 구현 가능
   - Phase trait은 각 Phase마다 boilerplate 필요
   
4. **성능**: watch 채널은 lock-free atomic 연산
   - trait object는 vtable lookup 오버헤드

### 결론

**SessionRegistry + watch 채널**은 Phase trait보다 **더 간단하고, 더 빠르고, 더 실용적**인 선택이었습니다. 아키텍처 계획 문서는 이상적인 설계를 제시했지만, 실제 구현에서는 YAGNI(You Aren't Gonna Need It) 원칙에 따라 필요한 것만 구현했습니다.

---

## 추가 참고 자료

- `src-tauri/src/crawl_engine/runtime/session_registry.rs` - SessionRegistry 구현
- `src-tauri/src/commands/crawling/actor_system.rs` - pause/resume/shutdown 커맨드
- `src-tauri/src/application/state.rs` - AppState와 shutdown_tx 관리
- [tokio::sync::watch 문서](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)
