# Live Production Line UI 구현 계획

## 🎯 프로젝트 개요

`guide/interesting_visual_proposal.md`에 제시된 "Live Production Line" 컨셉을 현재 백엔드 아키텍처와 연동하여 구현하는 종합 계획입니다.

### 핵심 컨셉
- **이중 채널 이벤트 시스템**: 고주파 원자적 태스크 이벤트 + 저주파 상태 스냅샷
- **실시간 프로덕션 라인 시각화**: 태스크를 조립 라인의 부품처럼 시각화
- **반응형 애니메이션**: 성공/실패 상태에 따른 "팝" 효과 및 실시간 업데이트

---

## 🏗️ 현재 백엔드 상태 분석

### ✅ 이미 구현된 이벤트 시스템

#### 1. 기존 저주파 이벤트 (State Snapshots)
```rust
// src-tauri/src/application/events.rs
pub enum CrawlingEvent {
    ProgressUpdate(CrawlingProgress),    // crawling-progress
    TaskUpdate(CrawlingTaskStatus),      // crawling-task-update  
    StageChange { from, to, message },   // crawling-stage-change
    Error { error_id, message, stage },  // crawling-error
    DatabaseUpdate(DatabaseStats),       // database-update
    Completed(CrawlingResult),           // crawling-completed
}
```

#### 2. 고주파 원자적 이벤트 시스템 (이미 구현됨)
```rust
// src-tauri/src/application/events.rs
impl EventEmitter {
    pub async fn emit_atomic_task_event(&self, event: AtomicTaskEvent) -> EventResult
    pub async fn emit_task_started(&self, task_id: TaskId, task_type: String) -> EventResult
    pub async fn emit_task_completed(&self, task_id: TaskId, task_type: String, duration_ms: u64) -> EventResult
    pub async fn emit_task_failed(&self, task_id: TaskId, task_type: String, error_message: String, retry_count: u32) -> EventResult
}
```

#### 3. 프론트엔드 구독 시스템 (이미 구현됨)
```typescript
// src/services/tauri-api.ts
export class TauriApiService {
    async subscribeToAtomicTaskEvents(callback: (event: AtomicTaskEvent) => void): Promise<UnlistenFn>
    async subscribeToProgress(callback: (progress: CrawlingProgress) => void): Promise<UnlistenFn>
    async subscribeToStageChange(callback: (data: { from: string; to: string; message: string }) => void): Promise<UnlistenFn>
}
```

---

## 🎨 구현 Phase 별 계획

### Phase 1: 백엔드 이벤트 확장 (1주차)

#### 1.1 원자적 태스크 이벤트 확장
현재 구현된 기본 원자적 이벤트를 확장하여 더 세분화된 정보 제공

```rust
// 신규 추가: src-tauri/src/domain/atomic_events.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicTaskEvent {
    pub task_id: TaskId,
    pub task_type: TaskType,
    pub stage: TaskStage,
    pub batch_id: Option<BatchId>,
    pub page_id: Option<u32>,
    pub product_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: AtomicEventType,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    PageFetch,      // 페이지 가져오기
    HtmlParse,      // HTML 파싱
    ProductParse,   // 제품 정보 추출
    DatabaseSave,   // 데이터베이스 저장
    Validation,     // 데이터 검증
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStage {
    Queued,         // 대기 중
    Processing,     // 처리 중
    Completed,      // 완료
    Failed,         // 실패
    Retrying,       // 재시도 중
}
```

#### 1.2 배치 및 스테이지 그룹화 이벤트
```rust
// 신규 추가: src-tauri/src/domain/batch_events.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEvent {
    pub batch_id: BatchId,
    pub batch_type: BatchType,
    pub stage: BatchStage,
    pub task_count: u32,
    pub completed_tasks: u32,
    pub failed_tasks: u32,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub estimated_completion: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchType {
    PageBatch { start_page: u32, end_page: u32 },
    ProductBatch { product_count: u32 },
    DatabaseBatch { operation_type: String },
}
```

#### 1.3 성능 메트릭 이벤트
```rust
// 신규 추가: src-tauri/src/domain/performance_events.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvent {
    pub metric_type: MetricType,
    pub value: f64,
    pub unit: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    TaskThroughput,     // 초당 처리된 태스크 수
    PageLoadTime,       // 페이지 로딩 시간
    ParseTime,          // 파싱 시간
    DatabaseWriteTime,  // DB 쓰기 시간
    MemoryUsage,        // 메모리 사용량
    ErrorRate,          // 오류율
}
```

