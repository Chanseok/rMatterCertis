import { Component, createEffect, For, Show } from 'solid-js';
import { appStore } from '../stores/appStore';

const CrawlingDashboard: Component = () => {
  const { state, stopCrawling, setActiveTab } = appStore;

  // 진행률에 따른 색상 결정
  const getProgressColor = () => {
    const percentage = state.crawling.progress.percentage;
    if (percentage < 30) return 'bg-gradient-to-r from-red-500 to-red-600';
    if (percentage < 70) return 'bg-gradient-to-r from-yellow-500 to-orange-500';
    return 'bg-gradient-to-r from-green-500 to-emerald-600';
  };

  // 상태에 따른 상태 표시 스타일
  const getStatusStyle = () => {
    switch (state.crawling.status) {
      case 'running': return 'status-running animate-pulse';
      case 'completed': return 'status-completed';
      case 'error': return 'status-error';
      case 'paused': return 'status-paused';
      default: return 'status-idle';
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

  // 상태 아이콘
  const getStatusIcon = () => {
    switch (state.crawling.status) {
      case 'idle': return '⏸️';
      case 'running': return '🚀';
      case 'paused': return '⏸️';
      case 'completed': return '✅';
      case 'error': return '❌';
      default: return '❓';
    }
  };

  return (
    <div class="min-h-screen bg-mesh-gradient relative overflow-hidden">
      {/* Background decorative elements */}
      <div class="absolute inset-0 overflow-hidden pointer-events-none">
        <div class="absolute -top-40 -right-40 w-80 h-80 bg-purple-500/20 rounded-full blur-3xl animate-pulse"></div>
        <div class="absolute -bottom-40 -left-40 w-80 h-80 bg-blue-500/20 rounded-full blur-3xl animate-pulse" style="animation-delay: 1s"></div>
      </div>
      
      <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 relative z-10">
        {/* 헤더 섹션 */}
        <div class="mb-12 animate-fade-in">
          <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between">
            <div class="mb-6 sm:mb-0">
              <h1 class="text-5xl font-bold text-gradient mb-4 floating">크롤링 대시보드</h1>
              <p class="text-xl text-white/80 backdrop-blur-sm">CSA-IoT Matter 제품 크롤링 현황을 실시간으로 모니터링합니다</p>
            </div>
            <div class="flex flex-wrap gap-4">
              <Show when={state.crawling.status === 'idle' || state.crawling.status === 'completed' || state.crawling.status === 'error'}>
                <button
                  onClick={() => setActiveTab('form')}
                  class="btn-primary shadow-2xl neon-glow hover-lift"
                >
                  <span class="mr-3 text-xl floating">🚀</span>
                  새 크롤링 시작
                </button>
              </Show>
              <Show when={state.crawling.status === 'running'}>
                <button
                  onClick={stopCrawling}
                  class="btn-danger shadow-2xl neon-glow hover-lift"
                >
                  <span class="mr-3 text-xl animate-pulse">⏹️</span>
                  크롤링 중지
                </button>
              </Show>
            </div>
          </div>
        </div>

        {/* 상태 카드 그리드 */}
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-8 mb-12">
          {/* 현재 상태 */}
          <div class="card-glass animate-slide-up hover-lift neon-glow" style="animation-delay: 0s">
            <div class="card-body">
              <div class="flex items-center justify-between mb-4">
                <h3 class="text-sm font-bold text-white/80 uppercase tracking-wider">현재 상태</h3>
                <span class="text-3xl floating">{getStatusIcon()}</span>
              </div>
              <div class={getStatusStyle()}>
                {getStatusText()}
              </div>
            </div>
          </div>

          {/* 진행률 */}
          <div class="card-glass animate-slide-up hover-lift neon-glow-purple" style="animation-delay: 0.1s">
            <div class="card-body">
              <div class="flex items-center justify-between mb-4">
                <h3 class="text-sm font-bold text-white/80 uppercase tracking-wider">진행률</h3>
                <span class="text-3xl floating">📊</span>
              </div>
              <div class="space-y-4">
                <div class="flex justify-between items-center">
                  <span class="text-3xl font-bold text-white drop-shadow-lg">
                    {state.crawling.progress.percentage.toFixed(1)}%
                  </span>
                </div>
                <div class="progress-bar">
                  <div 
                    class={`progress-fill ${getProgressColor()} shadow-lg`}
                    style={`width: ${state.crawling.progress.percentage}%`}
                  />
                </div>
              </div>
            </div>
          </div>

          {/* 처리된 페이지 */}
          <div class="card-glass animate-slide-up hover-lift neon-glow-green" style="animation-delay: 0.2s">
            <div class="card-body">
              <div class="flex items-center justify-between mb-4">
                <h3 class="text-sm font-bold text-white/80 uppercase tracking-wider">처리된 페이지</h3>
                <span class="text-3xl floating">📄</span>
              </div>
              <div class="text-3xl font-bold text-white drop-shadow-lg">
                {state.crawling.progress.processedPages}
                <span class="text-lg font-normal text-white/60 ml-2">
                  / {state.crawling.progress.totalPages}
                </span>
              </div>
            </div>
          </div>

          {/* 추출된 제품 */}
          <div class="card-glass animate-slide-up hover-lift neon-glow" style="animation-delay: 0.3s">
            <div class="card-body">
              <div class="flex items-center justify-between mb-4">
                <h3 class="text-sm font-bold text-white/80 uppercase tracking-wider">추출된 제품</h3>
                <span class="text-3xl floating">🛍️</span>
              </div>
              <div class="text-3xl font-bold text-green-400 drop-shadow-lg neon-glow-green">
                {state.crawling.results.totalProducts}
              </div>
            </div>
          </div>
        </div>

        {/* 현재 작업 정보 */}
        <Show when={state.crawling.status === 'running' && state.crawling.progress.currentUrl}>
          <div class="card-neon mb-12 animate-bounce-in hover-lift">
            <div class="card-header">
              <h3 class="text-xl font-bold text-white flex items-center drop-shadow-lg">
                <span class="mr-3 text-3xl animate-rotate">⚙️</span>
                현재 처리 중
              </h3>
            </div>
            <div class="card-body">
              <div class="glass backdrop-blur-xl rounded-2xl p-6 border border-white/30">
                <p class="text-sm font-bold text-white/90 mb-3 uppercase tracking-wider">현재 URL:</p>
                <p class="text-sm font-mono text-blue-300 break-all glass p-4 rounded-xl border border-blue-400/30 shadow-xl">
                  {state.crawling.progress.currentUrl}
                </p>
              </div>
            </div>
          </div>
        </Show>

        {/* 크롤링 설정 정보 */}
        <div class="card-glass mb-12 hover-lift neon-glow">
          <div class="card-header">
            <h3 class="text-xl font-bold text-white flex items-center drop-shadow-lg">
              <span class="mr-3 text-3xl floating">⚙️</span>
              크롤링 설정
            </h3>
          </div>
          <div class="card-body">
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-8">
              <div class="space-y-3">
                <p class="text-sm font-bold text-white/80 uppercase tracking-wider">시작 URL</p>
                <p class="text-sm text-white/90 font-mono glass p-4 rounded-xl truncate backdrop-blur-xl border border-white/20">
                  {state.crawling.config.startUrl}
                </p>
              </div>
              <div class="space-y-3">
                <p class="text-sm font-bold text-white/80 uppercase tracking-wider">최대 페이지</p>
                <p class="text-lg font-bold text-white glass p-4 rounded-xl backdrop-blur-xl border border-white/20 text-center">
                  {state.crawling.config.maxPages}
                </p>
              </div>
              <div class="space-y-3">
                <p class="text-sm font-bold text-white/80 uppercase tracking-wider">동시 요청</p>
                <p class="text-lg font-bold text-white glass p-4 rounded-xl backdrop-blur-xl border border-white/20 text-center">
                  {state.crawling.config.concurrentRequests}
                </p>
              </div>
              <div class="space-y-3">
                <p class="text-sm font-bold text-white/80 uppercase tracking-wider">요청 간격</p>
                <p class="text-lg font-bold text-white glass p-4 rounded-xl backdrop-blur-xl border border-white/20 text-center">
                  {state.crawling.config.delayMs}ms
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* 오류 목록 */}
        <Show when={state.crawling.results.errors.length > 0}>
          <div class="card-glass mb-12 hover-lift">
            <div class="card-header">
              <h3 class="text-xl font-bold text-red-400 flex items-center drop-shadow-lg neon-glow">
                <span class="mr-3 text-3xl animate-bounce">⚠️</span>
                오류 목록 ({state.crawling.results.errors.length})
              </h3>
            </div>
            <div class="card-body">
              <div class="space-y-4 max-h-60 overflow-y-auto custom-scrollbar">
                <For each={state.crawling.results.errors}>
                  {(error, index) => (
                    <div class="glass border-l-4 border-red-400 p-4 rounded-r-xl backdrop-blur-xl hover-lift animate-scale-in" style={`animation-delay: ${index() * 0.1}s`}>
                      <p class="text-sm text-white drop-shadow-sm">
                        <span class="font-bold text-red-300">#{index() + 1}:</span> {error}
                      </p>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </div>
        </Show>

        {/* 빠른 액션 버튼들 */}
        <div class="flex flex-wrap gap-6 justify-center">
          <button
            onClick={() => setActiveTab('results')}
            class="btn-success shadow-2xl neon-glow-green hover-lift"
          >
            <span class="mr-3 text-xl floating">📋</span>
            결과 보기
          </button>
          <button
            onClick={() => setActiveTab('settings')}
            class="btn-secondary shadow-2xl hover-lift"
          >
            <span class="mr-3 text-xl floating">⚙️</span>
            설정
          </button>
        </div>
      </div>
    </div>
  );
};

export default CrawlingDashboard;
