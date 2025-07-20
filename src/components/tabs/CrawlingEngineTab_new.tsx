/**
 * CrawlingEngineTab - Advanced Crawling Engine 통합 탭
 * Phase 4A의 5단계 파이프라인을 UI에서 제어하고 모니터링
 */

import { Component, createSignal, onMount, Show, For } from 'solid-js';

interface CrawlingConfig {
  start_page: number;
  end_page: number;
  batch_size: number;
  concurrency: number;
  delay_ms: number;
}

interface CrawlingProgress {
  stage: number;
  stage_name: string;
  progress_percentage: number;
  items_processed: number;
  current_message: string;
  estimated_remaining_time?: number;
}

interface Product {
  id: string;
  url: string;
  name: string;
  company: string;
  certification_number: string;
  created_at: string;
}

export const CrawlingEngineTab: Component = () => {
  // State management
  const [config, setConfig] = createSignal<CrawlingConfig>({
    start_page: 1,
    end_page: 1,
    batch_size: 3,
    concurrency: 1,
    delay_ms: 2000
  });

  const [isRunning, setIsRunning] = createSignal(false);
  const [progress, setProgress] = createSignal<CrawlingProgress | null>(null);
  const [recentProducts, setRecentProducts] = createSignal<Product[]>([]);
  const [logs, setLogs] = createSignal<string[]>([]);
  const [siteStatus, setSiteStatus] = createSignal<any>(null);

  // Mock Tauri API for development
  const invoke = async (command: string, args?: any) => {
    console.log(`Mock invoke: ${command}`, args);
    
    // Simulate API responses
    switch (command) {
      case 'check_site_status':
        return {
          is_accessible: true,
          total_pages: 485,
          health_score: 0.8,
          response_time_ms: 1500
        };
      case 'get_recent_products':
        return [
          {
            id: '1',
            url: 'https://csa-iot.org/csa_product/test-1/',
            name: '테스트 제품 1',
            company: '테스트 회사',
            certification_number: 'CERT-001',
            created_at: new Date().toISOString()
          }
        ];
      case 'start_advanced_crawling_test':
        // Simulate progress updates
        setTimeout(() => {
          setProgress({
            stage: 0,
            stage_name: 'Site Status Check',
            progress_percentage: 20,
            items_processed: 1,
            current_message: '사이트 상태 확인 중...'
          });
        }, 1000);
        
        setTimeout(() => {
          setProgress({
            stage: 5,
            stage_name: 'Database Save',
            progress_percentage: 100,
            items_processed: 12,
            current_message: '완료!'
          });
          setIsRunning(false);
        }, 5000);
        
        return { success: true, products_collected: 12 };
      default:
        return { success: true };
    }
  };

  // Initialize and load data
  onMount(async () => {
    await checkSiteStatus();
    await loadRecentProducts();
  });

  // API functions
  const checkSiteStatus = async () => {
    try {
      const status = await invoke('check_site_status');
      setSiteStatus(status);
      addLog(`✅ 사이트 상태 확인 완료`);
    } catch (error) {
      addLog(`❌ 사이트 상태 확인 실패: ${error}`);
    }
  };

  const loadRecentProducts = async () => {
    try {
      const products = await invoke('get_recent_products', { limit: 10 });
      setRecentProducts(products as Product[]);
      addLog(`📋 최근 제품 ${(products as Product[]).length}개 로드됨`);
    } catch (error) {
      addLog(`❌ 제품 로드 실패: ${error}`);
    }
  };

  const startCrawling = async () => {
    if (isRunning()) return;

    try {
      setIsRunning(true);
      addLog(`🚀 Advanced Crawling Engine 시작`);
      
      const result = await invoke('start_advanced_crawling_test', {
        startPage: config().start_page,
        endPage: config().end_page,
        batchSize: config().batch_size,
        concurrency: config().concurrency,
        delayMs: config().delay_ms
      });
      
      addLog(`✅ 크롤링 완료: ${JSON.stringify(result)}`);
      await loadRecentProducts();
    } catch (error) {
      addLog(`❌ 크롤링 실패: ${error}`);
      setIsRunning(false);
    }
  };

  const stopCrawling = async () => {
    setIsRunning(false);
    addLog('⏹️ 크롤링 중단됨');
  };

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [`[${timestamp}] ${message}`, ...prev.slice(0, 49)]);
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
