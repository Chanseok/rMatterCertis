import { Component, Show, createEffect, For, onMount } from 'solid-js';
import { appStore } from './stores/appStore';
import { crawlerStore } from './stores/crawlerStore';
import { tauriApi } from './services/tauri-api';
import CrawlingDashboard from './components/CrawlingDashboard';
import { CrawlingForm } from './components/CrawlingForm';
import CrawlingResults from './components/CrawlingResults';

// 알림 컴포넌트
const NotificationToast: Component<{ notification: any; onRemove: () => void }> = (props) => {
  const getNotificationStyles = () => {
    const baseStyles = "fixed top-4 right-4 max-w-sm w-full bg-white border rounded-lg shadow-lg p-4 z-50 transition-all duration-300";
    switch (props.notification.type) {
      case 'success': return `${baseStyles} border-green-500 bg-green-50`;
      case 'error': return `${baseStyles} border-red-500 bg-red-50`;
      case 'warning': return `${baseStyles} border-yellow-500 bg-yellow-50`;
      default: return `${baseStyles} border-blue-500 bg-blue-50`;
    }
  };

  const getIconColor = () => {
    switch (props.notification.type) {
      case 'success': return 'text-green-600';
      case 'error': return 'text-red-600';
      case 'warning': return 'text-yellow-600';
      default: return 'text-blue-600';
    }
  };

  return (
    <div class={getNotificationStyles()}>
      <div class="flex items-start">
        <div class={`flex-shrink-0 ${getIconColor()}`}>
          <Show when={props.notification.type === 'success'}>✓</Show>
          <Show when={props.notification.type === 'error'}>✕</Show>
          <Show when={props.notification.type === 'warning'}>⚠</Show>
          <Show when={props.notification.type === 'info'}>ℹ</Show>
        </div>
        <div class="ml-3 flex-1">
          <p class="text-sm font-medium text-gray-900">
            {props.notification.message}
          </p>
        </div>
        <div class="ml-4 flex-shrink-0">
          <button
            onClick={props.onRemove}
            class="inline-flex text-gray-400 hover:text-gray-600 focus:outline-none"
          >
            ✕
          </button>
        </div>
      </div>
    </div>
  );
};

