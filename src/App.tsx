import { Component, Show, createEffect, For, onMount } from 'solid-js';
import { uiStore } from './stores/uiStore';
import { crawlerStore } from './stores/crawlerStore';
import { realtimeManager } from './services/realtime-manager';
import CrawlingDashboard from './components/CrawlingDashboard';
import { CrawlingForm } from './components/CrawlingForm';
import CrawlingResults from './components/CrawlingResults';

// 알림 컴포넌트
const NotificationToast: Component<{ notification: any; onRemove: () => void }> = (props) => {
  const getNotificationStyles = () => {
    const baseStyles = "glass-card backdrop-blur-2xl border shadow-2xl animate-slide-up hover-lift";
    switch (props.notification.type) {
      case 'success': return `${baseStyles} bg-green-500/20 border-green-400/50 neon-glow-green`;
      case 'error': return `${baseStyles} bg-red-500/20 border-red-400/50 neon-glow`;
      case 'warning': return `${baseStyles} bg-yellow-500/20 border-yellow-400/50`;
      default: return `${baseStyles} bg-blue-500/20 border-blue-400/50 neon-glow`;
    }
  };

  const getIconColor = () => {
    switch (props.notification.type) {
      case 'success': return 'text-green-400';
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      default: return 'text-blue-400';
    }
  };

  const getIcon = () => {
    switch (props.notification.type) {
      case 'success': return '✅';
      case 'error': return '❌';
      case 'warning': return '⚠️';
      default: return 'ℹ️';
    }
  };

  return (
    <div class={getNotificationStyles()}>
      <div class="flex items-start p-6">
        <div class={`flex-shrink-0 text-2xl ${getIconColor()} floating`}>
          {getIcon()}
        </div>
        <div class="ml-4 flex-1">
          <p class="text-sm font-semibold text-white drop-shadow-lg">
            {props.notification.message}
          </p>
        </div>
        <div class="ml-4 flex-shrink-0">
          <button
            onClick={props.onRemove}
            class="inline-flex text-white/60 hover:text-white focus:outline-none transition-all duration-300 p-1 rounded-lg hover:bg-white/20"
          >
            <span class="text-xl">×</span>
          </button>
        </div>
      </div>
    </div>
  );
};

