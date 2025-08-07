/**
 * CrawlingEngineTab - Advanced Crawling Engine 통합 탭
 * Phase 4A의 5단계 파이프라인을 UI에서 제어하고 모니터링
 */

import { Component, createSignal, onMount, onCleanup, Show, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { tauriApi } from '../../services/tauri-api';
import type { 
  CrawlingProgressInfo, 
  SiteStatusInfo, 
  ProductInfo, 
  CrawlingSession, 
  DatabaseStats,
  ApiResponse,
  CrawlingRangeRequest,
  CrawlingRangeResponse
} from '../../types/advanced-engine';

export const CrawlingEngineTab: Component = () => {
  // 기본 설정값을 반환하는 더미 함수 (백엔드가 설정 파일을 직접 읽음)
  // const userConfig = () => ({
  //   user: {
  //     crawling: {
  //       page_range_limit: 6,
  //       crawling_mode: 'incremental',
  //       auto_adjust_range: true,
  //       workers: {
  //         list_page_max_concurrent: 5,
  //         product_detail_max_concurrent: 10
  //       },
  //       product_list_retry_count: 2,
  //       product_detail_retry_count: 2,
  //       error_threshold_percent: 10,
  //       gap_detection_threshold: 5,
  //       binary_search_max_depth: 10,
  //       enable_data_validation: true,
  //       auto_add_to_local_db: true
  //     },
  //     batch: {
  //       batch_size: 12,
  //       batch_delay_ms: 1000,
  //       enable_batch_processing: true
  //     },
  //     max_concurrent_requests: 3,
  //     request_delay_ms: 1000
  //   },
  //   advanced: {
  //     request_timeout_seconds: 30,
  //     retry_delay_ms: 2000
  //   }
  // });

  // 더미 함수 - 실제로는 백엔드가 설정 파일을 자동으로 읽음
  // const loadUserConfig = () => {
  //   addLog('ℹ️ 백엔드가 설정 파일을 자동으로 읽어 사용합니다');
  // };
  
  // const [showAdvancedSettings, setShowAdvancedSettings] = createSignal(false);
  const [siteStatus, setSiteStatus] = createSignal<SiteStatusInfo | null>(null);
  const [progress, setProgress] = createSignal<CrawlingProgressInfo | null>(null);
  const [recentProducts, setRecentProducts] = createSignal<ProductInfo[]>([]);
  const [logs, setLogs] = createSignal<string[]>([]);
  const [isRunning, setIsRunning] = createSignal(false);
  const [isPaused, setIsPaused] = createSignal(false);
  const [currentSessionId, setCurrentSessionId] = createSignal<string | null>(null);
  const [dbStats, setDbStats] = createSignal<DatabaseStats | null>(null);
  const [crawlingRange, setCrawlingRange] = createSignal<CrawlingRangeResponse | null>(null);
  const [showSiteStatus, setShowSiteStatus] = createSignal(true);
  const [batchSize, setBatchSize] = createSignal(3); // 기본값 3, 실제 설정에서 로드됨

  // Log helper
  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-19), `[${timestamp}] ${message}`]);
  };

  // 설정 로드
  const loadConfig = async () => {
    try {
      const backendConfig = await tauriApi.getComprehensiveCrawlerConfig();
      setBatchSize(backendConfig.batch_size);
      addLog(`📋 설정 로드 완료: batch_size=${backendConfig.batch_size}`);
    } catch (error) {
      addLog(`❌ 설정 로드 실패: ${error}`);
    }
  };

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
  };

  // Initialize and load data
  onMount(async () => {
    addLog('🎯 Advanced Crawling Engine 탭 로드됨');
    
    await checkSiteStatus(); // 이 함수 내에서 이미 calculateCrawlingRange() 호출됨
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
    
    try {
      setIsRunning(true);
      
      addLog(`🚀 Actor System Crawling 시작 - 실시간 이벤트 모니터링`);
      
      // ✅ Actor 시스템 방식: 실시간 이벤트가 있는 크롤링
      const sessionId = await invoke<string>('start_crawling_session');
      
      setCurrentSessionId(sessionId);
      addLog(`✅ Actor 시스템 크롤링 세션 시작: ${sessionId}`);
      
    } catch (error) {
      setIsRunning(false);
      addLog(`❌ Actor 시스템 크롤링 시작 실패: ${error}`);
      console.error('Actor 시스템 크롤링 시작 오류:', error);
    }
  };

  // 가짜 Actor 시스템 크롤링 (실제로는 ServiceBased)
  const startFakeActorSystemWithCalculatedRange = async () => {
    if (isRunning()) return;
    
    setIsRunning(true);
    addLog(`🎭 가짜 Actor 시스템 크롤링 시작 (실제로는 ServiceBased)`);

    try {
      const result = await invoke('start_actor_system_crawling', {
        request: {
          // 🧠 CrawlingPlanner가 모든 범위를 자동 계산하므로 0으로 설정 (By Design)
          start_page: 0,
          end_page: 0,
          concurrency: 64,
          batch_size: 3,
          delay_ms: 100
        }
      });
      addLog(`✅ 가짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      addLog('🎭 가짜 Actor 시스템이 활성화되었습니다 (실제로는 ServiceBased 엔진).');
      
    } catch (error) {
      console.error('가짜 Actor 시스템 크롤링 시작 실패:', error);
      addLog(`❌ 가짜 Actor 시스템 크롤링 시작 실패: ${error}`);
      setIsRunning(false);
    }
  };

  // 진짜 Actor 시스템 설정 기반 크롤링
  const startRealActorSystemWithCalculatedRange = async () => {
    if (isRunning()) return;
    
    setIsRunning(true);
    addLog('🎭 진짜 Actor 시스템 크롤링 시작 (CrawlingPlanner 설정 기반)');

    try {
      // 먼저 배치 플랜을 계산해서 설정값을 가져옵니다
      const crawlingRange = await invoke('calculate_crawling_range') as CrawlingRangeResponse;
      const configBasedBatchSize = crawlingRange?.batch_plan?.batch_size || 9; // 기본값 9
      
      addLog(`📋 설정 기반 배치 크기: ${configBasedBatchSize}`);
      
      const result = await invoke('start_actor_system_crawling', {
        request: {
          // 🧠 CrawlingPlanner 설정을 기반으로 한 값들 사용
          start_page: 0,     // By Design: 프론트엔드에서 범위 지정하지 않음
          end_page: 0,       // By Design: 프론트엔드에서 범위 지정하지 않음  
          concurrency: 64,
          batch_size: configBasedBatchSize, // 설정파일에서 읽은 값 사용
          delay_ms: 100
        }
      });
      addLog(`✅ 진짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      addLog('🎭 진짜 Actor 시스템이 활성화되었습니다. CrawlingPlanner 설정 기반으로 SessionActor가 실행됩니다.');
      
    } catch (error) {
      console.error('진짜 Actor 시스템 크롤링 시작 실패:', error);
      addLog(`❌ 진짜 Actor 시스템 크롤링 시작 실패: ${error}`);
      setIsRunning(false);
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
                <div class="flex items-center space-x-2">
                  <h2 class="text-lg font-semibold text-gray-900">🌐 사이트 상태</h2>
                  <button
                    onClick={() => setShowSiteStatus(!showSiteStatus())}
                    class="text-gray-500 hover:text-gray-700 transition-colors"
                  >
                    {showSiteStatus() ? '🔽' : '▶️'}
                  </button>
                </div>
                <button
                  onClick={checkSiteStatus}
                  class="px-3 py-1.5 text-sm bg-blue-100 text-blue-700 rounded-md hover:bg-blue-200"
                >
                  새로고침
                </button>
              </div>
              
              <Show when={showSiteStatus()}>
                <Show
                  when={siteStatus()}
                  fallback={<p class="text-gray-500">사이트 상태를 확인 중...</p>}
                >
                  <div class="space-y-4">
                    {/* 기본 사이트 정보 */}
                    <div class="grid grid-cols-2 gap-4">
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
                        <div class="flex justify-between">
                          <span class="text-gray-600">예상 제품 수:</span>
                          <span class="font-medium">{siteStatus()?.estimated_total_products || 0}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">마지막 페이지 제품:</span>
                          <span class="font-medium">{siteStatus()?.products_on_last_page || 0}</span>
                        </div>
                      </div>
                      
                      <div class="space-y-2 text-sm">
                        <div class="flex justify-between">
                          <span class="text-gray-600">상태 점수:</span>
                          <span class={`font-medium ${
                            (siteStatus()?.health_score || 0) > 0.8 ? 'text-green-600' : 
                            (siteStatus()?.health_score || 0) > 0.5 ? 'text-yellow-600' : 'text-red-600'
                          }`}>
                            {((siteStatus()?.health_score || 0) * 100).toFixed(1)}%
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">응답 시간:</span>
                          <span class="font-medium">{siteStatus()?.response_time_ms || 0}ms</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">마지막 확인:</span>
                          <span class="font-medium text-xs">방금 전</span>
                        </div>
                      </div>
                    </div>

                    {/* 크롤링 범위 정보 */}
                    <Show when={crawlingRange()?.success}>
                      <div class="border-t pt-4">
                        <h3 class="font-medium text-gray-900 mb-2">📊 권장 크롤링 범위</h3>
                        <div class="bg-blue-50 border border-blue-200 rounded-md p-3">
                          <div class="flex items-center justify-between">
                            <span class="text-sm text-blue-700">
                              페이지 {crawlingRange()?.range?.[0]} → {crawlingRange()?.range?.[1]} 
                              ({(crawlingRange()?.range?.[0] || 0) - (crawlingRange()?.range?.[1] || 0) + 1}페이지)
                            </span>
                            <span class="text-xs text-blue-600 font-mono">
                              {crawlingRange()?.crawling_info?.strategy || 'auto'}
                            </span>
                          </div>
                          <p class="text-xs text-blue-600 mt-1">
                            {crawlingRange()?.message || '자동 계산된 최적 범위'}
                          </p>
                        </div>
                      </div>
                    </Show>

                    {/* 데이터베이스 현황 */}
                    <Show when={dbStats()}>
                      <div class="border-t pt-4">
                        <h3 class="font-medium text-gray-900 mb-2">💾 로컬 데이터베이스</h3>
                        <div class="grid grid-cols-2 gap-4 text-sm">
                          <div class="flex justify-between">
                            <span class="text-gray-600">저장된 제품:</span>
                            <span class="font-medium">{dbStats()?.total_products || 0}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-600">오늘 추가:</span>
                            <span class="font-medium">{dbStats()?.products_added_today || 0}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-600">마지막 업데이트:</span>
                            <span class="font-medium text-xs">
                              {dbStats()?.last_updated ? 
                                new Date(dbStats()!.last_updated!).toLocaleDateString() : 
                                '데이터 없음'
                              }
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-600">DB 크기:</span>
                            <span class="font-medium">
                              {dbStats()?.database_size_bytes ? 
                                `${(dbStats()!.database_size_bytes / 1024 / 1024).toFixed(1)}MB` : 
                                '0MB'
                              }
                            </span>
                          </div>
                        </div>
                      </div>
                    </Show>
                  </div>
                </Show>
              </Show>
            </div>

            {/* Actor System Controls */}
            <div class="bg-gradient-to-r from-purple-50 to-indigo-50 rounded-lg shadow-sm border border-purple-200 p-6 mb-6">
              <h2 class="text-lg font-semibold text-purple-900 mb-4">🎭 Actor 시스템 크롤링</h2>
              <div class="space-y-4">
                
                {/* Calculated Range Display */}
                <Show when={crawlingRange()?.range}>
                  <div class="bg-purple-100 border border-purple-300 rounded-md p-3">
                    <div class="text-sm text-purple-800">
                      <strong>📊 CrawlingPlanner 계산 결과:</strong><br/>
                      크롤링 범위: <span class="font-mono font-bold">{crawlingRange()?.range?.[0]} → {crawlingRange()?.range?.[1]}</span> 
                      ({(crawlingRange()?.range?.[0] || 0) - (crawlingRange()?.range?.[1] || 0) + 1} 페이지)<br/>
                      <span class="text-xs">• 설정, 사이트 상태, DB 상태를 종합하여 자동 계산됨</span>
                      
                      {/* Batch Execution Plan */}
                      <div class="mt-3 pt-3 border-t border-purple-200">
                        <strong>📦 배치 실행 계획 (batch_size={crawlingRange()?.batch_plan?.batch_size || 'N/A'}):</strong><br/>
                        <div class="mt-1 space-y-1">
                          {(() => {
                            const batchPlan = crawlingRange()?.batch_plan;
                            if (!batchPlan || !batchPlan.batches.length) return null;
                            
                            return batchPlan.batches.map((batch) => (
                              <div class="text-xs font-mono bg-purple-50 px-2 py-1 rounded">
                                <span class="text-purple-700">Batch {batch.batch_id + 1}:</span> 
                                <span class="text-purple-900"> [{batch.pages.join(', ')}]</span>
                                <span class="text-purple-600"> ({batch.pages.length}페이지, ~{batch.estimated_products}제품)</span>
                              </div>
                            ));
                          })()}
                        </div>
                        
                        {/* 추가 배치 계획 정보 */}
                        {crawlingRange()?.batch_plan && (
                          <div class="mt-2 text-xs text-purple-600">
                            <div>• 총 배치 수: {crawlingRange()!.batch_plan.total_batches}개</div>
                            <div>• 동시 실행 제한: {crawlingRange()!.batch_plan.concurrency_limit}</div>
                            <div>• 실행 전략: {crawlingRange()!.batch_plan.execution_strategy}</div>
                            <div>• 예상 소요 시간: {Math.floor(crawlingRange()!.batch_plan.estimated_duration_seconds / 60)}분</div>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </Show>

                {/* Debug: Batch Plan Calculation Button */}
                <button
                  onClick={calculateCrawlingRange}
                  class="w-full py-2 px-4 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 font-medium text-sm"
                >
                  🔍 크롤링 범위 및 배치 플랜 계산
                  <span class="text-xs block mt-1">설정파일 batch_size=9로 배치 플랜을 생성합니다</span>
                </button>

                {/* Real Actor System Main Button */}
                <button
                  onClick={startRealActorSystemWithCalculatedRange}
                  class="w-full py-3 px-4 bg-purple-600 text-white rounded-md hover:bg-purple-700 font-medium disabled:bg-gray-400 disabled:cursor-not-allowed"
                  disabled={isRunning()}
                >
                  🎭 진짜 Actor 시스템으로 크롤링 시작 (설정 기반)
                  <span class="text-xs block mt-1">CrawlingPlanner가 자동으로 범위와 배치를 계산합니다</span>
                </button>
                
                {/* Fake Actor System Button */}
                <button
                  onClick={startFakeActorSystemWithCalculatedRange}
                  class="w-full py-3 px-4 bg-orange-600 text-white rounded-md hover:bg-orange-700 font-medium disabled:bg-gray-400 disabled:cursor-not-allowed"
                  disabled={isRunning()}
                >
                  🎭 가짜 Actor 시스템으로 크롤링 시작 (ServiceBased 엔진)
                  <span class="text-xs block mt-1">백엔드에서 자동으로 범위를 계산합니다</span>
                </button>                
              </div>
            </div>

            {/* Crawling Controls */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <h2 class="text-lg font-semibold text-gray-900 mb-4">🎮 크롤링 제어</h2>
              <div class="space-y-3">
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
