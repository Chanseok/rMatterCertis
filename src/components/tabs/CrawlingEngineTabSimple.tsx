import { createSignal, Show, onMount, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface CrawlingRangeRequest {
  total_pages_on_site: number;
  products_on_last_page: number;
}

interface CrawlingRangeResponse {
  success: boolean;
  range?: [number, number];
  progress: {
    total_products: number;
    saved_products: number;
    progress_percentage: number;
    max_page_id?: number;
    max_index_in_page?: number;
    is_completed: boolean;
  };
}

export default function CrawlingEngineTabSimple() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [crawlingRange, setCrawlingRange] = createSignal<CrawlingRangeResponse | null>(null);
  const [statusMessage, setStatusMessage] = createSignal<string>('크롤링 준비 완료');
  const [logs, setLogs] = createSignal<string[]>([]);

  // 크롤링 범위 자동 계산
  const loadCrawlingRange = async () => {
    try {
      const request: CrawlingRangeRequest = {
        total_pages_on_site: 485,
        products_on_last_page: 11,
      };
      
      const response = await invoke<CrawlingRangeResponse>('calculate_crawling_range', { request });
      setCrawlingRange(response);
      
      const startPage = response.range?.[0] || 0;
      const endPage = response.range?.[1] || 0;
      addLog(`📊 크롤링 범위 계산 완료: ${startPage} → ${endPage}`);
    } catch (error) {
      console.error('크롤링 범위 계산 실패:', error);
      addLog(`❌ 크롤링 범위 계산 실패: ${error}`);
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

  onMount(() => {
    loadCrawlingRange();
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
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
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
          </div>
        </Show>

        {/* 제어 버튼 */}
        <div class="flex space-x-4 mb-6">
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
            onClick={loadCrawlingRange}
            disabled={isRunning()}
            class="px-6 py-3 rounded-lg font-medium text-blue-600 border border-blue-600 hover:bg-blue-50 disabled:opacity-50"
          >
            📊 범위 다시 계산
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
      </div>
    </div>
  );
}
