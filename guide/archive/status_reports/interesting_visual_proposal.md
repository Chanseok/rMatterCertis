# 가이드: 실시간 공정 대시보드 UI 구현 방안

**문서 목적:** `.local/prompts6`에서 논의된 "실시간 공정 대시보드" 컨셉을 실제 구현으로 연결하기 위한 구체적인 프론트엔드/백엔드 연동 가이드를 제공합니다. 이 문서는 현재 백엔드 구현을 기반으로, 최종 목표 달성을 위한 명확한 로드맵과 기술 명세를 포함합니다.

--- 

## 1. 최종 목표: "The Live Production Line" UI

우리가 구현할 UI는 단순한 대시보드를 넘어, 크롤링의 전 과정을 하나의 **살아있는 생산 라인**처럼 보여주는 동적인 경험을 제공해야 합니다.

-   **거시적 관점 (미션 브리핑):** 사용자는 전체 데이터 수집 현황과 현재 진행 중인 "배치 작업"의 목표, 예상 완료 시간을 한눈에 파악합니다.
-   **미시적 관점 (공정 라인):** 현재 실행 중인 배치의 각 단계(Stage)가 별도의 레인(Lane)에 표시되고, 그 안에서 개별 작업(페이지, 제품) 아이템들이 실시간으로 상태(대기, 처리 중, 재시도, 완료)를 바꾸며 움직이는 모습을 보여줍니다.
-   **재미 요소:** 완료된 아이템은 "푱!" 하는 시각적 효과와 함께 사라지며, 전체 시스템은 사용자의 제어(운영 프로파일 변경)에 따라 실시간으로 처리 속도와 리소스 사용량을 조절하는 모습을 보여줍니다.

## 2. 백엔드 연동 계획: "듀얼 채널" 이벤트 시스템의 현실화

이 목표를 달성하려면, 백엔드는 두 종류의 정보를 UI에 제공해야 합니다. (`proposal5.md`의 개념을 구체화)

### 2.1. 채널 1: 상태 스냅샷 (거시 정보)

-   **이벤트:** `event://system-state-update`
-   **주기:** 1~2초
-   **역할:** 전체 진행률, ETA, 리소스 사용량 등 무겁고 종합적인 정보를 담은 `SystemStatePayload`를 전송합니다. **현재 백엔드 구현은 이 방식에 가깝지만, Payload 내용을 확장해야 합니다.**

### 2.2. 채널 2: 원자적 이벤트 (미시 정보)

-   **이벤트:** `event://atomic-task-update`
-   **주기:** 이벤트 발생 즉시
-   **역할:** 개별 작업 아이템의 상태 변화(`TaskStarted`, `TaskCompleted` 등)를 실시간으로 전송하여, UI가 즉각적인 애니메이션을 표현할 수 있게 합니다. **이 부분은 신규 구현이 필요합니다.**

### 2.3. 백엔드 구현 가이드 (요약)

-   **`Orchestrator` 수정:** `process_single_task_static` 함수가 끝나는 지점에서, `TaskResult`를 `AtomicTaskEvent`로 변환하여 `emit`하는 로직을 추가해야 합니다.
-   **`SystemStateBroadcaster` 구현:** `Orchestrator` 내에 주기적으로 `SystemStatePayload`를 `emit`하는 별도의 비동기 태스크를 구현해야 합니다.
-   **Payload 확장:** `SystemStatePayload`에 `macroProgress`, `prediction`, `liveMetrics` 등 거시적 정보를 포함하도록 구조체를 확장해야 합니다.

--- 

## 3. 프론트엔드 구현 가이드 (SolidJS)

### 3.1. 상태 관리 설계 (`createStore`)

가장 먼저, 이 복잡하고 중첩된 UI 상태를 관리할 중앙 스토어를 설계합니다.

-   **위치:** `@src/stores/crawlingProcessStore.ts`
-   **코드:**
    ```typescript
    import { createStore } from "solid-js/store";

    // 개별 작업 아이템 (페이지, 제품)
    export interface TaskItem {
      id: string; // 페이지 번호 또는 제품 URL
      status: 'pending' | 'active' | 'retrying' | 'success' | 'error';
      retryCount: number;
    }

    // 스테이지 (공정 라인의 한 레인)
    export interface Stage {
      name: 'ListPageCollection' | 'DetailPageCollection' | 'DatabaseSave';
      status: 'pending' | 'active' | 'completed';
      items: TaskItem[];
    }

    // 배치 (하나의 큰 작업 단위)
    export interface Batch {
      id: number;
      status: 'pending' | 'active' | 'completed' | 'error';
      progress: number; // 0-100
      stages: {
        listPage: Stage;
        detailPage: Stage;
        dbSave: Stage;
      };
    }

    // 거시적 정보
    export interface MacroState {
        totalKnownItems: number;
        itemsCollectedTotal: number;
        sessionTargetItems: number;
        sessionCollectedItems: number;
        sessionETASeconds: number;
        itemsPerMinute: number;
    }

    // 전체 세션 상태
    export interface CrawlingSessionState {
      sessionId: string | null;
      isRunning: boolean;
      macroState: MacroState;
      batches: Batch[];
      activeBatchId: number | null;
    }

    // 스토어 생성
    const [sessionStore, setSessionStore] = createStore<CrawlingSessionState>(/* 초기 상태 */);

    export { sessionStore, setSessionStore };
    ```

