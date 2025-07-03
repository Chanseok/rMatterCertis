/**
 * StatusTab - 상태 & 제어 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, createMemo, Show } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { crawlerStore } from '../../stores/crawlerStore';
import { CrawlingService, ComprehensiveStatusResponse } from '../../services/crawlingService';

export const StatusTab: Component = () => {
  const [isControlExpanded, setIsControlExpanded] = createSignal(true);
  const [isStatusExpanded, setIsStatusExpanded] = createSignal(true);
  const [isCompareExpanded, setIsCompareExpanded] = createSignal(true);
  const [isLoading, setIsLoading] = createSignal(false);
  const [statusCheckResult, setStatusCheckResult] = createSignal<ComprehensiveStatusResponse | null>(null);
  const [statusError, setStatusError] = createSignal<string | null>(null);

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
      console.log('Starting backend site status check...');
      
      const result = await CrawlingService.checkSiteStatus();
      console.log('Backend site status check result:', result);
      setStatusCheckResult(result);
      
      // 프론트엔드 스토어 상태도 업데이트
      await crawlerStore.refreshStatus();
    } catch (error) {
      console.error('Failed to check site status:', error);
      setStatusError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoading(false);
    }
  };

  const handleStart = async () => {
    try {
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
    } catch (error) {
      console.error('Failed to start crawling:', error);
    }
  };

  const handleStop = async () => {
    try {
      await crawlerStore.stopCrawling();
    } catch (error) {
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
  const getRecommendedAction = (result: ComprehensiveStatusResponse | null) => {
    if (!result || !result.comparison) return null;
    
    const { recommended_action } = result.comparison;
    switch (recommended_action) {
      case 'crawling_needed':
        return { text: '크롤링 필요', color: 'text-blue-600' };
      case 'cleanup_needed':
        return { text: '정리 필요', color: 'text-yellow-600' };
      case 'up_to_date':
        return { text: '최신 상태', color: 'text-green-600' };
      default:
        return { text: '정보 없음', color: 'text-gray-600' };
    }
  };
  
  const getHealthStatusText = (result: ComprehensiveStatusResponse | null) => {
    if (!result || !result.site_status) return { text: '정보 없음', color: 'text-gray-600' };
    
    const score = result.site_status.health_score;
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
                마지막 확인: {statusCheckResult()?.site_status.last_check 
                  ? new Date(statusCheckResult()!.site_status.last_check).toLocaleString() 
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
            <div class="grid grid-cols-2 gap-4">
              <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
                <h3 class="text-lg font-medium mb-2">사이트 정보</h3>
                <ul class="space-y-2">
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">접근 가능:</span> 
                    <span class={statusCheckResult()?.site_status.accessible ? 'text-green-600' : 'text-red-600'}>
                      {statusCheckResult()?.site_status.accessible ? '예' : '아니오'}
                    </span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">총 페이지 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.site_status.total_pages || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">예상 제품 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.site_status.estimated_products || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">응답 시간:</span> 
                    <span class="font-medium">{statusCheckResult()?.site_status.response_time_ms || 0}ms</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">건강 상태:</span> 
                    <span class={getHealthStatusText(statusCheckResult()).color}>
                      {getHealthStatusText(statusCheckResult()).text} 
                      ({(statusCheckResult()?.site_status.health_score || 0).toFixed(2)})
                    </span>
                  </li>
                </ul>
              </div>
              
              <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
                <h3 class="text-lg font-medium mb-2">데이터베이스 정보</h3>
                <ul class="space-y-2">
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">총 제품 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.database_analysis.total_products || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">고유 제품 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.database_analysis.unique_products || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">중복 수:</span> 
                    <span class="font-medium">{statusCheckResult()?.database_analysis.duplicate_count || 0}</span>
                  </li>
                  <li class="flex justify-between">
                    <span class="text-gray-600 dark:text-gray-400">데이터 품질 점수:</span> 
                    <span class="font-medium">
                      {(statusCheckResult()?.database_analysis.data_quality_score || 0).toFixed(2)}
                    </span>
                  </li>
                </ul>
              </div>
            </div>
          </Show>
        </div>
      </ExpandableSection>
      
      {/* 비교 및 권장 작업 */}
      <Show when={statusCheckResult()?.comparison}>
        <ExpandableSection 
          title="비교 및 권장 작업" 
          isExpanded={isCompareExpanded()} 
          onToggle={() => setIsCompareExpanded(!isCompareExpanded())}
        >
          <div class="space-y-4 p-2">
            <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <h3 class="text-lg font-medium mb-2">데이터 비교</h3>
                  <ul class="space-y-2">
                    <li class="flex justify-between">
                      <span class="text-gray-600 dark:text-gray-400">웹사이트 제품 수:</span> 
                      <span class="font-medium">{statusCheckResult()?.site_status.estimated_products || 0}</span>
                    </li>
                    <li class="flex justify-between">
                      <span class="text-gray-600 dark:text-gray-400">데이터베이스 제품 수:</span> 
                      <span class="font-medium">{statusCheckResult()?.database_analysis.total_products || 0}</span>
                    </li>
                    <li class="flex justify-between">
                      <span class="text-gray-600 dark:text-gray-400">차이:</span> 
                      <span class={statusCheckResult()?.comparison?.difference || 0 > 0 ? 'text-blue-600' : 'text-green-600'}>
                        {statusCheckResult()?.comparison?.difference || 0} 제품
                      </span>
                    </li>
                    <li class="flex justify-between">
                      <span class="text-gray-600 dark:text-gray-400">동기화 비율:</span> 
                      <span class="font-medium">
                        {(statusCheckResult()?.comparison?.sync_percentage || 0).toFixed(1)}%
                      </span>
                    </li>
                  </ul>
                </div>
                
                <div>
                  <h3 class="text-lg font-medium mb-2">권장 작업</h3>
                  <div class="flex items-center justify-center h-full">
                    <div class={`text-xl font-bold ${getRecommendedAction(statusCheckResult())?.color}`}>
                      {getRecommendedAction(statusCheckResult())?.text}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </ExpandableSection>
      </Show>
    </div>
  );
};
