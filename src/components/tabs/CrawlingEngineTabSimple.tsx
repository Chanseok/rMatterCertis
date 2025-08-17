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
        }
        if (name === 'actor-session-completed') {
          setIsRunning(false);
          setStatusMessage('크롤링 완료');
          addLog('🏁 세션 완료');
        }
        if (name === 'actor-session-failed') {
          setIsRunning(false);
          setStatusMessage('크롤링 실패');
          addLog(`❌ 세션 실패: ${JSON.stringify(payload)}`);
        }
        if (name === 'actor-session-timeout' || name === 'actor-shutdown-completed') {
          setIsRunning(false);
          setStatusMessage('크롤링 종료');
          addLog('🛑 세션 종료');
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
            </div>
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
                <div class="text-sm text-gray-600">완료율</div>
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
