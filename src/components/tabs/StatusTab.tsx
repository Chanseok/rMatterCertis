/**
 * StatusTab - 상태 & 제어 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, createMemo, Show, onMount } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { crawlerStore } from '../../stores/crawlerStore';
import { CrawlingService } from '../../services/crawlingService';
import { loggingService } from '../../services/loggingService';
import type { SiteStatus } from '../../types/crawling';
import { 
  getDataChangeStatusDisplayName, 
  getDataChangeStatusColor,
  getSeverityLevelDisplayName,
  getSeverityLevelColor,
  getRecommendedActionDisplayName 
} from '../../types/crawling';

export const StatusTab: Component = () => {
  const [isControlExpanded, setIsControlExpanded] = createSignal(true);
  const [isStatusExpanded, setIsStatusExpanded] = createSignal(true);
  const [isLoading, setIsLoading] = createSignal(false);
  const [statusCheckResult, setStatusCheckResult] = createSignal<SiteStatus | null>(null);
  const [statusError, setStatusError] = createSignal<string | null>(null);
  const [isCleaningLogs, setIsCleaningLogs] = createSignal(false);
  const [cleanupResult, setCleanupResult] = createSignal<string | null>(null);

  // Initialize logging component
  loggingService.setComponent('StatusTab');

  const stageInfo = createMemo(() => {
    const stage = crawlerStore.currentStage();
    const stages = {
      'ListCrawling': { text: '1단계: 목록 수집', color: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300' },
      'Verification': { text: '2단계: 검증', color: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300' },
      'DetailCrawling': { text: '3단계: 상세정보', color: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300' },
      'Idle': { text: '대기 중', color: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300' }
    };
    return stages[stage as keyof typeof stages] || stages['Idle'];
  });

  // 상태 체크 실행 함수
  const handleStatusCheck = async () => {
    try {
      setIsLoading(true);
      setStatusError(null);
      
      await loggingService.logUserAction('StatusCheck', { action: 'site_status_check_initiated' });
      console.log('Starting backend site status check...');
      
      const startTime = Date.now();
      const result = await CrawlingService.checkSiteStatus();
      const duration = Date.now() - startTime;
      
      console.log('Backend site status check result:', result);
      console.log('Data change status structure:', JSON.stringify(result.data_change_status, null, 2));
      
      await loggingService.logApiCall('POST', 'check_site_status', duration);
      await loggingService.info(`Site status check completed: ${result.total_pages} pages, ${result.estimated_products} products`);
      
      setStatusCheckResult(result);
      
      // 프론트엔드 스토어 상태도 업데이트
      await crawlerStore.refreshStatus();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error('Failed to check site status:', error);
      
      await loggingService.error(`Site status check failed: ${errorMessage}`);
      await loggingService.logUserAction('StatusCheck', { action: 'site_status_check_failed', error: errorMessage });
      
      setStatusError(errorMessage);
    } finally {
      setIsLoading(false);
    }
  };

  // 로그 정리 실행 함수
  const handleLogCleanup = async () => {
    try {
      setIsCleaningLogs(true);
      setCleanupResult(null);
      
      await loggingService.logUserAction('LogCleanup', { action: 'log_cleanup_initiated' });
      console.log('Starting log cleanup...');
      
      const result = await loggingService.cleanupLogs();
      console.log('Log cleanup result:', result);
      
      setCleanupResult(result);
      await loggingService.info(`Log cleanup completed: ${result}`);
      
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error('Failed to cleanup logs:', error);
      
      await loggingService.error(`Log cleanup failed: ${errorMessage}`);
      await loggingService.logUserAction('LogCleanup', { action: 'log_cleanup_failed', error: errorMessage });
      
      setCleanupResult(`실패: ${errorMessage}`);
    } finally {
      setIsCleaningLogs(false);
    }
  };

  const handleStart = async () => {
    try {
      await loggingService.logUserAction('CrawlingStart', { action: 'crawling_start_initiated' });
      
      // 기본 설정으로 크롤링 시작 (실제로는 설정 탭에서 가져와야 함)
      const defaultConfig = {
        start_page: 1,
        end_page: 100,
        concurrency: 6,
        delay_ms: 1000,
        page_range_limit: 100,
        product_list_retry_count: 3,
        product_detail_retry_count: 3,
        products_per_page: 12,
        auto_add_to_local_db: true,
        auto_status_check: true,
        crawler_type: "advanced",
        batch_size: 50,
        batch_delay_ms: 5000,
        enable_batch_processing: true,
        batch_retry_limit: 3,
        base_url: "",
        matter_filter_url: "",
        page_timeout_ms: 30000,
        product_detail_timeout_ms: 15000,
        initial_concurrency: 6,
        detail_concurrency: 12,
        retry_concurrency: 3,
        min_request_delay_ms: 500,
        max_request_delay_ms: 2000,
        retry_start: 1,
        retry_max: 3,
        cache_ttl_ms: 300000,
        headless_browser: true,
        max_concurrent_tasks: 12,
        request_delay: 1000,
        logging: {
          level: "info",
          enable_stack_trace: true,
          enable_timestamp: true,
          components: {}
        }
      };
      
      await crawlerStore.startCrawling(defaultConfig);
      await loggingService.info('Crawling started successfully');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      await loggingService.error(`Failed to start crawling: ${errorMessage}`);
      console.error('Failed to start crawling:', error);
    }
  };

  const handleStop = async () => {
    try {
      await loggingService.logUserAction('CrawlingStop', { action: 'crawling_stop_initiated' });
      await crawlerStore.stopCrawling();
      await loggingService.info('Crawling stopped successfully');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      await loggingService.error(`Failed to stop crawling: ${errorMessage}`);
      console.error('Failed to stop crawling:', error);
    }
  };

  const isRunning = createMemo(() => {
    const status = crawlerStore.status();
    return status === 'Running';
  });

  const progressPercent = createMemo(() => {
    const progress = crawlerStore.progress();
    if (!progress || !progress.total || progress.total === 0) {
      return 0;
    }
    return Math.round((progress.current / progress.total) * 100);
  });

  // 상태 체크 결과에서 값을 계산하는 함수들
  const getHealthStatusText = (result: SiteStatus | null) => {
    if (!result) return { text: '정보 없음', color: 'text-gray-600' };
    
    const score = result.health_score;
    if (score >= 0.8) return { text: '좋음', color: 'text-green-600' };
    if (score >= 0.5) return { text: '보통', color: 'text-yellow-600' };
    return { text: '나쁨', color: 'text-red-600' };
  };

  return (
    <div class="flex flex-col space-y-4 p-4">
      {/* 현재 상태 및 제어 섹션 */}
      <ExpandableSection 
        title="크롤링 상태 및 제어" 
        isExpanded={isControlExpanded()} 
        onToggle={() => setIsControlExpanded(!isControlExpanded())}
      >
        <div class="space-y-4 p-2">
          <div class="grid grid-cols-2 gap-4">
            <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
              <h3 class="text-lg font-medium mb-2">현재 상태</h3>
              <div class={`inline-block px-3 py-1 rounded-full text-sm font-medium ${stageInfo().color}`}>
                {stageInfo().text}
              </div>
              
              <Show when={isRunning()}>
                <div class="mt-4">
                  <div class="w-full bg-gray-200 rounded-full h-2.5 dark:bg-gray-700">
                    <div class="bg-blue-600 h-2.5 rounded-full" style={{ width: `${progressPercent()}%` }}></div>
                  </div>
                  <p class="text-sm mt-1">{progressPercent()}% 완료</p>
                </div>
              </Show>
            </div>
            
            <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
              <h3 class="text-lg font-medium mb-2">제어</h3>
              <div class="space-y-3">
                <div class="flex space-x-2">
                  <button 
                    class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
                    onClick={handleStart}
                    disabled={isRunning()}
                  >
                    크롤링 시작
                  </button>
                  <button 
                    class="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50"
                    onClick={handleStop}
                    disabled={!isRunning()}
                  >
                    중지
                  </button>
                </div>
                <div class="flex space-x-2">
                  <button 
                    class="px-3 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600 disabled:opacity-50 text-sm"
                    onClick={handleLogCleanup}
                    disabled={isCleaningLogs()}
                  >
                    {isCleaningLogs() ? '정리 중...' : '로그 정리'}
                  </button>
                </div>
                
                {/* 로그 정리 결과 표시 */}
                <Show when={cleanupResult()}>
                  <div class="p-2 bg-green-50 border border-green-200 rounded text-sm text-green-700">
                    {cleanupResult()}
                  </div>
                </Show>
              </div>
            </div>
          </div>
        </div>
      </ExpandableSection>

      {/* 사이트 상태 확인 섹션 */}
      <ExpandableSection 
        title="사이트 상태 체크" 
        isExpanded={isStatusExpanded()} 
        onToggle={() => setIsStatusExpanded(!isStatusExpanded())}
      >
        <div class="space-y-4 p-2">
          <div class="flex justify-between items-center">
            <button 
              class="px-4 py-2 bg-indigo-500 text-white rounded hover:bg-indigo-600 disabled:opacity-50"
              onClick={handleStatusCheck}
              disabled={isLoading()}
            >
              {isLoading() ? '확인 중...' : '상태 체크'}
            </button>
            
            <Show when={statusCheckResult()}>
              <div class="text-sm text-gray-500">
                마지막 확인: {statusCheckResult()?.last_check_time 
                  ? new Date(statusCheckResult()!.last_check_time).toLocaleString() 
                  : '없음'}
              </div>
            </Show>
          </div>
          
          <Show when={statusError()}>
            <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
              <p class="font-bold">오류 발생</p>
              <p>{statusError()}</p>
            </div>
          </Show>
          
          <Show when={statusCheckResult()}>
            <div class="space-y-4">
              {/* 사이트 정보 섹션 */}
              <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
                <h3 class="text-lg font-medium mb-2">사이트 정보</h3>
                <ul class="space-y-2">
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">접근 가능:</span> 
                    <span class={statusCheckResult()?.is_accessible ? 'text-green-600' : 'text-red-600'}>
                      {statusCheckResult()?.is_accessible ? '예' : '아니오'}
                    </span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">총 페이지 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.total_pages || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">예상 제품 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.estimated_products || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">응답 시간:</span> 
                    <span class="font-medium">{statusCheckResult()?.response_time_ms || 0}ms</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">건강 상태:</span> 
                    <span class={getHealthStatusText(statusCheckResult()).color}>
                      {getHealthStatusText(statusCheckResult()).text} 
                      ({(statusCheckResult()?.health_score || 0).toFixed(2)})
                    </span>
                  </li>
                </ul>
              </div>
              
              {/* 데이터 변화 상태 섹션 */}
              <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
                <h3 class="text-lg font-medium mb-2">데이터 변화 상태</h3>
                <div class="space-y-3">
                  <div class="flex justify-between items-center">
                    <span class="text-gray-600 dark:text-gray-400">상태:</span>
                    <span class={getDataChangeStatusColor(statusCheckResult()?.data_change_status!)}>
                      {getDataChangeStatusDisplayName(statusCheckResult()?.data_change_status!)}
                    </span>
                  </div>
                  
                  {/* 상세 데이터 정보 */}
                  <Show when={statusCheckResult()?.data_change_status}>
                    <div class="mt-4 p-3 bg-gray-50 dark:bg-gray-700 rounded-lg">
                      <h4 class="text-sm font-medium mb-2">상세 정보</h4>
                      <div class="space-y-1 text-sm">
                        {/* Raw JSON for debugging */}
                        <div class="font-mono text-xs text-gray-600 dark:text-gray-400 p-2 bg-gray-100 dark:bg-gray-800 rounded">
                          <strong>원본 데이터:</strong><br/>
                          {JSON.stringify(statusCheckResult()?.data_change_status, null, 2)}
                        </div>
                        
                        {/* Safe data extraction */}
                        <Show when={statusCheckResult()?.data_change_status && typeof statusCheckResult()?.data_change_status === 'object'}>
                          <div class="mt-2">
                            <Show when={(statusCheckResult()?.data_change_status as any)?.Increased}>
                              <div class="space-y-1 text-green-600">
                                <div class="flex justify-between">
                                  <span>이전 개수:</span>
                                  <span>{(statusCheckResult()?.data_change_status as any)?.Increased?.previous_count || 'N/A'}</span>
                                </div>
                                <div class="flex justify-between">
                                  <span>현재 개수:</span>
                                  <span>{(statusCheckResult()?.data_change_status as any)?.Increased?.new_count || 'N/A'}</span>
                                </div>
                              </div>
                            </Show>
                            
                            <Show when={(statusCheckResult()?.data_change_status as any)?.Decreased}>
                              <div class="space-y-1 text-red-600">
                                <div class="flex justify-between">
                                  <span>이전 개수:</span>
                                  <span>{(statusCheckResult()?.data_change_status as any)?.Decreased?.previous_count || 'N/A'}</span>
                                </div>
                                <div class="flex justify-between">
                                  <span>현재 개수:</span>
                                  <span>{(statusCheckResult()?.data_change_status as any)?.Decreased?.current_count || 'N/A'}</span>
                                </div>
                                <div class="flex justify-between">
                                  <span>감소량:</span>
                                  <span>-{(statusCheckResult()?.data_change_status as any)?.Decreased?.decrease_amount || 'N/A'}</span>
                                </div>
                              </div>
                            </Show>
                            
                            <Show when={(statusCheckResult()?.data_change_status as any)?.Stable}>
                              <div class="flex justify-between text-blue-600">
                                <span>제품 개수:</span>
                                <span>{(statusCheckResult()?.data_change_status as any)?.Stable?.count || 'N/A'}</span>
                              </div>
                            </Show>
                            
                            <Show when={(statusCheckResult()?.data_change_status as any)?.Initial}>
                              <div class="flex justify-between text-gray-600">
                                <span>초기 개수:</span>
                                <span>{(statusCheckResult()?.data_change_status as any)?.Initial?.count || 'N/A'}</span>
                              </div>
                            </Show>
                          </div>
                        </Show>
                      </div>
                    </div>
                  </Show>
                  
                  {/* 데이터 감소 경고 및 권장 사항 */}
                  <Show when={statusCheckResult()?.decrease_recommendation}>
                    <div class="mt-4 p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
                      <div class="flex items-center mb-2">
                        <svg class="w-5 h-5 text-yellow-400 mr-2" fill="currentColor" viewBox="0 0 20 20">
                          <path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
                        </svg>
                        <h4 class="text-lg font-medium text-yellow-800">데이터 감소 감지</h4>
                      </div>
                      
                      <div class="space-y-2">
                        <div class="flex justify-between">
                          <span class="text-yellow-700">권장 조치:</span>
                          <span class="font-medium text-yellow-800">
                            {getRecommendedActionDisplayName(statusCheckResult()?.decrease_recommendation?.action_type!)}
                          </span>
                        </div>
                        
                        <div class="flex justify-between">
                          <span class="text-yellow-700">심각도:</span>
                          <span class={getSeverityLevelColor(statusCheckResult()?.decrease_recommendation?.severity!)}>
                            {getSeverityLevelDisplayName(statusCheckResult()?.decrease_recommendation?.severity!)}
                          </span>
                        </div>
                        
                        <div class="mt-3">
                          <p class="text-yellow-700 text-sm">{statusCheckResult()?.decrease_recommendation?.description}</p>
                        </div>
                        
                        <Show when={statusCheckResult()?.decrease_recommendation?.action_steps?.length}>
                          <div class="mt-3">
                            <h5 class="text-yellow-800 font-medium mb-2">권장 단계:</h5>
                            <ol class="list-decimal list-inside space-y-1 text-sm text-yellow-700">
                              {statusCheckResult()?.decrease_recommendation?.action_steps?.map((step) => (
                                <li>{step}</li>
                              ))}
                            </ol>
                          </div>
                        </Show>
                      </div>
                    </div>
                  </Show>
                </div>
              </div>
            </div>
          </Show>
        </div>
      </ExpandableSection>
    </div>
  );
};
