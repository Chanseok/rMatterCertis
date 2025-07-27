/**
 * CrawlingEngineTab - Advanced Crawling Engine 통합 탭
 * Phase 4A의 5단계 파이프라인을 UI에서 제어하고 모니터링
 */

import { Component, createSignal, onMount, onCleanup, Show, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { 
  CrawlingProgressInfo, 
  SiteStatusInfo, 
  ProductInfo, 
  CrawlingSession, 
  DatabaseStats,
  ApiResponse,
  StartCrawlingRequest,
  CrawlingRangeRequest,
  CrawlingRangeResponse
} from '../../types/advanced-engine';

export const CrawlingEngineTab: Component = () => {
  // ❌ REMOVED: userConfig - 설정 전송 API 제거로 불필요
  // const [userConfig, setUserConfig] = createSignal<any>(null);
  const [showAdvancedSettings, setShowAdvancedSettings] = createSignal(false);
  
  const [siteStatus, setSiteStatus] = createSignal<SiteStatusInfo | null>(null);
  const [progress, setProgress] = createSignal<CrawlingProgressInfo | null>(null);
  const [recentProducts, setRecentProducts] = createSignal<ProductInfo[]>([]);
  const [logs, setLogs] = createSignal<string[]>([]);
  const [isRunning, setIsRunning] = createSignal(false);
  const [isPaused, setIsPaused] = createSignal(false);
  const [currentSessionId, setCurrentSessionId] = createSignal<string | null>(null);
  const [dbStats, setDbStats] = createSignal<DatabaseStats | null>(null);
  const [crawlingRange, setCrawlingRange] = createSignal<CrawlingRangeResponse | null>(null);

  // Log helper
  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-19), `[${timestamp}] ${message}`]);
  };

  // ❌ REMOVED: loadUserConfig - 설정 전송 API 제거로 불필요
  // 백엔드가 matter_certis_config.json 파일을 자동으로 읽어서 사용

  // 크롤링 범위 계산
  const calculateCrawlingRange = async () => {
    try {
      addLog('🔍 크롤링 범위 계산 함수 시작...');
      
      const siteInfo = siteStatus();
      if (!siteInfo) {
        addLog('❌ 크롤링 범위 계산 실패: 사이트 상태 정보 없음');
        console.warn('siteStatus is null:', siteInfo);
        return;
      }

      addLog(`🔍 사이트 정보 확인됨: ${siteInfo.total_pages}페이지, 마지막 페이지 ${siteInfo.products_on_last_page}개 제품`);

      const request: CrawlingRangeRequest = {
        total_pages_on_site: siteInfo.total_pages,
        products_on_last_page: siteInfo.products_on_last_page
      };

      addLog(`🔍 크롤링 범위 계산 중... (총 ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품)`);
      
      console.log('Calling calculate_crawling_range with request:', request);
      
      const response = await invoke<CrawlingRangeResponse>('calculate_crawling_range', { request });
      
      console.log('Response from calculate_crawling_range:', response);
      
      if (response?.success && response?.range) {
        setCrawlingRange(response);
        const [start_page, end_page] = response.range;
        const total_pages_to_crawl = start_page - end_page + 1;
        addLog(`✅ 계산된 크롤링 범위: ${start_page} → ${end_page} (${total_pages_to_crawl} 페이지)`);
        console.log('Successfully set crawling range:', response);
      } else {
        addLog(`❌ 크롤링 범위 계산 실패: ${response?.message || '알 수 없는 오류'}`);
        console.error('Failed to calculate crawling range:', response);
      }
    } catch (error) {
      addLog(`❌ 크롤링 범위 계산 오류: ${error}`);
      console.error('크롤링 범위 계산 오류:', error);
    }
  };  // Initialize and load data
  onMount(async () => {
    addLog('🎯 Advanced Crawling Engine 탭 로드됨');
    
    // ❌ REMOVED: await loadUserConfig() - 설정 전송 API 제거
    await checkSiteStatus(); // 이 함수 내에서 이미 calculateCrawlingRange() 호출됨
    await loadRecentProducts();
    await loadDatabaseStats();
    
    // checkSiteStatus() 내에서 이미 호출되므로 중복 호출 방지
    // await calculateCrawlingRange();
    
    // Tauri 이벤트 리스너 등록
    const unlistenProgress = await listen('crawling-progress', (event) => {
      const progressData = event.payload as CrawlingProgressInfo;
      setProgress(progressData);
      addLog(`🔄 진행률: ${progressData.progress_percentage.toFixed(1)}% - ${progressData.current_message}`);
    });
    
    const unlistenCompleted = await listen('crawling-completed', (event) => {
      const sessionData = event.payload as CrawlingSession;
      setIsRunning(false);
      setIsPaused(false);
      setCurrentSessionId(null);
      addLog(`✅ 크롤링 완료: 세션 ${sessionData.session_id}`);
      loadRecentProducts(); // 완료 후 제품 목록 새로고침
    });
    
    const unlistenFailed = await listen('crawling-failed', (event) => {
      const sessionData = event.payload as CrawlingSession;
      setIsRunning(false);
      setIsPaused(false);
      setCurrentSessionId(null);
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
        
        // 사이트 상태 업데이트 후 크롤링 범위 재계산
        addLog('🔍 사이트 상태 확인 완료, 크롤링 범위 계산 시작...');
        console.log('About to call calculateCrawlingRange from checkSiteStatus');
        await calculateCrawlingRange();
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
    
    // ❌ REMOVED: config 의존성 제거 - 백엔드가 matter_certis_config.json 자동 로딩
    // const config = userConfig();
    // if (!config) {
    //   addLog('❌ 설정을 먼저 로드해야 합니다');
    //   return;
    // }

    try {
      setIsRunning(true);
      
      addLog(`🚀 Smart Crawling 시작 - 백엔드가 자동으로 최적 범위 계산`);
      
      // ✅ 새로운 방식: 백엔드가 설정 파일을 읽고 자동으로 크롤링 시작
      const session = await invoke<CrawlingSession>('start_smart_crawling');
      
      setCurrentSessionId(session.session_id);
      addLog(`✅ 크롤링 세션 시작: ${session.session_id}`);
      
    } catch (error) {
      setIsRunning(false);
      addLog(`❌ 크롤링 시작 실패: ${error}`);
      console.error('크롤링 시작 오류:', error);
    }
  };

  const pauseCrawling = async () => {
    if (!currentSessionId()) {
      addLog('❌ 활성 세션이 없습니다');
      return;
    }

    try {
      const response = await invoke<ApiResponse<any>>('pause_crawling_session', {
        session_id: currentSessionId()
      });
      
      if (response.success) {
        setIsPaused(true);
        addLog(`⏸️ 크롤링 일시 중지: ${currentSessionId()}`);
      } else {
        addLog(`❌ 일시 중지 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 일시 중지 오류: ${error}`);
    }
  };

  const resumeCrawling = async () => {
    if (!currentSessionId()) {
      addLog('❌ 활성 세션이 없습니다');
      return;
    }

    try {
      const response = await invoke<ApiResponse<any>>('resume_crawling_session', {
        session_id: currentSessionId()
      });
      
      if (response.success) {
        setIsPaused(false);
        addLog(`▶️ 크롤링 재개: ${currentSessionId()}`);
      } else {
        addLog(`❌ 재개 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 재개 오류: ${error}`);
    }
  };

  const stopCrawling = async () => {
    if (!currentSessionId()) {
      setIsRunning(false);
      setIsPaused(false);
      addLog('⏹️ 크롤링 중단됨');
      return;
    }

    try {
      const response = await invoke<ApiResponse<any>>('stop_crawling_session', {
        session_id: currentSessionId()
      });
      
      if (response.success) {
        setIsRunning(false);
        setIsPaused(false);
        setCurrentSessionId(null);
        addLog(`⏹️ 크롤링 완전 중단: ${currentSessionId()}`);
      } else {
        addLog(`❌ 중단 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 중단 오류: ${error}`);
    }
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

            {/* Configured Range Display (Read-Only) */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <h2 class="text-lg font-semibold text-gray-900 mb-4">📄 설정된 크롤링 범위</h2>
              <Show 
                when={userConfig()} 
                fallback={
                  <div class="bg-red-50 border border-red-200 rounded-lg p-4">
                    <div class="flex items-start space-x-3">
                      <span class="text-red-500 text-lg">⚠️</span>
                      <div>
                        <h3 class="text-sm font-semibold text-red-800 mb-2">설정을 불러올 수 없습니다</h3>
                        <p class="text-sm text-red-700 mb-3">
                          크롤링을 시작하기 전에 설정을 올바르게 로드해야 합니다.
                        </p>
                        <div class="space-y-2">
                          <button
                            onClick={loadUserConfig}
                            class="px-4 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 font-medium text-sm"
                          >
                            🔄 설정 다시 로드
                          </button>
                          <div class="text-xs text-red-600">
                            문제가 지속되면 Settings Tab에서 설정을 확인하고 저장해 주세요.
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                }
              >
                <div class="space-y-4">
                  <div class="bg-blue-50 border border-blue-200 rounded-lg p-4">
                    <div class="grid grid-cols-2 gap-4 mb-4">
                      <div>
                        <label class="block text-sm font-medium text-gray-700 mb-2">시작 페이지 (가장 오래된)</label>
                        <div class="w-full px-3 py-2 bg-gray-100 border border-gray-300 rounded-md text-lg font-semibold text-center">
                          {(() => {
                            const totalPages = siteStatus()?.total_pages || 485;
                            
                            // 가장 오래된 제품 페이지 (485)
                            return totalPages;
                          })()}
                        </div>
                      </div>
                      <div>
                        <label class="block text-sm font-medium text-gray-700 mb-2">종료 페이지 (상대적으로 최신)</label>
                        <div class="w-full px-3 py-2 bg-gray-100 border border-gray-300 rounded-md text-lg font-semibold text-center">
                          {(() => {
                            const config = userConfig()?.user?.crawling;
                            const totalPages = siteStatus()?.total_pages || 485;
                            const pageLimit = config?.page_range_limit || 6;
                            
                            // 상대적으로 최신 제품 페이지 (480)
                            return Math.max(1, totalPages - pageLimit + 1);
                          })()}
                        </div>
                      </div>
                    </div>
                    
                    <div class="text-xs text-blue-700">
                      <div class="flex items-center space-x-2 mb-1">
                        <span>📝</span>
                        <span>크롤링 모드: <strong>{userConfig()?.user?.crawling?.crawling_mode || 'incremental'}</strong></span>
                      </div>
                      <div class="flex items-center space-x-2 mb-1">
                        <span>🔧</span>
                        <span>자동 범위 조정: <strong>{userConfig()?.user?.crawling?.auto_adjust_range ? '활성화' : '비활성화'}</strong></span>
                      </div>
                      <div class="flex items-center space-x-2">
                        <span>📊</span>
                        <span>크롤링 순서: <strong>485 → 484 → 483 → 482 → 481 → 480 (오래된 제품부터)</strong></span>
                      </div>
                    </div>
                  </div>

                  {/* Auto-Generated Strategy Display */}
                  <Show when={siteStatus() && dbStats()}>
                    <div class="bg-green-50 border border-green-200 rounded-md p-4">
                      <h3 class="text-sm font-semibold text-green-800 mb-2">🤖 자동 생성된 크롤링 전략</h3>
                      <div class="text-xs text-green-700 space-y-1">
                        <div class="flex justify-between">
                          <span>크롤링 페이지 수:</span>
                          <span class="font-medium">
                            {(() => {
                              const config = userConfig()?.user?.crawling;
                              return config?.page_range_limit || 6;
                            })()} 페이지
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>크롤링 범위:</span>
                          <span class="font-medium">
                            {(() => {
                              const config = userConfig()?.user?.crawling;
                              const totalPages = siteStatus()?.total_pages || 485;
                              const pageLimit = config?.page_range_limit || 6;
                              const oldestPage = totalPages; // 가장 오래된 (485)
                              const newestPage = Math.max(1, totalPages - pageLimit + 1); // 상대적으로 최신 (480)
                              return `${oldestPage} → ${newestPage} (오래된 순)`;
                            })()}
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>예상 제품 수:</span>
                          <span class="font-medium">
                            {(() => {
                              const config = userConfig()?.user?.crawling;
                              const pageLimit = config?.page_range_limit || 6;
                              return Math.round(pageLimit * 12); // 페이지당 평균 12개 제품
                            })()} 개
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>배치 크기 (설정값):</span>
                          <span class="font-medium">
                            {userConfig()?.user?.batch?.batch_size || 12}개
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>실제 배치 개수:</span>
                          <span class="font-medium">
                            {(() => {
                              const config = userConfig()?.user;
                              const pageLimit = config?.crawling?.page_range_limit || 6;
                              const batchSize = config?.batch?.batch_size || 12;
                              return Math.max(1, Math.ceil(pageLimit / batchSize));
                            })()} 배치
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>동시 실행 수:</span>
                          <span class="font-medium">
                            {userConfig()?.user?.max_concurrent_requests || 3}개
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>요청 간격:</span>
                          <span class="font-medium">
                            {userConfig()?.user?.request_delay_ms || 1000}ms
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>예상 소요 시간:</span>
                          <span class="font-medium">
                            {(() => {
                              const config = userConfig()?.user?.crawling;
                              const pageLimit = config?.page_range_limit || 6;
                              const delayMs = userConfig()?.user?.request_delay_ms || 1000;
                              return Math.round((pageLimit * delayMs) / 60000 * 2.5);
                            })()} 분
                          </span>
                        </div>
                      </div>
                    </div>
                  </Show>

                  {/* 고급 설정 (접기/펼치기) */}
                  <div class="bg-yellow-50 border border-yellow-200 rounded-md p-4">
                    <div 
                      class="flex items-center justify-between cursor-pointer"
                      onClick={() => setShowAdvancedSettings(!showAdvancedSettings())}
                    >
                      <h3 class="text-sm font-semibold text-yellow-800">⚙️ 고급 설정 (읽기 전용)</h3>
                      <span class="text-yellow-600">
                        {showAdvancedSettings() ? '🔼' : '🔽'}
                      </span>
                    </div>
                    
                    <Show when={showAdvancedSettings()}>
                      <div class="mt-3 pt-3 border-t border-yellow-300">
                        <div class="text-xs text-yellow-700 space-y-2">
                          <div class="grid grid-cols-2 gap-4">
                            <div>
                              <strong>배치 처리 설정:</strong>
                              <div class="ml-2">
                                • 배치 크기: {userConfig()?.user?.batch?.batch_size || 12}개<br/>
                                • 배치 지연: {userConfig()?.user?.batch?.batch_delay_ms || 1000}ms<br/>
                                • 배치 활성화: {userConfig()?.user?.batch?.enable_batch_processing ? '예' : '아니오'}
                              </div>
                            </div>
                            <div>
                              <strong>동시성 설정:</strong>
                              <div class="ml-2">
                                • 최대 동시 요청: {userConfig()?.user?.max_concurrent_requests || 3}개<br/>
                                • 목록 페이지 동시성: {userConfig()?.user?.crawling?.workers?.list_page_max_concurrent || 5}개<br/>
                                • 상세 페이지 동시성: {userConfig()?.user?.crawling?.workers?.product_detail_max_concurrent || 10}개
                              </div>
                            </div>
                          </div>
                          
                          <div class="grid grid-cols-2 gap-4">
                            <div>
                              <strong>재시도 설정:</strong>
                              <div class="ml-2">
                                • 목록 페이지 재시도: {userConfig()?.user?.crawling?.product_list_retry_count || 2}회<br/>
                                • 상세 페이지 재시도: {userConfig()?.user?.crawling?.product_detail_retry_count || 2}회<br/>
                                • 오류 허용 임계값: {userConfig()?.user?.crawling?.error_threshold_percent || 10}%
                              </div>
                            </div>
                            <div>
                              <strong>타이밍 설정:</strong>
                              <div class="ml-2">
                                • 요청 지연: {userConfig()?.user?.request_delay_ms || 1000}ms<br/>
                                • 요청 타임아웃: {userConfig()?.advanced?.request_timeout_seconds || 30}초<br/>
                                • 재시도 지연: {userConfig()?.advanced?.retry_delay_ms || 2000}ms
                              </div>
                            </div>
                          </div>
                          
                          <div class="pt-2 border-t border-yellow-300">
                            <strong>데이터 무결성:</strong>
                            <div class="ml-2">
                              • 누락 탐지 임계값: {userConfig()?.user?.crawling?.gap_detection_threshold || 5}개<br/>
                              • Binary Search 깊이: {userConfig()?.user?.crawling?.binary_search_max_depth || 10}회<br/>
                              • 데이터 검증: {userConfig()?.user?.crawling?.enable_data_validation ? '활성화' : '비활성화'}<br/>
                              • 자동 DB 저장: {userConfig()?.user?.crawling?.auto_add_to_local_db ? '활성화' : '비활성화'}
                            </div>
                          </div>
                        </div>
                      </div>
                    </Show>
                  </div>

                  {/* 크롤링 제어 버튼 */}
                  <div class="bg-gray-50 border border-gray-200 rounded-md p-4">
                    <h4 class="text-sm font-semibold text-gray-800 mb-3">🎮 크롤링 제어</h4>
                    <div class="grid grid-cols-1 gap-3">
                      {/* 첫 번째 줄: 시작 버튼 */}
                      <Show 
                        when={!isRunning()}
                        fallback={
                          <div class="bg-blue-100 border border-blue-300 rounded-md p-2 text-center">
                            <span class="text-sm text-blue-800 font-medium">
                              {isPaused() ? '⏸️ 일시 중지됨' : '⏳ 크롤링 실행 중...'}
                            </span>
                          </div>
                        }
                      >
                        <button
                          onClick={startCrawling}
                          class="w-full py-2.5 px-4 bg-blue-600 text-white rounded-md hover:bg-blue-700 font-medium"
                        >
                          🚀 크롤링 시작
                        </button>
                      </Show>

                      {/* 두 번째 줄: 일시 중지/재개 및 정지 버튼 */}
                      <Show when={isRunning()}>
                        <div class="grid grid-cols-2 gap-2">
                          <Show 
                            when={!isPaused()}
                            fallback={
                              <button
                                onClick={resumeCrawling}
                                class="py-2 px-3 bg-green-600 text-white rounded-md hover:bg-green-700 font-medium text-sm"
                              >
                                ▶️ 재개
                              </button>
                            }
                          >
                            <button
                              onClick={pauseCrawling}
                              class="py-2 px-3 bg-yellow-600 text-white rounded-md hover:bg-yellow-700 font-medium text-sm"
                            >
                              ⏸️ 일시 중지
                            </button>
                          </Show>
                          <button
                            onClick={stopCrawling}
                            class="py-2 px-3 bg-red-600 text-white rounded-md hover:bg-red-700 font-medium text-sm"
                          >
                            ⏹️ 완전 정지
                          </button>
                        </div>
                      </Show>
                    </div>
                    
                    {/* 상태 정보 */}
                    <Show when={currentSessionId()}>
                      <div class="mt-3 pt-3 border-t border-gray-200">
                        <div class="text-xs text-gray-600">
                          <div class="flex justify-between">
                            <span>세션 ID:</span>
                            <span class="font-mono">{currentSessionId()?.substring(0, 8)}...</span>
                          </div>
                          <div class="flex justify-between">
                            <span>상태:</span>
                            <span class={`font-medium ${
                              isPaused() ? 'text-yellow-600' : (isRunning() ? 'text-green-600' : 'text-gray-600')
                            }`}>
                              {isPaused() ? '일시 중지' : (isRunning() ? '실행 중' : '대기')}
                            </span>
                          </div>
                        </div>
                      </div>
                    </Show>
                  </div>

                  <div class="bg-amber-50 border border-amber-200 rounded-md p-3">
                    <div class="flex items-start space-x-2">
                      <span class="text-amber-600 text-sm">💡</span>
                      <div class="text-sm text-amber-800">
                        <strong>설정 변경:</strong> 크롤링 범위나 모드를 변경하려면 <strong>Settings Tab</strong>에서 수정하세요.
                      </div>
                    </div>
                  </div>
                </div>
              </Show>
            </div>

            {/* Dynamic Crawling Range Display */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-900">🤖 계산된 크롤링 범위</h2>
                <button
                  onClick={calculateCrawlingRange}
                  class="px-3 py-1.5 text-sm bg-green-100 text-green-700 rounded-md hover:bg-green-200"
                >
                  다시 계산
                </button>
              </div>
              <Show 
                when={crawlingRange()} 
                fallback={
                  <div class="bg-gray-50 border border-gray-200 rounded-lg p-4">
                    <div class="flex items-start space-x-3">
                      <span class="text-gray-500 text-lg">⏳</span>
                      <div>
                        <h3 class="text-sm font-semibold text-gray-800 mb-2">크롤링 범위 계산 중...</h3>
                        <p class="text-sm text-gray-600">
                          데이터베이스 분석을 통해 최적의 크롤링 범위를 계산하고 있습니다.
                        </p>
                      </div>
                    </div>
                  </div>
                }
              >
                <div class="space-y-4">
                  {/* 사이트 정보 */}
                  <div class="bg-blue-50 border border-blue-200 rounded-md p-4">
                    <h3 class="text-sm font-semibold text-blue-800 mb-3">🌐 사이트 정보</h3>
                    <div class="grid grid-cols-3 gap-4 text-sm">
                      <div class="space-y-1">
                        <span class="text-blue-700 block">총 페이지 수:</span>
                        <span class="font-medium text-blue-800">
                          {crawlingRange()?.site_info?.total_pages || '-'}페이지
                        </span>
                      </div>
                      <div class="space-y-1">
                        <span class="text-blue-700 block">마지막 페이지 제품:</span>
                        <span class="font-medium text-blue-800">
                          {crawlingRange()?.site_info?.products_on_last_page || '-'}개
                        </span>
                      </div>
                      <div class="space-y-1">
                        <span class="text-blue-700 block">추정 총 제품 수:</span>
                        <span class="font-medium text-blue-800">
                          {crawlingRange()?.site_info?.estimated_total_products || '-'}개
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* 로컬 DB 정보 */}
                  <div class="bg-purple-50 border border-purple-200 rounded-md p-4">
                    <h3 class="text-sm font-semibold text-purple-800 mb-3">💾 로컬 데이터베이스 정보</h3>
                    <div class="grid grid-cols-3 gap-4 text-sm">
                      <div class="space-y-1">
                        <span class="text-purple-700 block">수집된 제품 수:</span>
                        <span class="font-medium text-purple-800">
                          {crawlingRange()?.local_db_info?.total_saved_products || '-'}개
                        </span>
                      </div>
                      <div class="space-y-1">
                        <span class="text-purple-700 block">마지막 크롤링 페이지:</span>
                        <span class="font-medium text-purple-800">
                          {crawlingRange()?.local_db_info?.last_crawled_page || '-'}페이지
                        </span>
                      </div>
                      <div class="space-y-1">
                        <span class="text-purple-700 block">수집 진행률:</span>
                        <span class="font-medium text-purple-800">
                          {crawlingRange()?.local_db_info?.coverage_percentage ? 
                            `${crawlingRange()?.local_db_info?.coverage_percentage.toFixed(1)}%` : '-'}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* 크롤링 계획 */}
                  <div class="bg-green-50 border border-green-200 rounded-md p-4">
                    <h3 class="text-sm font-semibold text-green-800 mb-3">📋 크롤링 계획</h3>
                    <div class="grid grid-cols-2 gap-4 text-sm">
                      <div class="space-y-2">
                        <div class="flex justify-between">
                          <span class="text-green-700">시작 페이지:</span>
                          <span class="font-medium text-green-800">
                            {crawlingRange()?.range?.[0] || '-'}
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-green-700">종료 페이지:</span>
                          <span class="font-medium text-green-800">
                            {crawlingRange()?.range?.[1] || '-'}
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-green-700">크롤링 전략:</span>
                          <span class="font-medium text-green-800">
                            {(() => {
                              const strategy = crawlingRange()?.crawling_info?.strategy;
                              if (strategy === 'full') return '전체 크롤링';
                              if (strategy === 'partial') return '부분 크롤링';
                              if (strategy === 'none') return '완료됨';
                              return '-';
                            })()}
                          </span>
                        </div>
                      </div>
                      <div class="space-y-2">
                        <div class="flex justify-between">
                          <span class="text-green-700">크롤링할 페이지:</span>
                          <span class="font-medium text-green-800">
                            {crawlingRange()?.crawling_info?.pages_to_crawl || '-'}페이지
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-green-700">예상 신규 제품:</span>
                          <span class="font-medium text-green-800">
                            {crawlingRange()?.crawling_info?.estimated_new_products || '-'}개
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* 상세 메시지 */}
                  <div class="bg-gray-50 border border-gray-200 rounded-md p-4">
                    <h3 class="text-sm font-semibold text-gray-800 mb-2">� 계산 결과</h3>
                    <p class="text-sm text-gray-700">
                      {crawlingRange()?.message || '계산 중...'}
                    </p>
                  </div>
                        <div class="flex justify-between text-xs text-blue-600 mb-1">
                          <span>분석 진행률</span>
                          <span>{crawlingRange()?.progress?.progress_percentage?.toFixed(1)}%</span>
                        </div>
                        <div class="w-full bg-blue-200 rounded-full h-2">
                          <div 
                            class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                            style={`width: ${crawlingRange()?.progress?.progress_percentage || 0}%`}
                          ></div>
                        </div>
                      </div>
                    </Show>
                  </div>

                  {/* 비교 표시 */}
                  <Show when={userConfig()?.user?.crawling && siteStatus()}>
                    <div class="bg-yellow-50 border border-yellow-200 rounded-md p-4">
                      <h3 class="text-sm font-semibold text-yellow-800 mb-3">⚖️ 설정값과 비교</h3>
                      <div class="text-xs text-yellow-700 space-y-1">
                        <div class="flex justify-between">
                          <span>설정된 범위:</span>
                          <span class="font-medium">
                            {(() => {
                              const config = userConfig()?.user?.crawling;
                              const totalPages = siteStatus()?.total_pages || 485;
                              const pageLimit = config?.page_range_limit || 6;
                              const oldestPage = totalPages; // 가장 오래된 (485)
                              const newestPage = Math.max(1, totalPages - pageLimit + 1); // 상대적으로 최신 (480)
                              return `${oldestPage} → ${newestPage}`;
                            })()}
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span>계산된 범위:</span>
                          <span class="font-medium">
                            {(() => {
                              const range = crawlingRange()?.range;
                              if (range && range.length === 2) {
                                return `${range[0]} → ${range[1]}`;
                              }
                              return '-';
                            })()}
                          </span>
                        </div>
                        <div class="pt-2 text-xs text-yellow-600">
                          <Show 
                            when={(() => {
                              const config = userConfig()?.user?.crawling;
                              const pageLimit = config?.page_range_limit || 6;
                              const range = crawlingRange()?.range;
                              const calculatedPages = range && range.length === 2 ? range[0] - range[1] + 1 : 0;
                              return calculatedPages !== pageLimit;
                            })()}
                            fallback={<span>✅ 설정값과 계산값이 일치합니다.</span>}
                          >
                            <span>⚠️ 설정값과 계산값이 다릅니다. DB 분석 결과를 우선 적용합니다.</span>
                          </Show>
                        </div>
                      </div>
                    </div>
                  </Show>
                </div>
              </Show>
            </div>

            {/* Progress */}
            <Show when={progress()}>
              <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <div class="flex justify-between items-center mb-4">
                  <h2 class="text-lg font-semibold text-gray-900">📊 진행 상황</h2>
                  {/* 빠른 제어 버튼 */}
                  <div class="flex gap-2">
                    <Show 
                      when={!isPaused()}
                      fallback={
                        <button
                          onClick={resumeCrawling}
                          class="px-3 py-1.5 text-xs bg-green-600 text-white rounded hover:bg-green-700"
                        >
                          ▶️ 재개
                        </button>
                      }
                    >
                      <button
                        onClick={pauseCrawling}
                        class="px-3 py-1.5 text-xs bg-yellow-600 text-white rounded hover:bg-yellow-700"
                      >
                        ⏸️ 일시 중지
                      </button>
                    </Show>
                    <button
                      onClick={stopCrawling}
                      class="px-3 py-1.5 text-xs bg-red-600 text-white rounded hover:bg-red-700"
                    >
                      ⏹️ 정지
                    </button>
                  </div>
                </div>
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
                        class={`h-2 rounded-full transition-all duration-300 ${
                          isPaused() ? 'bg-yellow-500' : 'bg-blue-600'
                        }`}
                        style={`width: ${progress()?.progress_percentage || 0}%`}
                      />
                    </div>
                  </div>
                  <div class={`rounded-md p-3 ${
                    isPaused() ? 'bg-yellow-50 border border-yellow-200' : 'bg-gray-50'
                  }`}>
                    <p class={`text-sm ${
                      isPaused() ? 'text-yellow-800' : 'text-gray-700'
                    }`}>
                      {isPaused() ? '⏸️ 일시 중지됨' : `💬 ${progress()?.current_message}`}
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
