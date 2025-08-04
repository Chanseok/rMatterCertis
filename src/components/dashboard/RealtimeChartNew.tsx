// SolidJS + solid-chartjs 실시간 차트 컴포넌트

import { Component, createSignal, createEffect, onMount, onCleanup, Show } from 'solid-js';
import { Line } from 'solid-chartjs';
import {
  Chart,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
  Filler,
} from 'chart.js';
import 'chartjs-adapter-date-fns';
import type { ChartDataPoint, ChartMetricType } from '../../types/dashboard';
import { DashboardAPI } from '../../services/dashboardAPI';

// Chart.js 등록
Chart.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
  Filler
);

interface RealtimeChartProps {
  metricType: ChartMetricType;
  title: string;
  timeRangeMinutes?: number;
  height?: number;
  width?: number;
  updateIntervalMs?: number;
}

const RealtimeChart: Component<RealtimeChartProps> = (props) => {
  const [chartData, setChartData] = createSignal<any>({
    labels: [],
    datasets: []
  });
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [rawData, setRawData] = createSignal<ChartDataPoint[]>([]);
  
  let updateTimer: number;
  let unsubscribeMetrics: (() => void) | null = null;

  // 차트 옵션 설정
  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: 'top' as const,
      },
      title: {
        display: true,
        text: props.title,
      },
    },
    scales: {
      x: {
        type: 'time' as const,
        time: {
          displayFormats: {
            minute: 'HH:mm',
            hour: 'HH:mm'
          }
        },
        title: {
          display: true,
          text: '시간'
        }
      },
      y: {
        beginAtZero: true,
        title: {
          display: true,
          text: getYAxisLabel(props.metricType)
        }
      }
    },
    elements: {
      point: {
        radius: 2,
        hoverRadius: 4
      },
      line: {
        tension: 0.4
      }
    },
    animation: {
      duration: 750,
      easing: 'easeInOutQuart' as const
    }
  };

  // Y축 레이블 설정
  function getYAxisLabel(metricType: ChartMetricType): string {
    switch (metricType) {
      case 'requests_per_second':
        return '요청/초';
      case 'success_rate':
        return '성공률 (%)';
      case 'response_time':
        return '응답시간 (ms)';
      case 'memory_usage':
        return '메모리 사용량 (MB)';
      case 'cpu_usage':
        return 'CPU 사용률 (%)';
      case 'pages_processed':
        return '처리된 페이지 수';
      case 'products_collected':
        return '수집된 제품 수';
      default:
        return '값';
    }
  }

  // 차트 색상 설정
  function getChartColor(metricType: ChartMetricType) {
    const colors = {
      requests_per_second: {
        border: 'rgb(59, 130, 246)',
        background: 'rgba(59, 130, 246, 0.1)'
      },
      success_rate: {
        border: 'rgb(34, 197, 94)',
        background: 'rgba(34, 197, 94, 0.1)'
      },
      response_time: {
        border: 'rgb(239, 68, 68)',
        background: 'rgba(239, 68, 68, 0.1)'
      },
      memory_usage: {
        border: 'rgb(168, 85, 247)',
        background: 'rgba(168, 85, 247, 0.1)'
      },
      cpu_usage: {
        border: 'rgb(245, 158, 11)',
        background: 'rgba(245, 158, 11, 0.1)'
      },
      pages_processed: {
        border: 'rgb(16, 185, 129)',
        background: 'rgba(16, 185, 129, 0.1)'
      },
      products_collected: {
        border: 'rgb(99, 102, 241)',
        background: 'rgba(99, 102, 241, 0.1)'
      }
    };
    return colors[metricType] || colors.requests_per_second;
  }

  // 데이터 업데이트 함수
  const updateChartData = async () => {
    try {
      setError(null);
      const dataPoints = await DashboardAPI.getRealtimeMetrics(props.metricType);
      
      setRawData(dataPoints);
      
      // Chart.js 데이터 형식으로 변환
      const labels = dataPoints.map((point: ChartDataPoint) => {
        const timestamp = typeof point.timestamp === 'number' ? point.timestamp * 1000 : new Date(point.timestamp).getTime();
        return new Date(timestamp);
      });
      const values = dataPoints.map((point: ChartDataPoint) => point.value);
      
      const color = getChartColor(props.metricType);
      
      setChartData({
        labels,
        datasets: [
          {
            label: props.title,
            data: values,
            borderColor: color.border,
            backgroundColor: color.background,
            fill: true,
            tension: 0.4,
            pointRadius: 2,
            pointHoverRadius: 4,
            borderWidth: 2,
          }
        ]
      });
      
      setIsLoading(false);
    } catch (err) {
      console.error('차트 데이터 업데이트 실패:', err);
      setError(err instanceof Error ? err.message : '데이터 로드 실패');
      setIsLoading(false);
    }
  };

  // 실시간 메트릭 이벤트 구독
  const subscribeToMetrics = async () => {
    const unsubscribe = await DashboardAPI.subscribeToMetrics((dataPoints: ChartDataPoint[]) => {
      // 현재 메트릭 타입에 해당하는 데이터만 필터링
      const relevantData = dataPoints.filter(point => point.metric_type === props.metricType);
      
      if (relevantData.length > 0) {
        setRawData(prev => {
          const newData = [...prev, ...relevantData];
          
          // 시간 범위에 맞게 데이터 필터링
          const timeRange = props.timeRangeMinutes || 30;
          const cutoffTime = new Date(Date.now() - timeRange * 60 * 1000);
          const filteredData = newData.filter(point => 
            new Date(point.timestamp) >= cutoffTime
          );
          
          // Chart.js 데이터 업데이트
          const labels = filteredData.map(point => new Date(point.timestamp));
          const values = filteredData.map(point => point.value);
          const color = getChartColor(props.metricType);
          
          setChartData({
            labels,
            datasets: [
              {
                label: props.title,
                data: values,
                borderColor: color.border,
                backgroundColor: color.background,
                fill: true,
                tension: 0.4,
                pointRadius: 2,
                pointHoverRadius: 4,
                borderWidth: 2,
              }
            ]
          });
          
          return filteredData;
        });
      }
    });
    
    unsubscribeMetrics = unsubscribe;
  };

  // 컴포넌트 마운트 시 초기화
  onMount(async () => {
    try {
      // 대시보드 서비스 초기화
      await DashboardAPI.initDashboard();
      console.log('✅ Dashboard service initialized for Chart.js');
    } catch (error) {
      console.warn('⚠️ Dashboard initialization failed, continuing with mock data:', error);
    }
    
    await updateChartData();
    subscribeToMetrics();
    
    // 주기적 업데이트 타이머 설정
    const interval = props.updateIntervalMs || 5000;
    updateTimer = setInterval(updateChartData, interval);
  });

  // 컴포넌트 언마운트 시 정리
  onCleanup(() => {
    if (updateTimer) {
      clearInterval(updateTimer);
    }
    if (unsubscribeMetrics) {
      unsubscribeMetrics();
    }
  });

  // 메트릭 타입 변경 시 데이터 새로 로드
  createEffect(() => {
    props.metricType; // 의존성 추적
    updateChartData();
  });

  return (
    <div class="realtime-chart-container" style={{
      height: `${props.height || 400}px`,
      width: `${props.width || 600}px`,
      padding: '16px',
      border: '1px solid #e5e7eb',
      'border-radius': '8px',
      'background-color': 'white'
    }}>
      <Show when={error()}>
        <div class="error-message" style={{
          color: '#ef4444',
          'text-align': 'center',
          padding: '20px'
        }}>
          <p>❌ {error()}</p>
          <button 
            onClick={updateChartData}
            style={{
              'margin-top': '10px',
              padding: '8px 16px',
              'background-color': '#3b82f6',
              color: 'white',
              border: 'none',
              'border-radius': '4px',
              cursor: 'pointer'
            }}
          >
            다시 시도
          </button>
        </div>
      </Show>
      
      <Show when={isLoading() && !error()}>
        <div class="loading-spinner" style={{
          'text-align': 'center',
          padding: '20px'
        }}>
          <div style={{
            display: 'inline-block',
            width: '40px',
            height: '40px',
            border: '4px solid #f3f4f6',
            'border-top': '4px solid #3b82f6',
            'border-radius': '50%',
            animation: 'spin 1s linear infinite'
          }}></div>
          <p style={{ 'margin-top': '10px' }}>차트 데이터 로딩 중...</p>
        </div>
      </Show>
      
      <Show when={!isLoading() && !error() && chartData().datasets.length > 0}>
        <div style={{ height: '100%', width: '100%' }}>
          <Line 
            data={chartData()} 
            options={chartOptions}
            height={props.height || 400}
            width={props.width || 600}
          />
        </div>
      </Show>
      
      <Show when={!isLoading() && !error() && chartData().datasets.length === 0}>
        <div class="no-data-message" style={{
          'text-align': 'center',
          padding: '20px',
          color: '#6b7280'
        }}>
          <p>📊 표시할 데이터가 없습니다</p>
          <p style={{ 'font-size': '14px', 'margin-top': '8px' }}>
            크롤링을 시작하면 실시간 데이터가 표시됩니다.
          </p>
        </div>
      </Show>
    </div>
  );
};

export default RealtimeChart;
