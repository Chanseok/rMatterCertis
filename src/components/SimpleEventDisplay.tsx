/**
 * SimpleEventDisplay.tsx
 * @description 백엔드에서 전달되는 다양한 이벤트들을 간단하고 직관적으로 표시하는 컴포넌트
 */
import { Component, createMemo, createSignal, onMount, onCleanup, For, createEffect, Show } from 'solid-js';
import { tauriApi } from '../services/tauri-api';
import type { CrawlingProgress, CrawlingResult } from '../types/crawling';
import type { AtomicTaskEvent } from '../types/events';
import { eventStore } from '../stores/eventStore';

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
  // Final session summary (actor-session-report backed)
  const [sessionSummary, setSessionSummary] = createSignal<null | {
    session_id: string;
    duration_ms: number;
    batches_processed: number;
    total_pages: number;
    total_success: number;
    total_failed: number;
    total_retries: number;
    products_inserted: number;
    products_updated: number;
    duplicates_skipped?: number;
    timestamp?: string;
  }>(null);
  // Accumulator for duplicates reported at batch level
  let _duplicatesAccum = 0;

  let cleanupFunctions: (() => void)[] = [];

  // Map actor stage types to our display labels (stage_type only)
  const mapStageName = (stageNameOrType?: string): string | undefined => {
    if (!stageNameOrType) return undefined;
    const s = stageNameOrType.toLowerCase();
    if (s.includes('status') || s.includes('check')) return 'Stage 0: 상태 확인';
    if (s.includes('listpage') || s.includes('productlist') || s.includes('list')) return 'Stage 1: 제품 목록 수집';
    if (s.includes('detail') || s.includes('productdetail')) return 'Stage 2: 세부 정보 수집';
    if (s.includes('validation')) return 'Stage 3: 데이터 검증';
    if (s.includes('saving') || s.includes('database')) return 'Stage 4: 데이터베이스 저장';
    return undefined;
  };

  // Fold global actor events to update stage/status/stat counters
  const processedIds = new Set<string>();
  const incStageCurrent = (label: string) => setStageProgress(prev => prev.map(s => s.name === label ? { ...s, current: s.current + 1 } : s));
  const setStageTotal = (label: string, total: number) => setStageProgress(prev => prev.map(s => s.name === label ? { ...s, total } : s));
  const setStageStatus = (label: string, status: StageProgress['status']) => setStageProgress(prev => prev.map(s => s.name === label ? { ...s, status } : s));

  createEffect(() => {
    const items = eventStore.events();
    // process from oldest to newest to preserve ordering
    for (let i = items.length - 1; i >= 0; i--) {
      const ev = items[i];
      if (processedIds.has(ev.id)) continue;
      const name = ev.name || '';
      const p: any = ev.payload || {};

      // Session lifecycle
      if (name === 'actor-session-started') {
        setIsCrawling(true);
        // reset stages for new run
        setStageProgress([
          { name: 'Stage 0: 상태 확인', current: 0, total: 1, status: 'running' },
          { name: 'Stage 1: 제품 목록 수집', current: 0, total: 0, status: 'idle' },
          { name: 'Stage 2: 세부 정보 수집', current: 0, total: 0, status: 'idle' },
          { name: 'Stage 3: 데이터 검증', current: 0, total: 0, status: 'idle' },
          { name: 'Stage 4: 데이터베이스 저장', current: 0, total: 0, status: 'idle' },
        ]);
  // reset summary accumulators for new session
  _duplicatesAccum = 0;
  setSessionSummary(null);
      }
      if (name === 'actor-session-completed' || name === 'actor-session-failed' || name === 'actor-session-timeout') {
        setIsCrawling(false);
        setStageProgress(prev => prev.map(s => ({ ...s, status: name === 'actor-session-completed' ? 'completed' : 'error' })));
      }

      // Batch info can hint totals for Stage 1
      if (name === 'actor-batch-started') {
        const totalPages = p?.pages_in_batch ?? p?.pages ?? p?.items_total ?? 0;
        if (totalPages > 0) setStageTotal('Stage 1: 제품 목록 수집', totalPages);
      }

      // Stage lifecycle
      if (name === 'actor-stage-started') {
        const label = mapStageName(p?.stage_type);
        if (label) setStageStatus(label, 'running');
      }
      if (name === 'actor-stage-completed') {
        const label = mapStageName(p?.stage_type);
        if (label) {
          setStageStatus(label, 'completed');
          // items_processed is nested under `result.processed_items` in new actor events
          const processed = typeof p?.result?.processed_items === 'number'
            ? p.result.processed_items
            : (typeof p?.items_processed === 'number' ? p.items_processed : undefined);
          if (typeof processed === 'number') {
            setStageTotal(label, processed);
            setStageProgress(prev => prev.map(s => s.name === label ? { ...s, current: processed } : s));
          }
        }
      }
      if (name === 'actor-stage-failed') {
        const label = mapStageName(p?.stage_type);
        if (label) setStageStatus(label, 'error');
      }

      // Per-item/group progress heuristics
      if (name === 'actor-page-task-completed') {
        incStageCurrent('Stage 1: 제품 목록 수집');
      }
      // Stage 2: prefer grouped lifecycle; fall back to per-product lifecycle if needed
      if (name === 'actor-product-lifecycle-group' && (p?.phase === 'fetch')) {
        const inc = Number(p?.group_size ?? p?.succeeded ?? 0) || 0;
        for (let i = 0; i < inc; i++) incStageCurrent('Stage 2: 세부 정보 수집');
      }
      if (name === 'actor-product-lifecycle') {
        const status = String(p?.status || '').toLowerCase();
        if (status === 'fetch_completed' || status === 'fetch_completed_group') {
          incStageCurrent('Stage 2: 세부 정보 수집');
        }
      }

      // Validation → Stage 3
      if (name === 'actor-validation-started') setStageStatus('Stage 3: 데이터 검증', 'running');
      if (name === 'actor-validation-completed') setStageStatus('Stage 3: 데이터 검증', 'completed');

      // Persistence/DB → Stage 4
      if (name === 'actor-persistence-anomaly') setStageStatus('Stage 4: 데이터베이스 저장', 'error');
      if (name === 'actor-database-stats') {
        // Accept both legacy total_products and new total_product_details
        const total = p?.total_product_details ?? p?.total_products ?? p?.total ?? statistics().totalProducts;
        if (typeof total === 'number') setStatistics(prev => ({ ...prev, totalProducts: total }));
        setStageStatus('Stage 4: 데이터베이스 저장', 'running');
      }

      // Batch/session level reports → accumulate and finalize a compact session summary
      if (name === 'actor-batch-report') {
        const dup = typeof p?.duplicates_skipped === 'number' ? p.duplicates_skipped : 0;
        _duplicatesAccum += dup;
      }
      if (name === 'actor-session-report') {
        setSessionSummary({
          session_id: p?.session_id,
          duration_ms: Number(p?.duration_ms ?? 0),
          batches_processed: Number(p?.batches_processed ?? 0),
          total_pages: Number(p?.total_pages ?? 0),
          total_success: Number(p?.total_success ?? 0),
          total_failed: Number(p?.total_failed ?? 0),
          total_retries: Number(p?.total_retries ?? 0),
          products_inserted: Number(p?.products_inserted ?? 0),
          products_updated: Number(p?.products_updated ?? 0),
          duplicates_skipped: _duplicatesAccum || undefined,
          timestamp: p?.timestamp || p?.backend_ts,
        });
      }

      // Errors/Anomalies count heuristic
      if (name.includes('error') || name.includes('failed') || name === 'actor-validation-anomaly' || name === 'actor-validation-divergence') {
        setStatistics(prev => ({ ...prev, errorItems: prev.errorItems + 1 }));
      }

      // New/Updated heuristic (try to read fields if present)
      if (name === 'actor-product-lifecycle' || name === 'actor-product-lifecycle-group') {
        const inserted = p?.inserted ?? p?.persist_inserted ?? p?.new_items ?? 0;
        const updated = p?.updated ?? p?.persist_updated ?? p?.updated_items ?? 0;
        if (inserted || updated) {
          setStatistics(prev => ({ ...prev, newItems: prev.newItems + (inserted || 0), updatedItems: prev.updatedItems + (updated || 0) }));
        }
      }

      processedIds.add(ev.id);
    }
  });

  // Map global buffered events to local display items (latest first, cap 50)
  const displayedEvents = createMemo<EventItem[]>(() => {
    const items = eventStore.events();
    return items.slice(0, 50).map((g) => {
      const status: EventItem['status'] = /error|failed/i.test(g.name) ? 'error' : 'info';
      let type: EventItem['type'] = 'system';
      if (/stage|progress/i.test(g.name)) type = 'stage';
      if (/product|detail|task/i.test(g.name)) type = 'product';
      if (/db|database/i.test(g.name)) type = 'system';
      if (/error|failed/i.test(g.name)) type = 'error';
      return {
        id: g.id,
        timestamp: new Date(g.ts).toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
        type,
        title: g.name,
        message: typeof g.payload === 'string' ? g.payload : JSON.stringify(g.payload).slice(0, 180),
        status,
      } as EventItem;
    });
  });

  // 테스트용 크롤링 시작 함수
  const startTestCrawling = async () => {
    try {
  setIsCrawling(true);

      // 백엔드 크롤링 API 호출 (간단한 테스트용)
      await tauriApi.startCrawling(5); // 5페이지까지 크롤링

    } catch (error) {
  setIsCrawling(false);
    }
  };

  // 크롤링 중지 함수
  const stopCrawling = async () => {
    try {
      await tauriApi.stopCrawling();
  setIsCrawling(false);
    } catch (error) {
    }
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
  // Optional: update progress derived from atomic events as needed
      });

      // 에러 이벤트 구독
      const errorUnlisten = await tauriApi.subscribeToErrors((error) => {
  // Could update error counters if desired
      });

      // 스테이지 변경 이벤트 구독
      const stageUnlisten = await tauriApi.subscribeToStageChange((data) => {
        // 이전 스테이지 완료 처리
        setStageProgress(prev => prev.map(stage => 
          stage.name.includes(data.from) ? { ...stage, status: 'completed' } : stage
        ));
      });

      // 완료 이벤트 구독
      const completionUnlisten = await tauriApi.subscribeToCompletion((result) => {
        const successRate = result.total_processed > 0 ? 
          ((result.total_processed - result.errors) / result.total_processed) * 100 : 0;
          
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
        // No-op for log; could drive a per-stage metric if needed
      });

      // 데이터베이스 통계 이벤트 구독
      const dbUnlisten = await tauriApi.subscribeToDatabaseUpdates((stats) => {
        // Optional: reflect DB stats in a card
      });

      // 계층형 상세 크롤링 이벤트 구독
      const detailUnlisten = await tauriApi.subscribeToDetailedCrawlingEvents((ev) => {
        // No-op for log; the global store already buffers these
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

  // Small helpers
  const fmtMs = (ms: number) => {
    if (!ms || ms < 1000) return `${ms|0} ms`;
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const remS = s % 60;
    return m > 0 ? `${m}m ${remS}s` : `${s}s`;
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
          <span>총 이벤트: {displayedEvents().length}</span>
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
                      {stage.total > 0 ? `${stage.current}/${stage.total}` : ''}
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
          {/* 최종 세션 요약 (있을 때만 표시) */}
          <Show when={!!sessionSummary()}>
            <div class="mt-4 border-t pt-3">
              <h3 class="text-md font-semibold text-gray-700 mb-2">세션 요약</h3>
              <div class="grid grid-cols-2 gap-2 text-sm">
                <div class="text-gray-600">세션</div><div class="text-gray-800 truncate" title={sessionSummary()!.session_id}>{sessionSummary()!.session_id}</div>
                <div class="text-gray-600">소요시간</div><div class="text-gray-800">{fmtMs(sessionSummary()!.duration_ms)}</div>
                <div class="text-gray-600">배치</div><div class="text-gray-800">{sessionSummary()!.batches_processed}</div>
                <div class="text-gray-600">페이지</div><div class="text-gray-800">{sessionSummary()!.total_success}/{sessionSummary()!.total_pages} 성공</div>
                <div class="text-gray-600">실패/재시도</div><div class="text-gray-800">{sessionSummary()!.total_failed} 실패, {sessionSummary()!.total_retries} 재시도</div>
                <div class="text-gray-600">DB 저장</div><div class="text-gray-800">+{sessionSummary()!.products_inserted} ins, +{sessionSummary()!.products_updated} upd{sessionSummary()!.duplicates_skipped != null ? `, ${sessionSummary()!.duplicates_skipped} dup` : ''}</div>
              </div>
            </div>
          </Show>
        </div>

        {/* 실시간 이벤트 로그 */}
        <div class="bg-white rounded-lg shadow-md p-4">
          <h2 class="text-lg font-semibold text-gray-800 mb-4">실시간 이벤트</h2>
          <div class="h-80 overflow-y-auto space-y-2">
            <For each={displayedEvents()}>
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