### Phase 2: SolidJS 스토어 아키텍처 구현 (2주차)

#### 2.1 Live Production Line 스토어 구조
```typescript
// 신규 생성: src/stores/liveProductionStore.ts
interface LiveProductionState {
    missions: Mission[];
    batches: Map<BatchId, Batch>;
    taskItems: Map<TaskId, TaskItem>;
    stages: Map<StageId, Stage>;
    conveyorBelt: ConveyorBeltState;
    performanceMetrics: PerformanceMetrics;
    uiSettings: UISettings;
}

interface Mission {
    id: string;
    name: string;
    description: string;
    totalPages: number;
    estimatedDuration: number;
    status: MissionStatus;
    startTime: Date;
    progress: number;
}

interface Batch {
    id: BatchId;
    missionId: string;
    batchType: BatchType;
    taskItems: TaskId[];
    currentStage: StageId;
    status: BatchStatus;
    startTime: Date;
    estimatedCompletion: Date | null;
    completionRate: number;
}

interface TaskItem {
    id: TaskId;
    batchId: BatchId;
    taskType: TaskType;
    currentStage: TaskStage;
    status: TaskStatus;
    startTime: Date;
    completionTime: Date | null;
    duration: number | null;
    retryCount: number;
    errorMessage: string | null;
    metadata: Record<string, any>;
}

interface Stage {
    id: StageId;
    name: string;
    description: string;
    position: { x: number; y: number };
    capacity: number;
    currentLoad: number;
    averageProcessingTime: number;
    successRate: number;
}
```

#### 2.2 이벤트 처리 시스템
```typescript
// 신규 생성: src/services/liveProductionEventHandler.ts
export class LiveProductionEventHandler {
    private store: LiveProductionStore;
    private eventBuffer: Map<string, any[]> = new Map();
    private batchProcessor: BatchEventProcessor;

    constructor(store: LiveProductionStore) {
        this.store = store;
        this.batchProcessor = new BatchEventProcessor(this.processBatchedEvents.bind(this));
    }

    // 고주파 원자적 이벤트 처리
    handleAtomicTaskEvent(event: AtomicTaskEvent): void {
        // 이벤트 버퍼링으로 UI 성능 최적화
        this.batchProcessor.addEvent('atomic-task', event);
        
        // 즉시 처리가 필요한 경우 (예: 오류 이벤트)
        if (event.event_type === 'Failed') {
            this.store.updateTaskItemStatus(event.task_id, {
                status: 'failed',
                errorMessage: event.metadata.error_message,
                retryCount: event.metadata.retry_count || 0
            });
        }
    }

    // 저주파 상태 스냅샷 이벤트 처리
    handleSystemStateUpdate(event: CrawlingProgress): void {
        this.store.updateMissionProgress({
            currentStage: event.current_stage,
            progress: event.percentage,
            status: event.status,
            newItems: event.new_items,
            updatedItems: event.updated_items,
            errors: event.errors
        });
    }

    private processBatchedEvents(eventType: string, events: any[]): void {
        switch (eventType) {
            case 'atomic-task':
                this.processBatchedAtomicEvents(events as AtomicTaskEvent[]);
                break;
            default:
                console.warn(`Unknown batched event type: ${eventType}`);
        }
    }

    private processBatchedAtomicEvents(events: AtomicTaskEvent[]): void {
        // 배치 단위로 UI 업데이트 최적화
        const updates = events.reduce((acc, event) => {
            acc[event.task_id] = {
                status: event.event_type,
                timestamp: event.timestamp,
                duration: event.metadata.duration_ms
            };
            return acc;
        }, {} as Record<TaskId, any>);

        this.store.batchUpdateTaskItems(updates);
    }
}
```

### Phase 3: 컴포넌트 아키텍처 구현 (3주차)

#### 3.1 메인 대시보드 컴포넌트
```tsx
// 신규 생성: src/components/LiveProductionDashboard.tsx
export function LiveProductionDashboard() {
    const productionState = useLiveProductionStore();
    
    return (
        <div class="live-production-dashboard">
            <MissionBriefingPanel />
            <ConveyorBeltVisualization />
            <BatchAnchorsPanel />
            <PerformanceMetricsPanel />
            <StageStationsGrid />
        </div>
    );
}
```