### 3.2. 컴포넌트 구조

-   **`<CrawlingProcessDashboard>`:** 최상위 컴포넌트. `sessionStore`를 관리하고, `onMount`에서 백엔드 이벤트 리스너를 등록합니다.
-   **`<MissionBriefingPanel>`:** `sessionStore.macroState`를 props로 받아 거시적 정보를 표시합니다.
-   **`<BatchAnchors>`:** `sessionStore.batches`를 `<For>`로 순회하며 앵커 UI를 렌더링합니다.
-   **`<ActiveBatchView>`:** `sessionStore.activeBatchId`에 해당하는 배치의 상세 정보를 표시합니다.
-   **`<StageLane>`:** `batch.stages`의 각 스테이지를 받아, 그 안의 `items`를 `<For>`로 순회하며 `<TaskItemCard>`를 렌더링합니다.
-   **`<TaskItemCard>`:** `TaskItem` 객체를 props로 받아, `item.status`에 따라 다른 스타일과 애니메이션을 적용합니다.

### 3.3. 이벤트 핸들링 로직

`<CrawlingProcessDashboard>` 컴포넌트 내에서 두 종류의 이벤트를 처리합니다.

```typescript
import { listen } from '@tauri-apps/api/event';
import { setSessionStore } from './crawlingProcessStore';

onMount(async () => {
  // 1. 거시적 상태 업데이트 리스너
  const unlistenSnapshot = await listen('system-state-update', (event) => {
    const payload = event.payload as SystemStatePayload; // 백엔드와 타입 일치 필요
    // setSessionStore를 이용해 macroState와 전체적인 상태 업데이트
    setSessionStore('macroState', payload.macroState);
    setSessionStore('isRunning', payload.isRunning);
  });

  // 2. 미시적 상태 업데이트 리스너
  const unlistenAtomic = await listen('atomic-task-update', (event) => {
    const payload = event.payload as AtomicTaskEvent;
    // setSessionStore의 중첩된 경로 업데이트 기능을 사용하여
    // 정확히 해당 아이템의 상태만 변경
    // 예: setSessionStore('batches', b => b.id === payload.batchId, 'stages', s => s.name === payload.stageName, 'items', i => i.id === payload.itemId, 'status', payload.newStatus);
  });

  onCleanup(() => {
    unlistenSnapshot();
    unlistenAtomic();
  });
});
```

### 3.4. 애니메이션 구현 ("푱!")

-   **위치:** `<TaskItemCard>` 컴포넌트
-   **로직:** `item.status`가 `success`로 변경되는 것을 감지하고, 짧은 시간 동안 CSS 클래스를 추가했다가 제거합니다.
-   **CSS:**
    ```css
    @keyframes pop-out {
      0% { transform: scale(1); opacity: 1; background-color: #22c55e; }
      50% { transform: scale(1.1); opacity: 0.8; }
      100% { transform: scale(0.5); opacity: 0; }
    }

    .item-success-animation {
      animation: pop-out 0.5s ease-out forwards;
    }
    ```
-   **SolidJS 코드:**
    ```typescript
    const [isAnimating, setIsAnimating] = createSignal(false);

    createEffect(() => {
      if (props.item.status === 'success') {
        setIsAnimating(true);
        // 애니메이션이 끝난 후 실제 DOM에서 아이템을 제거하려면
        // 상위 컴포넌트의 상태를 변경해야 함
        setTimeout(() => setIsAnimating(false), 500);
      }
    });

    return <div classList={{ 'item-success-animation': isAnimating() }}>...</div>
    ```

## 4. 단계별 구현 로드맵

1.  **Phase 1: Backend - 이벤트 시스템 구축**
    -   [ ] 백엔드에 `AtomicTaskEvent` 구조체 및 이벤트 채널을 추가합니다.
    -   [ ] `Orchestrator`가 작업 완료 시 `atomic-task-update` 이벤트를 `emit`하도록 수정합니다.
    -   [ ] `SystemStatePayload`를 확장하고, 주기적으로 `system-state-update`를 `emit`하는 로직을 구현합니다.

2.  **Phase 2: Frontend - 구조 및 정적 UI 구현**
    -   [ ] `crawlingProcessStore.ts`에 제안된 스토어 구조를 작성합니다.
    -   [ ] 목업(mock) 데이터를 사용하여 `<CrawlingProcessDashboard>`와 하위 컴포넌트들의 레이아웃을 구현합니다.

3.  **Phase 3: Frontend - 동적 연동**
    -   [ ] 백엔드 이벤트 리스너를 추가하고, 수신된 이벤트에 따라 `setSessionStore`를 호출하여 UI가 실시간으로 변경되도록 구현합니다.
    -   [ ] `TaskItemCard`의 상태 변화 및 애니메이션 로직을 구현합니다.

4.  **Phase 4: 최종 폴리싱**
    -   [ ] FUI 스타일의 CSS를 적용하여 전체적인 디자인 완성도를 높입니다.
    -   [ ] 부드러운 전환 효과와 추가적인 사운드 이펙트 등을 통해 사용자 경험을 극대화합니다.
