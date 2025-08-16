/**
 * SimpleEventDisplay.tsx
 * @description 백엔드에서 전달되는 다양한 이벤트들을 간단하고 직관적으로 표시하는 컴포넌트
 */
import { Component, createSignal, onMount, onCleanup, For } from 'solid-js';
import { tauriApi } from '../services/tauri-api';
import type { CrawlingProgress, CrawlingResult } from '../types/crawling';
import type { AtomicTaskEvent } from '../types/events';

interface EventItem {
  id: string;
  timestamp: string;
  type: 'stage' | 'product' | 'error' | 'system';
  title: string;
  message: string;
  status: 'info' | 'success' | 'warning' | 'error';
}

interface StageProgress {
  name: string;
  current: number;
  total: number;
  status: 'idle' | 'running' | 'completed' | 'error';
}

export const SimpleEventDisplay: Component = () => {
  // State
  const [events, setEvents] = createSignal<EventItem[]>([]);
  const [stageProgress, setStageProgress] = createSignal<StageProgress[]>([
    { name: 'Stage 0: 상태 확인', current: 0, total: 1, status: 'idle' },
    { name: 'Stage 1: 제품 목록 수집', current: 0, total: 0, status: 'idle' },
    { name: 'Stage 2: 세부 정보 수집', current: 0, total: 0, status: 'idle' },
    { name: 'Stage 3: 데이터 검증', current: 0, total: 0, status: 'idle' },
    { name: 'Stage 4: 데이터베이스 저장', current: 0, total: 0, status: 'idle' },
  ]);
  const [statistics, setStatistics] = createSignal({
    totalProducts: 0,
    newItems: 0,
    updatedItems: 0,
    skippedItems: 0,
    errorItems: 0,
    processingRate: 0
  });
  const [isCrawling, setIsCrawling] = createSignal(false);

  let cleanupFunctions: (() => void)[] = [];

  // 테스트용 크롤링 시작 함수
  const startTestCrawling = async () => {
    try {
      setIsCrawling(true);
      addEvent({
        type: 'system',
        title: '크롤링 시작',
        message: '테스트 크롤링을 시작합니다...',
        status: 'info'
      });

      // 백엔드 크롤링 API 호출 (간단한 테스트용)
      await tauriApi.startCrawling(5); // 5페이지까지 크롤링

    } catch (error) {
      setIsCrawling(false);
      addEvent({
        type: 'error',
        title: '크롤링 시작 실패',
        message: `오류: ${error}`,
        status: 'error'
      });
    }
  };

  // 크롤링 중지 함수
  const stopCrawling = async () => {
    try {
      await tauriApi.stopCrawling();
      setIsCrawling(false);
      addEvent({
        type: 'system',
        title: '크롤링 중지',
        message: '크롤링이 중지되었습니다.',
        status: 'warning'
      });
    } catch (error) {
      addEvent({
        type: 'error',
        title: '크롤링 중지 실패',
        message: `오류: ${error}`,
        status: 'error'
      });
    }
  };

  // 이벤트 추가 함수
  const addEvent = (event: Omit<EventItem, 'id' | 'timestamp'>) => {
    const newEvent: EventItem = {
      ...event,
      id: Math.random().toString(36).substr(2, 9),
      timestamp: new Date().toLocaleTimeString('ko-KR', { 
        hour: '2-digit', 
        minute: '2-digit', 
        second: '2-digit' 
      })
    };
    
    setEvents(prev => [newEvent, ...prev.slice(0, 49)]); // 최대 50개 유지
  };

  // Stage 진행 상황 업데이트
  const updateStageProgress = (stageName: string, current: number, total: number, status: StageProgress['status']) => {
    setStageProgress(prev => prev.map(stage => 
      stage.name.includes(stageName) ? { ...stage, current, total, status } : stage
    ));
  };

  // 통계 업데이트
  const updateStatistics = (newStats: Partial<typeof statistics>) => {
    setStatistics(prev => ({ ...prev, ...newStats }));
  };

  onMount(async () => {
    try {
      // 크롤링 진행 상황 이벤트 구독
      const progressUnlisten = await tauriApi.subscribeToProgress((progress) => {
        const percentage = progress.percentage ?? 0;
        addEvent({
          type: 'system',
          title: '진행 상황 업데이트',
          message: `${progress.current_stage}: ${progress.current}/${progress.total} (${percentage.toFixed(1)}%)`,
          status: 'info'
        });

        // Stage 진행 상황 업데이트
        updateStageProgress(progress.current_stage, progress.current, progress.total, 'running');
        
        // 통계 업데이트
        updateStatistics({
          newItems: progress.new_items,
          updatedItems: progress.updated_items,
          errorItems: progress.errors
        });
      });

      // 원자적 작업 이벤트 구독
      const atomicUnlisten = await tauriApi.subscribeToAtomicTaskUpdates((event) => {
        let status: EventItem['status'] = 'info';
        let title = '작업 진행';
        
        switch (event.status) {
          case 'Success':
            status = 'success';
            title = '작업 완료';
            break;
          case 'Error':
            status = 'error';
            title = '작업 실패';
            break;
          case 'Active':
            status = 'info';
            title = '작업 실행 중';
            break;
          case 'Retrying':
            status = 'warning';
            title = '작업 재시도';
            break;
        }

        addEvent({
          type: 'product',
          title,
          message: `${event.stage_name}: ${event.task_id.slice(0, 8)}... (${(event.progress * 100).toFixed(1)}%)`,
          status
        });
      });

      // 에러 이벤트 구독
      const errorUnlisten = await tauriApi.subscribeToErrors((error) => {
        addEvent({
          type: 'error',
          title: '오류 발생',
          message: `${error.stage}: ${error.message}`,
          status: 'error'
        });
      });

      // 스테이지 변경 이벤트 구독
      const stageUnlisten = await tauriApi.subscribeToStageChange((data) => {
        addEvent({
          type: 'stage',
          title: '스테이지 전환',
          message: `${data.from} → ${data.to}: ${data.message}`,
          status: 'info'
        });

        // 이전 스테이지 완료 처리
        setStageProgress(prev => prev.map(stage => 
          stage.name.includes(data.from) ? { ...stage, status: 'completed' } : stage
        ));
      });

      // 완료 이벤트 구독
      const completionUnlisten = await tauriApi.subscribeToCompletion((result) => {
        const successRate = result.total_processed > 0 ? 
          ((result.total_processed - result.errors) / result.total_processed) * 100 : 0;
          
        addEvent({
          type: 'system',
          title: '크롤링 완료',
          message: `총 ${result.total_processed}개 항목 처리 완료 (성공률: ${successRate.toFixed(1)}%)`,
          status: 'success'
        });

        updateStatistics({
          totalProducts: result.total_processed,
          newItems: result.new_items,
          updatedItems: result.updated_items,
          errorItems: result.errors,
          processingRate: successRate
        });

        // 모든 스테이지 완료 처리
        setStageProgress(prev => prev.map(stage => ({ ...stage, status: 'completed' })));
        
        // 크롤링 상태 업데이트
        setIsCrawling(false);
      });

      // 세부 태스크 상태 이벤트 구독
      const taskUnlisten = await tauriApi.subscribeToTaskStatus((task) => {
        addEvent({
          type: 'product',
          title: '태스크 업데이트',
          message: `${task.stage}: ${task.status} - ${task.message ?? ''}`,
          status: task.status === 'Completed' ? 'success' : task.status === 'Failed' ? 'error' : task.status === 'Retrying' ? 'warning' : 'info'
        });
      });

      // 데이터베이스 통계 이벤트 구독
      const dbUnlisten = await tauriApi.subscribeToDatabaseUpdates((stats) => {
        addEvent({
          type: 'system',
          title: '데이터베이스 업데이트',
          message: `총 제품 ${stats.total_products}, 최근 업데이트 ${stats.last_updated}`,
          status: 'info'
        });
      });

      // 계층형 상세 크롤링 이벤트 구독
      const detailUnlisten = await tauriApi.subscribeToDetailedCrawlingEvents((ev) => {
        const name = ev?.event_name || 'detailed-crawling-event';
        addEvent({
          type: 'system',
          title: name,
          message: typeof ev === 'string' ? ev : JSON.stringify(ev).slice(0, 180),
          status: 'info'
        });
      });

      // 정리 함수 등록
      cleanupFunctions = [
        progressUnlisten,
        taskUnlisten,
        dbUnlisten,
        detailUnlisten,
        atomicUnlisten,
        errorUnlisten,
        stageUnlisten,
        completionUnlisten
      ];

    } catch (error) {
      console.error('이벤트 구독 설정 중 오류:', error);
      addEvent({
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

  // 상태별 색상 매핑
  const getStatusColor = (status: EventItem['status']) => {
    switch (status) {
      case 'success': return 'bg-green-100 border-green-400 text-green-800';
      case 'error': return 'bg-red-100 border-red-400 text-red-800';
      case 'warning': return 'bg-yellow-100 border-yellow-400 text-yellow-800';
      default: return 'bg-blue-100 border-blue-400 text-blue-800';
    }
  };

  const getStageStatusColor = (status: StageProgress['status']) => {
    switch (status) {
      case 'completed': return 'bg-green-500';
      case 'running': return 'bg-blue-500';
      case 'error': return 'bg-red-500';
      default: return 'bg-gray-300';
    }
  };

  return (
    <div class="w-full h-screen p-4 bg-gray-50">
      {/* 헤더 */}
      <div class="mb-6">
        <div class="flex justify-between items-center mb-2">
          <h1 class="text-2xl font-bold text-gray-800">크롤링 진행 상황</h1>
          <div class="flex gap-2">
            <button 
              onClick={startTestCrawling}
              disabled={isCrawling()}
              class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:bg-gray-400 disabled:cursor-not-allowed"
            >
              {isCrawling() ? '실행 중...' : '🚀 테스트 크롤링 시작'}
            </button>
            <button 
              onClick={stopCrawling}
              disabled={!isCrawling()}
              class="px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:bg-gray-400 disabled:cursor-not-allowed"
            >
              ⏹️ 중지
            </button>
          </div>
        </div>
        <div class="flex gap-4 text-sm text-gray-600">
          <span>총 이벤트: {events().length}</span>
          <span>처리율: {statistics().processingRate.toFixed(1)}%</span>
          <span>총 제품: {statistics().totalProducts}</span>
          <span class={`font-semibold ${isCrawling() ? 'text-green-600' : 'text-gray-500'}`}>
            상태: {isCrawling() ? '실행 중' : '대기 중'}
          </span>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 h-5/6">
        {/* 스테이지 진행 상황 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <h2 class="text-lg font-semibold text-gray-800 mb-4">처리 단계</h2>
          <div class="space-y-3">
            <For each={stageProgress()}>
              {(stage) => (
                <div class="space-y-2">
                  <div class="flex justify-between items-center">
                    <span class="text-sm font-medium text-gray-700">{stage.name}</span>
                    <span class="text-xs text-gray-500">
                      {stage.current}/{stage.total}
                    </span>
                  </div>
                  <div class="w-full bg-gray-200 rounded-full h-2">
                    <div 
                      class={`h-2 rounded-full transition-all duration-300 ${getStageStatusColor(stage.status)}`}
                      style={{ width: `${stage.total > 0 ? (stage.current / stage.total) * 100 : 0}%` }}
                    ></div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>

        {/* 통계 정보 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <h2 class="text-lg font-semibold text-gray-800 mb-4">처리 통계</h2>
          <div class="grid grid-cols-2 gap-4">
            <div class="text-center p-3 bg-green-50 rounded-lg">
              <div class="text-2xl font-bold text-green-600">{statistics().newItems}</div>
              <div class="text-sm text-gray-600">신규 항목</div>
            </div>
            <div class="text-center p-3 bg-blue-50 rounded-lg">
              <div class="text-2xl font-bold text-blue-600">{statistics().updatedItems}</div>
              <div class="text-sm text-gray-600">업데이트</div>
            </div>
            <div class="text-center p-3 bg-gray-50 rounded-lg">
              <div class="text-2xl font-bold text-gray-600">{statistics().skippedItems}</div>
              <div class="text-sm text-gray-600">스킵</div>
            </div>
            <div class="text-center p-3 bg-red-50 rounded-lg">
              <div class="text-2xl font-bold text-red-600">{statistics().errorItems}</div>
              <div class="text-sm text-gray-600">오류</div>
            </div>
          </div>
        </div>

        {/* 실시간 이벤트 로그 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <h2 class="text-lg font-semibold text-gray-800 mb-4">실시간 이벤트</h2>
          <div class="h-80 overflow-y-auto space-y-2">
            <For each={events()}>
              {(event) => (
                <div class={`p-3 rounded-lg border-l-4 ${getStatusColor(event.status)}`}>
                  <div class="flex justify-between items-start mb-1">
                    <span class="text-sm font-semibold">{event.title}</span>
                    <span class="text-xs text-gray-500">{event.timestamp}</span>
                  </div>
                  <p class="text-sm">{event.message}</p>
                </div>
              )}
            </For>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SimpleEventDisplay;
