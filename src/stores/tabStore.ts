/**
 * Tab Store - 새로운 탭 기반 UI를 위한 상태 관리
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { createStore } from 'solid-js/store';

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
  activeTab: 'settings',
  tabs: [
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
      id: 'status',
      label: '상태 & 제어',
      icon: '📊',
      theme: {
        bg: 'bg-blue-50',
        border: 'border-blue-200',
        text: 'text-blue-700',
        accent: 'from-blue-500 to-indigo-500'
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
      id: 'liveProduction',
      label: 'Live Production',
      icon: '🎬',
      theme: {
        bg: 'bg-red-50',
        border: 'border-red-200',
        text: 'text-red-700',
        accent: 'from-red-500 to-pink-500'
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
    },
    {
      id: 'newArchTest',
      label: '새 아키텍처 테스트',
      icon: '🏗️',
      theme: {
        bg: 'bg-slate-50',
        border: 'border-slate-200',
        text: 'text-slate-700',
        accent: 'from-slate-500 to-gray-500'
      }
    },
    {
      id: 'actorSystem',
      label: 'Actor System',
      icon: '🎭',
      theme: {
        bg: 'bg-gradient-to-br from-purple-50 to-indigo-50',
        border: 'border-purple-200',
        text: 'text-purple-700',
        accent: 'from-purple-500 to-indigo-500'
      }
    }
  ],
  expandedSections: {}
});

// 액션 함수들
export const setActiveTab = (tabId: string) => {
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
