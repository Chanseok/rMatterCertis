/**
 * UI Store - UI 전용 상태 관리 (리팩토링)
 * 
 * 크롤링 관련 상태는 crawlerStore로 분리하고,
 * 순수 UI 상태와 알림 관리만 담당합니다.
 */

import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';

// UI 전용 상태 타입 정의
export interface UIState {
  // 탭 및 네비게이션
  activeTab: 'dashboard' | 'form' | 'results' | 'settings' | 'database';
  sidebarOpen: boolean;
  
  // 테마
  theme: 'light' | 'dark' | 'system';
  
  // 레이아웃
  showDetails: boolean;
  compactMode: boolean;
  
  // 모달 상태
  modals: {
    settings: boolean;
    about: boolean;
    help: boolean;
    export: boolean;
    backup: boolean;
  };
}

// 알림 타입 정의
export interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title?: string;
  message: string;
  timestamp: number;
  duration?: number; // ms, undefined면 수동 닫기
  action?: {
    label: string;
    callback: () => void;
  };
}

// 앱 전체 상태 타입 정의 (UI만)
export interface AppState {
  ui: UIState;
  notifications: Notification[];
}

// 초기 상태 정의
const initialState: AppState = {
  ui: {
    activeTab: 'dashboard',
    sidebarOpen: true,
    theme: 'system',
    showDetails: false,
    compactMode: false,
    modals: {
      settings: false,
      about: false,
      help: false,
      export: false,
      backup: false,
    },
  },
  notifications: [],
};

// 스토어 생성 함수
function createAppStore() {
  const [state, setState] = createStore<AppState>(initialState);
  const [sessionId, setSessionId] = createSignal<string | null>(null);

  // 크롤링 관련 액션들 - 이 부분은 나중에 crawlerStore로 완전히 이동
  const startCrawling = async () => {
    try {
      // 이 함수는 crawlerStore로 이동되었습니다.
      addNotification('success', '크롤링이 시작되었습니다.');
    } catch (error) {
      addNotification('error', `크롤링 시작 실패: ${error}`);
    }
  };

  const stopCrawling = async () => {
    try {
      if (sessionId()) {
        // 이 함수는 crawlerStore로 이동되었습니다.
      }
      setSessionId(null);
      addNotification('info', '크롤링이 중지되었습니다.');
    } catch (error) {
      addNotification('error', `크롤링 중지 실패: ${error}`);
    }
  };

  // UI 관련 액션들
  const setActiveTab = (tab: AppState['ui']['activeTab']) => {
    setState('ui', 'activeTab', tab);
  };

  const toggleSidebar = () => {
    setState('ui', 'sidebarOpen', !state.ui.sidebarOpen);
  };

  const toggleTheme = () => {
    setState('ui', 'theme', state.ui.theme === 'light' ? 'dark' : 'light');
  };

  // 알림 관련 액션들
  const addNotification = (type: AppState['notifications'][0]['type'], message: string) => {
    const notification = {
      id: crypto.randomUUID(),
      type,
      message,
      timestamp: Date.now(),
    };
    setState('notifications', (prev) => [...prev, notification]);

    // 5초 후 자동 제거
    setTimeout(() => {
      removeNotification(notification.id);
    }, 5000);
  };

  const removeNotification = (id: string) => {
    setState('notifications', (prev) => prev.filter(n => n.id !== id));
  };

  // 설정 업데이트 - crawlerStore로 이동
  const updateConfig = (_: Partial<Record<string, unknown>>) => {
    // 빈 함수: 이 기능은 crawlerStore로 이동되었습니다.
  };

  return {
    state,
    sessionId,
    // 크롤링 액션들
    startCrawling,
    stopCrawling,
    // UI 액션들
    setActiveTab,
    toggleSidebar,
    toggleTheme,
    // 알림 액션들
    addNotification,
    removeNotification,
    // 설정 액션들
    updateConfig,
  };
}

// 싱글턴 스토어 인스턴스
export const appStore = createAppStore();

// 타입 안전한 스토어 사용을 위한 헬퍼
export type AppStore = ReturnType<typeof createAppStore>;
