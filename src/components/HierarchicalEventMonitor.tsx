/**
 * HierarchicalEventMonitor.tsx
 * @description 계층적 이벤트 시스템을 위한 완전히 새로운 모니터링 UI
 * Rust 백엔드의 ConcurrencyEvent 시스템과 완전히 통합된 계층적 표시
 */

import { Component, createSignal, createMemo, onMount, onCleanup, For, Show, createEffect } from 'solid-js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// 생성된 타입들 import
import type { ConcurrencyEvent } from '../types/generated/ConcurrencyEvent';
import type { SessionEventType } from '../types/generated/SessionEventType';
import type { BatchEventType } from '../types/generated/BatchEventType';
import type { StageType } from '../types/generated/StageType';
import type { TaskLifecycleEvent } from '../types/generated/TaskLifecycleEvent';

// 계층적 이벤트 노드 정의
interface HierarchicalEventNode {
  id: string;
  type: 'session' | 'batch' | 'stage' | 'task';
  title: string;
  status: 'running' | 'success' | 'warning' | 'error' | 'pending' | 'completed';
  timestamp: string;
  metadata: Record<string, any>;
  children: HierarchicalEventNode[];
  isExpanded: boolean;
  parent?: string;
  level: number;
}

// 이벤트 통계
interface EventStatistics {
  totalSessions: number;
  activeBatches: number;
  activeStages: number;
  activeTasks: number;
  completedSessions: number;
  errorCount: number;
  successRate: number;
}

