/**
 * StatusTab - 상태 & 제어 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, createMemo, Show } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { crawlerStore } from '../../stores/crawlerStore';

export const StatusTab: Component = () => {
  const [isControlExpanded, setIsControlExpanded] = createSignal(true);
  const [isCompareExpanded, setIsCompareExpanded] = createSignal(true);

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
        products_per_page: 25,
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

  const handleStatusCheck = async () => {
    try {
      await crawlerStore.refreshStatus();
    } catch (error) {
      console.error('Failed to check status:', error);
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

  return (
    <div class="space-y-6">
      {/* 현재 상태 표시 */}
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 border border-gray-200 dark:border-gray-700">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">크롤링 상태</h3>
          <span class={`px-3 py-1 rounded-full text-sm font-medium ${stageInfo().color}`}>
            {stageInfo().text}
          </span>
        </div>
        
        {/* 진행률 표시 */}
        <div class="space-y-4">
          <div class="flex justify-between text-sm text-gray-600 dark:text-gray-400">
            <span>진행률</span>
            <span>{progressPercent()}%</span>
          </div>
          <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3">
            <div 
              class="h-full bg-gradient-to-r from-blue-500 to-indigo-600 rounded-full transition-all duration-500"
              style={{ width: `${progressPercent()}%` }}
            />
          </div>
          
          <Show when={crawlerStore.progress()}>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mt-4">
              <div class="text-center">
                <div class="text-2xl font-bold text-blue-600 dark:text-blue-400">
                  {crawlerStore.progress()?.current || 0}
                </div>
                <div class="text-sm text-gray-600 dark:text-gray-400">처리됨</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-gray-600 dark:text-gray-400">
                  {crawlerStore.progress()?.total || 0}
                </div>
                <div class="text-sm text-gray-600 dark:text-gray-400">전체</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-green-600 dark:text-green-400">
                  {crawlerStore.progress()?.new_items || 0}
                </div>
                <div class="text-sm text-gray-600 dark:text-gray-400">신규</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-red-600 dark:text-red-400">
                  {crawlerStore.progress()?.errors || 0}
                </div>
                <div class="text-sm text-gray-600 dark:text-gray-400">실패</div>
              </div>
            </div>
          </Show>
        </div>
      </div>

      {/* 제어 버튼 */}
      <ExpandableSection
        title="크롤링 제어"
        isExpanded={isControlExpanded()}
        onToggle={setIsControlExpanded}
        icon="🎮"
      >
        <div class="flex flex-wrap gap-4">
          <button
            onClick={handleStart}
            disabled={isRunning()}
            class="px-6 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
          >
            {isRunning() ? '실행 중...' : '크롤링 시작'}
          </button>
          
          <button
            onClick={handleStop}
            disabled={!isRunning()}
            class="px-6 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2"
          >
            중지
          </button>
          
          <button 
            onClick={handleStatusCheck}
            class="px-6 py-2 bg-gray-600 text-white rounded-md hover:bg-gray-700 transition-colors focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2"
          >
            상태 체크
          </button>
        </div>
      </ExpandableSection>

      {/* 사이트-로컬 비교 */}
      <ExpandableSection
        title="사이트-로컬 비교"
        isExpanded={isCompareExpanded()}
        onToggle={setIsCompareExpanded}
        icon="📊"
      >
        <div class="grid grid-cols-2 gap-4">
          <div class="text-center p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
            <div class="text-2xl font-bold text-blue-600 dark:text-blue-400">
              {crawlerStore.progress()?.total || 0}
            </div>
            <div class="text-sm text-gray-600 dark:text-gray-400">사이트 제품 수</div>
          </div>
          <div class="text-center p-4 bg-purple-50 dark:bg-purple-900/20 rounded-lg">
            <div class="text-2xl font-bold text-purple-600 dark:text-purple-400">
              {crawlerStore.progress()?.current || 0}
            </div>
            <div class="text-sm text-gray-600 dark:text-gray-400">로컬 DB 제품 수</div>
          </div>
        </div>
        
        {/* 진행률 바 */}
        <div class="mt-4">
          <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3">
            <div 
              class="h-full bg-gradient-to-r from-blue-500 to-purple-600 rounded-full transition-all duration-500"
              style={{ width: `${progressPercent()}%` }}
            />
          </div>
          <div class="text-center text-sm text-gray-600 dark:text-gray-400 mt-2">
            동기화율: {progressPercent()}%
          </div>
        </div>
      </ExpandableSection>

      {/* 동시 작업 시각화 */}
      <Show when={isRunning()}>
        <div class="bg-gradient-to-br from-blue-50 to-purple-50 dark:from-blue-900/20 dark:to-purple-900/20 rounded-lg p-4 border border-blue-200 dark:border-blue-700">
          <h4 class="text-md font-semibold text-blue-700 dark:text-blue-300 mb-3">동시 진행 작업</h4>
          <div class="grid grid-cols-6 md:grid-cols-12 gap-2">
            {Array.from({ length: 12 }, (_, i) => (
              <div 
                class={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold transition-all duration-300
                  ${i < (crawlerStore.state.currentConfig?.concurrency || 6) 
                    ? 'bg-blue-400 text-white animate-pulse shadow-lg' 
                    : 'bg-gray-300 dark:bg-gray-600 text-gray-500 dark:text-gray-400'}`}
              >
                {i < (crawlerStore.state.currentConfig?.concurrency || 6) ? '▶' : '⏸'}
              </div>
            ))}
          </div>
          <div class="text-center text-sm text-gray-600 dark:text-gray-400 mt-3">
            {crawlerStore.state.currentConfig?.concurrency || 6}개 작업이 동시에 실행 중입니다
          </div>
        </div>
      </Show>

      {/* 에러 로그 */}
      <Show when={crawlerStore.lastError()}>
        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-700 rounded-lg p-4">
          <h4 class="text-md font-semibold text-red-700 dark:text-red-300 mb-2">최근 오류</h4>
          <p class="text-sm text-red-600 dark:text-red-400">
            {crawlerStore.lastError()}
          </p>
        </div>
      </Show>
    </div>
  );
};