const App: Component = () => {
  // 새로운 스토어들 사용
  const ui = uiStore;
  const crawler = crawlerStore;

  // 앱 초기화
  onMount(async () => {
    try {
      console.log('🚀 앱 초기화 시작...');
      await realtimeManager.initialize();
      console.log('✅ 앱 초기화 완료');
    } catch (error) {
      console.error('❌ 앱 초기화 실패:', error);
      ui.showError('앱 초기화에 실패했습니다. 페이지를 새로고침해주세요.', '초기화 오류');
    }
  });

  // 테마 적용
  createEffect(() => {
    document.documentElement.classList.toggle('dark', ui.theme === 'dark');
  });

  // 활성 탭에 따른 컴포넌트 렌더링
  const renderActiveTab = () => {
    switch (ui.activeTab) {
      case 'dashboard':
        return <CrawlingDashboard />;
      case 'form':
        return (
          <CrawlingForm 
            onSuccess={() => ui.setActiveTab('dashboard')}
            onCancel={() => ui.setActiveTab('dashboard')}
          />
        );
      case 'results':
        return <CrawlingResults />;
      case 'settings':
        return <div class="p-6">설정 페이지 (준비 중)</div>;
      default:
        return <CrawlingDashboard />;
    }
  };

  return (
    <div class="min-h-screen bg-aurora transition-all duration-1000">
      {/* 사이드바 */}
      <Show when={ui.sidebarOpen}>
        <div class="sidebar sidebar-open glass-card backdrop-blur-2xl border-r border-white/20 shadow-2xl">
          <div class="flex items-center justify-between h-16 px-6 border-b border-white/20">
            <h1 class="text-xl font-bold text-gradient floating flex items-center">
              <span class="mr-2 text-2xl animate-rotate">🚀</span>
              Matter Certis v2
            </h1>
            <button
              onClick={ui.toggleSidebar}
              class="text-white/70 hover:text-white transition-all duration-300 p-2 rounded-xl hover:bg-white/20 neon-glow"
            >
              <span class="text-xl">×</span>
            </button>
          </div>
          
          {/* 네비게이션 메뉴 */}
          <nav class="mt-8 px-4">
            <div class="space-y-3">
              <button
                onClick={() => ui.setActiveTab('dashboard')}
                class={`w-full text-left px-6 py-4 rounded-2xl text-sm font-semibold transition-all duration-300 flex items-center hover-lift ${
                  ui.activeTab === 'dashboard'
                    ? 'bg-white/30 text-white shadow-xl backdrop-blur-xl border border-white/30 neon-glow'
                    : 'text-white/70 hover:bg-white/20 hover:text-white glass'
                }`}
              >
                <span class="mr-4 text-xl floating">📊</span>
                대시보드
              </button>
              <button
                onClick={() => ui.setActiveTab('form')}
                class={`w-full text-left px-6 py-4 rounded-2xl text-sm font-semibold transition-all duration-300 flex items-center hover-lift ${
                  ui.activeTab === 'form'
                    ? 'bg-white/30 text-white shadow-xl backdrop-blur-xl border border-white/30 neon-glow'
                    : 'text-white/70 hover:bg-white/20 hover:text-white glass'
                }`}
              >
                <span class="mr-4 text-xl floating">🚀</span>
                크롤링 시작
              </button>
              <button
                onClick={() => ui.setActiveTab('results')}
                class={`w-full text-left px-6 py-4 rounded-2xl text-sm font-semibold transition-all duration-300 flex items-center hover-lift ${
                  ui.activeTab === 'results'
                    ? 'bg-white/30 text-white shadow-xl backdrop-blur-xl border border-white/30 neon-glow'
                    : 'text-white/70 hover:bg-white/20 hover:text-white glass'
                }`}
              >
                <span class="mr-4 text-xl floating">📋</span>
                결과
              </button>
              <button
                onClick={() => ui.setActiveTab('settings')}
                class={`w-full text-left px-6 py-4 rounded-2xl text-sm font-semibold transition-all duration-300 flex items-center hover-lift ${
                  ui.activeTab === 'settings'
                    ? 'bg-white/30 text-white shadow-xl backdrop-blur-xl border border-white/30 neon-glow'
                    : 'text-white/70 hover:bg-white/20 hover:text-white glass'
                }`}
              >
                <span class="mr-4 text-xl floating">⚙️</span>
                설정
              </button>
            </div>
          </nav>

          {/* 상태 표시 */}
          <div class="absolute bottom-24 left-4 right-4">
            <div class="glass-card backdrop-blur-xl rounded-2xl p-6 border border-white/30 shadow-xl hover-lift">
              <div class="flex items-center space-x-4">
                <div class={`w-4 h-4 rounded-full ${
                  crawler.status() === 'Running' ? 'bg-green-400 animate-pulse neon-glow-green' :
                  crawler.status() === 'Error' ? 'bg-red-400 neon-glow' :
                  'bg-gray-400'
                }`} />
                <span class="text-sm text-white font-semibold">
                  {crawler.status() === 'Running' ? '크롤링 중' :
                   crawler.status() === 'Error' ? '오류' :
                   '대기'}
                </span>
              </div>
            </div>
          </div>

          {/* 하단 테마 설정 */}
          <div class="absolute bottom-4 left-4 right-4">
            <button
              onClick={ui.toggleTheme}
              class="w-full px-6 py-4 text-sm text-white/80 hover:text-white hover:bg-white/20 transition-all duration-300 rounded-2xl flex items-center justify-center glass hover-lift"
            >
              <span class="mr-3 text-xl floating">
                {ui.theme === 'light' ? '🌙' : '☀️'}
              </span>
              {ui.theme === 'light' ? '다크 모드' : '라이트 모드'}
            </button>
          </div>
        </div>
      </Show>

      {/* 메인 콘텐츠 */}
      <div class={`transition-all duration-500 ${ui.sidebarOpen ? 'ml-64' : 'ml-0'}`}>
        {/* 헤더 */}
        <header class="glass-card backdrop-blur-2xl shadow-xl border-b border-white/20 sticky top-0 z-40">
          <div class="flex items-center justify-between h-16 px-6">
            <Show when={!ui.sidebarOpen}>
              <button
                onClick={ui.toggleSidebar}
                class="p-3 text-gray-600 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-white/20 rounded-xl transition-all duration-300 hover-lift"
              >
                <span class="text-xl">☰</span>
              </button>
            </Show>
            
            {/* 브랜드 로고 (사이드바가 닫혔을 때) */}
            <Show when={!ui.sidebarOpen}>
              <div class="text-xl font-bold text-gradient floating flex items-center">
                <span class="mr-2 text-2xl animate-rotate">🚀</span>
                Matter Certis v2
              </div>
            </Show>
            
            {/* 상태 표시 */}
            <div class="flex items-center space-x-4">
              <div class="flex items-center space-x-4 glass backdrop-blur-xl rounded-full px-6 py-3 border border-white/30 shadow-xl hover-lift">
                <div class={`w-3 h-3 rounded-full ${
                  crawler.status() === 'Running' ? 'bg-green-500 animate-pulse neon-glow-green' :
                  crawler.status() === 'Error' ? 'bg-red-500 neon-glow' :
                  'bg-gray-400'
                }`} />
                <span class="text-sm text-gray-700 dark:text-gray-300 font-semibold">
                  {crawler.status() === 'Running' ? '크롤링 중' :
                   crawler.status() === 'Error' ? '오류' :
                   '대기'}
                </span>
              </div>
            </div>
          </div>
        </header>

        {/* 페이지 내용 */}
        <main class="flex-1 relative overflow-hidden">
          <div class="animate-fade-in">
            {renderActiveTab()}
          </div>
        </main>
      </div>

      {/* 알림 토스트 컨테이너 */}
      <div class="fixed top-6 right-6 z-50 space-y-4 max-w-sm w-full">
        <For each={ui.notifications}>
          {(notification) => (
            <div class="animate-scale-in">
              <NotificationToast
                notification={notification}
                onRemove={() => ui.removeNotification(notification.id)}
              />
            </div>
          )}
        </For>
      </div>
    </div>
  );
};

export default App;
