# Live Production Line UI 이벤트 통신 분석 및 개선 방안

## 📋 현재 이벤트 통신 상태 분석

### ✅ 잘 구현된 부분

1. **Backend 이벤트 구조 정의**
   - `SystemStatePayload`: 거시적 시스템 상태 정보
   - `AtomicTaskEvent`: 미시적 개별 작업 이벤트 
   - `LiveSystemState`: Live Production Line용 종합 상태

2. **Frontend 타입 정의**
   - `src/types/events.ts`에서 Backend Rust 구조체와 일치하는 TypeScript 타입 정의
   - 타입 안정성 확보

3. **SystemStateBroadcaster 구현**
   - 2초마다 시스템 상태 자동 브로드캐스트
   - 백그라운드 태스크로 실행

### ❌ 주요 문제점

1. **AtomicTaskEvent 발송 누락**
   ```rust
   // 크롤링 v4 명령에서 AtomicTaskEvent 발송이 구현되지 않음
   // 각 단위 작업(페이지 수집, 상세 정보 수집)의 생명주기 이벤트 누락
   ```

2. **LiveSystemState 발송 누락**
   ```rust
   // broadcast_live_state 메서드는 구현되어 있지만 실제 호출되지 않음
   pub async fn broadcast_live_state(&mut self) -> anyhow::Result<()> {
       // 구현은 되어 있으나 사용되지 않음
   }
   ```

3. **이벤트 타입 불일치**
   - 기존 `subscribeToAllCrawlingEvents`와 새로운 이벤트 시스템 사이의 불일치
   - Frontend에서 올바른 이벤트 구독 메서드 사용 필요

## 🔧 개선 방안

### 1. TauriApi 서비스 개선 (완료)

**추가된 메서드:**
```typescript
// 시스템 상태 업데이트 구독
subscribeToSystemStateUpdates(callback: (state: SystemStatePayload) => void)

// 원자적 작업 이벤트 구독
subscribeToAtomicTaskUpdates(callback: (event: AtomicTaskEvent) => void)

// Live Production Line 종합 상태 구독
subscribeToLiveSystemState(callback: (state: LiveSystemState) => void)

// 통합 구독 메서드
subscribeToLiveProductionLineEvents(callbacks: {
  onSystemStateUpdate?: (state: SystemStatePayload) => void;
  onAtomicTaskUpdate?: (event: AtomicTaskEvent) => void;
  onLiveStateUpdate?: (state: LiveSystemState) => void;
})
```

### 2. CrawlingProcessDashboard 개선 (완료)

**Dynamic Graph 구현:**
```typescript
// 동적 그래프 노드 생성
const createBatchNode = (batchId: number, position: Vector3)
const createStageNode = (stageName: string, batchId: number, position: Vector3)

// 실시간 그래프 업데이트
const updateProductionLineGraph = (liveState: LiveSystemState) => {
  // 배치 노드 동적 생성
  // 스테이지 노드 동적 생성
  // 노드 간 연결 관계 형성
}
```

**개선된 이벤트 처리:**
```typescript
// 시스템 상태 → 성능 정보 업데이트
onSystemStateUpdate: (state) => {
  setPerformanceStats({
    estimatedTimeRemaining: state.session_eta_seconds * 1000,
    itemsPerMinute: state.items_per_minute,
    currentBatchSize: state.session_target_items
  });
}

// 원자적 작업 → 페이지 상태 관리
onAtomicTaskUpdate: (event) => {
  if (event.status === 'Active') {
    setRunningPages(prev => [...prev, event.task_id]);
  }
  // 성공/실패 처리
}

// Live 상태 → 동적 그래프 업데이트
onLiveStateUpdate: (liveState) => {
  updateProductionLineGraph(liveState);
}
```

### 3. Backend 개선 필요 사항

**🔴 우선 순위 높음:**

1. **AtomicTaskEvent 발송 구현**
   ```rust
   // src-tauri/src/commands/crawling_v4.rs 수정 필요
   // 각 단위 작업 시작/완료 시 이벤트 발송
   
   // 예시:
   broadcaster.emit_atomic_task_event(AtomicTaskEvent {
       task_id: page_url.clone(),
       batch_id: batch_id,
       stage_name: "ListPageCollection".to_string(),
       status: TaskStatus::Active,
       progress: 0.0,
       message: Some("Starting page collection".to_string()),
       timestamp: Utc::now(),
   })?;
   ```

2. **LiveSystemState 발송 구현**
   ```rust
   // SystemStateBroadcaster에서 live_state 브로드캐스트 활성화
   // 크롤링 진행 중에만 live_state 발송하도록 조건 추가
   
   if basic_state.is_running {
       self.broadcast_live_state().await?;
   }
   ```

3. **크롤링 엔진과 브로드캐스터 연동**
   ```rust
   // ServiceBasedBatchCrawlingEngine에서 브로드캐스터 사용
   // 각 단위 작업 완료 시 이벤트 발송
   ```

**🟡 우선 순위 중간:**

1. **성능 정보 계산 개선**
   - 예상 소요 시간 계산 로직 정확도 향상
   - 메모리 사용량 모니터링 추가

2. **배치 관리 시스템**
   - 동적 배치 생성 및 관리
   - 배치별 진행률 추적

## 🎯 Live Production Line UI 완성을 위한 실행 계획

### Phase 1: Backend 이벤트 발송 구현 (필수)
1. `crawling_v4.rs`에 `AtomicTaskEvent` 발송 로직 추가
2. `SystemStateBroadcaster`에서 `LiveSystemState` 발송 활성화
3. 크롤링 엔진과 브로드캐스터 연동

### Phase 2: 동적 그래프 완성 (핵심)
1. 실시간 배치 노드 생성/삭제
2. 스테이지 노드 동적 연결
3. 작업 완료 시 시각적 효과 추가

### Phase 3: 성능 최적화 (부가)
1. 이벤트 발송 빈도 최적화
2. 메모리 사용량 모니터링
3. 대용량 데이터 처리 최적화

## 📊 예상 결과

개선 완료 후 사용자는 다음을 확인할 수 있습니다:

1. **실시간 진척 상황**: 각 페이지 수집 작업의 시작/완료 상태
2. **동적 그래프 형성**: 배치 → 스테이지 → 작업 순서의 시각적 표현
3. **성능 정보**: 예상 소요 시간, 성공률, 처리 속도 등
4. **개발 로그**: 이벤트 수신 상태 및 디버깅 정보

이로써 사용자가 원하는 "예술적이고 동적이고 재미있는" Live Production Line UI가 완성됩니다.