export const HierarchicalEventMonitor: Component = () => {
  // 상태 관리
  const [events, setEvents] = createSignal<ConcurrencyEvent[]>([]);
  const [hierarchicalTree, setHierarchicalTree] = createSignal<HierarchicalEventNode[]>([]);
  const [statistics, setStatistics] = createSignal<EventStatistics>({
    totalSessions: 0,
    activeBatches: 0,
    activeStages: 0,
    activeTasks: 0,
    completedSessions: 0,
    errorCount: 0,
    successRate: 0
  });

  const [autoScroll, setAutoScroll] = createSignal(true);
  const [expandAll, setExpandAll] = createSignal(false);
  const [filterLevel, setFilterLevel] = createSignal<string>('all');

  let cleanupFunctions: (() => void)[] = [];
  let eventContainer: HTMLDivElement | undefined;

  // 이벤트 구독 설정
  onMount(async () => {
    console.log('🔄 HierarchicalEventMonitor 초기화 중...');
    
    try {
      // ConcurrencyEvent 이벤트 구독
      const concurrencyEventListener = await listen<ConcurrencyEvent>('concurrency-event', (event) => {
        console.log('📊 받은 ConcurrencyEvent:', event.payload);
        setEvents(prev => {
          const newEvents = [...prev, event.payload];
          return newEvents.slice(-1000); // 최대 1000개 이벤트 유지
        });
      });
      cleanupFunctions.push(concurrencyEventListener);

      // 세션 이벤트 구독 (기존 호환성)
      const sessionEventListener = await listen<any>('session-event', (event) => {
        console.log('🔄 받은 Session Event:', event.payload);
        // ConcurrencyEvent로 변환
        const concurrencyEvent: ConcurrencyEvent = {
          type: 'SessionEvent',
          payload: {
            session_id: event.payload.session_id || `session-${Date.now()}`,
            event_type: event.payload.event_type || 'Started',
            metadata: event.payload.metadata || {},
            timestamp: event.payload.timestamp || new Date().toISOString(),
          }
        };
        setEvents(prev => [...prev.slice(-999), concurrencyEvent]);
      });
      cleanupFunctions.push(sessionEventListener);

      // 배치 이벤트 구독
      const batchEventListener = await listen<any>('batch-event', (event) => {
        console.log('📦 받은 Batch Event:', event.payload);
        const concurrencyEvent: ConcurrencyEvent = {
          type: 'BatchEvent',
          payload: {
            session_id: event.payload.session_id || `session-${Date.now()}`,
            batch_id: event.payload.batch_id || `batch-${Date.now()}`,
            event_type: event.payload.event_type || 'Started',
            metadata: event.payload.metadata || {},
            timestamp: event.payload.timestamp || new Date().toISOString(),
          }
        };
        setEvents(prev => [...prev.slice(-999), concurrencyEvent]);
      });
      cleanupFunctions.push(batchEventListener);

      // 태스크 라이프사이클 이벤트 구독
      const taskEventListener = await listen<any>('task-lifecycle', (event) => {
        console.log('🔧 받은 Task Lifecycle Event:', event.payload);
        const concurrencyEvent: ConcurrencyEvent = {
          type: 'TaskLifecycle',
          payload: {
            context: event.payload.context || {
              session_id: `session-${Date.now()}`,
              batch_id: `batch-${Date.now()}`,
              stage_name: 'Unknown',
              task_id: `task-${Date.now()}`,
              task_url: 'unknown',
              start_time: new Date().toISOString(),
              worker_id: null,
            },
            event: event.payload.event || { 
              Created: { 
                url: 'unknown', 
                task_type: { name: 'Unknown', estimated_duration_ms: null, dependencies: [] }, 
                priority: 'Normal', 
                estimated_completion: new Date().toISOString() 
              } 
            }
          }
        };
        setEvents(prev => [...prev.slice(-999), concurrencyEvent]);
      });
      cleanupFunctions.push(taskEventListener);

      console.log('✅ 모든 이벤트 리스너 설정 완료');
      
      // 초기 더미 데이터 추가 (테스트용)
      const initialEvent: ConcurrencyEvent = {
        type: 'SessionEvent',
        payload: {
          session_id: 'session-initial',
          event_type: 'Started',
          metadata: { message: '초기 세션 시작됨' },
          timestamp: new Date().toISOString(),
        }
      };
      setEvents([initialEvent]);
      
    } catch (error) {
      console.error('❌ 이벤트 리스너 설정 실패:', error);
    }
  });

  // 정리
  onCleanup(() => {
    cleanupFunctions.forEach(cleanup => {
      try {
        cleanup();
      } catch (error) {
        console.error('이벤트 리스너 정리 실패:', error);
      }
    });
  });

  // 이벤트를 계층구조로 변환
  const buildHierarchicalTree = (events: ConcurrencyEvent[]): HierarchicalEventNode[] => {
    const nodes = new Map<string, HierarchicalEventNode>();
    const rootNodes: HierarchicalEventNode[] = [];

    // 이벤트들을 노드로 변환
    events.forEach(event => {
      const node = convertEventToNode(event);
      if (node) {
        nodes.set(node.id, node);
      }
    });

    // 계층구조 구성
    nodes.forEach(node => {
      if (node.parent && nodes.has(node.parent)) {
        const parent = nodes.get(node.parent)!;
        parent.children.push(node);
      } else {
        rootNodes.push(node);
      }
    });

    // 시간순 정렬
    const sortByTimestamp = (a: HierarchicalEventNode, b: HierarchicalEventNode) => {
      return new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime();
    };

    rootNodes.sort(sortByTimestamp);
    nodes.forEach(node => {
      node.children.sort(sortByTimestamp);
    });

    return rootNodes;
  };

  // ConcurrencyEvent를 HierarchicalEventNode로 변환
  const convertEventToNode = (event: ConcurrencyEvent): HierarchicalEventNode | null => {
    switch (event.type) {
      case 'SessionEvent':
        return {
          id: `session-${event.payload.session_id}`,
          type: 'session',
          title: `Session ${event.payload.session_id} - ${event.payload.event_type}`,
          status: getStatusFromSessionEvent(event.payload.event_type),
          timestamp: event.payload.timestamp,
          metadata: event.payload.metadata,
          children: [],
          isExpanded: true,
          level: 0
        };

      case 'BatchEvent':
        return {
          id: `batch-${event.payload.batch_id}`,
          type: 'batch',
          title: `Batch ${event.payload.batch_id} - ${event.payload.event_type}`,
          status: getStatusFromBatchEvent(event.payload.event_type),
          timestamp: event.payload.timestamp,
          metadata: event.payload.metadata,
          children: [],
          isExpanded: true,
          parent: `session-${event.payload.session_id}`,
          level: 1
        };

      case 'StageEvent':
        const stageTitle = getStageTitle(event.payload.stage_type);
        return {
          id: `stage-${event.payload.batch_id || 'global'}-${stageTitle}-${Date.now()}`,
          type: 'stage',
          title: `${stageTitle} - ${getTaskLifecycleEventType(event.payload.event_type)}`,
          status: getStatusFromTaskEvent(event.payload.event_type),
          timestamp: event.payload.timestamp,
          metadata: event.payload.metadata,
          children: [],
          isExpanded: true,
          parent: event.payload.batch_id ? `batch-${event.payload.batch_id}` : `session-${event.payload.session_id}`,
          level: event.payload.batch_id ? 2 : 1
        };

      case 'TaskLifecycle':
        return {
          id: `task-${event.payload.context.task_id}`,
          type: 'task',
          title: `Task ${event.payload.context.task_id} - ${getTaskLifecycleEventType(event.payload.event)}`,
          status: getStatusFromTaskEvent(event.payload.event),
          timestamp: event.payload.context.start_time,
          metadata: { 
            url: event.payload.context.task_url, 
            stage: event.payload.context.stage_name,
            worker: event.payload.context.worker_id 
          },
          children: [],
          isExpanded: false,
          parent: `batch-${event.payload.context.batch_id}`,
          level: 3
        };

      default:
        return null;
    }
  };

  // 도우미 함수들
  const getStatusFromSessionEvent = (eventType: SessionEventType): 'running' | 'success' | 'warning' | 'error' | 'pending' | 'completed' => {
    switch (eventType) {
      case 'Started': return 'running';
      case 'SiteStatusCheck': return 'running';
      case 'BatchPlanning': return 'running';
      case 'Completed': return 'completed';
      case 'Failed': return 'error';
      case 'Cancelled': return 'warning';
      case 'Paused': return 'warning';
      case 'Resumed': return 'running';
      default: return 'pending';
    }
  };

  const getStatusFromBatchEvent = (eventType: BatchEventType): 'running' | 'success' | 'warning' | 'error' | 'pending' | 'completed' => {
    switch (eventType) {
      case 'Created': return 'pending';
      case 'Started': return 'running';
      case 'InProgress': return 'running';
      case 'Completed': return 'completed';
      case 'Failed': return 'error';
      case 'Retrying': return 'warning';
      default: return 'pending';
    }
  };

  const getStatusFromTaskEvent = (event: TaskLifecycleEvent): 'running' | 'success' | 'warning' | 'error' | 'pending' | 'completed' => {
    if (typeof event === 'object') {
      if ('Created' in event) return 'pending';
      if ('Queued' in event) return 'pending';
      if ('Started' in event) return 'running';
      if ('Progress' in event) return 'running';
      if ('Succeeded' in event) return 'completed';
      if ('Failed' in event) return 'error';
      if ('Retrying' in event) return 'warning';
      if ('Cancelled' in event) return 'warning';
      if ('TimedOut' in event) return 'error';
    }
    return 'pending';
  };

  const getTaskLifecycleEventType = (event: TaskLifecycleEvent): string => {
    if (typeof event === 'object') {
      return Object.keys(event)[0];
    }
    return 'Unknown';
  };

  const getStageTitle = (stageType: StageType): string => {
    if (typeof stageType === 'string') {
      return stageType;
    }
    if (typeof stageType === 'object') {
      if ('ProductList' in stageType) {
        return `ProductList (Page ${stageType.ProductList.page_number})`;
      }
      if ('ProductDetails' in stageType) {
        return `ProductDetails (${stageType.ProductDetails.product_id})`;
      }
    }
    return 'Unknown Stage';
  };

  // 레벨별 스타일 설정
  const getLevelConfig = (level: number) => {
    const configs = [
      { 
        color: 'bg-blue-50 border-l-blue-500 text-blue-800',
        indent: 0,
        icon: '🎯',
        name: 'Session'
      },
      { 
        color: 'bg-green-50 border-l-green-500 text-green-800',
        indent: 20,
        icon: '📦',
        name: 'Batch'
      },
      { 
        color: 'bg-purple-50 border-l-purple-500 text-purple-800',
        indent: 40,
        icon: '⚡',
        name: 'Stage'
      },
      { 
        color: 'bg-orange-50 border-l-orange-500 text-orange-800',
        indent: 60,
        icon: '🔧',
        name: 'Task'
      }
    ];
    return configs[Math.min(level, configs.length - 1)];
  };

  // 상태별 아이콘
  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'running': return '⏳';
      case 'success': return '✅';
      case 'completed': return '✅';
      case 'warning': return '⚠️';
      case 'error': return '❌';
      case 'pending': return '🔹';
      default: return '🔹';
    }
  };

  // 이벤트 통계 업데이트
  const updateStatistics = () => {
    const tree = hierarchicalTree();
    const stats: EventStatistics = {
      totalSessions: tree.length,
      activeBatches: 0,
      activeStages: 0,
      activeTasks: 0,
      completedSessions: 0,
      errorCount: 0,
      successRate: 0
    };

    const countNodes = (nodes: HierarchicalEventNode[]) => {
      nodes.forEach(node => {
        switch (node.type) {
          case 'session':
            if (node.status === 'success' || node.status === 'completed') {
              stats.completedSessions++;
            }
            break;
          case 'batch':
            if (node.status === 'running') stats.activeBatches++;
            break;
          case 'stage':
            if (node.status === 'running') stats.activeStages++;
            break;
          case 'task':
            if (node.status === 'running') stats.activeTasks++;
            break;
        }
        
        if (node.status === 'error') {
          stats.errorCount++;
        }
        
        countNodes(node.children);
      });
    };

    countNodes(tree);
    
    const totalEvents = stats.totalSessions + stats.activeBatches + stats.activeStages + stats.activeTasks;
    stats.successRate = totalEvents > 0 ? ((totalEvents - stats.errorCount) / totalEvents) * 100 : 0;
    
    setStatistics(stats);
  };

  // 노드 토글
  const toggleNode = (nodeId: string) => {
    setHierarchicalTree(prev => {
      const toggleNodeRecursive = (nodes: HierarchicalEventNode[]): HierarchicalEventNode[] => {
        return nodes.map(node => {
          if (node.id === nodeId) {
            return { ...node, isExpanded: !node.isExpanded };
          }
          return { ...node, children: toggleNodeRecursive(node.children) };
        });
      };
      return toggleNodeRecursive(prev);
    });
  };

  // 전체 확장/축소
  const toggleExpandAll = () => {
    const newExpandState = !expandAll();
    setExpandAll(newExpandState);
    
    setHierarchicalTree(prev => {
      const setExpandRecursive = (nodes: HierarchicalEventNode[]): HierarchicalEventNode[] => {
        return nodes.map(node => ({
          ...node,
          isExpanded: newExpandState,
          children: setExpandRecursive(node.children)
        }));
      };
      return setExpandRecursive(prev);
    });
  };

  // 가짜 Actor 시스템 테스트 함수 (실제로는 ServiceBased)
  const startFakeActorSystemTest = async () => {
    try {
      console.log('🎭 가짜 Actor 시스템 테스트 시작');
      
      const result = await invoke('start_actor_based_crawling', {
        request: {
          start_page: 1,
          end_page: 3,
          concurrency: 4,
          batch_size: 2,
          delay_ms: 500
        }
      });
      
      console.log(`✅ 가짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      
      // 이벤트 모니터에 알림 추가
      const testEvent: ConcurrencyEvent = {
        type: 'SessionEvent',
        payload: {
          session_id: `fake-actor-test-${Date.now()}`,
          event_type: 'Started',
          metadata: { message: '가짜 Actor 시스템 테스트 크롤링 시작됨 (실제로는 ServiceBased)', test: 'true' },
          timestamp: new Date().toISOString(),
        }
      };
      
      setEvents(prev => [...prev, testEvent]);
    } catch (error) {
      console.error('❌ 가짜 Actor 시스템 테스트 실패:', error);
    }
  };

  // 진짜 Actor 시스템 테스트 함수
  const startRealActorSystemTest = async () => {
    try {
      console.log('🎭 진짜 Actor 시스템 테스트 시작');
      
      const result = await invoke('start_real_actor_crawling', {
        request: {
          // CrawlingPlanner가 모든 설정을 자동 계산하므로 파라미터 불필요
        }
      });
      
      console.log(`✅ 진짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      
      // 이벤트 모니터에 알림 추가
      const testEvent: ConcurrencyEvent = {
        type: 'SessionEvent',
        payload: {
          session_id: `real-actor-test-${Date.now()}`,
          event_type: 'Started',
          metadata: { message: '진짜 Actor 시스템 테스트 크롤링 시작됨', test: 'true' },
          timestamp: new Date().toISOString(),
        }
      };
      
      setEvents(prev => [...prev, testEvent]);
      
      setEvents(prev => [...prev, testEvent]);
      
    } catch (error) {
      console.error('Actor 시스템 테스트 실패:', error);
      
      // 에러 이벤트 추가
      const errorEvent: ConcurrencyEvent = {
        type: 'SessionEvent',
        payload: {
          session_id: `error-session-${Date.now()}`,
          event_type: 'Failed',
          metadata: { message: `Actor 시스템 테스트 실패: ${error}`, error: 'true' },
          timestamp: new Date().toISOString(),
        }
      };
      
      setEvents(prev => [...prev, errorEvent]);
    }
  };

  // 필터링된 트리
  const filteredTree = createMemo(() => {
    const tree = hierarchicalTree();
    const level = filterLevel();
    
    if (level === 'all') return tree;
    
    const filterByLevel = (nodes: HierarchicalEventNode[]): HierarchicalEventNode[] => {
      return nodes.filter(node => {
        if (level === 'session' && node.type === 'session') return true;
        if (level === 'batch' && node.type === 'batch') return true;
        if (level === 'stage' && node.type === 'stage') return true;
        if (level === 'task' && node.type === 'task') return true;
        return false;
      }).map(node => ({
        ...node,
        children: filterByLevel(node.children)
      }));
    };
    
    return filterByLevel(tree);
  });

  // 이벤트 처리 효과
  createEffect(() => {
    const tree = buildHierarchicalTree(events());
    setHierarchicalTree(tree);
    updateStatistics();
    
    if (autoScroll() && eventContainer) {
      setTimeout(() => {
        eventContainer!.scrollTop = eventContainer!.scrollHeight;
      }, 100);
    }
  });

  // 컴포넌트 마운트/언마운트
  onMount(async () => {
    try {
      // 실시간 이벤트 구독 설정
      // 여기서 실제 이벤트 스트림을 구독해야 합니다
      console.log('🎯 계층적 이벤트 모니터 초기화됨');
      
      // 테스트 이벤트 추가 (실제 구현에서는 제거)
      const testEvents: ConcurrencyEvent[] = [
        {
          type: 'SessionEvent',
          payload: {
            session_id: 'session-001',
            event_type: 'Started',
            metadata: { user: 'admin' },
            timestamp: new Date().toISOString()
          }
        }
      ];
      setEvents(testEvents);
      
    } catch (error) {
      console.error('❌ 계층적 이벤트 모니터 초기화 실패:', error);
    }
  });

  onCleanup(() => {
    cleanupFunctions.forEach(cleanup => cleanup());
  });

  // 노드 렌더링
  const renderNode = (node: HierarchicalEventNode) => {
    const config = getLevelConfig(node.level);
    const hasChildren = node.children.length > 0;

    return (
      <div class="event-node">
        <div 
          class={`border-l-4 p-3 mb-2 cursor-pointer transition-all duration-200 hover:shadow-md ${config.color}`}
          style={{ 'margin-left': `${config.indent}px` }}
          onClick={() => hasChildren && toggleNode(node.id)}
        >
          <div class="flex items-center justify-between">
            <div class="flex items-center space-x-2">
              <span class="text-lg">{config.icon}</span>
              <Show when={hasChildren}>
                <span class="text-sm">
                  {node.isExpanded ? '📂' : '📁'}
                </span>
              </Show>
              <span class="text-lg">{getStatusIcon(node.status)}</span>
              <span class="font-medium">{node.title}</span>
            </div>
            <div class="flex items-center space-x-2 text-xs text-gray-500">
              <span>{new Date(node.timestamp).toLocaleTimeString()}</span>
              <Show when={hasChildren}>
                <span class="bg-gray-200 px-2 py-1 rounded">
                  {node.children.length} items
                </span>
              </Show>
            </div>
          </div>
          
          <Show when={Object.keys(node.metadata).length > 0}>
            <div class="mt-2 text-xs text-gray-600">
              <For each={Object.entries(node.metadata)}>
                {([key, value]) => (
                  <span class="inline-block mr-4">
                    <strong>{key}:</strong> {String(value)}
                  </span>
                )}
              </For>
            </div>
          </Show>
        </div>
        
        <Show when={node.isExpanded && hasChildren}>
          <div class="children">
            <For each={node.children}>
              {child => renderNode(child)}
            </For>
          </div>
        </Show>
      </div>
    );
  };

  return (
    <div class="hierarchical-event-monitor h-full flex flex-col bg-gray-50">
      {/* 헤더 */}
      <div class="bg-white border-b border-gray-200 p-4">
        <div class="flex items-center justify-between">
          <h2 class="text-xl font-bold text-gray-800">계층적 이벤트 모니터</h2>
          <div class="flex items-center space-x-4">
            <button
              onClick={startRealActorSystemTest}
              class="px-4 py-2 bg-purple-600 text-white rounded hover:bg-purple-700 transition-colors font-medium"
            >
              🎭 진짜 Actor 테스트
            </button>
            <button
              onClick={startFakeActorSystemTest}
              class="px-4 py-2 bg-orange-600 text-white rounded hover:bg-orange-700 transition-colors font-medium"
            >
              🎭 가짜 Actor 테스트
            </button>
            <button
              onClick={toggleExpandAll}
              class="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
            >
              {expandAll() ? '전체 축소' : '전체 확장'}
            </button>
            <label class="flex items-center">
              <input
                type="checkbox"
                checked={autoScroll()}
                onChange={(e) => setAutoScroll(e.target.checked)}
                class="mr-2"
              />
              자동 스크롤
            </label>
          </div>
        </div>
        
        {/* 통계 */}
        <div class="mt-4 grid grid-cols-2 md:grid-cols-4 lg:grid-cols-7 gap-4">
          <div class="bg-blue-100 p-3 rounded">
            <div class="text-xs font-medium text-blue-800">총 세션</div>
            <div class="text-lg font-bold text-blue-900">{statistics().totalSessions}</div>
          </div>
          <div class="bg-green-100 p-3 rounded">
            <div class="text-xs font-medium text-green-800">활성 배치</div>
            <div class="text-lg font-bold text-green-900">{statistics().activeBatches}</div>
          </div>
          <div class="bg-purple-100 p-3 rounded">
            <div class="text-xs font-medium text-purple-800">활성 스테이지</div>
            <div class="text-lg font-bold text-purple-900">{statistics().activeStages}</div>
          </div>
          <div class="bg-orange-100 p-3 rounded">
            <div class="text-xs font-medium text-orange-800">활성 태스크</div>
            <div class="text-lg font-bold text-orange-900">{statistics().activeTasks}</div>
          </div>
          <div class="bg-gray-100 p-3 rounded">
            <div class="text-xs font-medium text-gray-800">완료 세션</div>
            <div class="text-lg font-bold text-gray-900">{statistics().completedSessions}</div>
          </div>
          <div class="bg-red-100 p-3 rounded">
            <div class="text-xs font-medium text-red-800">에러 수</div>
            <div class="text-lg font-bold text-red-900">{statistics().errorCount}</div>
          </div>
          <div class="bg-indigo-100 p-3 rounded">
            <div class="text-xs font-medium text-indigo-800">성공률</div>
            <div class="text-lg font-bold text-indigo-900">{statistics().successRate.toFixed(1)}%</div>
          </div>
        </div>
        
        {/* 필터 */}
        <div class="mt-4">
          <select
            value={filterLevel()}
            onChange={(e) => setFilterLevel(e.target.value)}
            class="border rounded px-3 py-1"
          >
            <option value="all">모든 레벨</option>
            <option value="session">세션만</option>
            <option value="batch">배치만</option>
            <option value="stage">스테이지만</option>
            <option value="task">태스크만</option>
          </select>
        </div>
      </div>

      {/* 이벤트 트리 */}
      <div 
        ref={eventContainer}
        class="flex-1 overflow-auto p-4 events-container"
      >
        <Show 
          when={filteredTree().length > 0}
          fallback={
            <div class="text-center text-gray-500 mt-8">
              <div class="text-4xl mb-4">📋</div>
              <div>아직 이벤트가 없습니다</div>
              <div class="text-sm mt-2">크롤링을 시작하면 실시간으로 이벤트가 표시됩니다</div>
            </div>
          }
        >
          <For each={filteredTree()}>
            {node => renderNode(node)}
          </For>
        </Show>
      </div>
    </div>
  );
};

export default HierarchicalEventMonitor;
