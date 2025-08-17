import { createSignal, Show, onMount, onCleanup, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
// Types are relaxed locally to avoid tight coupling during integration
import { tauriApi } from '../../services/tauri-api';
import EventConsole from '../dev/EventConsole';

export default function CrawlingEngineTabSimple() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [crawlingRange, setCrawlingRange] = createSignal<any | null>(null);
  const [statusMessage, setStatusMessage] = createSignal<string>('크롤링 준비 완료');
  const [logs, setLogs] = createSignal<string[]>([]);
  const [showConsole, setShowConsole] = createSignal<boolean>(true);
  const [isValidating, setIsValidating] = createSignal(false);
  const [isSyncing, setIsSyncing] = createSignal(false);
  const [syncRanges, setSyncRanges] = createSignal<string>('');
  const [validationPages, setValidationPages] = createSignal<number | ''>('');
  // Batch progress (best-effort estimation)
  const [batchInfo, setBatchInfo] = createSignal<{ current: number; totalEstimated?: number; batchId?: string; pagesInBatch?: number }>({ current: 0 });
  // Lightweight runtime monitor for Stage 1 (list pages) and Stage 2 (detail)
  const [pageStats, setPageStats] = createSignal({ started: 0, completed: 0, failed: 0, retried: 0, totalEstimated: 0, inflight: 0 });
  const [detailStats, setDetailStats] = createSignal({ started: 0, completed: 0, failed: 0, retried: 0, inflight: 0 });
  // Stage 1 unique tracking (per page) to avoid double counting and track retries
  const pageSeen = new Set<number>();
  const pageCompleted = new Set<number>();
  const pageFailedFinal = new Set<number>();
  const pageAttempts = new Map<number, number>();
  // Stage 2 unique tracking to avoid double counting due to retries or re-queues
  const detailSeen = new Set<string>(); // detail_ids seen at least once
  const detailCompleted = new Set<string>(); // detail_ids completed once
  const detailFailedFinal = new Set<string>(); // detail_ids that finally failed
  const detailAttempts = new Map<string, number>(); // detail_id -> attempts
  const [downshiftInfo, setDownshiftInfo] = createSignal<null | { newLimit?: number; reason?: string }>(null);
  // Stage 3: Validation stats (lightweight)
  const [validationStats, setValidationStats] = createSignal({
    started: false,
    completed: false,
    targetPages: 0,
    pagesScanned: 0,
    divergences: 0,
    anomalies: 0,
    productsChecked: 0,
    lastPage: null as number | null,
    lastAssignedStart: null as number | null,
    lastAssignedEnd: null as number | null,
  });
  // Animation toggles
  const [validationPulse, setValidationPulse] = createSignal(false);
  const [persistFlash, setPersistFlash] = createSignal(false);
  // Stage 4: DB snapshot (latest observed)
  const [dbSnapshot, setDbSnapshot] = createSignal<{
    total?: number;
    minPage?: number | null;
    maxPage?: number | null;
    inserted?: number;
    updated?: number;
  }>({});
  // Stage 5: Persist (grouped snapshot)
  const [persistStats, setPersistStats] = createSignal<{
    attempted: number;
    succeeded: number;
    failed: number;
    duplicates: number;
    durationMs: number;
  }>({ attempted: 0, succeeded: 0, failed: 0, duplicates: 0, durationMs: 0 });
  // Stage 4: DB snapshot animation toggle
  const [dbFlash, setDbFlash] = createSignal(false);
  // Global effects toggle
  const [effectsOn, setEffectsOn] = createSignal(true);

  // 크롤링 범위 계산
  const calculateCrawlingRange = async () => {
    addLog('📊 크롤링 범위 계산 중...');
    
    try {
      // 먼저 사이트 상태를 확인해서 실제 total_pages를 얻습니다
      addLog('🌐 사이트 상태 확인 중...');
      const siteStatusResponse = await invoke<any>('check_advanced_site_status');
      
      if (!siteStatusResponse?.data) {
        throw new Error('사이트 상태 확인 실패');
      }
      
      const siteStatus = siteStatusResponse.data;
      addLog(`✅ 사이트 상태 확인 완료: ${siteStatus.total_pages}페이지, 마지막 페이지 ${siteStatus.products_on_last_page}개 제품`);
      
  const request: any = {
        total_pages_on_site: siteStatus.total_pages,
        products_on_last_page: siteStatus.products_on_last_page,
      };
      
      addLog(`📋 크롤링 범위 계산 요청: ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품`);
      
  const response = await invoke<any>('calculate_crawling_range', { request });
      setCrawlingRange(response);
      
      const startPage = response.range?.[0] || 0;
      const endPage = response.range?.[1] || 0;
      addLog(`📊 크롤링 범위 계산 완료: ${startPage} → ${endPage}`);
    } catch (error) {
      console.error('크롤링 범위 계산 실패:', error);
      addLog(`❌ 크롤링 범위 계산 실패: ${error}`);
    }
  };  
  
  // 통합 Actor 기반 크롤링 (경량 설정)
  const startLightUnified = async () => {
    if (isRunning()) return;

    setIsRunning(true);
    setStatusMessage('🎭 통합 파이프라인(라이트) 시작 중...');
    addLog('🎭 통합 파이프라인 시작 (라이트 설정)');

    try {
      const res = await tauriApi.startUnifiedCrawling({
        mode: 'advanced',
        overrideConcurrency: 8,
        overrideBatchSize: 3,
        delayMs: 100,
      });
      addLog(`✅ 통합 파이프라인(라이트) 세션 시작: ${JSON.stringify(res)}`);
      setStatusMessage('🎭 통합 파이프라인 실행 중 (라이트)');
    } catch (error) {
      console.error('통합 파이프라인(라이트) 시작 실패:', error);
      addLog(`❌ 통합 파이프라인(라이트) 시작 실패: ${error}`);
      setStatusMessage('크롤링 실패');
      setIsRunning(false);
    }
  };

  // 통합 Actor 기반 크롤링 (하이 설정)
  const startUnifiedAdvanced = async () => {
    if (isRunning()) return;

    setIsRunning(true);
    setStatusMessage('🎭 통합 파이프라인(하이) 시작 중...');
    addLog('🎭 통합 파이프라인 시작 (하이 설정)');

    try {
      const res = await tauriApi.startUnifiedCrawling({
        mode: 'advanced',
        overrideConcurrency: 64,
        overrideBatchSize: 3,
        delayMs: 100,
      });
      addLog(`✅ 통합 파이프라인(하이) 세션 시작: ${JSON.stringify(res)}`);
      setStatusMessage('🎭 통합 파이프라인 실행 중 (하이)');
    } catch (error) {
      console.error('통합 파이프라인(하이) 시작 실패:', error);
      addLog(`❌ 통합 파이프라인(하이) 시작 실패: ${error}`);
      setStatusMessage('크롤링 실패');
      setIsRunning(false);
    }
  };

  // 스마트 크롤링 시작 (Phase 1: 설정 파일 기반)
  const startSmartCrawling = async () => {
    if (isRunning()) return;
    
    setIsRunning(true);
    setStatusMessage('크롤링 시작 중...');
    addLog('🚀 스마트 크롤링 시작');

    try {
      const result = await invoke('start_smart_crawling');
      addLog(`✅ 크롤링 세션 시작: ${JSON.stringify(result)}`);
      setStatusMessage('크롤링 실행 중');
      
      // 실제 구현에서는 여기에 크롤링 진행 상황 모니터링 추가
      
    } catch (error) {
      console.error('크롤링 시작 실패:', error);
      addLog(`❌ 크롤링 시작 실패: ${error}`);
      setStatusMessage('크롤링 준비 완료');
      setIsRunning(false);
    }
  };

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [`[${timestamp}] ${message}`, ...prev.slice(0, 19)]);
  };

  // Validation run
  const startValidationRun = async () => {
    if (isValidating()) return;
    setIsValidating(true);
    addLog('🧪 Validation 시작');
    try {
      const res = await tauriApi.startValidation({
        scanPages: typeof validationPages() === 'number' ? (validationPages() as number) : undefined,
      });
      addLog(`✅ Validation 요청 완료: ${JSON.stringify(res)}`);
    } catch (e) {
      console.error(e);
      addLog(`❌ Validation 실패: ${e}`);
    } finally {
      setIsValidating(false);
    }
  };

  // Sync run
  const startSyncRun = async () => {
    if (isSyncing()) return;
    setIsSyncing(true);
    const ranges = syncRanges().trim();
    addLog(`🔄 Sync 시작 ${ranges ? `(범위: ${ranges})` : '(자동 범위)'}`);
    try {
      const res = ranges
        ? await tauriApi.startPartialSync(ranges)
        : await tauriApi.startRepairSync();
      addLog(`✅ Sync 완료: ${JSON.stringify(res)}`);
    } catch (e) {
      console.error(e);
      addLog(`❌ Sync 실패: ${e}`);
    } finally {
      setIsSyncing(false);
    }
  };

  onMount(() => {
    calculateCrawlingRange();

    const unsubs: Array<() => void> = [];

    // Listen to unified Actor session lifecycle to toggle buttons/status
    tauriApi
      .subscribeToActorBridgeEvents((name, payload) => {
        if (name === 'actor-session-started') {
          setIsRunning(true);
          setStatusMessage('크롤링 실행 중 (세션 시작)');
          addLog('🎬 세션 시작');
          // reset runtime stats
          setPageStats({ started: 0, completed: 0, failed: 0, retried: 0, totalEstimated: 0, inflight: 0 });
          setDetailStats({ started: 0, completed: 0, failed: 0, retried: 0, inflight: 0 });
          setBatchInfo({ current: 0 });
          // clear Stage 1 tracking
          pageSeen.clear();
          pageCompleted.clear();
          pageFailedFinal.clear();
          pageAttempts.clear();
          // clear unique tracking as a new session begins
          detailSeen.clear();
          detailCompleted.clear();
          detailFailedFinal.clear();
          detailAttempts.clear();
          setDownshiftInfo(null);
          setValidationStats({ started: false, completed: false, targetPages: 0, pagesScanned: 0, divergences: 0, anomalies: 0, productsChecked: 0, lastPage: null, lastAssignedStart: null, lastAssignedEnd: null });
          setDbSnapshot({});
          setPersistStats({ attempted: 0, succeeded: 0, failed: 0, duplicates: 0, durationMs: 0 });
        }
        if (name === 'actor-session-completed') {
          setIsRunning(false);
          setStatusMessage('크롤링 완료');
          addLog('🏁 세션 완료');
          setBatchInfo(prev => ({ ...prev }));
        }
        if (name === 'actor-session-failed') {
          setIsRunning(false);
          setStatusMessage('크롤링 실패');
          addLog(`❌ 세션 실패: ${JSON.stringify(payload)}`);
          setBatchInfo(prev => ({ ...prev }));
        }
        if (name === 'actor-session-timeout' || name === 'actor-shutdown-completed') {
          setIsRunning(false);
          setStatusMessage('크롤링 종료');
          addLog('🛑 세션 종료');
          setBatchInfo(prev => ({ ...prev }));
        }

        // Estimate totals from batch starts (pages in batch)
        if (name === 'actor-batch-started') {
          const t = (payload?.pages_in_batch ?? payload?.pages ?? payload?.items_total ?? payload?.pages_count ?? 0) as number;
          if (typeof t === 'number' && t > 0) {
            setPageStats(prev => ({ ...prev, totalEstimated: prev.totalEstimated + t }));
          }
          // Update batch info
          setBatchInfo(prev => {
            const current = (prev.current || 0) + 1;
            let totalEstimated = prev.totalEstimated;
            const pagesTotal = (crawlingRange()?.crawling_info?.pages_to_crawl as number) || 0;
            const batchSizeGuess = Number(t) || Number(payload?.batch_size ?? 0) || 0;
            if (!totalEstimated && pagesTotal > 0 && batchSizeGuess > 0) {
              totalEstimated = Math.max(1, Math.ceil(pagesTotal / batchSizeGuess));
            }
            return { current, totalEstimated, batchId: payload?.batch_id ?? prev.batchId, pagesInBatch: t || prev.pagesInBatch };
          });
        }
        if (name === 'actor-batch-completed') {
          // Keep current count; nothing to do for now.
        }
        // Stage 1 (list page) itemized with de-duplication and retry tracking
  if (name === 'actor-page-task-started') {
          const pageNum = Number(payload?.page ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          const prevAttempts = pageAttempts.get(pageNum) ?? 0;
          pageAttempts.set(pageNum, prevAttempts + 1);
          if (!pageSeen.has(pageNum)) {
            pageSeen.add(pageNum);
            setPageStats(prev => {
              const started = pageSeen.size; // unique pages
              const inflight = Math.max(0, started - (prev.completed + prev.failed));
              return { ...prev, started, inflight };
            });
          }
        }
        if (name === 'actor-page-task-completed') {
          const pageNum = Number(payload?.page ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          if (!pageCompleted.has(pageNum)) pageCompleted.add(pageNum);
          if (!pageSeen.has(pageNum)) pageSeen.add(pageNum);
          setPageStats(prev => {
            const started = pageSeen.size;
            const completed = pageCompleted.size;
            const inflight = Math.max(0, started - (completed + prev.failed));
            return { ...prev, started, completed, inflight };
          });
        }
    if (name === 'actor-page-task-failed') {
          const pageNum = Number(payload?.page ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          const final = Boolean(payload?.final_failure);
          const prevAttempts = pageAttempts.get(pageNum) ?? 0;
          pageAttempts.set(pageNum, prevAttempts + 1);
          if (!pageSeen.has(pageNum)) pageSeen.add(pageNum);
          if (final) {
            pageFailedFinal.add(pageNum);
          } else {
      setPageStats(prev => ({ ...prev, retried: prev.retried + 1 }));
          }
          setPageStats(prev => {
            const started = pageSeen.size;
            const failed = pageFailedFinal.size;
            const inflight = Math.max(0, started - (prev.completed + failed));
            return { ...prev, started, failed, inflight };
          });
        }
        // Stage 2 (product detail) itemized - deduplicate by detail_id and track retries
  if (name === 'actor-detail-task-started') {
          const isBatchScope = (payload?.batch_id != null) || (payload?.scope === 'batch');
          if (!isBatchScope) return; // ignore session-scoped/simulated events
          const id = String(payload?.detail_id ?? '');
          if (!id) return;
          const prevAttempts = detailAttempts.get(id) ?? 0;
          detailAttempts.set(id, prevAttempts + 1);
          if (!detailSeen.has(id)) {
            detailSeen.add(id);
            setDetailStats(prev => {
              const started = detailSeen.size; // unique
              const inflight = Math.max(0, started - (prev.completed + prev.failed));
              return { ...prev, started, inflight };
            });
          }
        }
        if (name === 'actor-detail-task-completed') {
          const isBatchScope = (payload?.batch_id != null) || (payload?.scope === 'batch');
          if (!isBatchScope) return; // ignore session-scoped/simulated events
          const id = String(payload?.detail_id ?? '');
          if (!id) return;
          if (!detailCompleted.has(id)) {
            detailCompleted.add(id);
            // ensure it is counted as started at least once
            if (!detailSeen.has(id)) detailSeen.add(id);
          }
          setDetailStats(prev => {
            const started = detailSeen.size;
            const completed = detailCompleted.size;
            const inflight = Math.max(0, started - (completed + prev.failed));
            return { ...prev, started, completed, inflight };
          });
        }
    if (name === 'actor-detail-task-failed') {
          const isBatchScope = (payload?.batch_id != null) || (payload?.scope === 'batch');
          if (!isBatchScope) return; // ignore session-scoped/simulated events
          const id = String(payload?.detail_id ?? '');
          if (!id) return;
          const final = Boolean(payload?.final_failure);
          // count attempts
          const prevAttempts = detailAttempts.get(id) ?? 0;
          detailAttempts.set(id, prevAttempts + 1);
          if (final && !detailFailedFinal.has(id)) {
            detailFailedFinal.add(id);
          } else {
      // non-final failure -> retry will happen
      setDetailStats(prev => ({ ...prev, retried: prev.retried + 1 }));
          }
          // ensure started is tracked
          if (!detailSeen.has(id)) detailSeen.add(id);
          setDetailStats(prev => {
            const started = detailSeen.size;
            const failed = detailFailedFinal.size;
            const inflight = Math.max(0, started - (prev.completed + failed));
            return { ...prev, started, failed, inflight };
          });
        }
        if (name === 'actor-detail-concurrency-downshifted') {
          setDownshiftInfo({ newLimit: payload?.new_limit, reason: payload?.reason });
        }

        // Stage 3 (Validation) events
        if (name === 'actor-validation-started') {
          const target = Number(payload?.scan_pages ?? 0) || 0;
          setValidationStats({ started: true, completed: false, targetPages: target, pagesScanned: 0, divergences: 0, anomalies: 0, productsChecked: 0, lastPage: null, lastAssignedStart: null, lastAssignedEnd: null });
        }
        if (name === 'actor-validation-page-scanned') {
          setValidationStats(prev => ({
            ...prev,
            pagesScanned: prev.pagesScanned + 1,
            // Optional: we can accumulate products_found into productsChecked
            productsChecked: prev.productsChecked + (Number(payload?.products_found ?? 0) || 0),
            lastPage: Number(payload?.physical_page ?? prev.lastPage ?? 0) || prev.lastPage,
            lastAssignedStart: Number(payload?.assigned_start_offset ?? prev.lastAssignedStart ?? 0) || prev.lastAssignedStart,
            lastAssignedEnd: Number(payload?.assigned_end_offset ?? prev.lastAssignedEnd ?? 0) || prev.lastAssignedEnd,
          }));
          // trigger subtle pulse animation
          if (effectsOn()) {
            setValidationPulse(true);
            setTimeout(() => setValidationPulse(false), 300);
          }
        }
        if (name === 'actor-validation-divergence') {
          setValidationStats(prev => ({ ...prev, divergences: prev.divergences + 1 }));
        }
        if (name === 'actor-validation-anomaly') {
          setValidationStats(prev => ({ ...prev, anomalies: prev.anomalies + 1 }));
        }
        if (name === 'actor-validation-completed') {
          setValidationStats(prev => ({
            ...prev,
            completed: true,
            pagesScanned: Number(payload?.pages_scanned ?? prev.pagesScanned) || prev.pagesScanned,
            productsChecked: Number(payload?.products_checked ?? prev.productsChecked) || prev.productsChecked,
            divergences: Number(payload?.divergences ?? prev.divergences) || prev.divergences,
            anomalies: Number(payload?.anomalies ?? prev.anomalies) || prev.anomalies,
          }));
        }

        // Stage 4 (DB) snapshots and session summary
        if (name === 'actor-database-stats') {
          setDbSnapshot(prev => ({
            ...prev,
            total: Number(payload?.total_product_details ?? prev.total ?? 0) || prev.total,
            minPage: payload?.min_page ?? prev.minPage ?? null,
            maxPage: payload?.max_page ?? prev.maxPage ?? null,
          }));
          if (effectsOn()) {
            setDbFlash(true);
            setTimeout(() => setDbFlash(false), 500);
          }
        }
        if (name === 'actor-session-report') {
          setDbSnapshot(prev => ({
            ...prev,
            inserted: Number(payload?.products_inserted ?? prev.inserted ?? 0) || prev.inserted,
            updated: Number(payload?.products_updated ?? prev.updated ?? 0) || prev.updated,
          }));
        }
        // Stage 5 (Persist) grouped lifecycle snapshot
        if (name === 'actor-product-lifecycle-group' && (payload?.phase === 'persist')) {
          const attempted = Number(payload?.group_size ?? 0) || 0;
          const succeeded = Number(payload?.succeeded ?? 0) || 0;
          const failed = Number(payload?.failed ?? 0) || 0;
          const duplicates = Number(payload?.duplicates ?? 0) || 0;
          const durationMs = Number(payload?.duration_ms ?? 0) || 0;
          setPersistStats({ attempted, succeeded, failed, duplicates, durationMs });
          // flash Stage 5 panel
          if (effectsOn()) {
            setPersistFlash(true);
            setTimeout(() => setPersistFlash(false), 500);
          }
        }
      })
      .then((un) => unsubs.push(un))
      .catch((e) => console.warn('[CrawlingEngineTabSimple] actor bridge subscribe failed', e));

    // Legacy completion/stopped fallbacks
    tauriApi
      .subscribeToCompletion(() => {
        setIsRunning(false);
        setStatusMessage('크롤링 완료');
        addLog('🏁 완료 이벤트 수신');
      })
      .then((un) => unsubs.push(un))
      .catch(() => {});

    tauriApi
      .subscribeToCrawlingStopped(() => {
        setIsRunning(false);
        setStatusMessage('크롤링 중지됨');
        addLog('⏹️ 중지 이벤트 수신');
      })
      .then((un) => unsubs.push(un))
      .catch(() => {});

    onCleanup(() => {
      unsubs.forEach((u) => u());
    });
  });

  return (
    <div class="w-full max-w-6xl mx-auto">
      <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6 mb-6">
        <h1 class="text-2xl font-bold text-gray-900 mb-2">🤖 스마트 크롤링 엔진</h1>
        <p class="text-gray-600 text-sm mb-4">
          설정 파일 기반 자동 크롤링 시스템 - 별도 설정 전송 없이 즉시 시작
        </p>

        {/* 상태 표시 */}
        <div class="mb-6">
          <div class={`px-4 py-3 rounded-lg border ${isRunning() 
            ? 'bg-blue-50 border-blue-200 text-blue-700' 
            : 'bg-green-50 border-green-200 text-green-700'
          }`}>
            <div class="flex items-center space-x-2">
              <span>{isRunning() ? '🔄' : '✅'}</span>
              <span class="font-medium">{statusMessage()}</span>
              <Show when={isRunning() && (batchInfo().current > 0)}>
                <span class="text-xs ml-2 px-2 py-0.5 rounded bg-blue-100 text-blue-700">
                  배치 {batchInfo().current}{batchInfo().totalEstimated ? `/${batchInfo().totalEstimated}` : ''}
                </span>
              </Show>
              <Show when={isRunning() && batchInfo().batchId}>
                <span class="text-[10px] ml-1 text-gray-500">({batchInfo().batchId})</span>
              </Show>
            </div>
          </div>
        </div>

        {/* Stage1/Stage2 Runtime Monitor */}
  <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div class={`bg-white rounded-lg border p-4 ${validationPulse() ? 'pulse-once' : ''}`}>
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 1: 제품 목록 수집</h3>
              <span class="text-xs text-gray-500">
                {(() => {
                  const cr = crawlingRange();
                  const fallback = (cr?.crawling_info?.pages_to_crawl ?? (((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1) || 0)) as number;
                  const est = pageStats().totalEstimated || fallback || 0;
                  return est > 0 ? `예상 ${est}p` : '';
                })()}
              </span>
            </div>
            <div class="grid grid-cols-5 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600">{pageStats().started}</div>
                <div class="text-xs text-gray-600">시작</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{pageStats().completed}</div>
                <div class="text-xs text-gray-600">완료</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">{pageStats().inflight}</div>
                <div class="text-xs text-gray-600">진행중</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">{pageStats().failed}</div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-violet-50 rounded p-2">
                <div class="text-xl font-bold text-violet-600">{pageStats().retried}</div>
                <div class="text-xs text-gray-600">재시도</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div class="h-2 rounded-full bg-blue-500 transition-all" style={{ width: `${(() => {
                const cr = crawlingRange();
                const fallback = (cr?.crawling_info?.pages_to_crawl ?? (((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1) || 0)) as number;
                const denom = pageStats().totalEstimated || fallback || 0;
                return denom > 0 ? Math.min(100, (pageStats().completed / denom) * 100) : 0;
              })()}%` }}></div>
            </div>
          </div>

          <div class="bg-white rounded-lg border p-4">
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 2: 세부 정보 수집</h3>
              <Show when={!!downshiftInfo()}>
                <span class="text-[10px] px-2 py-1 bg-yellow-100 text-yellow-700 rounded" title={downshiftInfo()?.reason || ''}>↓ 제한 {downshiftInfo()?.newLimit ?? '-'}
                </span>
              </Show>
              <span class="text-xs text-gray-500">
                {(() => {
                  const est = (crawlingRange()?.crawling_info?.estimated_new_products ?? 0) as number;
                  return est > 0 ? `예상 ${est}` : '';
                })()}
              </span>
            </div>
            <div class="grid grid-cols-5 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600">{detailStats().started}</div>
                <div class="text-xs text-gray-600">시작</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{detailStats().completed}</div>
                <div class="text-xs text-gray-600">완료</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">{detailStats().inflight}</div>
                <div class="text-xs text-gray-600">진행중</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">{detailStats().failed}</div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-violet-50 rounded p-2">
                <div class="text-xl font-bold text-violet-600">{detailStats().retried}</div>
                <div class="text-xs text-gray-600">재시도</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div class="h-2 rounded-full bg-purple-500 transition-all" style={{ width: `${(() => {
                const denom = (crawlingRange()?.crawling_info?.estimated_new_products as number) || detailStats().started || 0;
                return denom > 0 ? Math.min(100, (detailStats().completed / denom) * 100) : 0;
              })()}%` }}></div>
            </div>
          </div>
        </div>

  {/* Stage3/Stage4/Stage5 Mini Panels */}
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          {/* Stage 3: Validation */}
          <div class="bg-white rounded-lg border p-4">
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 3: Validation</h3>
              <span class="text-xs text-gray-500">
                {validationStats().started ? (validationStats().completed ? '완료' : '진행 중') : '대기'}
              </span>
            </div>
            <div class="grid grid-cols-4 gap-2 text-center">
              <div class="bg-indigo-50 rounded p-2">
                <div class="text-xl font-bold text-indigo-600">{validationStats().targetPages}</div>
                <div class="text-xs text-gray-600">대상 페이지</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{validationStats().pagesScanned}</div>
                <div class="text-xs text-gray-600">스캔</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">{validationStats().divergences}</div>
                <div class="text-xs text-gray-600">불일치</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">{validationStats().anomalies}</div>
                <div class="text-xs text-gray-600">이상</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div class="h-2 rounded-full bg-indigo-500 transition-all" style={{ width: `${(() => {
                const t = validationStats().targetPages || 0;
                const s = validationStats().pagesScanned || 0;
                return t > 0 ? Math.min(100, (s / t) * 100) : 0;
              })()}%` }}></div>
            </div>
            <Show when={validationStats().lastPage != null}>
              <div class="mt-2 text-[11px] text-gray-500">
                최근 스캔: 페이지 {validationStats().lastPage} (오프셋 {validationStats().lastAssignedStart ?? '-'}–{validationStats().lastAssignedEnd ?? '-'})
              </div>
            </Show>
          </div>
          {/* Stage 4: DB Snapshot */}
          <div class={`bg-white rounded-lg border p-4 ${dbFlash() && effectsOn() ? 'flash-db' : ''}`}>
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 4: DB 저장 스냅샷</h3>
              <span class="text-xs text-gray-500">최근 보고 기준</span>
            </div>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-2 text-center">
              <div class="bg-sky-50 rounded p-2">
                <div class="text-xl font-bold text-sky-600">{dbSnapshot().total ?? '-'}</div>
                <div class="text-xs text-gray-600">총 상세 수</div>
              </div>
              <div class="bg-purple-50 rounded p-2">
                <div class="text-xl font-bold text-purple-600">{dbSnapshot().minPage ?? '-'}</div>
                <div class="text-xs text-gray-600">DB 최소 페이지</div>
              </div>
              <div class="bg-purple-50 rounded p-2">
                <div class="text-xl font-bold text-purple-600">{dbSnapshot().maxPage ?? '-'}</div>
                <div class="text-xs text-gray-600">DB 최대 페이지</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{dbSnapshot().inserted ?? 0}/{dbSnapshot().updated ?? 0}</div>
                <div class="text-xs text-gray-600">삽입/업데이트(세션)</div>
              </div>
            </div>
          </div>

          {/* Stage 5: Persist 요약 */}
          <div class={`bg-white rounded-lg border p-4 ${persistFlash() && effectsOn() ? 'flash-save' : ''}`}>
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 5: 저장 요약</h3>
              <span class="text-xs text-gray-500">그룹 이벤트</span>
            </div>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600">{persistStats().attempted}</div>
                <div class="text-xs text-gray-600">시도</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{persistStats().succeeded}</div>
                <div class="text-xs text-gray-600">성공</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">{persistStats().failed}</div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">{persistStats().duplicates}</div>
                <div class="text-xs text-gray-600">중복</div>
              </div>
            </div>
            <div class="mt-2 text-xs text-gray-500">소요 시간: {persistStats().durationMs}ms</div>
          </div>
        </div>

        {/* 크롤링 범위 정보 */}
        <Show when={crawlingRange()}>
          <div class="bg-gray-50 rounded-lg p-4 mb-6">
            <h3 class="text-lg font-semibold text-gray-900 mb-3">📊 계산된 크롤링 범위</h3>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
              <div class="text-center">
                <div class="text-2xl font-bold text-blue-600">{crawlingRange()?.range?.[0] || 0}</div>
                <div class="text-sm text-gray-600">시작 페이지</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-green-600">{crawlingRange()?.range?.[1] || 0}</div>
                <div class="text-sm text-gray-600">종료 페이지</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-purple-600">
                  {crawlingRange()?.progress?.total_products || 0}
                </div>
                <div class="text-sm text-gray-600">총 제품 수</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-orange-600">
                  {crawlingRange()?.progress?.progress_percentage.toFixed(1) || 0}%
                </div>
                <div class="text-sm text-gray-600">커버리지</div>
              </div>
            </div>

            {/* 사이트 정보 섹션 */}
            <div class="border-t pt-4">
              <h4 class="text-md font-medium text-gray-800 mb-3">🌐 사이트 정보</h4>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-blue-600">{crawlingRange()?.site_info?.total_pages || 0}</div>
                  <div class="text-xs text-gray-600">사이트 총 페이지 수</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-green-600">{crawlingRange()?.site_info?.products_on_last_page || 0}</div>
                  <div class="text-xs text-gray-600">마지막 페이지 제품 수</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-purple-600">{crawlingRange()?.site_info?.estimated_total_products || 0}</div>
                  <div class="text-xs text-gray-600">추정 총 제품 수</div>
                </div>
              </div>
            </div>

            {/* 로컬 DB 정보 섹션 */}
            <div class="border-t pt-4">
              <h4 class="text-md font-medium text-gray-800 mb-3">💾 로컬 DB 정보</h4>
              <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-4">
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-indigo-600">{crawlingRange()?.local_db_info?.total_saved_products || 0}</div>
                  <div class="text-xs text-gray-600">수집한 제품 수</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-teal-600">{crawlingRange()?.local_db_info?.last_crawled_page || 'N/A'}</div>
                  <div class="text-xs text-gray-600">마지막 크롤링 페이지</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-pink-600">{crawlingRange()?.local_db_info?.coverage_percentage?.toFixed(1) || 0}%</div>
                  <div class="text-xs text-gray-600">DB 커버리지</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-cyan-600">{crawlingRange()?.crawling_info?.pages_to_crawl || 0}</div>
                  <div class="text-xs text-gray-600">크롤링할 페이지 수</div>
                </div>
              </div>
            </div>

            {/* 크롤링 전략 정보 */}
            <div class="border-t pt-4">
              <h4 class="text-md font-medium text-gray-800 mb-3">🎯 크롤링 전략</h4>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div class="bg-white rounded p-3 border">
                  <div class="text-sm text-gray-600">전략</div>
                  <div class="text-lg font-semibold text-gray-800 capitalize">{crawlingRange()?.crawling_info?.strategy || 'unknown'}</div>
                </div>
                <div class="bg-white rounded p-3 border">
                  <div class="text-sm text-gray-600">예상 신규 제품</div>
                  <div class="text-lg font-semibold text-gray-800">{crawlingRange()?.crawling_info?.estimated_new_products || 0}</div>
                </div>
              </div>
            </div>
          </div>
        </Show>

  {/* 제어 버튼 */}
  <div class="flex flex-wrap gap-4 mb-6 items-end">
          <button
            onClick={startSmartCrawling}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-blue-600 hover:bg-blue-700'
            }`}
          >
            {isRunning() ? '크롤링 실행 중...' : '🚀 스마트 크롤링 시작'}
          </button>
          
          <button
            onClick={startUnifiedAdvanced}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-purple-600 hover:bg-purple-700'
            }`}
          >
            {isRunning() ? '통합 파이프라인 실행 중...' : '🎭 통합 파이프라인 (하이)'}
          </button>
          
          <button
            onClick={startLightUnified}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-orange-600 hover:bg-orange-700'
            }`}
          >
            {isRunning() ? '통합 파이프라인 실행 중...' : '🎭 통합 파이프라인 (라이트)'}
          </button>
          
          <button
            onClick={calculateCrawlingRange}
            disabled={isRunning()}
            class="px-6 py-3 rounded-lg font-medium text-blue-600 border border-blue-600 hover:bg-blue-50 disabled:opacity-50"
          >
            📊 범위 다시 계산
          </button>
          {/* Validation Controls */}
          <div class="flex items-center gap-2">
            <input
              type="number"
              min="1"
              class="w-28 px-3 py-2 border rounded-md text-sm"
              placeholder="검증 페이지 수"
              value={validationPages() as any}
              onInput={(e) => {
                const v = (e.currentTarget.value || '').trim();
                setValidationPages(v === '' ? '' : Number(v));
              }}
            />
            <button
              onClick={startValidationRun}
              disabled={isValidating()}
              class={`px-4 py-2 rounded-lg font-medium text-white ${
                isValidating() ? 'bg-gray-400 cursor-not-allowed' : 'bg-emerald-600 hover:bg-emerald-700'
              }`}
            >
              {isValidating() ? '검증 실행 중...' : '🧪 Validation 실행'}
            </button>
          </div>
          {/* Sync Controls */}
          <div class="flex items-center gap-2">
            <input
              type="text"
              class="w-64 px-3 py-2 border rounded-md text-sm"
              placeholder="Sync 범위 (예: 498-492,489,487-485)"
              value={syncRanges()}
              onInput={(e) => setSyncRanges(e.currentTarget.value)}
            />
            <button
              onClick={startSyncRun}
              disabled={isSyncing()}
              class={`px-4 py-2 rounded-lg font-medium text-white ${
                isSyncing() ? 'bg-gray-400 cursor-not-allowed' : 'bg-teal-600 hover:bg-teal-700'
              }`}
            >
              {isSyncing() ? 'Sync 실행 중...' : '🔄 Sync 실행'}
            </button>
          </div>
          <button
            onClick={() => setShowConsole(!showConsole())}
            class="px-6 py-3 rounded-lg font-medium text-gray-700 border border-gray-300 hover:bg-gray-50"
          >
            {showConsole() ? '🧪 이벤트 콘솔 숨기기' : '🧪 이벤트 콘솔 보기'}
          </button>
          {/* Effects toggle */}
          <label class="flex items-center gap-2 text-sm text-gray-600 select-none">
            <input type="checkbox" checked={effectsOn()} onInput={(e) => setEffectsOn(e.currentTarget.checked)} />
            애니메이션 효과
          </label>
        </div>

        {/* 실시간 로그 */}
        <div class="bg-black rounded-lg p-4">
          <h3 class="text-sm font-semibold text-white mb-2">📝 실시간 로그</h3>
          <div class="font-mono text-xs text-green-400 h-64 overflow-y-auto">
            <Show 
              when={logs().length > 0} 
              fallback={<div class="text-gray-500">로그 대기 중...</div>}
            >
              <For each={logs()}>
                {(log) => (
                  <div class="mb-1">{log}</div>
                )}
              </For>
            </Show>
          </div>
        </div>

        {/* Actor 이벤트 콘솔 (개발용) */}
        <Show when={showConsole()}>
          <div class="mt-6 border rounded-lg">
            <div class="px-4 py-2 border-b bg-gray-50 text-sm text-gray-700">Actor 이벤트 콘솔</div>
            <EventConsole />
          </div>
        </Show>
      </div>
    </div>
  );
}
