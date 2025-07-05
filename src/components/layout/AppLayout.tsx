/**
 * AppLayout - 새로운 탭 기반 애플리케이션 메인 레이아웃
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { JSX, createMemo, Component, onMount, onCleanup } from 'solid-js';
import { tabState, restoreLastActiveTab } from '../../stores/tabStore';
import { windowState } from '../../stores/windowStore';
import { Header } from './Header';
import { TabNavigation } from './TabNavigation';

interface AppLayoutProps {
  children: JSX.Element;
}

export const AppLayout: Component<AppLayoutProps> = (props) => {
  const activeTabTheme = createMemo(() => 
    tabState.tabs.find(tab => tab.id === tabState.activeTab)?.theme
  );

  // 앱 초기화 시 윈도우 상태와 탭 상태 복원
  onMount(async () => {
    console.log('🚀 Initializing AppLayout...');
    
    try {
      // 윈도우 상태 복원
      await windowState.restoreState();
      
      // 마지막 활성 탭 복원
      if (windowState.isInitialized && windowState.state.lastActiveTab) {
        restoreLastActiveTab(windowState.state.lastActiveTab);
        console.log('✅ Restored last active tab:', windowState.state.lastActiveTab);
      }
      
      console.log('✅ AppLayout initialization completed');
    } catch (error) {
      console.error('❌ Failed to initialize AppLayout:', error);
    }

    // 윈도우 이벤트 리스너 등록
    const handleResize = () => {
      const { innerWidth, innerHeight } = window;
      windowState.setSize(innerWidth, innerHeight);
    };

    const handleBeforeUnload = () => {
      // 앱 종료 시 상태 저장
      windowState.saveState();
    };

    window.addEventListener('resize', handleResize);
    window.addEventListener('beforeunload', handleBeforeUnload);
    
    // 키보드 단축키로 줌 조절
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        if (e.key === '+' || e.key === '=') {
          e.preventDefault();
          windowState.zoomIn();
        } else if (e.key === '-') {
          e.preventDefault();
          windowState.zoomOut();
        } else if (e.key === '0') {
          e.preventDefault();
          windowState.resetZoom();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);

    onCleanup(() => {
      window.removeEventListener('resize', handleResize);
      window.removeEventListener('beforeunload', handleBeforeUnload);
      window.removeEventListener('keydown', handleKeyDown);
    });
  });

  return (
    <div class="flex flex-col h-screen bg-gradient-to-br from-slate-50 to-gray-100 dark:from-slate-900 dark:to-gray-900">
      {/* 헤더 */}
      <Header />
      
      {/* 탭 네비게이션 */}
      <TabNavigation />
      
      {/* 메인 컨텐츠 */}
      <main class={`flex-1 ${activeTabTheme()?.bg || 'bg-gray-50'} dark:bg-gray-800 transition-colors duration-200`}>
        <div class="px-6 py-6 h-full overflow-auto">
          {props.children}
        </div>
      </main>
    </div>
  );
};
