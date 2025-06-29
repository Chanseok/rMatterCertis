// SolidJS 전역 상태 관리 스토어
import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// 크롤링 상태 타입 정의
export interface CrawlingState {
  status: 'idle' | 'running' | 'paused' | 'completed' | 'error';
  progress: {
    percentage: number;
    processedPages: number;
    totalPages: number;
    currentUrl: string;
  };
  results: {
    totalProducts: number;
    extractedData: any[];
    errors: string[];
  };
  config: {
    startUrl: string;
    maxPages: number;
    concurrentRequests: number;
    delayMs: number;
  };
}

// 앱 전체 상태 타입 정의
export interface AppState {
  crawling: CrawlingState;
  ui: {
    activeTab: 'dashboard' | 'form' | 'results' | 'settings';
    sidebarOpen: boolean;
    theme: 'light' | 'dark';
  };
  notifications: Array<{
    id: string;
    type: 'info' | 'success' | 'warning' | 'error';
    message: string;
    timestamp: number;
  }>;
}

// 초기 상태 정의
const initialState: AppState = {
  crawling: {
    status: 'idle',
    progress: {
      percentage: 0,
      processedPages: 0,
      totalPages: 0,
      currentUrl: '',
    },
    results: {
      totalProducts: 0,
      extractedData: [],
      errors: [],
    },
    config: {
      startUrl: 'https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=',
      maxPages: 100,
      concurrentRequests: 3,
      delayMs: 1000,
    },
  },
  ui: {
    activeTab: 'dashboard',
    sidebarOpen: true,
    theme: 'light',
  },
  notifications: [],
};

// 스토어 생성 함수
function createAppStore() {
  const [state, setState] = createStore<AppState>(initialState);
  const [sessionId, setSessionId] = createSignal<string | null>(null);

  // 크롤링 관련 액션들
  const startCrawling = async () => {
    try {
      setState('crawling', 'status', 'running');
      setState('crawling', 'progress', 'percentage', 0);
      
      // TODO: Tauri 백엔드 호출
      const response = await invoke('start_crawling', {
        config: state.crawling.config
      }) as { sessionId: string };
      
      setSessionId(response.sessionId);
      addNotification('success', '크롤링이 시작되었습니다.');
    } catch (error) {
      setState('crawling', 'status', 'error');
      addNotification('error', `크롤링 시작 실패: ${error}`);
    }
  };

  const stopCrawling = async () => {
    try {
      if (sessionId()) {
        await invoke('stop_crawling', { sessionId: sessionId() });
      }
      setState('crawling', 'status', 'idle');
      setSessionId(null);
      addNotification('info', '크롤링이 중지되었습니다.');
    } catch (error) {
      addNotification('error', `크롤링 중지 실패: ${error}`);
    }
  };

  const updateProgress = (progress: Partial<CrawlingState['progress']>) => {
    setState('crawling', 'progress', progress);
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

  // 설정 업데이트
  const updateConfig = (config: Partial<CrawlingState['config']>) => {
    setState('crawling', 'config', config);
  };

  return {
    state,
    sessionId,
    // 크롤링 액션들
    startCrawling,
    stopCrawling,
    updateProgress,
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
