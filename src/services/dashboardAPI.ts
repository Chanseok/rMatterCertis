// Dashboard API - Tauri 명령어들을 위한 API 인터페이스

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { 
  DashboardState, 
  ChartDataPoint, 
  ChartMetricType,
  ActiveCrawlingSession,
  DashboardEvent,
  AlertMessage 
} from '../types/dashboard';

export class DashboardAPI {
  /**
   * 대시보드 초기화
   */
  static async initDashboard(): Promise<void> {
    return invoke('init_realtime_dashboard');
  }

  /**
   * 현재 대시보드 상태 조회
   */
  static async getDashboardState(): Promise<DashboardState> {
    return invoke('get_dashboard_state');
  }

  /**
   * 차트 데이터 조회
   */
  static async getChartData(
    metricType: ChartMetricType, 
    timeRangeMinutes: number = 60
  ): Promise<ChartDataPoint[]> {
    return invoke('get_chart_data', {
      metricType,
      timeRangeMinutes
    });
  }

  /**
   * 활성 세션 목록 조회
   */
  static async getActiveSessions(): Promise<ActiveCrawlingSession[]> {
    return invoke('get_active_sessions');
  }

  /**
   * 세션 상세 정보 조회
   */
  static async getSessionDetails(sessionId: string): Promise<ActiveCrawlingSession | null> {
    return invoke('get_session_details', { sessionId });
  }

  /**
   * 알림 목록 조회
   */
  static async getAlerts(): Promise<AlertMessage[]> {
    return invoke('get_dashboard_alerts');
  }

  /**
   * 알림 해제
   */
  static async dismissAlert(alertId: string): Promise<void> {
    return invoke('dismiss_alert', { alertId });
  }

  /**
   * 대시보드 중지
   */
  static async stopDashboard(): Promise<void> {
    return invoke('stop_realtime_dashboard');
  }

  /**
   * 대시보드 이벤트 구독
   */
  static async subscribeToDashboardEvents(
    callback: (state: DashboardState) => void
  ): Promise<() => void> {
    const unsubscribe = await listen<DashboardState>('dashboard-state-updated', (event) => {
      callback(event.payload);
    });
    
    return unsubscribe;
  }

  /**
   * 크롤링 시작
   */
  static async startCrawling(): Promise<void> {
    return invoke('start_realtime_crawling');
  }

  /**
   * 크롤링 중지
   */
  static async stopCrawling(): Promise<void> {
    return invoke('stop_realtime_crawling');
  }

  /**
   * 이벤트 구독
   */
  static async subscribeToEvents(
    callback: (event: DashboardEvent) => void
  ): Promise<() => void> {
    const unsubscribe = await listen<DashboardEvent>('dashboard-event', (event) => {
      callback(event.payload);
    });
    
    return unsubscribe;
  }

  /**
   * 메트릭 업데이트 이벤트 구독
   */
  static async subscribeToMetrics(
    callback: (chartData: ChartDataPoint[]) => void
  ): Promise<() => void> {
    const unsubscribe = await listen<ChartDataPoint[]>('metrics-updated', (event) => {
      callback(event.payload);
    });
    
    return unsubscribe;
  }

  /**
   * 진행률 업데이트 이벤트 구독
   */
  static async subscribeToProgress(
    callback: (sessionId: string, progress: any) => void
  ): Promise<() => void> {
    const unsubscribe = await listen<{sessionId: string, progress: any}>('progress-updated', (event) => {
      const { sessionId, progress } = event.payload;
      callback(sessionId, progress);
    });
    
    return unsubscribe;
  }

  /**
   * 알림 이벤트 구독
   */
  static async subscribeToAlerts(
    callback: (alert: AlertMessage) => void
  ): Promise<() => void> {
    const unsubscribe = await listen<AlertMessage>('alert-triggered', (event) => {
      callback(event.payload);
    });
    
    return unsubscribe;
  }
}

// 편의를 위한 단축 함수들
export const dashboardAPI = DashboardAPI;
