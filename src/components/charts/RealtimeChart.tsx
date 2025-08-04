import React, { useState, useEffect, useRef } from 'react';
import {
  Chart as ChartJS,
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
import { Line } from 'react-chartjs-2';
import 'chartjs-adapter-date-fns';
import { ChartDataPoint, RealtimePerformanceMetrics } from '../../types/dashboard';
import { dashboardApi, dashboardEvents } from '../../api/dashboard';

ChartJS.register(
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
  metricType: string;
  title: string;
  color: string;
  unit?: string;
  height?: number;
  timeRangeMinutes?: number;
}

export const RealtimeChart: React.FC<RealtimeChartProps> = ({
  metricType,
  title,
  color,
  unit = '',
  height = 300,
  timeRangeMinutes = 60
}) => {
  const [chartData, setChartData] = useState<ChartDataPoint[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const chartRef = useRef<ChartJS>(null);

  // 차트 데이터 로드
  const loadChartData = async () => {
    try {
      const data = await dashboardApi.getChartData(metricType, timeRangeMinutes);
      setChartData(data);
      setIsLoading(false);
    } catch (error) {
      console.error(`Failed to load chart data for ${metricType}:`, error);
      setIsLoading(false);
    }
  };

  // 실시간 업데이트 설정
  useEffect(() => {
    let unsubscribe: (() => void) | undefined;

    const setupRealtimeUpdates = async () => {
      // 초기 데이터 로드
      await loadChartData();

      // 실시간 업데이트 리스너
      unsubscribe = await dashboardEvents.onChartDataUpdate((newData) => {
        setChartData(prevData => {
          // 메트릭 타입이 일치하는 데이터만 업데이트
          const filteredNewData = newData.filter(point => point.metric_type === metricType);
          if (filteredNewData.length === 0) return prevData;

          // 기존 데이터와 새 데이터 합치기 (시간 기준 정렬)
          const combined = [...prevData, ...filteredNewData];
          const sorted = combined.sort((a, b) => 
            new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
          );

          // 시간 범위 내의 데이터만 유지
          const cutoffTime = new Date(Date.now() - timeRangeMinutes * 60 * 1000);
          return sorted.filter(point => new Date(point.timestamp) >= cutoffTime);
        });
      });
    };

    setupRealtimeUpdates();

    // 주기적 새로고침 (10초마다)
    const interval = setInterval(loadChartData, 10000);

    return () => {
      if (unsubscribe) unsubscribe();
      clearInterval(interval);
    };
  }, [metricType, timeRangeMinutes]);

  // Chart.js 데이터 포맷
  const formatChartData = () => {
    if (chartData.length === 0) {
      return {
        labels: [],
        datasets: [{
          label: title,
          data: [],
          borderColor: color,
          backgroundColor: `${color}20`,
          fill: true,
          tension: 0.4,
          pointRadius: 2,
          pointHoverRadius: 4,
        }]
      };
    }

    return {
      labels: chartData.map(point => point.timestamp),
      datasets: [{
        label: title,
        data: chartData.map(point => point.value),
        borderColor: color,
        backgroundColor: `${color}20`,
        fill: true,
        tension: 0.4,
        pointRadius: 2,
        pointHoverRadius: 4,
      }]
    };
  };

  // Chart.js 옵션
  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    animation: {
      duration: 750,
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
          text: `${title} ${unit}`
        }
      }
    },
    plugins: {
      legend: {
        display: true,
        position: 'top' as const,
      },
      tooltip: {
        mode: 'index' as const,
        intersect: false,
        callbacks: {
          label: function(context: any) {
            return `${context.dataset.label}: ${context.parsed.y}${unit}`;
          },
          title: function(context: any) {
            return new Date(context[0].parsed.x).toLocaleString();
          }
        }
      }
    },
    interaction: {
      mode: 'nearest' as const,
      axis: 'x' as const,
      intersect: false
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64 bg-gray-50 rounded-lg">
        <div className="text-gray-500">차트 데이터 로딩 중...</div>
      </div>
    );
  }

  return (
    <div className="bg-white p-4 rounded-lg shadow-sm border">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-gray-800">{title}</h3>
        <div className="flex items-center space-x-2 text-sm text-gray-500">
          <span>최근 {timeRangeMinutes}분</span>
          <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
        </div>
      </div>
      <div style={{ height: `${height}px` }}>
        <Line ref={chartRef} data={formatChartData()} options={chartOptions} />
      </div>
    </div>
  );
};
