/**
 * StatusTab - 상태 & 제어 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, createMemo, Show, onMount } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { CrawlingProgressDisplay } from '../crawling/CrawlingProgressDisplay';
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
      console.log('Data change status structure:', JSON.stringify((result as any).data_change_status, null, 2));
      
      // Extract site status from the nested response
      const siteStatus = (result as any).site_status || result;
      
      await loggingService.logApiCall('POST', 'check_site_status', duration);
      await loggingService.info(`Site status check completed: ${siteStatus.total_pages || 'unknown'} pages, ${siteStatus.estimated_products || 'unknown'} products`);
      
      // Convert the backend response to the expected SiteStatus format
      const convertedResult: SiteStatus = {
        is_accessible: siteStatus.accessible || siteStatus.is_accessible,
        response_time_ms: siteStatus.response_time_ms,
        total_pages: siteStatus.total_pages,
        estimated_products: siteStatus.estimated_products,
        last_check_time: siteStatus.last_check || siteStatus.last_check_time,
        health_score: siteStatus.health_score,
        data_change_status: siteStatus.data_change_status,
        decrease_recommendation: siteStatus.decrease_recommendation
      };
      
      setStatusCheckResult(convertedResult);
      
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

  // 사이트 로컬 비교를 위한 헬퍼 함수들
  const getLocalProductCount = (result: SiteStatus | null): number => {
    if (!result) return 0;
    // 데이터베이스 분석 결과에서 제품 수를 가져옴
    const dbAnalysis = (result as any)?.data_change_status?.database_analysis;
    return dbAnalysis?.total_products || 0;
  };

  const getDifferenceText = (result: SiteStatus | null): string => {
    if (!result) return '정보 없음';
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    const diff = siteCount - localCount;
    
    if (diff > 0) {
      return `+${diff.toLocaleString()}개`;
    } else if (diff < 0) {
      return `${diff.toLocaleString()}개`;
    } else {
      return '동일';
    }
  };

  const getDifferenceColor = (result: SiteStatus | null): string => {
    if (!result) return 'text-gray-600';
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    const diff = siteCount - localCount;
    
    if (diff > 0) return 'text-red-600'; // 빨간색: 새 데이터 있음
    if (diff < 0) return 'text-orange-600'; // 주황색: 데이터 감소
    return 'text-green-600'; // 초록색: 동일
  };

  const getCrawlingNeededText = (result: SiteStatus | null): string => {
    if (!result) return '정보 없음';
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    return siteCount > localCount ? '예' : '아니오';
  };

  const getCrawlingNeededColor = (result: SiteStatus | null): string => {
    if (!result) return 'text-gray-600';
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    return siteCount > localCount ? 'text-red-600' : 'text-green-600';
  };

  const getRecommendedRange = (result: SiteStatus | null): string => {
    if (!result) return '정보 없음';
    const totalPages = result.total_pages || 0;
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    
    if (siteCount <= localCount) return '크롤링 불필요';
    
    // 간단한 범위 계산: 마지막 몇 페이지만 크롤링
    const estimatedNewPages = Math.ceil((siteCount - localCount) / 12); // 페이지당 12개 제품 가정
    const startPage = Math.max(1, totalPages - estimatedNewPages + 1);
    const endPage = totalPages;
    
    return `${endPage} ~ ${startPage} 페이지 (예상: ${(siteCount - localCount).toLocaleString()}개)`;
  };

  const getDbPercentage = (result: SiteStatus | null): number => {
    if (!result) return 0;
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    if (siteCount === 0) return 0;
    return Math.min(100, (localCount / siteCount) * 100);
  };

  const getInconsistencyWarning = (result: SiteStatus | null): string | null => {
    if (!result) return null;
    const localCount = getLocalProductCount(result);
    const siteCount = result.estimated_products || 0;
    const totalPages = result.total_pages || 0;
    
    // 페이지 수와 제품 수의 일관성 체크
    const expectedProducts = totalPages * 12; // 페이지당 12개 제품 가정
    
    if (Math.abs(expectedProducts - siteCount) > siteCount * 0.2) {
      return `Page count inconsistency: Site reports ${totalPages} pages, DB suggests ${Math.ceil(siteCount / 12)} pages`;
    }
    
    if (localCount > siteCount) {
      return `Local database has more products (${localCount.toLocaleString()}) than site (${siteCount.toLocaleString()})`;
    }
    
    return null;
  };

  return (
    <div class="flex flex-col space-y-4 p-4">
      {/* 크롤링 상태 및 제어 섹션 */}
      <ExpandableSection 
        title="크롤링 제어" 
        isExpanded={isControlExpanded()} 
        onToggle={() => setIsControlExpanded(!isControlExpanded())}
      >
        <div class="space-y-4 p-2">
          {/* 간단한 제어 버튼 */}
          <div class="flex space-x-3 justify-center">
            <button 
              class="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 font-medium"
              onClick={handleStart}
              disabled={isRunning()}
            >
              🚀 크롤링 시작
            </button>
            <button 
              class="px-6 py-3 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:opacity-50 font-medium"
              onClick={handleStop}
              disabled={!isRunning()}
            >
              ⏹️ 중지
            </button>
          </div>
          
          {/* 상세 진행 상황 표시 */}
          <CrawlingProgressDisplay 
            progress={crawlerStore.progress()} 
            isRunning={isRunning()}
          />
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
              {/* 사이트 로컬 비교 섹션 - 스크린샷과 동일한 디자인 */}
              <div class="bg-green-50 dark:bg-green-900/20 p-4 rounded-lg border border-green-200 dark:border-green-800">
                <div class="flex items-center mb-4">
                  <div class="flex items-center space-x-2">
                    <div class="w-4 h-4 bg-green-500 rounded-full flex items-center justify-center">
                      <svg class="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                      </svg>
                    </div>
                    <h3 class="text-lg font-semibold text-green-800 dark:text-green-200">상태 체크 완료!</h3>
                  </div>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                  {/* 로컬 DB 정보 */}
                  <div class="bg-white dark:bg-gray-700 p-4 rounded-lg shadow-sm border">
                    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3 flex items-center">
                      <div class="w-3 h-3 bg-red-500 rounded-full mr-2"></div>
                      로컬 DB
                    </h4>
                    <div class="space-y-3">
                      <div class="flex justify-between items-center">
                        <span class="text-sm text-gray-600 dark:text-gray-400">마지막 업데이트:</span>
                        <span class="text-sm font-mono text-gray-800 dark:text-gray-200">
                          {statusCheckResult()?.last_check_time 
                            ? new Date(statusCheckResult()!.last_check_time).toLocaleDateString() + " " + 
                              new Date(statusCheckResult()!.last_check_time).toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})
                            : '2024-12-31 09:30'}
                        </span>
                      </div>
                      
                      <div class="flex justify-between items-center">
                        <span class="text-sm text-gray-600 dark:text-gray-400">제품 수:</span>
                        <span class="text-2xl font-bold text-red-600 dark:text-red-400">
                          {getLocalProductCount(statusCheckResult()).toLocaleString()}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* 사이트 정보 */}
                  <div class="bg-white dark:bg-gray-700 p-4 rounded-lg shadow-sm border">
                    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3 flex items-center">
                      <div class="w-3 h-3 bg-blue-500 rounded-full mr-2"></div>
                      웹사이트
                    </h4>
                    <div class="space-y-3">
                      <div class="flex justify-between items-center">
                        <span class="text-sm text-gray-600 dark:text-gray-400">페이지 수:</span>
                        <span class="text-lg font-bold text-blue-600 dark:text-blue-400">
                          {statusCheckResult()?.total_pages || 0}
                        </span>
                      </div>
                      
                      <div class="flex justify-between items-center">
                        <span class="text-sm text-gray-600 dark:text-gray-400">제품 수:</span>
                        <span class="text-2xl font-bold text-blue-600 dark:text-blue-400">
                          {(statusCheckResult()?.estimated_products || 0).toLocaleString()}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

                {/* 차이 분석 */}
                <div class="mt-6 bg-gray-50 dark:bg-gray-800 p-4 rounded-lg">
                  <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-4">비교 결과</h4>
                  <div class="space-y-3">
                    <div class="flex justify-between items-center">
                      <span class="text-sm text-gray-600 dark:text-gray-400">차이:</span>
                      <span class={`text-xl font-bold ${getDifferenceColor(statusCheckResult())}`}>
                        {getDifferenceText(statusCheckResult())}
                      </span>
                    </div>
                    
                    <div class="flex justify-between items-center">
                      <span class="text-sm text-gray-600 dark:text-gray-400">크롤링 필요:</span>
                      <span class={`text-lg font-bold ${getCrawlingNeededColor(statusCheckResult())}`}>
                        {getCrawlingNeededText(statusCheckResult())}
                      </span>
                    </div>
                    
                    <div class="pt-2 border-t">
                      <div class="text-sm text-gray-600 dark:text-gray-400 mb-1">권장 크롤링 범위:</div>
                      <div class="text-sm font-medium text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20 p-2 rounded">
                        {getRecommendedRange(statusCheckResult())}
                      </div>
                    </div>
                  </div>
                </div>

                {/* 진행바 - DB vs 사이트 비교 */}
                <div class="mt-6">
                  <div class="flex justify-between text-sm text-gray-600 dark:text-gray-400 mb-2">
                    <span>DB</span>
                    <span>사이트</span>
                  </div>
                  <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-4 relative">
                    <div 
                      class="bg-red-400 h-4 rounded-l-full flex items-center justify-center"
                      style={`width: ${getDbPercentage(statusCheckResult())}%`}
                    >
                      <span class="text-xs text-white font-medium">
                        {getLocalProductCount(statusCheckResult())}
                      </span>
                    </div>
                    <div class="absolute right-2 top-0 h-4 flex items-center">
                      <span class="text-xs text-gray-700 dark:text-gray-300 font-medium">
                        {(statusCheckResult()?.estimated_products || 0).toLocaleString()}
                      </span>
                    </div>
                  </div>
                </div>

                {/* 경고 메시지 (있는 경우) */}
                <Show when={getInconsistencyWarning(statusCheckResult())}>
                  <div class="mt-4 p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg flex items-start space-x-3">
                    <div class="flex-shrink-0">
                      <svg class="w-5 h-5 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
                      </svg>
                    </div>
                    <div>
                      <h4 class="text-sm font-medium text-yellow-800 dark:text-yellow-200">상태 체크 검증 경고</h4>
                      <p class="text-sm text-yellow-700 dark:text-yellow-300 mt-1">
                        {getInconsistencyWarning(statusCheckResult())}
                      </p>
                    </div>
                  </div>
                </Show>
              </div>
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
