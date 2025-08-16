// Main Realtime Dashboard - Chart.js 기반 종합 대시보드

import { Component, createSignal, onMount, onCleanup, For, Show } from 'solid-js';
// Archived UI note: this module is excluded by tsconfig during cleanup/phase-1
import RealtimeChart from './RealtimeChartNew';
import type { 
  DashboardState, 
  ChartMetricType, 
  ActiveCrawlingSession,
  AlertMessage 
} from '../../types/dashboard';
import { DashboardAPI } from '../../services/dashboardAPI';

interface MainDashboardProps {
  autoRefreshInterval?: number;
}

const MainDashboard: Component<MainDashboardProps> = (props) => {
  const [dashboardState, setDashboardState] = createSignal<DashboardState | null>(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  
  let refreshTimer: number;
  let eventUnsubscriber: (() => void) | null = null;

  // 차트 메트릭 타입 배열
  const chartMetrics: { type: ChartMetricType; title: string; }[] = [
    { type: 'requests_per_second', title: '초당 요청 수' },
    { type: 'success_rate', title: '성공률' },
    { type: 'response_time', title: '응답 시간' },
    { type: 'memory_usage', title: '메모리 사용량' },
    { type: 'cpu_usage', title: 'CPU 사용률' },
    { type: 'pages_processed', title: '처리된 페이지' },
    { type: 'products_collected', title: '수집된 제품' }
  ];

  // 대시보드 데이터 로드
  const loadDashboardData = async () => {
    try {
      setError(null);
      const state = await DashboardAPI.getDashboardState();
      setDashboardState(state);
      setIsLoading(false);
    } catch (err) {
      console.error('대시보드 데이터 로드 실패:', err);
      setError(err instanceof Error ? err.message : '데이터 로드 실패');
      setIsLoading(false);
    }
  };

  // 대시보드 이벤트 구독
  const subscribeToDashboardEvents = async () => {
    try {
      eventUnsubscriber = await DashboardAPI.subscribeToDashboardEvents((state: DashboardState) => {
        setDashboardState(state);
      });
    } catch (err) {
      console.error('대시보드 이벤트 구독 실패:', err);
    }
  };

  // 크롤링 시작
  const startCrawling = async () => {
    try {
      await DashboardAPI.startCrawling();
      await loadDashboardData();
    } catch (err) {
      console.error('크롤링 시작 실패:', err);
      setError('크롤링 시작에 실패했습니다.');
    }
  };

  // 크롤링 중지
  const stopCrawling = async () => {
    try {
      await DashboardAPI.stopCrawling();
      await loadDashboardData();
    } catch (err) {
      console.error('크롤링 중지 실패:', err);
      setError('크롤링 중지에 실패했습니다.');
    }
  };

  // 대시보드 초기화
  const initializeDashboard = async () => {
    try {
      await DashboardAPI.initDashboard();
      await loadDashboardData();
      await subscribeToDashboardEvents();
    } catch (err) {
      console.error('대시보드 초기화 실패:', err);
      setError('대시보드 초기화에 실패했습니다.');
    }
  };

  // 컴포넌트 마운트 시 초기화
  onMount(async () => {
    await initializeDashboard();
    
    // 주기적 새로고침 설정
    const interval = props.autoRefreshInterval || 10000;
    refreshTimer = setInterval(loadDashboardData, interval);
  });

  // 컴포넌트 언마운트 시 정리
  onCleanup(() => {
    if (refreshTimer) {
      clearInterval(refreshTimer);
    }
    if (eventUnsubscriber) {
      eventUnsubscriber();
    }
  });

  // 경고 메시지 색상 설정
  const getAlertColor = (level: string) => {
    switch (level) {
      case 'error':
        return '#ef4444';
      case 'warning':
        return '#f59e0b';
      case 'info':
        return '#3b82f6';
      default:
        return '#6b7280';
    }
  };

  return (
    <div class="main-dashboard" style={{
      padding: '20px',
      'background-color': '#f9fafb',
      'min-height': '100vh'
    }}>
      {/* 헤더 */}
      <div class="dashboard-header" style={{
        'margin-bottom': '20px',
        'background-color': 'white',
        padding: '20px',
        'border-radius': '8px',
        'box-shadow': '0 1px 3px rgba(0, 0, 0, 0.1)'
      }}>
        <h1 style={{
          margin: '0 0 10px 0',
          'font-size': '24px',
          'font-weight': 'bold',
          color: '#111827'
        }}>
          🚀 Matter Certis 실시간 크롤링 대시보드
        </h1>
        
        <div class="dashboard-controls" style={{
          display: 'flex',
          gap: '10px',
          'align-items': 'center'
        }}>
          <Show when={dashboardState()?.is_active}>
            <button 
              onClick={stopCrawling}
              style={{
                padding: '10px 20px',
                'background-color': '#ef4444',
                color: 'white',
                border: 'none',
                'border-radius': '6px',
                cursor: 'pointer',
                'font-weight': '500'
              }}
            >
              ⏹️ 크롤링 중지
            </button>
          </Show>
          
          <Show when={!dashboardState()?.is_active}>
            <button 
              onClick={startCrawling}
              style={{
                padding: '10px 20px',
                'background-color': '#10b981',
                color: 'white',
                border: 'none',
                'border-radius': '6px',
                cursor: 'pointer',
                'font-weight': '500'
              }}
            >
              ▶️ 크롤링 시작
            </button>
          </Show>
          
          <button 
            onClick={loadDashboardData}
            style={{
              padding: '10px 20px',
              'background-color': '#3b82f6',
              color: 'white',
              border: 'none',
              'border-radius': '6px',
              cursor: 'pointer',
              'font-weight': '500'
            }}
          >
            🔄 새로고침
          </button>
          
          <div class="status-indicator" style={{
            display: 'flex',
            'align-items': 'center',
            gap: '8px',
            'margin-left': '20px'
          }}>
            <div style={{
              width: '12px',
              height: '12px',
              'border-radius': '50%',
              'background-color': dashboardState()?.is_active ? '#10b981' : '#6b7280'
            }}></div>
            <span style={{ color: '#374151', 'font-weight': '500' }}>
              {dashboardState()?.is_active ? '실행 중' : '중지됨'}
            </span>
          </div>
        </div>
      </div>

      {/* 로딩 상태 */}
      <Show when={isLoading()}>
        <div class="loading-overlay" style={{
          'text-align': 'center',
          padding: '60px'
        }}>
          <div style={{
            display: 'inline-block',
            width: '48px',
            height: '48px',
            border: '4px solid #f3f4f6',
            'border-top': '4px solid #3b82f6',
            'border-radius': '50%',
            animation: 'spin 1s linear infinite'
          }}></div>
          <p style={{ 'margin-top': '16px', color: '#6b7280' }}>
            대시보드 로딩 중...
          </p>
        </div>
      </Show>

      {/* 에러 메시지 */}
      <Show when={error()}>
        <div class="error-banner" style={{
          'background-color': '#fef2f2',
          border: '1px solid #fecaca',
          color: '#dc2626',
          padding: '16px',
          'border-radius': '8px',
          'margin-bottom': '20px'
        }}>
          <p>❌ {error()}</p>
          <button 
            onClick={() => setError(null)}
            style={{
              'margin-top': '8px',
              padding: '6px 12px',
              'background-color': '#dc2626',
              color: 'white',
              border: 'none',
              'border-radius': '4px',
              cursor: 'pointer',
              'font-size': '12px'
            }}
          >
            닫기
          </button>
        </div>
      </Show>

      {/* 활성 세션 정보 */}
      <Show when={dashboardState() && dashboardState()!.active_sessions?.length > 0}>
        <div class="active-sessions" style={{
          'background-color': 'white',
          padding: '20px',
          'border-radius': '8px',
          'box-shadow': '0 1px 3px rgba(0, 0, 0, 0.1)',
          'margin-bottom': '20px'
        }}>
          <h2 style={{
            margin: '0 0 16px 0',
            'font-size': '18px',
            'font-weight': '600',
            color: '#111827'
          }}>
            📊 활성 크롤링 세션
          </h2>
          
          <For each={dashboardState()!.active_sessions}>
            {(session: ActiveCrawlingSession) => (
              <div class="session-card" style={{
                border: '1px solid #e5e7eb',
                'border-radius': '6px',
                padding: '12px',
                'margin-bottom': '8px'
              }}>
                <div style={{ display: 'flex', 'justify-content': 'space-between', 'align-items': 'center' }}>
                  <span style={{ 'font-weight': '500' }}>
                    세션 ID: {session.session_id}
                  </span>
                  <span style={{ color: '#6b7280', 'font-size': '14px' }}>
                    현재 단계: {session.current_stage}
                  </span>
                </div>
                <div style={{ 'margin-top': '8px', display: 'flex', gap: '16px' }}>
                  <span style={{ 'font-size': '14px', color: '#374151' }}>
                    시작: {new Date(session.start_time).toLocaleTimeString()}
                  </span>
                  <Show when={session.estimated_completion}>
                    <span style={{ 'font-size': '14px', color: '#374151' }}>
                      완료 예정: {new Date(session.estimated_completion!).toLocaleTimeString()}
                    </span>
                  </Show>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* 알림 메시지 */}
      <Show when={dashboardState() && dashboardState()!.alerts?.length > 0}>
        <div class="alerts" style={{
          'margin-bottom': '20px'
        }}>
          <For each={dashboardState()!.alerts.slice(0, 3)}>
            {(alert: AlertMessage) => (
              <div class="alert-item" style={{
                padding: '12px',
                'border-radius': '6px',
                'margin-bottom': '8px',
                'background-color': '#f9fafb',
                border: `1px solid ${getAlertColor(alert.level)}20`,
                'border-left': `4px solid ${getAlertColor(alert.level)}`
              }}>
                <div style={{ display: 'flex', 'justify-content': 'space-between', 'align-items': 'center' }}>
                  <span style={{ color: getAlertColor(alert.level), 'font-weight': '500' }}>
                    {alert.message}
                  </span>
                  <span style={{ 'font-size': '12px', color: '#6b7280' }}>
                    {new Date(alert.timestamp).toLocaleTimeString()}
                  </span>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* 실시간 차트 그리드 */}
      <Show when={!isLoading() && !error()}>
        <div class="charts-grid" style={{
          display: 'grid',
          'grid-template-columns': 'repeat(auto-fit, minmax(500px, 1fr))',
          gap: '20px'
        }}>
          <For each={chartMetrics}>
            {(metric) => (
              <div class="chart-container" style={{
                'background-color': 'white',
                'border-radius': '8px',
                'box-shadow': '0 1px 3px rgba(0, 0, 0, 0.1)',
                overflow: 'hidden'
              }}>
                <RealtimeChart 
                  metricType={metric.type}
                  title={metric.title}
                  timeRangeMinutes={30}
                  height={350}
                  updateIntervalMs={5000}
                />
              </div>
            )}
          </For>
        </div>
      </Show>
      
      {/* CSS 애니메이션 */}
      <style>{`
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
        
        .main-dashboard button:hover {
          transform: translateY(-1px);
          box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
          transition: all 0.2s ease;
        }
        
        .session-card:hover {
          background-color: #f9fafb;
          transition: background-color 0.2s ease;
        }
      `}</style>
    </div>
  );
};

export default MainDashboard;
