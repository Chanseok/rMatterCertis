/**
 * Tab Store - 새로운 탭 기반 UI를 위한 상태 관리
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { createStore } from 'solid-js/store';

// Optional dev tab: show only when VITE_SHOW_EVENTS === 'true'
const SHOW_EVENTS = (import.meta as any).env?.VITE_SHOW_EVENTS === 'true';

export interface TabConfig {
  id: string;
  label: string;
  icon: string;
  theme: {
    bg: string;
    border: string;
    text: string;
    accent: string;
  };
}

export interface TabState {
  activeTab: string;
  tabs: TabConfig[];
  expandedSections: Record<string, boolean>;
}

const [tabState, setTabState] = createStore<TabState>({
  activeTab: 'crawlingEngine',
  tabs: [

    {
      id: 'crawlingEngine',
      label: 'Advanced Engine',
      icon: '🔬',
      theme: {
        bg: 'bg-gradient-to-br from-blue-50 to-indigo-50',
        border: 'border-blue-200',
        text: 'text-blue-700',
        accent: 'from-blue-500 to-indigo-500'
      }
    },
    // Optional Events tab for debugging (hidden by default)
    ...(SHOW_EVENTS ? [{
      id: 'events',
      label: 'Events',
      icon: '📡',
      theme: {
        bg: 'bg-sky-50',
        border: 'border-sky-200',
        text: 'text-sky-700',
        accent: 'from-sky-500 to-blue-500'
      }
    }] : []),
    {
      id: 'settings',
      label: '설정',
      icon: '⚙️',
      theme: {
        bg: 'bg-emerald-50',
        border: 'border-emerald-200',
        text: 'text-emerald-700',
        accent: 'from-emerald-500 to-teal-500'
      }
    },
    {
      id: 'localDB',
      label: '로컬DB',
      icon: '🗄️',
      theme: {
        bg: 'bg-purple-50',
        border: 'border-purple-200',
        text: 'text-purple-700',
        accent: 'from-purple-500 to-violet-500'
      }
  },
    {
      id: 'analysis',
      label: '분석',
      icon: '📈',
      theme: {
        bg: 'bg-amber-50',
        border: 'border-amber-200',
        text: 'text-amber-700',
        accent: 'from-amber-500 to-orange-500'
      }
    }

    
  ],
  expandedSections: {}
});

// 액션 함수들
export const setActiveTab = (tabId: string) => {
  const previousTab = tabState.activeTab;
  
  // 이전 탭에서 리소스 정리
  if (previousTab && previousTab !== tabId) {
    cleanupTabResources(previousTab);
  }
  
  setTabState('activeTab', tabId);
  
  // windowStore에 마지막 활성 탭 저장
  import('../stores/windowStore').then(({ windowState }) => {
    windowState.setLastActiveTab(tabId);
  });
  
  // 탭 전환 애니메이션 효과
  const tabElement = document.querySelector(`[data-tab="${tabId}"]`);
  if (tabElement) {
    tabElement.classList.add('tab-focus-animation');
    setTimeout(() => {
      tabElement.classList.remove('tab-focus-animation');
    }, 2000);
  }
};

// 탭별 리소스 정리 함수
const cleanupTabResources = (tabId: string) => {
  switch (tabId) {
  // Archived tabs removed; basic cleanup only
    
    default:
      console.log(`🧹 Basic cleanup for tab: ${tabId}`);
      break;
  }
};

// 저장된 탭으로 복원
export const restoreLastActiveTab = (lastActiveTab: string) => {
  const validTab = tabState.tabs.find(tab => tab.id === lastActiveTab);
  if (validTab) {
    setTabState('activeTab', lastActiveTab);
  }
};

export const toggleExpandedSection = (sectionId: string) => {
  setTabState('expandedSections', sectionId, !tabState.expandedSections[sectionId]);
};

export const setExpandedSection = (sectionId: string, expanded: boolean) => {
  setTabState('expandedSections', sectionId, expanded);
};

export { tabState };