const App: Component = () => {
  const { state, setActiveTab, removeNotification, toggleSidebar, toggleTheme } = appStore;

  // 앱 시작 시 백엔드 설정 로드 및 초기화
  onMount(async () => {
    console.log('🔧 App started, initializing configuration...');
    
    try {
      // 첫 실행인지 확인
      console.log('🔍 Checking if this is first run...');
      try {
        const isFirstRun = await tauriApi.isFirstRun();
        console.log(`First run: ${isFirstRun}`);
        
        if (isFirstRun) {
          console.log('🎉 First run detected - initializing configuration...');
          
          // 앱 설정 초기화
          const initializedConfig = await tauriApi.initializeAppConfig();
          console.log('✅ Configuration initialized:', initializedConfig);
          
          // 앱 디렉토리 정보 가져오기
          const directories = await tauriApi.getAppDirectories();
          console.log('📁 App directories created:', directories);
        }
      } catch (firstRunError) {
        console.error('❌ Failed to check/initialize first run:', firstRunError);
      }
      
      // 각 설정을 개별적으로 로드해서 어느 부분에서 에러가 나는지 확인
      console.log('📡 Loading site configuration...');
      try {
        const siteConfig = await tauriApi.getSiteConfig();
        console.log('✅ Site config loaded:', siteConfig);
      } catch (siteError) {
        console.error('❌ Failed to load site config:', siteError);
      }
      
      console.log('📡 Loading comprehensive crawler configuration...');
      try {
        const comprehensiveConfig = await tauriApi.getComprehensiveCrawlerConfig();
        console.log('✅ Comprehensive crawler config loaded:', comprehensiveConfig);
      } catch (comprehensiveError) {
        console.error('❌ Failed to load comprehensive config:', comprehensiveError);
      }
      
      console.log('📡 Loading frontend configuration...');
      try {
        const frontendConfig = await tauriApi.getFrontendConfig();
        console.log('✅ Frontend config loaded:', frontendConfig);
      } catch (frontendError) {
        console.error('❌ Failed to load frontend config:', frontendError);
      }
      
      console.log('✅ Configuration loading completed');
      
    } catch (error) {
      console.error('❌ Critical error during configuration loading:', error);
      // 설정 로드 실패 시에도 앱이 계속 동작하도록 함
    }
  });

  // 테마 적용 (가이드의 createEffect 활용)
  createEffect(() => {
    document.documentElement.classList.toggle('dark', state.ui.theme === 'dark');
  });

  // 활성 탭에 따른 컴포넌트 렌더링
  const renderActiveTab = () => {
    switch (state.ui.activeTab) {
      case 'dashboard':
        return <CrawlingDashboard />;
      case 'form':
        return (
          <CrawlingForm 
            onSuccess={() => setActiveTab('dashboard')}
            onCancel={() => setActiveTab('dashboard')}
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
    <div class="min-h-screen bg-gray-50 dark:bg-gray-900 transition-colors">
      {/* 사이드바 */}
      <Show when={state.ui.sidebarOpen}>
        <div class="fixed inset-y-0 left-0 z-50 w-64 bg-white dark:bg-gray-800 shadow-lg transform transition-transform">
          <div class="flex items-center justify-between h-16 px-6 border-b border-gray-200 dark:border-gray-700">
            <h1 class="text-xl font-bold text-gray-900 dark:text-white">
              Matter Certis v2
            </h1>
            <button
              onClick={toggleSidebar}
              class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
            >
              ✕
            </button>
          </div>
          
          {/* 네비게이션 메뉴 */}
          <nav class="mt-6">
            <div class="px-3 space-y-1">
              <button
                onClick={() => setActiveTab('dashboard')}
                class={`w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  state.ui.activeTab === 'dashboard'
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                    : 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700'
                }`}
              >
                📊 대시보드
              </button>
              <button
                onClick={() => setActiveTab('form')}
                class={`w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  state.ui.activeTab === 'form'
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                    : 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700'
                }`}
              >
                🚀 크롤링 시작
              </button>
              <button
                onClick={() => setActiveTab('results')}
                class={`w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  state.ui.activeTab === 'results'
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                    : 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700'
                }`}
              >
                📋 결과
              </button>
              <button
                onClick={() => setActiveTab('settings')}
                class={`w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  state.ui.activeTab === 'settings'
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                    : 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700'
                }`}
              >
                ⚙️ 설정
              </button>
            </div>
          </nav>

          {/* 하단 설정 */}
          <div class="absolute bottom-4 left-4 right-4">
            <button
              onClick={toggleTheme}
              class="w-full px-3 py-2 text-sm text-gray-600 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-200 text-center"
            >
              {state.ui.theme === 'light' ? '🌙 다크 모드' : '☀️ 라이트 모드'}
            </button>
          </div>
        </div>
      </Show>

      {/* 메인 콘텐츠 */}
      <div class={`transition-all duration-300 ${state.ui.sidebarOpen ? 'ml-64' : 'ml-0'}`}>
        {/* 헤더 */}
        <header class="bg-white dark:bg-gray-800 shadow-sm border-b border-gray-200 dark:border-gray-700">
          <div class="flex items-center justify-between h-16 px-6">
            <Show when={!state.ui.sidebarOpen}>
              <button
                onClick={toggleSidebar}
                class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                ☰
              </button>
            </Show>
            
            {/* 상태 표시 */}
            <div class="flex items-center space-x-4">
              <div class="flex items-center space-x-2">
                <div class={`w-3 h-3 rounded-full ${
                  crawlerStore.status() === 'Running' ? 'bg-green-500 animate-pulse' :
                  crawlerStore.status() === 'Error' ? 'bg-red-500' :
                  'bg-gray-400'
                }`} />
                <span class="text-sm text-gray-600 dark:text-gray-400">
                  {crawlerStore.status() === 'Running' ? '크롤링 중' :
                   crawlerStore.status() === 'Error' ? '오류' :
                   '대기'}
                </span>
              </div>
            </div>
          </div>
        </header>

        {/* 페이지 내용 */}
        <main class="flex-1">
          {renderActiveTab()}
        </main>
      </div>

      {/* 알림 토스트 */}
      <div class="fixed top-4 right-4 z-50 space-y-2">
        <For each={state.notifications}>
          {(notification) => (
            <NotificationToast
              notification={notification}
              onRemove={() => removeNotification(notification.id)}
            />
          )}
        </For>
      </div>
    </div>
  );
};

export default App;