#### 3.2 컨베이어 벨트 시각화
```tsx
// 신규 생성: src/components/ConveyorBeltVisualization.tsx
export function ConveyorBeltVisualization() {
    const [taskItems, setTaskItems] = createSignal<TaskItem[]>([]);
    const [animationSpeed, setAnimationSpeed] = createSignal(1.0);

    // 태스크 아이템의 실시간 이동 애니메이션
    createEffect(() => {
        const items = liveProductionStore.getActiveTaskItems();
        setTaskItems(items);
        
        // 성공/실패에 따른 "팝" 효과 트리거
        items.forEach(item => {
            if (item.status === 'completed') {
                triggerSuccessAnimation(item.id);
            } else if (item.status === 'failed') {
                triggerFailureAnimation(item.id);
            }
        });
    });

    return (
        <div class="conveyor-belt">
            <div class="belt-track">
                <For each={taskItems()}>
                    {(item) => (
                        <TaskItemVisual 
                            item={item}
                            animationSpeed={animationSpeed()}
                            onStatusChange={(newStatus) => 
                                liveProductionStore.updateTaskItemStatus(item.id, newStatus)
                            }
                        />
                    )}
                </For>
            </div>
            <StageStations />
        </div>
    );
}
```

#### 3.3 태스크 아이템 시각화
```tsx
// 신규 생성: src/components/TaskItemVisual.tsx
export function TaskItemVisual(props: { 
    item: TaskItem; 
    animationSpeed: number;
    onStatusChange: (status: TaskStatus) => void;
}) {
    const [position, setPosition] = createSignal({ x: 0, y: 0 });
    const [isAnimating, setIsAnimating] = createSignal(false);

    // 태스크 상태에 따른 시각적 스타일
    const getItemStyle = () => ({
        background: getStatusColor(props.item.status),
        transform: `translate(${position().x}px, ${position().y}px) ${
            isAnimating() ? 'scale(1.2)' : 'scale(1.0)'
        }`,
        transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
        border: props.item.retryCount > 0 ? '2px dashed #f59e0b' : 'none'
    });

    // 성공 시 "팝" 애니메이션
    const triggerSuccessAnimation = () => {
        setIsAnimating(true);
        setTimeout(() => setIsAnimating(false), 300);
    };

    return (
        <div 
            class="task-item-visual"
            style={getItemStyle()}
            onAnimationEnd={() => setIsAnimating(false)}
        >
            <div class="task-id">{props.item.id}</div>
            <div class="task-type">{props.item.taskType}</div>
            <div class="progress-indicator">
                <div class="progress-bar" style={{ width: `${getProgress(props.item)}%` }} />
            </div>
            {props.item.errorMessage && (
                <div class="error-indicator" title={props.item.errorMessage}>
                    ⚠️
                </div>
            )}
        </div>
    );
}
```

#### 3.4 스테이지 스테이션 컴포넌트
```tsx
// 신규 생성: src/components/StageStations.tsx
export function StageStations() {
    const stages = useLiveProductionStore(state => state.stages);

    return (
        <div class="stage-stations">
            <For each={Array.from(stages.values())}>
                {(stage) => (
                    <StageStation 
                        stage={stage}
                        taskItems={liveProductionStore.getTaskItemsAtStage(stage.id)}
                    />
                )}
            </For>
        </div>
    );
}

export function StageStation(props: { stage: Stage; taskItems: TaskItem[] }) {
    return (
        <div class="stage-station" data-stage={props.stage.id}>
            <div class="station-header">
                <h3>{props.stage.name}</h3>
                <div class="capacity-indicator">
                    {props.stage.currentLoad} / {props.stage.capacity}
                </div>
            </div>
            <div class="station-queue">
                <For each={props.taskItems}>
                    {(item) => (
                        <div class="queued-task" data-task-id={item.id}>
                            {item.taskType}
                        </div>
                    )}
                </For>
            </div>
            <div class="station-metrics">
                <div class="processing-time">
                    평균: {props.stage.averageProcessingTime}ms
                </div>
                <div class="success-rate">
                    성공률: {Math.round(props.stage.successRate * 100)}%
                </div>
            </div>
        </div>
    );
}
```

### Phase 4: 고급 기능 및 최적화 (4주차)

