# 기본 백엔드 검증 및 Live Production Line UI 전 준비 계획

## 🎯 목표

Live Production Line UI 구현 전에 **기본 백엔드 인프라의 안정성과 이벤트 시스템이 완전히 검증된 상태**로 만드는 것

---

## 📋 1단계: 기본 크롤링 기능 검증 (1-2일)

### ✅ 이미 완료된 사항
- [x] 하드코딩된 크롤링 범위 계산 수정 완료
- [x] `site_status.products_on_last_page` 실제 값 사용
- [x] `config.app_managed.avg_products_per_page` 설정 값 사용
- [x] 백엔드 컴파일 정상 동작 확인

### 🔄 진행해야 할 검증 사항

#### 1.1 크롤링 범위 계산 동작 검증
```bash
# 테스트 방법
1. Tauri 앱 실행 (이미 진행 중)
2. 프론트엔드에서 크롤링 시작
3. 로그를 통해 범위 계산이 올바르게 되는지 확인
4. 하드코딩된 값이 아닌 실제 사이트 정보 사용 확인
```

#### 1.2 동시성(Concurrency) 크롤링 검증
```rust
// 확인해야 할 설정들
- MAX_CONCURRENT_REQUESTS: 동시 요청 수 제한
- 배치 처리 로직의 안정성
- 메모리 사용량 및 성능
```

#### 1.3 이벤트 시스템 기본 동작 검증
```typescript
// 현재 구현된 이벤트들이 정상 발생하는지 확인
- crawling-progress: 진행률 업데이트
- crawling-task-update: 작업 상태 업데이트  
- crawling-stage-change: 단계 변경
- crawling-error: 오류 발생
- crawling-completed: 완료
```

---

## 📋 2단계: 이벤트 시스템 확장 준비 (2-3일)

### 2.1 현재 AtomicTaskEvent 구조 분석
```rust
// src-tauri/src/domain/atomic_events.rs 현재 상태 확인
pub struct AtomicTaskEvent {
    // 현재 구조가 Live Production Line에 충분한지 검증
}
```

### 2.2 Live Production Line에 필요한 추가 이벤트 정의
```rust
// 필요한 이벤트 유형들:
1. TaskCreated: 태스크 생성 시
2. TaskQueued: 큐에 대기 상태로 추가
3. TaskStarted: 처리 시작
4. TaskProgress: 진행 중 상태 업데이트 (중간 진행률)
5. TaskCompleted: 성공적 완료
6. TaskFailed: 실패
7. TaskRetrying: 재시도 중
8. BatchStarted/BatchCompleted: 배치 단위 이벤트
```

### 2.3 이벤트 메타데이터 확장
```rust
// Live Production Line 시각화에 필요한 정보들
pub struct TaskMetadata {
    pub page_id: Option<u32>,
    pub product_id: Option<String>,
    pub processing_time_ms: Option<u64>,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub batch_id: Option<String>,
    pub stage_position: Option<(u32, u32)>, // (current_step, total_steps)
}
```

---

## 📋 3단계: 성능 및 안정성 검증 (2-3일)

### 3.1 메모리 사용량 모니터링
```bash
# 크롤링 중 시스템 리소스 사용량 확인
- 메모리 누수 여부
- CPU 사용량
- 네트워크 요청 패턴
```

### 3.2 대용량 크롤링 테스트
```bash
# 실제 환경과 유사한 부하 테스트
- 100+ 페이지 크롤링
- 동시 요청 처리 안정성
- 오류 복구 메커니즘 동작
```

### 3.3 이벤트 처리 성능 검증
```typescript
// 고주파 이벤트 처리 시 프론트엔드 성능
- 이벤트 누적으로 인한 메모리 증가
- UI 블로킹 현상
- 이벤트 손실 여부
```

---

## 📋 4단계: Live Production Line 기반 구조 준비 (1-2일)

### 4.1 백엔드 이벤트 발생 지점 확장
```rust
// 크롤링 엔진 내 주요 지점에서 원자적 이벤트 발생
impl ServiceBasedBatchCrawlingEngine {
    async fn emit_task_lifecycle_events(&self, task_info: TaskInfo) {
        // 각 태스크의 생명주기 전체에서 이벤트 발생
        self.event_emitter.emit_task_created(...).await;
        self.event_emitter.emit_task_queued(...).await;
        // ... 기타 생명주기 이벤트
    }
}
```

### 4.2 프론트엔드 이벤트 구독 최적화
```typescript
// src/services/tauri-api.ts 확장
export class TauriApiService {
    // 기존 이벤트 구독 최적화
    async subscribeToAtomicTaskEvents(
        callback: (event: AtomicTaskEvent) => void,
        options?: {
            bufferSize?: number;
            debounceMs?: number;
            eventTypes?: string[];
        }
    ): Promise<UnlistenFn>
}
```

### 4.3 이벤트 배치 처리 시스템 구현
```typescript
// 고주파 이벤트를 효율적으로 처리하기 위한 배치 시스템
class EventBatchProcessor {
    private eventBuffer: Map<string, AtomicTaskEvent[]> = new Map();
    private batchInterval: number = 16; // 60fps 기준
    
    addEvent(event: AtomicTaskEvent): void {
        // 이벤트 버퍼링 및 배치 처리
    }
    
    processBatch(): void {
        // UI 업데이트를 위한 배치 처리
    }
}
```

---

## 🎯 검증 기준 및 성공 조건

### 기본 기능 검증
- [ ] 크롤링 범위가 하드코딩 없이 정확히 계산됨
- [ ] 동시 크롤링이 안정적으로 동작함
- [ ] 메모리 누수가 없음
- [ ] 오류 복구가 정상 동작함

### 이벤트 시스템 검증
- [ ] 모든 이벤트가 정시에 발생됨
- [ ] 이벤트 손실이 없음
- [ ] 프론트엔드에서 실시간으로 수신됨
- [ ] 고주파 이벤트 처리 시 UI가 블로킹되지 않음

### 성능 검증
- [ ] 100페이지 크롤링 시 안정적 동작
- [ ] 이벤트 처리로 인한 성능 저하 < 5%
- [ ] 메모리 사용량이 합리적 범위 내
- [ ] 프론트엔드 렌더링이 60fps 유지

---

## 🚀 다음 단계: Live Production Line UI 구현

### 조건
위의 모든 검증이 완료되면 Live Production Line UI 구현 시작

### 우선순위
1. **백엔드 안정성 100% 확보**
2. **이벤트 시스템 완전 검증**
3. **성능 최적화 완료**
4. **Live Production Line UI 구현 시작**

---

## 💡 현재 진행 상황

### ✅ 완료
- 하드코딩 문제 해결
- 백엔드 컴파일 성공
- Tauri 앱 실행 중

### 🔄 진행 중
- Tauri 개발 서버 실행
- 기본 크롤링 기능 테스트 대기

### 📋 다음 작업
1. 앱이 완전히 로드되면 크롤링 테스트 시작
2. 로그 분석을 통한 범위 계산 검증
3. 이벤트 발생 패턴 확인

이 계획을 통해 **견고한 기반 위에 Live Production Line UI**를 구축할 수 있습니다.
