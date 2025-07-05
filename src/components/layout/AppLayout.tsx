/**
 * AppLayout - 새로운 탭 기반 애플리케이션 메인 레이아웃
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { JSX, createMemo, Component } from 'solid-js';
import { tabState } from '../../stores/tabStore';
import { Header } from './Header';
import { TabNavigation } from './TabNavigation';

interface AppLayoutProps {
  children: JSX.Element;
}

export const AppLayout: Component<AppLayoutProps> = (props) => {
  const activeTabTheme = createMemo(() => 
    tabState.tabs.find(tab => tab.id === tabState.activeTab)?.theme
  );

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