#### 4.1 실시간 성능 최적화
```typescript
// 신규 생성: src/utils/performanceOptimizer.ts
export class PerformanceOptimizer {
    private eventThrottler: Map<string, number> = new Map();
    private animationFrameQueue: (() => void)[] = [];
    private isProcessingAnimations = false;

    // 이벤트 스로틀링으로 과도한 업데이트 방지
    throttleEvent(eventType: string, callback: () => void, delay: number = 16): void {
        const now = Date.now();
        const lastRun = this.eventThrottler.get(eventType) || 0;
        
        if (now - lastRun >= delay) {
            callback();
            this.eventThrottler.set(eventType, now);
        }
    }

    // 애니메이션 큐로 부드러운 렌더링 보장
    queueAnimation(callback: () => void): void {
        this.animationFrameQueue.push(callback);
        
        if (!this.isProcessingAnimations) {
            this.processAnimationQueue();
        }
    }

    private processAnimationQueue(): void {
        this.isProcessingAnimations = true;
        
        const processNext = () => {
            if (this.animationFrameQueue.length > 0) {
                const callback = this.animationFrameQueue.shift()!;
                callback();
                requestAnimationFrame(processNext);
            } else {
                this.isProcessingAnimations = false;
            }
        };
        
        requestAnimationFrame(processNext);
    }
}
```

#### 4.2 CSS 애니메이션 시스템
```css
/* 신규 생성: src/styles/liveProduction.css */
.task-item-visual {
    width: 60px;
    height: 40px;
    border-radius: 8px;
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: bold;
    color: white;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    cursor: pointer;
}

/* 성공 시 "팝" 애니메이션 */
@keyframes success-pop {
    0% { transform: scale(1.0); }
    50% { transform: scale(1.3); box-shadow: 0 0 20px #10b981; }
    100% { transform: scale(1.1); }
}

/* 실패 시 "흔들림" 애니메이션 */
@keyframes failure-shake {
    0%, 100% { transform: translateX(0); }
    25% { transform: translateX(-5px); }
    75% { transform: translateX(5px); }
}

/* 재시도 시 "펄스" 애니메이션 */
@keyframes retry-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
}

.task-item-visual.success {
    animation: success-pop 0.6s cubic-bezier(0.4, 0, 0.2, 1);
}

.task-item-visual.failure {
    animation: failure-shake 0.5s ease-in-out;
}

.task-item-visual.retrying {
    animation: retry-pulse 1s ease-in-out infinite;
}

/* 컨베이어 벨트 이동 애니메이션 */
.belt-track {
    position: relative;
    height: 100px;
    background: linear-gradient(
        90deg,
        #374151 0%,
        #4b5563 25%,
        #374151 50%,
        #4b5563 75%,
        #374151 100%
    );
    background-size: 40px 100%;
    animation: belt-movement 2s linear infinite;
}

@keyframes belt-movement {
    0% { background-position-x: 0; }
    100% { background-position-x: 40px; }
}

/* 스테이지 스테이션 스타일 */
.stage-station {
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    padding: 16px;
    margin: 8px;
    background: white;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
}

.stage-station:hover {
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
    transform: translateY(-2px);
}

.capacity-indicator {
    background: #f3f4f6;
    border-radius: 20px;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: bold;
    color: #374151;
}

.capacity-indicator.high-load {
    background: #fef3c7;
    color: #92400e;
}

.capacity-indicator.overload {
    background: #fecaca;
    color: #991b1b;
}
```

#### 4.3 고급 이벤트 패턴 처리
```typescript
// 신규 생성: src/services/advancedEventPatterns.ts
export class AdvancedEventPatterns {
    private patternBuffer: EventPattern[] = [];
    private anomalyDetector: AnomalyDetector;

    constructor(private store: LiveProductionStore) {
        this.anomalyDetector = new AnomalyDetector();
    }

    // 패턴 기반 예측 분석
    detectPatterns(events: AtomicTaskEvent[]): EventPattern[] {
        const patterns: EventPattern[] = [];
        
        // 연속 실패 패턴 감지
        const failureStreaks = this.detectFailureStreaks(events);
        if (failureStreaks.length > 0) {
            patterns.push({
                type: 'failure-streak',
                confidence: 0.9,
                prediction: 'system-bottleneck',
                affectedStages: failureStreaks.map(f => f.stage)
            });
        }

        // 처리 속도 저하 패턴 감지
        const slowdowns = this.detectPerformanceDegradation(events);
        if (slowdowns.length > 0) {
            patterns.push({
                type: 'performance-degradation',
                confidence: 0.8,
                prediction: 'resource-exhaustion',
                affectedStages: slowdowns.map(s => s.stage)
            });
        }

        return patterns;
    }

    // 자동 복구 제안
    suggestRecoveryActions(patterns: EventPattern[]): RecoveryAction[] {
        const actions: RecoveryAction[] = [];

        patterns.forEach(pattern => {
            switch (pattern.type) {
                case 'failure-streak':
                    actions.push({
                        type: 'reduce-concurrency',
                        priority: 'high',
                        description: '동시 처리 수를 줄여 안정성 확보',
                        estimatedImpact: 'positive'
                    });
                    break;
                case 'performance-degradation':
                    actions.push({
                        type: 'increase-timeout',
                        priority: 'medium',
                        description: '타임아웃 값을 증가시켜 성공률 향상',
                        estimatedImpact: 'neutral'
                    });
                    break;
            }
        });

        return actions;
    }
}
```

