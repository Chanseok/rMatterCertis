import { Component, createEffect, For, Show } from 'solid-js';
import { appStore } from '../stores/appStore';

const CrawlingDashboard: Component = () => {
  const { state, stopCrawling, setActiveTab } = appStore;

  // 진행률에 따른 색상 결정
  const getProgressColor = () => {
    const percentage = state.crawling.progress.percentage;
    if (percentage < 30) return 'bg-red-500';
    if (percentage < 70) return 'bg-yellow-500';
    return 'bg-green-500';
  };

  // 상태에 따른 상태 표시 색상
  const getStatusColor = () => {
    switch (state.crawling.status) {
      case 'running': return 'text-blue-600 bg-blue-100';
      case 'completed': return 'text-green-600 bg-green-100';
      case 'error': return 'text-red-600 bg-red-100';
      case 'paused': return 'text-yellow-600 bg-yellow-100';
      default: return 'text-gray-600 bg-gray-100';
    }
  };

  // 상태 변경 로깅 (가이드의 createEffect 예시)
  createEffect(() => {
    console.log('크롤링 상태 변경:', state.crawling.status);
    console.log('진행률:', state.crawling.progress.percentage + '%');
  });

  // 상태에 따른 한글 표시
  const getStatusText = () => {
    switch (state.crawling.status) {
      case 'idle': return '대기 중';
      case 'running': return '실행 중';
      case 'paused': return '일시 정지';
      case 'completed': return '완료';
      case 'error': return '오류';
      default: return '알 수 없음';
    }
  };

  return (
    <div class="p-6 max-w-4xl mx-auto space-y-6">
      {/* 헤더 섹션 */}
      <div class="flex justify-between items-center">
        <h1 class="text-3xl font-bold text-gray-900">크롤링 대시보드</h1>
        <div class="flex space-x-3">
          <Show when={state.crawling.status === 'idle' || state.crawling.status === 'completed' || state.crawling.status === 'error'}>
            <button
              onClick={() => setActiveTab('form')}
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              새 크롤링 시작
            </button>
          </Show>
          <Show when={state.crawling.status === 'running'}>
            <button
              onClick={stopCrawling}
              class="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
            >
              크롤링 중지
            </button>
          </Show>
        </div>
      </div>

      {/* 상태 카드 그리드 */}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {/* 현재 상태 */}
        <div class="bg-white rounded-lg shadow-md p-6">
          <h3 class="text-sm font-medium text-gray-500 mb-2">현재 상태</h3>
          <div class={`inline-flex px-3 py-1 rounded-full text-sm font-medium ${getStatusColor()}`}>
            {getStatusText()}
          </div>
        </div>

        {/* 진행률 */}
        <div class="bg-white rounded-lg shadow-md p-6">
          <h3 class="text-sm font-medium text-gray-500 mb-2">진행률</h3>
          <div class="flex items-center space-x-3">
            <div class="flex-1 bg-gray-200 rounded-full h-2">
              <div 
                class={`h-2 rounded-full transition-all duration-300 ${getProgressColor()}`}
                style={`width: ${state.crawling.progress.percentage}%`}
              />
            </div>
            <span class="text-lg font-semibold text-gray-900">
              {state.crawling.progress.percentage.toFixed(1)}%
            </span>
          </div>
        </div>

        {/* 처리된 페이지 */}
        <div class="bg-white rounded-lg shadow-md p-6">
          <h3 class="text-sm font-medium text-gray-500 mb-2">처리된 페이지</h3>
          <div class="text-2xl font-bold text-gray-900">
            {state.crawling.progress.processedPages}
            <span class="text-sm font-normal text-gray-500">
              /{state.crawling.progress.totalPages}
            </span>
          </div>
        </div>

        {/* 추출된 제품 */}
        <div class="bg-white rounded-lg shadow-md p-6">
          <h3 class="text-sm font-medium text-gray-500 mb-2">추출된 제품</h3>
          <div class="text-2xl font-bold text-green-600">
            {state.crawling.results.totalProducts}
          </div>
        </div>
      </div>

      {/* 현재 작업 정보 */}
      <Show when={state.crawling.status === 'running' && state.crawling.progress.currentUrl}>
        <div class="bg-white rounded-lg shadow-md p-6">
          <h3 class="text-lg font-semibold text-gray-900 mb-3">현재 처리 중</h3>
          <div class="bg-gray-50 rounded-lg p-4">
            <p class="text-sm text-gray-600 mb-1">현재 URL:</p>
            <p class="text-sm font-mono text-blue-600 break-all">
              {state.crawling.progress.currentUrl}
            </p>
          </div>
        </div>
      </Show>

      {/* 크롤링 설정 정보 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <h3 class="text-lg font-semibold text-gray-900 mb-4">크롤링 설정</h3>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div>
            <p class="text-sm font-medium text-gray-700">시작 URL</p>
            <p class="text-sm text-gray-600 truncate">{state.crawling.config.startUrl}</p>
          </div>
          <div>
            <p class="text-sm font-medium text-gray-700">최대 페이지</p>
            <p class="text-sm text-gray-600">{state.crawling.config.maxPages}</p>
          </div>
          <div>
            <p class="text-sm font-medium text-gray-700">동시 요청</p>
            <p class="text-sm text-gray-600">{state.crawling.config.concurrentRequests}</p>
          </div>
          <div>
            <p class="text-sm font-medium text-gray-700">요청 간격</p>
            <p class="text-sm text-gray-600">{state.crawling.config.delayMs}ms</p>
          </div>
        </div>
      </div>

      {/* 오류 목록 */}
      <Show when={state.crawling.results.errors.length > 0}>
        <div class="bg-white rounded-lg shadow-md p-6">
          <h3 class="text-lg font-semibold text-red-600 mb-4">
            오류 목록 ({state.crawling.results.errors.length})
          </h3>
          <div class="space-y-2 max-h-60 overflow-y-auto">
            <For each={state.crawling.results.errors}>
              {(error, index) => (
                <div class="bg-red-50 border-l-4 border-red-400 p-3">
                  <p class="text-sm text-red-700">
                    #{index() + 1}: {error}
                  </p>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>

      {/* 빠른 액션 버튼들 */}
      <div class="flex flex-wrap gap-3">
        <button
          onClick={() => setActiveTab('results')}
          class="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors"
        >
          결과 보기
        </button>
        <button
          onClick={() => setActiveTab('settings')}
          class="px-4 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700 transition-colors"
        >
          설정
        </button>
      </div>
    </div>
  );
};

export default CrawlingDashboard;
