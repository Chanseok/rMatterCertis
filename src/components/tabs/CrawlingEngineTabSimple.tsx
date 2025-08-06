import { createSignal, Show, onMount, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { CrawlingRangeRequest, CrawlingRangeResponse } from '../../types/advanced-engine';

export default function CrawlingEngineTabSimple() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [crawlingRange, setCrawlingRange] = createSignal<CrawlingRangeResponse | null>(null);
  const [statusMessage, setStatusMessage] = createSignal<string>('크롤링 준비 완료');
  const [logs, setLogs] = createSignal<string[]>([]);

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
      
      const request: CrawlingRangeRequest = {
        total_pages_on_site: siteStatus.total_pages,
        products_on_last_page: siteStatus.products_on_last_page,
      };
      
      addLog(`📋 크롤링 범위 계산 요청: ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품`);
      
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
  
  // 가짜 Actor 시스템 크롤링 (실제로는 ServiceBased 엔진 사용)
  const startFakeActorCrawling = async () => {
    if (isRunning()) return;
    
    setIsRunning(true);
    setStatusMessage('🎭 가짜 Actor 시스템 크롤링 시작 중...');
    addLog('🎭 가짜 Actor 시스템 크롤링 시작 (실제로는 ServiceBased 엔진)');

    try {
      const result = await invoke('start_actor_system_crawling', {
        start_page: 0,     // 프론트엔드에서는 범위를 지정하지 않음 (CrawlingPlanner가 계산)
        end_page: 0,       // 프론트엔드에서는 범위를 지정하지 않음 (CrawlingPlanner가 계산)
        concurrency: 8,
        batch_size: 3,
        delay_ms: 100
      });
      addLog(`✅ 가짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      setStatusMessage('🎭 가짜 Actor 시스템 실행 중');
      
    } catch (error) {
      console.error('가짜 Actor 시스템 크롤링 시작 실패:', error);
      addLog(`❌ 가짜 Actor 시스템 크롤링 시작 실패: ${error}`);
      setStatusMessage('크롤링 실패');
    } finally {
      setTimeout(() => setIsRunning(false), 3000); // 3초 후 완료로 처리
    }
  };

  // 진짜 Actor 시스템 크롤링 시작
  const startRealActorCrawling = async () => {
    if (isRunning()) return;
    
    setIsRunning(true);
    setStatusMessage('🎭 진짜 Actor 시스템 크롤링 시작 중...');
    addLog('🎭 진짜 Actor 시스템 크롤링 시작');

    try {
      const result = await invoke('start_actor_system_crawling', {
        request: {
          start_page: 0,     // By Design: 프론트엔드에서 범위 지정하지 않음
          end_page: 0,       // By Design: 프론트엔드에서 범위 지정하지 않음
          concurrency: 64,
          batch_size: 3,
          delay_ms: 100
        }
      });
      addLog(`✅ 진짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      setStatusMessage('🎭 진짜 Actor 시스템 실행 중 (설정 기반)');
      
    } catch (error) {
      console.error('진짜 Actor 시스템 크롤링 시작 실패:', error);
      addLog(`❌ 진짜 Actor 시스템 크롤링 시작 실패: ${error}`);
      setStatusMessage('크롤링 실패');
    } finally {
      setTimeout(() => setIsRunning(false), 5000); // 5초 후 완료로 처리
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
    calculateCrawlingRange();
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
            onClick={startRealActorCrawling}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-purple-600 hover:bg-purple-700'
            }`}
          >
            {isRunning() ? '진짜 Actor 실행 중...' : '🎭 진짜 Actor 시스템 크롤링'}
          </button>
          
          <button
            onClick={startFakeActorCrawling}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-orange-600 hover:bg-orange-700'
            }`}
          >
            {isRunning() ? '가짜 Actor 실행 중...' : '🎭 가짜 Actor 시스템 크롤링'}
          </button>
          
          <button
            onClick={calculateCrawlingRange}
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