---

## 🚀 구현 우선순위 및 일정

### Week 1: 백엔드 이벤트 확장
- [ ] `AtomicTaskEvent` 구조체 확장 및 메타데이터 추가
- [ ] `BatchEvent` 및 `PerformanceEvent` 새로 구현
- [ ] 기존 크롤링 엔진에 새로운 이벤트 발생 지점 추가
- [ ] 이벤트 발생 테스트 및 디버깅

### Week 2: SolidJS 스토어 아키텍처
- [ ] `LiveProductionStore` 구현
- [ ] 이벤트 핸들러 시스템 구현
- [ ] 기존 `TauriApiService`와의 연동
- [ ] 상태 관리 로직 테스트

### Week 3: 컴포넌트 시각화
- [ ] `LiveProductionDashboard` 메인 컴포넌트
- [ ] `ConveyorBeltVisualization` 구현
- [ ] `TaskItemVisual` 및 애니메이션 시스템
- [ ] `StageStations` 및 관련 UI 컴포넌트

### Week 4: 최적화 및 고급 기능
- [ ] 성능 최적화 (이벤트 스로틀링, 애니메이션 큐)
- [ ] CSS 애니메이션 시스템 완성
- [ ] 패턴 감지 및 예측 기능
- [ ] 종합 테스트 및 문서화

---

## 🔧 기술적 고려사항

### 성능 최적화
1. **이벤트 배치 처리**: 고주파 이벤트를 배치로 처리하여 UI 블로킹 방지
2. **가상화**: 대량의 태스크 아이템 처리 시 가상 스크롤링 적용
3. **메모화**: 불필요한 리렌더링 방지를 위한 적극적인 메모화
4. **웹 워커**: 복잡한 계산은 웹 워커로 분리

### 접근성 및 사용성
1. **키보드 네비게이션**: 모든 인터랙티브 요소에 키보드 접근성 보장
2. **스크린 리더 지원**: ARIA 라벨 및 Live Region 활용
3. **색상 대비**: WCAG 2.1 AA 기준 준수
4. **반응형 디자인**: 다양한 화면 크기 지원

### 확장성
1. **플러그인 아키텍처**: 새로운 시각화 모드 쉽게 추가 가능
2. **테마 시스템**: 다크/라이트 모드 및 사용자 정의 색상 테마
3. **설정 관리**: 사용자별 대시보드 레이아웃 저장
4. **내보내기**: 시각화 상태를 이미지/동영상으로 내보내기

---

## 📊 예상 성과

### 개발자 경험 향상
- 크롤링 프로세스의 실시간 모니터링으로 디버깅 시간 단축
- 시각적 피드백을 통한 성능 병목 지점 즉시 식별
- 오류 패턴 분석을 통한 예방적 시스템 개선

### 시스템 신뢰성 향상
- 실시간 상태 모니터링으로 문제 조기 감지
- 자동 복구 제안으로 시스템 안정성 향상
- 성능 메트릭을 통한 최적화 지점 식별

### 사용자 경험 개선
- 직관적이고 매력적인 UI로 시스템 상태 파악 용이
- 실시간 피드백으로 작업 진행 상황 명확히 파악
- 오류 발생 시 즉시 시각적 알림으로 신속한 대응 가능

---

## 🎯 최종 목표

**Live Production Line UI**를 통해 크롤링 프로세스를 단순한 로그 기반 모니터링에서 **실시간 시각적 제어 시스템**으로 진화시키는 것이 최종 목표입니다. 이를 통해 개발자는 시스템 상태를 직관적으로 파악하고, 문제를 신속하게 해결하며, 전체적인 개발 생산성을 크게 향상시킬 수 있습니다.
