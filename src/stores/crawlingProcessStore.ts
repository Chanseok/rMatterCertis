import { createStore } from "solid-js/store";
import { SystemStatePayload, AtomicTaskEvent, BatchInfo, StageInfo, TaskStatus } from "../types/events";

// 개별 작업 아이템 (페이지, 제품)
export interface TaskItem {
  id: string; // 페이지 번호 또는 제품 URL
  status: TaskStatus;
  retryCount: number;
  completedAt?: string;
  message?: string;
}

// 스테이지 (공정 라인의 한 레인)
export interface Stage {
  name: 'ListPageCollection' | 'DetailPageCollection' | 'DatabaseSave';
  status: 'pending' | 'active' | 'completed';
  items: TaskItem[];
  progress: number; // 0-1
  total_items: number;
  completed_items: number;
  active_items: number;
  failed_items: number;
}

// 배치 (하나의 큰 작업 단위)
export interface Batch {
  id: number;
  status: 'pending' | 'active' | 'completed' | 'error';
  progress: number; // 0-1
  stages: {
    listPage: Stage;
    detailPage: Stage;
    dbSave: Stage;
  };
  pages_range: [number, number]; // [start, end]
  current_page: number;
  items_total: number;
  items_completed: number;
}

// 거시적 정보
export interface MacroState {
  totalKnownItems: number;
  itemsCollectedTotal: number;
  sessionTargetItems: number;
  sessionCollectedItems: number;
  sessionETASeconds: number;
  itemsPerMinute: number;
  currentStage: string;
  totalPages: number;
  lastDbCursor: { page: number; index: number } | null;
  analyzedAt: string | null;
}

// 전체 세션 상태
export interface CrawlingSessionState {
  sessionId: string | null;
  isRunning: boolean;
  macroState: MacroState;
  batches: Batch[];
  activeBatchId: number | null;
  recentCompletions: AtomicTaskEvent[];
  lastUpdated: string;
}

// 초기 상태 생성
const createInitialState = (): CrawlingSessionState => ({
  sessionId: null,
  isRunning: false,
  macroState: {
    totalKnownItems: 0,
    itemsCollectedTotal: 0,
    sessionTargetItems: 0,
    sessionCollectedItems: 0,
    sessionETASeconds: 0,
    itemsPerMinute: 0,
    currentStage: '',
    totalPages: 0,
    lastDbCursor: null,
    analyzedAt: null,
  },
  batches: [],
  activeBatchId: null,
  recentCompletions: [],
  lastUpdated: new Date().toISOString(),
});

// 스토어 생성
const [sessionStore, setSessionStore] = createStore<CrawlingSessionState>(createInitialState());

// 시스템 상태 업데이트 함수
export const updateSystemState = (payload: SystemStatePayload) => {
  setSessionStore({
    isRunning: payload.is_running,
    macroState: {
      totalKnownItems: payload.db_total_products,
      itemsCollectedTotal: payload.db_total_products,
      sessionTargetItems: payload.session_target_items,
      sessionCollectedItems: payload.session_collected_items,
      sessionETASeconds: payload.session_eta_seconds,
      itemsPerMinute: payload.items_per_minute,
      currentStage: payload.current_stage,
      totalPages: payload.total_pages,
      lastDbCursor: payload.last_db_cursor,
      analyzedAt: payload.analyzed_at,
    },
    lastUpdated: new Date().toISOString(),
  });
};

