/**
 * LiveCrawlingTab.tsx
 * 새로운 (깨끗한) Live Crawling UI - Actor 이벤트 기반 Phase 1 최소 구현
 * 기존 LiveProductionTab.tsx 는 제거 전 검증용으로 유지
 */
import { Component, createSignal, createEffect, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { useActorVisualizationStream } from '../../hooks/useActorVisualizationStream';
import { SessionStatusPanel } from '../actor-system/SessionStatusPanel';
import CrawlingProgressDashboard from '../visualization/CrawlingProgressDashboard';

export const LiveCrawlingTab: Component = () => {
  const [isRunning, setIsRunning] = createSignal(false);
  const [statusText, setStatusText] = createSignal('대기 중');
  const [activeBatches, setActiveBatches] = createSignal(0);
  const [pageStartedCount, setPageStartedCount] = createSignal(0);
  const [pageCompletedCount, setPageCompletedCount] = createSignal(0);
  // 기존 pageCount 용도 대체: UI 표시에선 시작/완료 동시 표시
  const [productCount] = createSignal(0); // TODO: 향후 제품 이벤트 연결
  const [errorCount, setErrorCount] = createSignal(0);
  const [progressPct, setProgressPct] = createSignal<number | null>(null);
  const [lastBatchReport, setLastBatchReport] = createSignal<any>(null);
  const [sessionReport, setSessionReport] = createSignal<any>(null);

  const { events } = useActorVisualizationStream(400);

  createEffect(() => {
    const list = events();
    if (!list.length) return;
    const last = list[list.length - 1];
    switch (last.rawName) {
      case 'actor-session-started':
        setIsRunning(true);
        setStatusText('실행 중');
        break;
      case 'actor-session-completed':
        setIsRunning(false);
        setStatusText('완료');
        break;
      case 'actor-batch-started':
        setActiveBatches(v => v + 1);
        break;
      case 'actor-batch-completed':
      case 'actor-batch-failed':
        setActiveBatches(v => Math.max(0, v - 1));
        break;
      case 'actor-page-task-started':
        setPageStartedCount(v => v + 1);
        break;
      case 'actor-page-task-completed':
        setPageCompletedCount(v => v + 1);
        break;
      case 'actor-page-task-failed':
        setErrorCount(v => v + 1);
        break;
      case 'actor-progress':
        if (last.progressPct != null) setProgressPct(last.progressPct!);
        break;
      case 'actor-batch-report':
        setLastBatchReport(last);
        break;
      case 'actor-session-report':
        setSessionReport(last);
        break;
      default:
        break;
    }
  });

  const start = async () => {
    try {
      setIsRunning(true);
      setStatusText('시작 중...');
  // Use proper ActorCrawlingRequest fields (batch_size/concurrency/delay_ms)
  await invoke('start_actor_system_crawling', { request: { batch_size: 50, concurrency: 10, delay_ms: 1000 } });
    } catch (e) {
      console.error('시작 실패', e);
      setStatusText('시작 실패');
      setIsRunning(false);
    }
  };

  const stop = async () => {
    try {
      setStatusText('중지 중...');
      await invoke('request_graceful_shutdown', { request: { timeout_ms: 15000 } });
    } catch (e) {
      console.error('중지 실패', e);
      setStatusText('중지 실패');
    }
  };

  const reset = () => {
    setIsRunning(false);
    setStatusText('대기 중');
    setActiveBatches(0);
  setPageStartedCount(0);
  setPageCompletedCount(0);
    setErrorCount(0);
  setProgressPct(null);
  setLastBatchReport(null);
  setSessionReport(null);
  };

  return (
    <div class="h-full flex flex-col" data-tab-root="liveCrawling">
      <div class="flex-shrink-0 border-b border-gray-200 p-4">
        <div class="flex items-center justify-between mb-4">
          <div>
            <h2 class="text-xl font-semibold text-gray-800">Live Crawling</h2>
            <p class="text-sm text-gray-600 mt-1">Actor 이벤트 기반 실시간 크롤링 진행 상황</p>
          </div>
          <div class="flex items-center space-x-3">
            <button onClick={start} disabled={isRunning()} class="px-5 py-2 bg-green-600 text-white rounded-md hover:bg-green-700 disabled:bg-gray-400 disabled:cursor-not-allowed">시작</button>
            <button onClick={stop} disabled={!isRunning()} class="px-5 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 disabled:bg-gray-400 disabled:cursor-not-allowed">중지</button>
            <button onClick={reset} disabled={isRunning()} class="px-4 py-2 bg-gray-500 text-white rounded-md hover:bg-gray-600 disabled:bg-gray-400 disabled:cursor-not-allowed">리셋</button>
          </div>
        </div>
        <div class="grid grid-cols-7 gap-4 text-sm">
          <div class="bg-blue-50 border border-blue-200 rounded-lg p-3"><div class="text-blue-600 font-medium">상태</div><div class="text-blue-800 font-semibold">{statusText()}</div></div>
          <div class="bg-green-50 border border-green-200 rounded-lg p-3"><div class="text-green-600 font-medium">활성 배치</div><div class="text-green-800 font-semibold">{activeBatches()}</div></div>
          <div class="bg-purple-50 border border-purple-200 rounded-lg p-3"><div class="text-purple-600 font-medium">페이지(시작)</div><div class="text-purple-800 font-semibold">{pageStartedCount().toLocaleString()}</div></div>
          <div class="bg-purple-50 border border-purple-200 rounded-lg p-3"><div class="text-purple-600 font-medium">페이지(완료)</div><div class="text-purple-800 font-semibold">{pageCompletedCount().toLocaleString()}</div></div>
          <div class="bg-indigo-50 border border-indigo-200 rounded-lg p-3"><div class="text-indigo-600 font-medium">제품 수</div><div class="text-indigo-800 font-semibold">{productCount().toLocaleString()}</div></div>
          <div class="bg-red-50 border border-red-200 rounded-lg p-3"><div class="text-red-600 font-medium">에러 수</div><div class="text-red-800 font-semibold">{errorCount()}</div></div>
          <div class="bg-amber-50 border border-amber-200 rounded-lg p-3"><div class="text-amber-600 font-medium">진행률</div><div class="text-amber-800 font-semibold">{progressPct() == null ? '-' : `${progressPct()!.toFixed(1)}%`}</div></div>
        </div>
      </div>
      <div class="px-4 pb-2 grid grid-cols-2 gap-3">
        <Show when={lastBatchReport()}>
          <div class="bg-white/70 dark:bg-neutral-800/50 border border-gray-200 dark:border-neutral-700 rounded-md p-3 text-[11px] font-mono">
            <div class="font-semibold text-gray-600 mb-1">Last Batch Report</div>
            <pre class="whitespace-pre-wrap leading-tight max-h-40 overflow-auto">{JSON.stringify(lastBatchReport(), null, 1)}</pre>
          </div>
        </Show>
        <Show when={sessionReport()}>
          <div class="bg-white/70 dark:bg-neutral-800/50 border border-gray-200 dark:border-neutral-700 rounded-md p-3 text-[11px] font-mono">
            <div class="font-semibold text-gray-600 mb-1">Session Report</div>
            <pre class="whitespace-pre-wrap leading-tight max-h-40 overflow-auto">{JSON.stringify(sessionReport(), null, 1)}</pre>
          </div>
        </Show>
      </div>
      {/* 최근 이벤트 패널 (디버그 시각화) */}
      <div class="px-4 pb-2 -mt-2">
        <div class="bg-white/70 dark:bg-neutral-800/50 border border-gray-200 dark:border-neutral-700 rounded-md p-3">
          <div class="flex items-center justify-between mb-1">
            <h3 class="text-xs font-semibold tracking-wide text-gray-600 dark:text-gray-300">Recent Actor Events</h3>
            <span class="text-[10px] text-gray-400">last 15 / total {events().length}</span>
          </div>
          <Show when={events().length} fallback={<div class="text-[11px] text-gray-400">아직 수신된 이벤트가 없습니다.</div>}>
            <ol class="text-[11px] font-mono space-y-0.5 max-h-28 overflow-auto leading-tight">
              <For each={events().slice(-15)}>{ev => (
                <li class="flex gap-2">
                  <span class="text-gray-400">#{ev.seq}</span>
                  <span class="px-1 rounded bg-gray-100 dark:bg-neutral-700 text-gray-700 dark:text-gray-200">{ev.rawName}</span>
                  <Show when={ev.batchId}><span class="text-emerald-600">{ev.batchId}</span></Show>
                  <Show when={ev.page !== undefined}><span class="text-indigo-600">p{ev.page}</span></Show>
                </li>
              )}</For>
            </ol>
          </Show>
        </div>
      </div>
      <div class="p-4 border-b border-gray-200 bg-gray-50/40 dark:bg-neutral-800/30">
        <SessionStatusPanel />
      </div>
      <div class="flex-1 overflow-hidden">
        <CrawlingProgressDashboard />
      </div>
    </div>
  );
};

export default LiveCrawlingTab;
