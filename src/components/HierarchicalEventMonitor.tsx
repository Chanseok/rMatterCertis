/**
 * HierarchicalEventMonitor.tsx
 * @description 이벤트를 계층구조로 표시하는 개선된 이벤트 모니터
 */
import { Component, createSignal, onMount, onCleanup, For, Show, createMemo } from 'solid-js';
import { tauriApi } from '../services/tauri-api';

// 이벤트 레벨 정의
type EventLevel = 'session' | 'batch' | 'stage' | 'page' | 'product' | 'detail';

// 계층적 이벤트 구조
interface HierarchicalEvent {
  id: string;
  level: EventLevel;
  parentId?: string;
  timestamp: string;
  type: 'start' | 'progress' | 'success' | 'retry' | 'error' | 'complete';
  title: string;
  message: string;
  status: 'running' | 'success' | 'warning' | 'error' | 'pending';
  metadata: {
    current?: number;
    total?: number;
    percentage?: number;
    attempt?: number;
    maxAttempts?: number;
    batchId?: number;
    pageNumber?: number;
    productIndex?: number;
    errorReason?: string;
    [key: string]: any;
  };
  children: HierarchicalEvent[];
  isExpanded: boolean;
}

// 이벤트 통계
interface EventStatistics {
  totalEvents: number;
  eventsByLevel: Record<EventLevel, number>;
  eventsByType: Record<string, number>;
  successRate: number;
  errorCount: number;
}