// 원자적 작업 이벤트 처리 함수
export const handleAtomicTaskEvent = (event: AtomicTaskEvent) => {
  // 최근 완료 목록에 추가 (최대 10개 유지)
  setSessionStore('recentCompletions', (prev) => {
    const updated = [event, ...prev];
    return updated.slice(0, 10);
  });

  // 활성 배치 찾기
  const activeBatch = sessionStore.batches.find(batch => batch.id === event.batch_id);
  if (!activeBatch) return;

  // 해당 스테이지 업데이트
  const stageName = event.stage_name as 'ListPageCollection' | 'DetailPageCollection' | 'DatabaseSave';
  const stageKey = stageName === 'ListPageCollection' ? 'listPage' : 
                   stageName === 'DetailPageCollection' ? 'detailPage' : 'dbSave';

  // 배치 인덱스 찾기
  const batchIndex = sessionStore.batches.findIndex(batch => batch.id === event.batch_id);
  if (batchIndex === -1) return;

  // 작업 아이템 업데이트
  const itemIndex = sessionStore.batches[batchIndex].stages[stageKey].items.findIndex(item => item.id === event.task_id);
  
  if (itemIndex !== -1) {
    setSessionStore('batches', batchIndex, 'stages', stageKey, 'items', itemIndex, {
      status: event.status,
      completedAt: event.status === 'Success' ? event.timestamp : undefined,
      message: event.message || undefined,
    });
  } else {
    // 새로운 작업 아이템 추가
    setSessionStore('batches', batchIndex, 'stages', stageKey, 'items', (prev) => [
      ...prev,
      {
        id: event.task_id,
        status: event.status,
        retryCount: 0,
        completedAt: event.status === 'Success' ? event.timestamp : undefined,
        message: event.message || undefined,
      }
    ]);
  }

  // 스테이지 통계 업데이트
  const stage = sessionStore.batches[batchIndex].stages[stageKey];
  const completedItems = stage.items.filter(item => item.status === 'Success').length;
  const activeItems = stage.items.filter(item => item.status === 'Active').length;
  const failedItems = stage.items.filter(item => item.status === 'Error').length;
  
  setSessionStore('batches', batchIndex, 'stages', stageKey, {
    completed_items: completedItems,
    active_items: activeItems,
    failed_items: failedItems,
    progress: stage.total_items > 0 ? completedItems / stage.total_items : 0,
    status: completedItems === stage.total_items ? 'completed' : 
            activeItems > 0 ? 'active' : 'pending',
  });

  // 배치 전체 진행률 업데이트
  const batch = sessionStore.batches[batchIndex];
  const totalCompleted = batch.stages.listPage.completed_items + 
                         batch.stages.detailPage.completed_items + 
                         batch.stages.dbSave.completed_items;
  const totalItems = batch.stages.listPage.total_items + 
                     batch.stages.detailPage.total_items + 
                     batch.stages.dbSave.total_items;
  
  setSessionStore('batches', batchIndex, {
    progress: totalItems > 0 ? totalCompleted / totalItems : 0,
    items_completed: totalCompleted,
    items_total: totalItems,
  });
};

// 새로운 배치 생성 함수
export const createNewBatch = (batchInfo: BatchInfo) => {
  const newBatch: Batch = {
    id: batchInfo.id,
    status: batchInfo.status as 'pending' | 'active' | 'completed' | 'error',
    progress: batchInfo.progress,
    pages_range: batchInfo.pages_range,
    current_page: batchInfo.current_page,
    items_total: batchInfo.items_total,
    items_completed: batchInfo.items_completed,
    stages: {
      listPage: {
        name: 'ListPageCollection',
        status: 'pending',
        items: [],
        progress: 0,
        total_items: 0,
        completed_items: 0,
        active_items: 0,
        failed_items: 0,
      },
      detailPage: {
        name: 'DetailPageCollection',
        status: 'pending',
        items: [],
        progress: 0,
        total_items: 0,
        completed_items: 0,
        active_items: 0,
        failed_items: 0,
      },
      dbSave: {
        name: 'DatabaseSave',
        status: 'pending',
        items: [],
        progress: 0,
        total_items: 0,
        completed_items: 0,
        active_items: 0,
        failed_items: 0,
      },
    },
  };

  setSessionStore('batches', (prev) => [...prev, newBatch]);
  setSessionStore('activeBatchId', newBatch.id);
};

// 스토어 초기화 함수
export const resetCrawlingSession = () => {
  setSessionStore(createInitialState());
};

export { sessionStore, setSessionStore };
