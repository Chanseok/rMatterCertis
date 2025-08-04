import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { 
  DashboardState, 
  ActiveCrawlingSession, 
  ChartDataPoint, 
  DashboardEvent,
  RealtimePerformanceMetrics
} from '../types/dashboard';

// Tauri Commands - Backend Dashboard API
export const dashboardApi = {
  // 대시보드 초기화
  async initializeDashboard(): Promise<DashboardState> {
    return await invoke('initialize_dashboard');
  },

  // 대시보드 상태 가져오기
  async getDashboardState(): Promise<DashboardState> {
    return await invoke('get_dashboard_state');
  },

  // 차트 데이터 가져오기 (특정 메트릭)
  async getChartData(
    metric_type: string, 
    time_range_minutes: number = 60
  ): Promise<ChartDataPoint[]> {
    return await invoke('get_dashboard_chart_data', {
      metricType: metric_type,
      timeRangeMinutes: time_range_minutes
    });
  },

  // 활성 세션 관리
  async startCrawlingSession(config: any): Promise<string> {
    return await invoke('start_crawling_session', { config });
  },

  async stopCrawlingSession(sessionId: string): Promise<void> {
    return await invoke('stop_crawling_session', { sessionId });
  },

  async getCrawlingSessionStatus(sessionId: string): Promise<ActiveCrawlingSession> {
    return await invoke('get_crawling_session_status', { sessionId });
  },

  // 성능 메트릭
  async getCurrentPerformanceMetrics(): Promise<RealtimePerformanceMetrics> {
    return await invoke('get_current_performance_metrics');
  },

  // 대시보드 설정
  async updateDashboardSettings(settings: any): Promise<void> {
    return await invoke('update_dashboard_settings', { settings });
  }
};

// Event Listeners - Realtime Updates
export const dashboardEvents = {
  // 실시간 대시보드 업데이트 이벤트 리스너
  async onDashboardUpdate(callback: (event: DashboardEvent) => void) {
    return await listen('dashboard_update', (event) => {
      callback(event.payload as DashboardEvent);
    });
  },

  // 크롤링 진행 상황 업데이트
  async onCrawlingProgress(callback: (progress: any) => void) {
    return await listen('crawling_progress', (event) => {
      callback(event.payload);
    });
  },

  // 성능 메트릭 업데이트
  async onPerformanceUpdate(callback: (metrics: RealtimePerformanceMetrics) => void) {
    return await listen('performance_update', (event) => {
      callback(event.payload as RealtimePerformanceMetrics);
    });
  },

  // 알림/경고 이벤트
  async onAlertTriggered(callback: (alert: any) => void) {
    return await listen('alert_triggered', (event) => {
      callback(event.payload);
    });
  },

  // 차트 데이터 업데이트
  async onChartDataUpdate(callback: (data: ChartDataPoint[]) => void) {
    return await listen('chart_data_update', (event) => {
      callback(event.payload as ChartDataPoint[]);
    });
  }
};

// Utility Functions
export const dashboardUtils = {
  // 차트 데이터 포맷팅
  formatChartData(dataPoints: ChartDataPoint[], metricType: string) {
    return {
      labels: dataPoints.map(point => new Date(point.timestamp).toLocaleTimeString()),
      datasets: [{
        label: metricType,
        data: dataPoints.map(point => point.value),
        borderColor: 'rgb(75, 192, 192)',
        backgroundColor: 'rgba(75, 192, 192, 0.2)',
        tension: 0.1
      }]
    };
  },

  // 성능 점수 계산
  calculatePerformanceScore(metrics: RealtimePerformanceMetrics): number {
    const responseTimeScore = Math.max(0, 100 - (metrics.average_response_time_ms / 10));
    const successRateScore = metrics.success_rate * 100;
    const resourceScore = Math.max(0, 100 - metrics.memory_usage_mb / 10 - metrics.cpu_usage_percent);
    
    return Math.round((responseTimeScore + successRateScore + resourceScore) / 3);
  },

  // 시간 범위 포맷팅
  formatTimeRange(minutes: number): string {
    if (minutes < 60) return `${minutes}분`;
    if (minutes < 1440) return `${Math.round(minutes / 60)}시간`;
    return `${Math.round(minutes / 1440)}일`;
  }
};