const HierarchicalEventMonitor: Component = () => {
  // State
  const [events, setEvents] = createSignal<HierarchicalEvent[]>([]);
  const [statistics, setStatistics] = createSignal<EventStatistics>({
    totalEvents: 0,
    eventsByLevel: {
      session: 0,
      batch: 0,
      stage: 0,
      page: 0,
      product: 0,
      detail: 0
    },
    eventsByType: {},
    successRate: 0,
    errorCount: 0
  });

  const [filter, setFilter] = createSignal<{
    level?: EventLevel;
    type?: string;
    status?: string;
  }>({});

  const [autoScroll, setAutoScroll] = createSignal(true);

  // Cleanup functions
  let cleanupFunctions: (() => void)[] = [];

  // 레벨별 설정
  const getLevelConfig = (level: EventLevel) => {
    const configs = {
      session: { 
        color: 'bg-blue-50 border-l-blue-500 text-blue-800',
        indent: 0,
        icon: '🎯',
        name: 'Session'
      },
      batch: { 
        color: 'bg-green-50 border-l-green-500 text-green-800',
        indent: 1,
        icon: '📦',
        name: 'Batch'
      },
      stage: { 
        color: 'bg-purple-50 border-l-purple-500 text-purple-800',
        indent: 2,
        icon: '⚡',
        name: 'Stage'
      },
      page: { 
        color: 'bg-yellow-50 border-l-yellow-500 text-yellow-800',
        indent: 3,
        icon: '📄',
        name: 'Page'
      },
      product: { 
        color: 'bg-orange-50 border-l-orange-500 text-orange-800',
        indent: 4,
        icon: '🔗',
        name: 'Product'
      },
      detail: { 
        color: 'bg-gray-50 border-l-gray-500 text-gray-800',
        indent: 5,
        icon: '🔍',
        name: 'Detail'
      }
    };
    return configs[level];
  };

  // 상태별 아이콘
  const getStatusIcon = (status: string, type: string) => {
    if (type === 'start') return '▶️';
    if (type === 'retry') return '🔄';
    if (status === 'running') return '⏳';
    if (status === 'success') return '✅';
    if (status === 'warning') return '⚠️';
    if (status === 'error') return '❌';
    return '🔹';
  };

  // 이벤트를 계층구조에 추가
  const addHierarchicalEvent = (eventData: Partial<HierarchicalEvent>) => {
    const newEvent: HierarchicalEvent = {
      id: eventData.id || `event-${Date.now()}-${Math.random()}`,
      level: eventData.level || 'detail',
      parentId: eventData.parentId,
      timestamp: new Date().toLocaleTimeString('ko-KR', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
      }),
      type: eventData.type || 'progress',
      title: eventData.title || 'Unknown Event',
      message: eventData.message || '',
      status: eventData.status || 'pending',
      metadata: eventData.metadata || {},
      children: [],
      isExpanded: true,
    };

    setEvents(prev => {
      const newEvents = [...prev];
      
      // 부모 이벤트가 있는 경우 자식으로 추가
      if (newEvent.parentId) {
        const addToParent = (events: HierarchicalEvent[]): boolean => {
          for (let i = 0; i < events.length; i++) {
            if (events[i].id === newEvent.parentId) {
              events[i].children.push(newEvent);
              return true;
            }
            if (addToParent(events[i].children)) {
              return true;
            }
          }
          return false;
        };
        
        if (!addToParent(newEvents)) {
          // 부모를 찾지 못한 경우 최상위에 추가
          newEvents.push(newEvent);
        }
      } else {
        // 부모가 없는 경우 최상위에 추가
        newEvents.push(newEvent);
      }
      
      return newEvents;
    });

    // 통계 업데이트
    updateStatistics(newEvent);

    // 자동 스크롤
    if (autoScroll()) {
      setTimeout(() => {
        const container = document.querySelector('.events-container');
        if (container) {
          container.scrollTop = container.scrollHeight;
        }
      }, 100);
    }
  };

  // 통계 업데이트
  const updateStatistics = (newEvent: HierarchicalEvent) => {
    setStatistics(prev => {
      const updated = { ...prev };
      updated.totalEvents++;
      updated.eventsByLevel[newEvent.level]++;
      updated.eventsByType[newEvent.type] = (updated.eventsByType[newEvent.type] || 0) + 1;
      
      if (newEvent.status === 'error') {
        updated.errorCount++;
      }
      
      const successCount = Object.entries(updated.eventsByType)
        .filter(([type]) => type === 'success' || type === 'complete')
        .reduce((sum, [, count]) => sum + count, 0);
      
      updated.successRate = updated.totalEvents > 0 ? 
        (successCount / updated.totalEvents) * 100 : 0;
      
      return updated;
    });
  };

  // 이벤트 필터링
  const filteredEvents = createMemo(() => {
    const currentFilter = filter();
    if (!currentFilter.level && !currentFilter.type && !currentFilter.status) {
      return events();
    }

    const filterEvent = (event: HierarchicalEvent): HierarchicalEvent | null => {
      let matches = true;
      
      if (currentFilter.level && event.level !== currentFilter.level) {
        matches = false;
      }
      if (currentFilter.type && event.type !== currentFilter.type) {
        matches = false;
      }
      if (currentFilter.status && event.status !== currentFilter.status) {
        matches = false;
      }

      const filteredChildren = event.children
        .map(child => filterEvent(child))
        .filter(child => child !== null) as HierarchicalEvent[];

      if (matches || filteredChildren.length > 0) {
        return {
          ...event,
          children: filteredChildren
        };
      }
      
      return null;
    };

    return events()
      .map(event => filterEvent(event))
      .filter(event => event !== null) as HierarchicalEvent[];
  });

  // 이벤트 확장/축소 토글
  const toggleEventExpansion = (eventId: string) => {
    const toggleInEvents = (events: HierarchicalEvent[]): HierarchicalEvent[] => {
      return events.map(event => {
        if (event.id === eventId) {
          return { ...event, isExpanded: !event.isExpanded };
        }
        return {
          ...event,
          children: toggleInEvents(event.children)
        };
      });
    };

    setEvents(prev => toggleInEvents(prev));
  };

  // DetailedCrawlingEvent 처리
  const handleDetailedCrawlingEvent = (detailedEvent: any) => {
    console.log('🔍 DetailedCrawlingEvent 수신:', detailedEvent);
    
    // DetailedCrawlingEvent를 HierarchicalEvent로 변환
    const hierarchicalEvent = convertDetailedEventToHierarchical(detailedEvent);
    if (hierarchicalEvent) {
      addHierarchicalEvent(hierarchicalEvent);
    }
  };

  // DetailedCrawlingEvent를 HierarchicalEvent로 변환
  const convertDetailedEventToHierarchical = (detailedEvent: any): Partial<HierarchicalEvent> | null => {
    if (!detailedEvent || typeof detailedEvent !== 'object') {
      console.warn('Invalid detailed event:', detailedEvent);
      return null;
    }

    // DetailedCrawlingEvent의 variant에 따라 변환
    const eventType = Object.keys(detailedEvent)[0];
    const eventData = detailedEvent[eventType];

    switch (eventType) {
      case 'SessionStarted':
        return {
          id: `session-${eventData.session_id}`,
          level: 'session',
          type: 'start',
          title: '크롤링 세션 시작',
          message: `세션 ${eventData.session_id} 시작`,
          status: 'running',
          metadata: { sessionId: eventData.session_id }
        };

      case 'SessionCompleted':
        return {
          id: `session-complete-${eventData.session_id}`,
          level: 'session',
          type: 'complete',
          title: '크롤링 세션 완료',
          message: `세션 ${eventData.session_id} 완료`,
          status: 'success',
          metadata: { sessionId: eventData.session_id }
        };

      case 'BatchCreated':
        return {
          id: `batch-${eventData.batch_id}`,
          level: 'batch',
          type: 'start',
          title: `배치 ${eventData.batch_id} 생성`,
          message: `총 ${eventData.total_batches}개 중 ${eventData.batch_id}번째 배치`,
          status: 'running',
          metadata: { batchId: eventData.batch_id, totalBatches: eventData.total_batches }
        };

      case 'BatchStarted':
        return {
          id: `batch-start-${eventData.batch_id}`,
          level: 'batch',
          type: 'progress',
          title: `배치 ${eventData.batch_id} 시작`,
          message: eventData.message,
          status: 'running',
          metadata: { batchId: eventData.batch_id }
        };

      case 'StageStarted':
        return {
          id: `stage-${eventData.stage}`,
          level: 'stage',
          type: 'start',
          title: `${eventData.stage} 스테이지 시작`,
          message: eventData.message,
          status: 'running',
          metadata: { stage: eventData.stage }
        };

      case 'StageCompleted':
        return {
          id: `stage-complete-${eventData.stage}`,
          level: 'stage',
          type: 'complete',
          title: `${eventData.stage} 스테이지 완료`,
          message: `${eventData.items_processed}개 항목 처리 완료`,
          status: 'success',
          metadata: { stage: eventData.stage, itemsProcessed: eventData.items_processed }
        };

      case 'PageStarted':
        return {
          id: `page-${eventData.page}-${eventData.batch_id}`,
          level: 'page',
          type: 'start',
          title: `페이지 ${eventData.page} 시작`,
          message: `배치 ${eventData.batch_id}에서 페이지 ${eventData.page} 처리 시작`,
          status: 'running',
          metadata: { page: eventData.page, batchId: eventData.batch_id }
        };

      case 'PageCompleted':
        return {
          id: `page-complete-${eventData.page}`,
          level: 'page',
          type: 'success',
          title: `페이지 ${eventData.page} 완료`,
          message: `${eventData.products_found}개 제품 발견`,
          status: 'success',
          metadata: { page: eventData.page, productsFound: eventData.products_found }
        };

      case 'ProductStarted':
        return {
          id: `product-${eventData.product_index}-${eventData.batch_id}`,
          level: 'product',
          type: 'start',
          title: `제품 ${eventData.product_index} 시작`,
          message: `배치 ${eventData.batch_id}에서 제품 ${eventData.product_index} 처리 시작`,
          status: 'running',
          metadata: { productIndex: eventData.product_index, batchId: eventData.batch_id }
        };

      case 'ProductRetryAttempt':
        return {
          id: `product-retry-${eventData.product_index}-${eventData.attempt}`,
          level: 'product',
          type: 'retry',
          title: `제품 ${eventData.product_index} 재시도`,
          message: `${eventData.attempt}/${eventData.max_attempts}번째 재시도 - ${eventData.reason}`,
          status: 'warning',
          metadata: { 
            productIndex: eventData.product_index, 
            attempt: eventData.attempt, 
            maxAttempts: eventData.max_attempts,
            reason: eventData.reason
          }
        };

      case 'ProductRetrySuccess':
        return {
          id: `product-retry-success-${eventData.product_index}`,
          level: 'product',
          type: 'success',
          title: `제품 ${eventData.product_index} 재시도 성공`,
          message: `${eventData.attempt}번째 재시도에서 성공`,
          status: 'success',
          metadata: { productIndex: eventData.product_index, attempt: eventData.attempt }
        };

      case 'ProductRetryFailed':
        return {
          id: `product-retry-failed-${eventData.product_index}`,
          level: 'product',
          type: 'error',
          title: `제품 ${eventData.product_index} 최종 실패`,
          message: `${eventData.max_attempts}번 재시도 후 최종 실패 - ${eventData.reason}`,
          status: 'error',
          metadata: { 
            productIndex: eventData.product_index, 
            maxAttempts: eventData.max_attempts,
            reason: eventData.reason
          }
        };

      case 'ErrorOccurred':
        return {
          id: `error-${Date.now()}`,
          level: 'detail',
          type: 'error',
          title: '오류 발생',
          message: `${eventData.stage}에서 오류: ${eventData.error}`,
          status: 'error',
          metadata: { stage: eventData.stage, error: eventData.error, recoverable: eventData.recoverable }
        };

      default:
        console.warn('Unknown DetailedCrawlingEvent type:', eventType, eventData);
        return {
          level: 'detail',
          type: 'progress',
          title: '알 수 없는 이벤트',
          message: `${eventType}: ${JSON.stringify(eventData)}`,
          status: 'pending',
          metadata: { rawEvent: detailedEvent }
        };
    }
  };

  // 렌더링: 재귀적 이벤트 렌더러
  const renderEvent = (event: HierarchicalEvent, depth: number = 0): any => {
    const config = getLevelConfig(event.level);
    const indentLevel = Math.min(config.indent + depth, 8); // 최대 8레벨

    return (
      <div class="mb-1">
        <div 
          class={`
            p-2 rounded border-l-4 cursor-pointer transition-all duration-200 hover:shadow-md
            ${config.color}
          `}
          style={{ 
            'margin-left': `${indentLevel * 16}px`,
            'border-left-width': `${4 - indentLevel * 0.5}px`
          }}
          onClick={() => event.children.length > 0 && toggleEventExpansion(event.id)}
        >
          <div class="flex items-center justify-between">
            <div class="flex items-center space-x-2 flex-1">
              <span class="text-sm">{config.icon}</span>
              <span class="text-sm">{getStatusIcon(event.status, event.type)}</span>
              <span class="font-semibold text-sm">{event.title}</span>
              <Show when={event.metadata.current && event.metadata.total}>
                <span class="text-xs bg-white px-1 rounded">
                  {event.metadata.current}/{event.metadata.total}
                </span>
              </Show>
              <Show when={event.metadata.percentage}>
                <span class="text-xs bg-white px-1 rounded">
                  {event.metadata.percentage?.toFixed(1)}%
                </span>
              </Show>
              <Show when={event.children.length > 0}>
                <span class="text-xs text-gray-600">
                  {event.isExpanded ? '▼' : '▶'} ({event.children.length})
                </span>
              </Show>
            </div>
            <span class="text-xs text-gray-600 whitespace-nowrap ml-2">
              {event.timestamp}
            </span>
          </div>
          <Show when={event.message}>
            <div class="text-sm mt-1 text-gray-700">
              {event.message}
            </div>
          </Show>
          <Show when={event.metadata.attempt}>
            <div class="text-xs mt-1 text-gray-600">
              재시도: {event.metadata.attempt}/{event.metadata.maxAttempts}
            </div>
          </Show>
        </div>
        
        <Show when={event.isExpanded && event.children.length > 0}>
          <div>
            <For each={event.children}>
              {(child) => renderEvent(child, depth + 1)}
            </For>
          </div>
        </Show>
      </div>
    );
  };

  onMount(async () => {
    try {
      // Session 시작 이벤트
      addHierarchicalEvent({
        id: 'session-start',
        level: 'session',
        type: 'start',
        title: '크롤링 세션 시작',
        message: '새로운 크롤링 세션이 시작되었습니다.',
        status: 'running'
      });

      // 새로운 세분화된 크롤링 이벤트 구독
      const detailedUnlisten = await tauriApi.subscribeToDetailedCrawlingEvents(handleDetailedCrawlingEvent);

      // 오류 이벤트 구독 (백업용)
      const errorUnlisten = await tauriApi.subscribeToErrors((error) => {
        addHierarchicalEvent({
          level: 'detail',
          type: 'error',
          title: '크롤링 오류',
          message: error.message || '알 수 없는 오류가 발생했습니다.',
          status: 'error',
          metadata: { errorReason: error.message }
        });
      });

      cleanupFunctions = [
        detailedUnlisten,
        errorUnlisten
      ];

    } catch (error) {
      console.error('이벤트 구독 설정 중 오류:', error);
      addHierarchicalEvent({
        level: 'session',
        type: 'error',
        title: '시스템 오류',
        message: '이벤트 구독을 설정할 수 없습니다.',
        status: 'error'
      });
    }
  });

  onCleanup(() => {
    cleanupFunctions.forEach(cleanup => cleanup());
  });

  return (
    <div class="p-6 bg-gray-50 min-h-screen">
      <div class="max-w-7xl mx-auto space-y-6">
        {/* 헤더 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <div class="flex justify-between items-center mb-4">
            <h1 class="text-2xl font-bold text-gray-800">계층적 이벤트 모니터</h1>
            <div class="flex items-center space-x-4">
              <label class="flex items-center space-x-2">
                <input 
                  type="checkbox" 
                  checked={autoScroll()} 
                  onChange={(e) => setAutoScroll(e.target.checked)}
                />
                <span class="text-sm">자동 스크롤</span>
              </label>
              <button 
                onClick={() => setEvents([])}
                class="px-3 py-1 bg-red-500 text-white rounded text-sm hover:bg-red-600"
              >
                이벤트 지우기
              </button>
            </div>
          </div>

          {/* 통계 */}
          <div class="grid grid-cols-6 gap-4 text-center">
            <div class="bg-blue-50 p-3 rounded">
              <div class="text-2xl font-bold text-blue-600">{statistics().totalEvents}</div>
              <div class="text-sm text-blue-800">총 이벤트</div>
            </div>
            <div class="bg-green-50 p-3 rounded">
              <div class="text-2xl font-bold text-green-600">{statistics().eventsByLevel.session}</div>
              <div class="text-sm text-green-800">Session</div>
            </div>
            <div class="bg-purple-50 p-3 rounded">
              <div class="text-2xl font-bold text-purple-600">{statistics().eventsByLevel.batch}</div>
              <div class="text-sm text-purple-800">Batch</div>
            </div>
            <div class="bg-yellow-50 p-3 rounded">
              <div class="text-2xl font-bold text-yellow-600">{statistics().eventsByLevel.stage}</div>
              <div class="text-sm text-yellow-800">Stage</div>
            </div>
            <div class="bg-orange-50 p-3 rounded">
              <div class="text-2xl font-bold text-orange-600">{statistics().eventsByLevel.page}</div>
              <div class="text-sm text-orange-800">Page</div>
            </div>
            <div class="bg-gray-50 p-3 rounded">
              <div class="text-2xl font-bold text-gray-600">{statistics().eventsByLevel.product}</div>
              <div class="text-sm text-gray-800">Product</div>
            </div>
          </div>

          <div class="mt-4 text-center">
            <div class="text-2xl font-bold text-gray-600">{statistics().eventsByLevel.detail}</div>
            <div class="text-sm text-gray-800">Detail</div>
          </div>
        </div>

        {/* 필터 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <h2 class="text-lg font-semibold text-gray-800 mb-3">필터</h2>
          <div class="flex flex-wrap gap-4">
            <select 
              value={filter().level || ''} 
              onChange={(e) => setFilter(prev => ({ ...prev, level: e.target.value as EventLevel || undefined }))}
              class="px-3 py-1 border rounded"
            >
              <option value="">모든 레벨</option>
              <option value="session">Session</option>
              <option value="batch">Batch</option>
              <option value="stage">Stage</option>
              <option value="page">Page</option>
              <option value="product">Product</option>
              <option value="detail">Detail</option>
            </select>

            <select 
              value={filter().type || ''} 
              onChange={(e) => setFilter(prev => ({ ...prev, type: e.target.value || undefined }))}
              class="px-3 py-1 border rounded"
            >
              <option value="">모든 타입</option>
              <option value="start">시작</option>
              <option value="progress">진행</option>
              <option value="success">성공</option>
              <option value="retry">재시도</option>
              <option value="error">오류</option>
              <option value="complete">완료</option>
            </select>

            <select 
              value={filter().status || ''} 
              onChange={(e) => setFilter(prev => ({ ...prev, status: e.target.value || undefined }))}
              class="px-3 py-1 border rounded"
            >
              <option value="">모든 상태</option>
              <option value="running">실행중</option>
              <option value="success">성공</option>
              <option value="warning">경고</option>
              <option value="error">오류</option>
              <option value="pending">대기</option>
            </select>
          </div>
        </div>

        {/* 실시간 이벤트 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <h2 class="text-lg font-semibold text-gray-800 mb-3">실시간 이벤트 ({filteredEvents().length}개)</h2>
          
          <div class="events-container max-h-96 overflow-y-auto space-y-1">
            <Show 
              when={filteredEvents().length > 0}
              fallback={
                <div class="text-center text-gray-500 py-8">
                  아직 이벤트가 없습니다. 크롤링을 시작하면 실시간으로 이벤트가 표시됩니다.
                </div>
              }
            >
              <For each={filteredEvents()}>
                {(event) => renderEvent(event)}
              </For>
            </Show>
          </div>
        </div>
      </div>
    </div>
  );
};

export { HierarchicalEventMonitor };
