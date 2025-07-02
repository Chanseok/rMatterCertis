import { Component, createEffect, For, onMount } from 'solid-js';
import { uiStore } from './stores/uiStore';
import { crawlerStore } from './stores/crawlerStore';
import { AppWithTabs } from './components/AppWithTabs';

// 알림 컴포넌트
const NotificationToast: Component<{ notification: any; onRemove: () => void }> = (props) => {
  const getNotificationStyles = () => {
    const baseStyles = "bg-white dark:bg-gray-800 border rounded-lg shadow-lg p-4 transition-all duration-300";
    switch (props.notification.type) {
      case 'success': return `${baseStyles} border-green-500 bg-green-50 dark:bg-green-900/20`;
      case 'error': return `${baseStyles} border-red-500 bg-red-50 dark:bg-red-900/20`;
      case 'warning': return `${baseStyles} border-yellow-500 bg-yellow-50 dark:bg-yellow-900/20`;
      default: return `${baseStyles} border-blue-500 bg-blue-50 dark:bg-blue-900/20`;
    }
  };

  const getIconColor = () => {
    switch (props.notification.type) {
      case 'success': return 'text-green-600 dark:text-green-400';
      case 'error': return 'text-red-600 dark:text-red-400';
      case 'warning': return 'text-yellow-600 dark:text-yellow-400';
      default: return 'text-blue-600 dark:text-blue-400';
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
      <div class="flex items-start">
        <div class={`flex-shrink-0 text-xl ${getIconColor()}`}>
          {getIcon()}
        </div>
        <div class="ml-3 flex-1">
          <p class="text-sm font-medium text-gray-900 dark:text-white">
            {props.notification.message}
          </p>
        </div>
        <div class="ml-4 flex-shrink-0">
          <button
            onClick={props.onRemove}
            class="text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 transition-colors"
          >
            ×
          </button>
        </div>
      </div>
    </div>
  );
};

const App: Component = () => {
  const ui = uiStore;
  const crawler = crawlerStore;

  // 백엔드 연결 초기화
  onMount(() => {
    crawler.initialize();
  });

  // 전역 키보드 단축키
  createEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Cmd/Ctrl + K로 사이드바 토글
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        ui.toggleSidebar();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  });

  return (
    <div class="min-h-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100 relative">
      {/* 새로운 탭 기반 UI */}
      <AppWithTabs />

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
