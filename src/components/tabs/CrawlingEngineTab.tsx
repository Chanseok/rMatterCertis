/**
 * CrawlingEngineTab - Advanced Crawling Engine 통합 탭
 * Phase 4A의 5단계 파이프라인을 UI에서 제어하고 모니터링
 */

import { Component, createSignal, onMount, onCleanup, Show, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { 
  AdvancedCrawlingConfig, 
  CrawlingProgressInfo, 
  SiteStatusInfo, 
  ProductInfo, 
  CrawlingSession, 
  DatabaseStats,
  ApiResponse,
  StartCrawlingRequest 
} from '../../types/advanced-engine';

interface SiteStatus {
  is_accessible: boolean;
  total_pages: number;
  health_score: number;
  response_time_ms: number;
}

export const CrawlingEngineTab: Component = () => {
  // 상태 관리
  const [config, setConfig] = createSignal<AdvancedCrawlingConfig>({
    start_page: 1,
    end_page: 10,
    batch_size: 5,
    concurrency: 3,
    delay_ms: 1000,
    retry_max: 3,
    enable_real_time_updates: true
  });
  
  const [siteStatus, setSiteStatus] = createSignal<SiteStatusInfo | null>(null);
  const [progress, setProgress] = createSignal<CrawlingProgressInfo | null>(null);
  const [recentProducts, setRecentProducts] = createSignal<ProductInfo[]>([]);
  const [logs, setLogs] = createSignal<string[]>([]);
  const [isRunning, setIsRunning] = createSignal(false);
  const [currentSession, setCurrentSession] = createSignal<CrawlingSession | null>(null);
  const [dbStats, setDbStats] = createSignal<DatabaseStats | null>(null);

  // Log helper
  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-19), `[${timestamp}] ${message}`]);
  };  // Initialize and load data
  onMount(async () => {
    addLog('🎯 Advanced Crawling Engine 탭 로드됨');
    
    await checkSiteStatus();
    await loadRecentProducts();
    await loadDatabaseStats();
    
    // Tauri 이벤트 리스너 등록
    const unlistenProgress = await listen('crawling-progress', (event) => {
      const progressData = event.payload as CrawlingProgressInfo;
      setProgress(progressData);
      addLog(`🔄 진행률: ${progressData.progress_percentage.toFixed(1)}% - ${progressData.current_message}`);
    });
    
    const unlistenCompleted = await listen('crawling-completed', (event) => {
      const sessionData = event.payload as CrawlingSession;
      setIsRunning(false);
      setCurrentSession(sessionData);
      addLog(`✅ 크롤링 완료: 세션 ${sessionData.session_id}`);
      loadRecentProducts(); // 완료 후 제품 목록 새로고침
    });
    
    const unlistenFailed = await listen('crawling-failed', (event) => {
      const sessionData = event.payload as CrawlingSession;
      setIsRunning(false);
      setCurrentSession(sessionData);
      addLog(`❌ 크롤링 실패: 세션 ${sessionData.session_id}`);
    });
    
    // 컴포넌트 언마운트 시 리스너 해제
    onCleanup(() => {
      unlistenProgress();
      unlistenCompleted();
      unlistenFailed();
    });
  });

  const loadDatabaseStats = async () => {
    try {
      const response = await invoke<ApiResponse<DatabaseStats>>('get_database_stats');
      
      if (response.success && response.data) {
        setDbStats(response.data);
        addLog(`📊 데이터베이스: 총 ${response.data.total_products}개 제품`);
      } else {
        addLog(`❌ DB 통계 로드 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ DB 통계 로드 오류: ${error}`);
    }
  };

  // API functions
  const checkSiteStatus = async () => {
    try {
      addLog('🌐 사이트 상태 확인 중...');
      const response = await invoke<ApiResponse<SiteStatusInfo>>('check_advanced_site_status');
      
      if (response.success && response.data) {
        setSiteStatus(response.data);
        addLog(`✅ 사이트 상태: ${response.data.total_pages}페이지, ${response.data.estimated_total_products}개 제품 예상`);
      } else {
        addLog(`❌ 사이트 상태 확인 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 사이트 상태 확인 오류: ${error}`);
    }
  };

  const loadRecentProducts = async () => {
    try {
      addLog('📋 최근 제품 로드 중...');
      const response = await invoke<ApiResponse<{ products: ProductInfo[] }>>('get_recent_products', { page: 1, limit: 10 });
      
      if (response.success && response.data) {
        setRecentProducts(response.data.products);
        addLog(`📋 최근 제품 ${response.data.products.length}개 로드됨`);
      } else {
        addLog(`❌ 제품 로드 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 제품 로드 오류: ${error}`);
    }
  };

  const startCrawling = async () => {
    if (isRunning()) return;

    try {
      setIsRunning(true);
      addLog(`🚀 Advanced Crawling Engine 시작`);
      
      const request: StartCrawlingRequest = {
        config: config()
      };
      
      const response = await invoke<ApiResponse<CrawlingSession>>('start_advanced_crawling', {
        request
      });
      
      if (response.success && response.data) {
        setCurrentSession(response.data);
        addLog(`✅ 크롤링 세션 시작: ${response.data.session_id}`);
      } else {
        addLog(`❌ 크롤링 시작 실패: ${response.error?.message || 'Unknown error'}`);
        setIsRunning(false);
      }
    } catch (error) {
      addLog(`❌ 크롤링 시작 오류: ${error}`);
      setIsRunning(false);
    }
  };

  const stopCrawling = async () => {
    setIsRunning(false);
    addLog('⏹️ 크롤링 중단됨');
  };

  const stageNames = [
    'Stage 0: 사이트 상태 확인',
    'Stage 1: 데이터베이스 분석', 
    'Stage 2: 제품 목록 수집',
    'Stage 3: 제품 상세정보 수집',
    'Stage 4: 데이터 처리 파이프라인',
    'Stage 5: 데이터베이스 저장'
  ];

  return (
    <div class="min-h-screen bg-gray-50 p-6">
      <div class="max-w-7xl mx-auto">
        <div class="mb-8">
          <h1 class="text-3xl font-bold text-gray-900 mb-2">
            🔬 Advanced Crawling Engine
          </h1>
          <p class="text-gray-600">
            Phase 4A 5단계 파이프라인 제어 및 모니터링
          </p>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div class="space-y-6">
            {/* Site Status */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-900">🌐 사이트 상태</h2>
                <button
                  onClick={checkSiteStatus}
                  class="px-3 py-1.5 text-sm bg-blue-100 text-blue-700 rounded-md hover:bg-blue-200"
                >
                  새로고침
                </button>
              </div>
              <Show
                when={siteStatus()}
                fallback={<p class="text-gray-500">사이트 상태를 확인 중...</p>}
              >
                <div class="space-y-2 text-sm">
                  <div class="flex justify-between">
                    <span class="text-gray-600">접근 가능:</span>
                    <span class={siteStatus()?.is_accessible ? "text-green-600" : "text-red-600"}>
                      {siteStatus()?.is_accessible ? "✅ 가능" : "❌ 불가능"}
                    </span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-gray-600">전체 페이지:</span>
                    <span class="font-medium">{siteStatus()?.total_pages || 0}</span>
                  </div>
                </div>
              </Show>
            </div>

            {/* Controls */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <h2 class="text-lg font-semibold text-gray-900 mb-4">⚙️ 크롤링 설정</h2>
              <div class="space-y-4">
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">시작 페이지</label>
                    <input
                      type="number"
                      value={config().start_page}
                      onInput={(e) => setConfig(prev => ({ 
                        ...prev, 
                        start_page: parseInt(e.currentTarget.value) || 1 
                      }))}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md"
                      disabled={isRunning()}
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">종료 페이지</label>
                    <input
                      type="number"
                      value={config().end_page}
                      onInput={(e) => setConfig(prev => ({ 
                        ...prev, 
                        end_page: parseInt(e.currentTarget.value) || 1 
                      }))}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md"
                      disabled={isRunning()}
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">배치 크기</label>
                    <input
                      type="number"
                      value={config().batch_size}
                      onInput={(e) => setConfig(prev => ({ 
                        ...prev, 
                        batch_size: parseInt(e.currentTarget.value) || 1 
                      }))}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md"
                      disabled={isRunning()}
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">동시 실행 수</label>
                    <input
                      type="number"
                      value={config().concurrency}
                      onInput={(e) => setConfig(prev => ({ 
                        ...prev, 
                        concurrency: parseInt(e.currentTarget.value) || 1 
                      }))}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md"
                      disabled={isRunning()}
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">요청 간 딜레이 (ms)</label>
                    <input
                      type="number"
                      value={config().delay_ms}
                      onInput={(e) => setConfig(prev => ({ 
                        ...prev, 
                        delay_ms: parseInt(e.currentTarget.value) || 1000 
                      }))}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md"
                      disabled={isRunning()}
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">재시도 횟수</label>
                    <input
                      type="number"
                      value={config().retry_max}
                      onInput={(e) => setConfig(prev => ({ 
                        ...prev, 
                        retry_max: parseInt(e.currentTarget.value) || 3 
                      }))}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md"
                      disabled={isRunning()}
                    />
                  </div>
                </div>
                
                <div class="flex items-center">
                  <input
                    type="checkbox"
                    id="real-time-updates"
                    checked={config().enable_real_time_updates}
                    onChange={(e) => setConfig(prev => ({ 
                      ...prev, 
                      enable_real_time_updates: e.currentTarget.checked 
                    }))}
                    class="mr-2"
                    disabled={isRunning()}
                  />
                  <label for="real-time-updates" class="text-sm font-medium text-gray-700">
                    실시간 업데이트 활성화
                  </label>
                </div>

                <div class="flex gap-3 pt-4">
                  <button
                    onClick={startCrawling}
                    disabled={isRunning()}
                    class={`flex-1 py-2.5 px-4 rounded-md font-medium ${
                      isRunning()
                        ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                        : 'bg-blue-600 text-white hover:bg-blue-700'
                    }`}
                  >
                    {isRunning() ? '⏳ 실행 중...' : '🚀 크롤링 시작'}
                  </button>
                  <Show when={isRunning()}>
                    <button
                      onClick={stopCrawling}
                      class="px-4 py-2.5 bg-red-600 text-white rounded-md hover:bg-red-700 font-medium"
                    >
                      ⏹️ 중단
                    </button>
                  </Show>
                </div>
              </div>
            </div>

            {/* Progress */}
            <Show when={progress()}>
              <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 class="text-lg font-semibold text-gray-900 mb-4">📊 진행 상황</h2>
                <div class="space-y-4">
                  <div>
                    <div class="flex justify-between items-center mb-2">
                      <span class="text-sm font-medium text-gray-700">
                        {stageNames[progress()?.stage || 0]}
                      </span>
                      <span class="text-sm text-gray-500">
                        {Math.round(progress()?.progress_percentage || 0)}%
                      </span>
                    </div>
                    <div class="w-full bg-gray-200 rounded-full h-2">
                      <div
                        class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                        style={`width: ${progress()?.progress_percentage || 0}%`}
                      />
                    </div>
                  </div>
                  <div class="bg-gray-50 rounded-md p-3">
                    <p class="text-sm text-gray-700">
                      💬 {progress()?.current_message}
                    </p>
                  </div>
                </div>
              </div>
            </Show>
          </div>

          <div class="space-y-6">
            {/* Recent Products */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-900">📦 최근 수집된 제품</h2>
                <button
                  onClick={loadRecentProducts}
                  class="px-3 py-1.5 text-sm bg-green-100 text-green-700 rounded-md hover:bg-green-200"
                >
                  새로고침
                </button>
              </div>
              <div class="space-y-3 max-h-80 overflow-y-auto">
                <Show
                  when={recentProducts().length > 0}
                  fallback={<p class="text-gray-500 text-sm">아직 수집된 제품이 없습니다.</p>}
                >
                  <For each={recentProducts()}>
                    {(product) => (
                      <div class="border border-gray-200 rounded-md p-3 bg-gray-50">
                        <h3 class="font-medium text-gray-900 text-sm">{product.name}</h3>
                        <p class="text-xs text-gray-600">{product.company}</p>
                        <p class="text-xs text-blue-600 font-mono">{product.certification_number}</p>
                      </div>
                    )}
                  </For>
                </Show>
              </div>
            </div>

            {/* Live Logs */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <h2 class="text-lg font-semibold text-gray-900 mb-4">📝 실시간 로그</h2>
              <div class="bg-gray-900 rounded-md p-4 h-80 overflow-y-auto font-mono text-sm">
                <Show
                  when={logs().length > 0}
                  fallback={<p class="text-gray-400">로그 대기 중...</p>}
                >
                  <For each={logs()}>
                    {(log) => (
                      <div class="text-green-400 mb-1">{log}</div>
                    )}
                  </For>
                </Show>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
