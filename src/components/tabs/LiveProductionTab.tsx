/**
 * LiveProductionTab.tsx
 * @description Live Production Line UI를 위한 탭 컴포넌트
 */

import { Component, createSignal, onMount, onCleanup, createEffect } from 'solid-js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import CrawlingProgressDashboard from '../visualization/CrawlingProgressDashboard';
import { SessionStatusPanel } from '../actor-system/SessionStatusPanel';
import { useActorVisualizationStream } from '../../hooks/useActorVisualizationStream';

export const LiveProductionTab: Component = () => {
  // 크롤링 상태 관리
  const [isCrawlingActive, setIsCrawlingActive] = createSignal(false);
  const [crawlingStatus, setCrawlingStatus] = createSignal('대기 중');
  const [activeBatches, setActiveBatches] = createSignal(0);
  const [totalPages, setTotalPages] = createSignal(0);
  const [totalProducts, setTotalProducts] = createSignal(0);
  const [errorCount, setErrorCount] = createSignal(0);
  const [productsInserted, setProductsInserted] = createSignal(0);
  const [productsUpdated, setProductsUpdated] = createSignal(0);
  const [lastBatchId, setLastBatchId] = createSignal<string | undefined>(undefined);
  // New lifecycle progress counters
  const [pagesFetchStarted, setPagesFetchStarted] = createSignal(0);
  const [pagesFetchCompleted, setPagesFetchCompleted] = createSignal(0);
  const [productsFetchStarted, setProductsFetchStarted] = createSignal(0);
  const [productsFetchCompleted, setProductsFetchCompleted] = createSignal(0);

  const { events } = useActorVisualizationStream(600);
  // Derive simple aggregates from events (minimal Phase 1 wiring)
  const [pageCount, setPageCount] = createSignal(0);
  const [productCount] = createSignal(0); // placeholder until product events exist
  const [batchActive, setBatchActive] = createSignal(0);

  // reactively update counters when new events appended
  const originalSetTotalPages = setTotalPages;
  const originalSetActiveBatches = setActiveBatches;
  const originalSetTotalProducts = setTotalProducts;

  // naive observer (Phase 1); could be replaced with memo derivations
  // events.subscribe?.?.(); // placeholder to avoid TS removal if not used

  // 크롤링 시작
  const startCrawling = async () => {
    try {
      setIsCrawlingActive(true);
      setCrawlingStatus('크롤링 시작 중...');
      await invoke('start_actor_system_crawling', {
        request: {
          override_batch_size: 50,
          override_max_concurrency: 10,
          override_delay_ms: 1000,
        }
      });
      setCrawlingStatus('크롤링 실행 중');
    } catch (error) {
      console.error('크롤링 시작 실패:', error);
      setCrawlingStatus('크롤링 시작 실패');
      setIsCrawlingActive(false);
    }
  };

  // 크롤링 중지
  const stopCrawling = async () => {
    try {
      setCrawlingStatus('크롤링 중지 중...');
      await invoke('request_graceful_shutdown', { request: { timeout_ms: 15000 } });
      setIsCrawlingActive(false);
      setCrawlingStatus('크롤링 중지됨');
    } catch (error) {
      console.error('크롤링 중지 실패:', error);
      setCrawlingStatus('크롤링 중지 실패');
    }
  };

  // 크롤링 상태 리셋
  const resetCrawling = () => {
    setIsCrawlingActive(false);
    setCrawlingStatus('대기 중');
    setActiveBatches(0);
    setTotalPages(0);
    setTotalProducts(0);
    setErrorCount(0);
  };

  // 컴포넌트 마운트
  // 이벤트 집계: actor-batch-report / actor-session-report 에서 제품 카운트 추출
  createEffect(() => {
    const evs = events();
    if (!evs.length) return;
    const latest = evs[evs.length - 1];
    // raw payload를 전체적으로 알 수 없으므로 window에서 마지막 보고 관련 이벤트들 스캔
    // 간단히 이름을 통해 필터링
    const batchReports = evs.filter(e => e.rawName === 'actor-batch-report');
    const sessionReports = evs.filter(e => e.rawName === 'actor-session-report');
    // 페이지 수 추정: batch-report pages_success 또는 pages_total 값이 payload에 포함되지만 visualization 이벤트에는 없으므로 backend 확장 전까지 기존 totalPages 유지
    // 제품 카운트는 session-report 최종값을 우선, 없으면 batch 누적(추후 raw payload 확장 시 개선)
    // Tauri emitter가 구조 flatten 하므로 window.electron? 접근 없이 invoke 대신 lightweight listen 재구현 필요 → 여기서는 전송된 enriched 객체를 얻지 못하므로 개선 TODO
    // 임시 전략: backend가 추후 actor-session-report payload에 products_inserted/products_updated 표시하는지 확인 후 직접 update
  });

  // Tauri low-level listener로 실제 payload 접근 (직접 수신) - 필요한 필드만 추출
  onMount(async () => {
    const un1 = await listen<any>('actor-batch-report', (e) => {
      const p = e.payload as any;
      if (typeof p?.products_inserted === 'number') {
        setProductsInserted((prev: number) => prev + p.products_inserted);
      }
      if (typeof p?.products_updated === 'number') {
        setProductsUpdated((prev: number) => prev + p.products_updated);
      }
      if (p?.batch_id) setLastBatchId(p.batch_id);
      if (typeof p?.pages_success === 'number') {
        setTotalPages((prev: number) => prev + p.pages_success);
      }
    });
    const un2 = await listen<any>('actor-session-report', (e) => {
      const p = e.payload as any;
      // 세션 리포트는 누적 값이므로 그대로 덮어씀
      if (typeof p?.products_inserted === 'number') setProductsInserted(p.products_inserted);
      if (typeof p?.products_updated === 'number') setProductsUpdated(p.products_updated);
      if (typeof p?.total_pages === 'number') setTotalPages(p.total_pages);
    });
    const un3 = await listen<any>('actor-stage-item-completed', (e) => {
      // DataSaving StageItemCompleted에서 collected_data 내 삽입/업데이트 JSON을 후처리하려면 backend가 수치 직접 전파 전까지 parsing 가능
      const p = e.payload as any;
      if (p?.item_type === 'Url' && p?.collected_data && typeof p.collected_data === 'string') {
        // heuristics: ignore (not metrics JSON)
        return;
      }
      if (p?.collected_data && typeof p.collected_data === 'string' && p.collected_data.startsWith('{')) {
        try {
          const data = JSON.parse(p.collected_data);
          if (typeof data.products_inserted === 'number') {
            setProductsInserted((prev: number) => prev + data.products_inserted);
          }
          if (typeof data.products_updated === 'number') {
            setProductsUpdated((prev: number) => prev + data.products_updated);
          }
        } catch (_) {/* ignore parse errors */}
      }
    });
    // Lifecycle listeners
    const un4 = await listen<any>('actor-page-lifecycle', (e) => {
      const p = e.payload as any;
      if (p?.status === 'fetch_started') setPagesFetchStarted(v => v + 1);
      if (p?.status === 'fetch_completed') setPagesFetchCompleted(v => v + 1);
    });
    const un5 = await listen<any>('actor-product-lifecycle', (e) => {
      const p = e.payload as any;
      if (p?.status === 'fetch_started') setProductsFetchStarted(v => v + 1);
      if (p?.status === 'fetch_completed') setProductsFetchCompleted(v => v + 1);
    });
    // cleanup
    onCleanup(() => { un1(); un2(); un3(); un4(); un5(); });
  });

  // 컴포넌트 언마운트
  onCleanup(() => {
    // 필요시 정리 작업
  });

  return (
    <div class="h-full flex flex-col">
      <div class="flex-shrink-0 border-b border-gray-200 p-4">
        <div class="flex items-center justify-between mb-4">
          <div>
            <h2 class="text-xl font-semibold text-gray-800">Live Production Line</h2>
            <p class="text-sm text-gray-600 mt-1">
              실시간 크롤링 프로세스를 3D 그래프로 시각화합니다
            </p>
          </div>
          
          {/* 크롤링 제어 버튼 */}
          <div class="flex items-center space-x-3">
            <button
              onClick={startCrawling}
              disabled={isCrawlingActive()}
              class="px-6 py-2 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:bg-gray-400 disabled:cursor-not-allowed flex items-center space-x-2"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h1m4 0h1m-6 4h1m4 0h1m-6-8h1m4 0h1M9 6h1m4 0h1M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
              </svg>
              <span>{isCrawlingActive() ? '크롤링 실행 중...' : '크롤링 시작'}</span>
            </button>
            
            <button
              onClick={stopCrawling}
              disabled={!isCrawlingActive()}
              class="px-6 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:bg-gray-400 disabled:cursor-not-allowed flex items-center space-x-2"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 10h6v4H9z" />
              </svg>
              <span>크롤링 중지</span>
            </button>
            
            <button
              onClick={resetCrawling}
              disabled={isCrawlingActive()}
              class="px-4 py-2 bg-gray-500 text-white rounded-lg hover:bg-gray-600 disabled:bg-gray-400 disabled:cursor-not-allowed"
            >
              리셋
            </button>
          </div>
        </div>
        
        {/* 실시간 상태 표시 */}
          <div class="grid grid-cols-9 gap-4 text-sm">
          <div class="bg-blue-50 border border-blue-200 rounded-lg p-3">
            <div class="text-blue-600 font-medium">상태</div>
            <div class="text-blue-800 font-semibold">{crawlingStatus()}</div>
          </div>
          
          <div class="bg-green-50 border border-green-200 rounded-lg p-3">
            <div class="text-green-600 font-medium">활성 배치</div>
            <div class="text-green-800 font-semibold">{activeBatches()}</div>
          </div>
          
          <div class="bg-purple-50 border border-purple-200 rounded-lg p-3">
            <div class="text-purple-600 font-medium">수집된 페이지</div>
            <div class="text-purple-800 font-semibold">{totalPages().toLocaleString()}</div>
          </div>
          
          <div class="bg-indigo-50 border border-indigo-200 rounded-lg p-3">
            <div class="text-indigo-600 font-medium">신규 제품</div>
            <div class="text-indigo-800 font-semibold">{productsInserted().toLocaleString()}</div>
          </div>
          <div class="bg-indigo-50 border border-indigo-200 rounded-lg p-3">
            <div class="text-indigo-600 font-medium">업데이트 제품</div>
            <div class="text-indigo-800 font-semibold">{productsUpdated().toLocaleString()}</div>
          </div>
          <div class="bg-indigo-50 border border-indigo-200 rounded-lg p-3">
            <div class="text-indigo-600 font-medium">총 제품 영향</div>
            <div class="text-indigo-800 font-semibold">{(productsInserted()+productsUpdated()).toLocaleString()}</div>
          </div>
          
          <div class="bg-red-50 border border-red-200 rounded-lg p-3">
            <div class="text-red-600 font-medium">에러 수</div>
            <div class="text-red-800 font-semibold">{errorCount()}</div>
          </div>
          <div class="bg-yellow-50 border border-yellow-200 rounded-lg p-3">
            <div class="text-yellow-600 font-medium">페이지 진행</div>
            <div class="text-yellow-800 font-semibold">{pagesFetchCompleted()}/{pagesFetchStarted()}</div>
          </div>
          <div class="bg-yellow-50 border border-yellow-200 rounded-lg p-3">
            <div class="text-yellow-600 font-medium">제품 진행</div>
            <div class="text-yellow-800 font-semibold">{productsFetchCompleted()}/{productsFetchStarted()}</div>
          </div>
        </div>
      </div>
      
      <div class="p-4 border-b border-gray-200 bg-gray-50/40 dark:bg-neutral-800/30">
        {/* Actor 시스템 세션 진행 & Detail 애니메이션 (공유 패널) */}
        <SessionStatusPanel />
      </div>
      <div class="flex-1 overflow-hidden">
        <CrawlingProgressDashboard />
      </div>
    </div>
  );
};
