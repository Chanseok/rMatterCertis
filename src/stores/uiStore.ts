/**
 * UI Store - UI 전용 상태 관리 (리팩토링 완료)
 * 
 * 크롤링 관련 상태는 crawlerStore로 분리하고,
 * 순수 UI 상태와 알림 관리만 담당합니다.
 */

import { createStore } from 'solid-js/store';

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
function createUIStore() {
  const [state, setState] = createStore<AppState>(initialState);

  // =========================================================================
  // UI 액션들
  // =========================================================================

  const setActiveTab = (tab: UIState['activeTab']) => {
    setState('ui', 'activeTab', tab);
  };

  const toggleSidebar = () => {
    setState('ui', 'sidebarOpen', !state.ui.sidebarOpen);
  };

  const setSidebarOpen = (open: boolean) => {
    setState('ui', 'sidebarOpen', open);
  };

  const setTheme = (theme: UIState['theme']) => {
    setState('ui', 'theme', theme);
    
    // 실제 테마 적용 로직
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      document.documentElement.classList.toggle('dark', mediaQuery.matches);
    } else {
      document.documentElement.classList.toggle('dark', theme === 'dark');
    }
  };

  const toggleTheme = () => {
    const currentTheme = state.ui.theme;
    const nextTheme = currentTheme === 'light' ? 'dark' : currentTheme === 'dark' ? 'system' : 'light';
    setTheme(nextTheme);
  };

  const setShowDetails = (show: boolean) => {
    setState('ui', 'showDetails', show);
  };

  const toggleDetails = () => {
    setState('ui', 'showDetails', !state.ui.showDetails);
  };

  const setCompactMode = (compact: boolean) => {
    setState('ui', 'compactMode', compact);
  };

  const toggleCompactMode = () => {
    setState('ui', 'compactMode', !state.ui.compactMode);
  };

  // =========================================================================
  // 모달 액션들
  // =========================================================================

  const openModal = (modalName: keyof UIState['modals']) => {
    setState('ui', 'modals', modalName, true);
  };

  const closeModal = (modalName: keyof UIState['modals']) => {
    setState('ui', 'modals', modalName, false);
  };

  const closeAllModals = () => {
    setState('ui', 'modals', {
      settings: false,
      about: false,
      help: false,
      export: false,
      backup: false,
    });
  };

  // =========================================================================
  // 알림 액션들
  // =========================================================================

  const addNotification = (
    type: Notification['type'], 
    message: string, 
    options?: {
      title?: string;
      duration?: number;
      action?: Notification['action'];
    }
  ) => {
    const notification: Notification = {
      id: crypto.randomUUID(),
      type,
      title: options?.title,
      message,
      timestamp: Date.now(),
      duration: options?.duration,
      action: options?.action,
    };
    
    setState('notifications', (prev) => [...prev, notification]);

    // 자동 제거 (duration이 지정된 경우 또는 기본 5초)
    const duration = options?.duration !== undefined ? options.duration : 5000;
    if (duration > 0) {
      setTimeout(() => {
        removeNotification(notification.id);
      }, duration);
    }

    return notification.id;
  };

  const removeNotification = (id: string) => {
    setState('notifications', (prev) => prev.filter(n => n.id !== id));
  };

  const clearAllNotifications = () => {
    setState('notifications', []);
  };

  // 편의 메서드들
  const showSuccess = (message: string, title?: string) => {
    return addNotification('success', message, { title });
  };

  const showError = (message: string, title?: string) => {
    return addNotification('error', message, { title, duration: 0 }); // 수동 닫기
  };

  const showWarning = (message: string, title?: string) => {
    return addNotification('warning', message, { title });
  };

  const showInfo = (message: string, title?: string) => {
    return addNotification('info', message, { title });
  };

  // =========================================================================
  // 초기화
  // =========================================================================

  const initialize = () => {
    // 초기 테마 설정
    setTheme(state.ui.theme);
    
    // 시스템 테마 변경 감지
    if (state.ui.theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      mediaQuery.addEventListener('change', (e) => {
        if (state.ui.theme === 'system') {
          document.documentElement.classList.toggle('dark', e.matches);
        }
      });
    }
  };

  // 초기화 실행
  initialize();

  return {
    // 상태
    state,
    
    // UI 액션
    setActiveTab,
    toggleSidebar,
    setSidebarOpen,
    setTheme,
    toggleTheme,
    setShowDetails,
    toggleDetails,
    setCompactMode,
    toggleCompactMode,
    
    // 모달 액션
    openModal,
    closeModal,
    closeAllModals,
    
    // 알림 액션
    addNotification,
    removeNotification,
    clearAllNotifications,
    showSuccess,
    showError,
    showWarning,
    showInfo,
    
    // 편의 접근자
    get activeTab() { return state.ui.activeTab; },
    get sidebarOpen() { return state.ui.sidebarOpen; },
    get theme() { return state.ui.theme; },
    get showDetails() { return state.ui.showDetails; },
    get compactMode() { return state.ui.compactMode; },
    get notifications() { return state.notifications; },
    get hasNotifications() { return state.notifications.length > 0; },
    get modals() { return state.ui.modals; },
    get hasOpenModal() { 
      return Object.values(state.ui.modals).some(open => open); 
    },
  };
}

// 싱글톤 스토어 인스턴스
export const uiStore = createUIStore();

// 기본 export (하위 호환성)
export const appStore = uiStore;
