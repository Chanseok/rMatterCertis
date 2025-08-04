// SolidJS + solid-chartjs 실시간 차트 컴포넌트

import { Component, createSignal, createEffect, onMount, onCleanup } from 'solid-js';
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
import { dashboardAPI } from '../../services/dashboardAPI';

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
    datasets: []
  });
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  
  let updateTimer: number;
  let unsubscribeMetrics: (() => void) | null = null;

  const margin = { top: 20, right: 30, bottom: 40, left: 50 };
  const width = (props.width || 600) - margin.left - margin.right;
  const height = (props.height || 300) - margin.top - margin.bottom;

  // 차트 초기화
  const initChart = () => {
    // 기존 SVG 제거
    d3.select(chartContainer).selectAll('*').remove();

    // SVG 생성
    svg = d3.select(chartContainer)
      .append('svg')
      .attr('width', width + margin.left + margin.right)
      .attr('height', height + margin.top + margin.bottom);

    const g = svg.append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    // 축 그룹 추가
    g.append('g')
      .attr('class', 'x-axis')
      .attr('transform', `translate(0,${height})`);

    g.append('g')
      .attr('class', 'y-axis');

    // 라인 경로 추가
    g.append('path')
      .attr('class', 'line')
      .style('fill', 'none')
      .style('stroke', getColorForMetric(props.metricType))
      .style('stroke-width', 2);

    // 데이터 포인트들
    g.append('g')
      .attr('class', 'dots');

    // 차트 제목
    svg.append('text')
      .attr('x', (width + margin.left + margin.right) / 2)
      .attr('y', margin.top / 2)
      .attr('text-anchor', 'middle')
      .style('font-size', '16px')
      .style('font-weight', 'bold')
      .style('fill', '#333')
      .text(props.title);
  };

  // 차트 업데이트
  const updateChart = (newData: ChartDataPoint[]) => {
    if (!svg || newData.length === 0) return;

    // 시간 범위 필터링
    const timeRange = props.timeRangeMinutes || 60;
    const cutoffTime = new Date(Date.now() - timeRange * 60 * 1000);
    const filteredData = newData
      .filter(d => new Date(d.timestamp) >= cutoffTime)
      .sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

    // 스케일 설정
    const xScale = d3.scaleTime()
      .domain(d3.extent(filteredData, d => new Date(d.timestamp)) as [Date, Date])
      .range([0, width]);

    const yScale = d3.scaleLinear()
      .domain(d3.extent(filteredData, d => d.value) as [number, number])
      .nice()
      .range([height, 0]);

    // 라인 생성기
    const line = d3.line<ChartDataPoint>()
      .x(d => xScale(new Date(d.timestamp)))
      .y(d => yScale(d.value))
      .curve(d3.curveMonotoneX);

    const g = svg.select('g');

    // 축 업데이트
    g.select('.x-axis')
      .transition()
      .duration(500)
      .call(d3.axisBottom(xScale).tickFormat(d3.timeFormat('%H:%M')));

    g.select('.y-axis')
      .transition()
      .duration(500)
      .call(d3.axisLeft(yScale));

    // 라인 업데이트
    g.select('.line')
      .datum(filteredData)
      .transition()
      .duration(500)
      .attr('d', line);

    // 데이터 포인트 업데이트
    const dots = g.select('.dots')
      .selectAll('.dot')
      .data(filteredData, (d: any) => d.timestamp);

    dots.enter()
      .append('circle')
      .attr('class', 'dot')
      .attr('r', 0)
      .attr('cx', d => xScale(new Date(d.timestamp)))
      .attr('cy', d => yScale(d.value))
      .style('fill', getColorForMetric(props.metricType))
      .transition()
      .duration(300)
      .attr('r', 3);

    dots.transition()
      .duration(500)
      .attr('cx', d => xScale(new Date(d.timestamp)))
      .attr('cy', d => yScale(d.value));

    dots.exit()
      .transition()
      .duration(300)
      .attr('r', 0)
      .remove();

    // 툴팁 추가
    g.selectAll('.dot')
      .on('mouseover', function(event, d) {
        const tooltip = d3.select('body').append('div')
          .attr('class', 'chart-tooltip')
          .style('position', 'absolute')
          .style('background', 'rgba(0, 0, 0, 0.8)')
          .style('color', 'white')
          .style('padding', '8px')
          .style('border-radius', '4px')
          .style('font-size', '12px')
          .style('pointer-events', 'none')
          .style('opacity', 0);

        tooltip.transition().duration(200).style('opacity', 1);
        tooltip.html(`${props.title}<br/>값: ${d.value}<br/>시간: ${new Date(d.timestamp).toLocaleTimeString()}`)
          .style('left', (event.pageX + 10) + 'px')
          .style('top', (event.pageY - 10) + 'px');
      })
      .on('mouseout', function() {
        d3.selectAll('.chart-tooltip').remove();
      });
  };

  // 메트릭 타입에 따른 색상 결정
  const getColorForMetric = (metricType: ChartMetricType): string => {
    const colors: Record<ChartMetricType, string> = {
      requests_per_second: '#3b82f6',
      success_rate: '#10b981',
      response_time: '#f59e0b',
      memory_usage: '#ef4444',
      cpu_usage: '#8b5cf6',
      pages_processed: '#06b6d4',
      products_collected: '#84cc16'
    };
    return colors[metricType] || '#6b7280';
  };

  // 데이터 로드
  const loadData = async () => {
    try {
      setIsLoading(true);
      setError(null);
      const chartData = await dashboardAPI.getChartData(
        props.metricType, 
        props.timeRangeMinutes || 60
      );
      setData(chartData);
    } catch (err) {
      setError(err instanceof Error ? err.message : '데이터 로드 실패');
    } finally {
      setIsLoading(false);
    }
  };

  // 실시간 업데이트 구독
  const subscribeToUpdates = async () => {
    try {
      unsubscribeMetrics = await dashboardAPI.subscribeToMetrics((newData) => {
        const relevantData = newData.filter(d => d.metric_type === props.metricType);
        if (relevantData.length > 0) {
          setData(prev => [...prev, ...relevantData]);
        }
      });
    } catch (err) {
      console.error('실시간 업데이트 구독 실패:', err);
    }
  };

  // 정기적 데이터 업데이트
  const startPeriodicUpdate = () => {
    const interval = props.updateIntervalMs || 5000;
    updateTimer = setInterval(loadData, interval) as any;
  };

  onMount(async () => {
    initChart();
    await loadData();
    await subscribeToUpdates();
    startPeriodicUpdate();
  });

  onCleanup(() => {
    if (updateTimer) {
      clearInterval(updateTimer);
    }
    if (unsubscribeMetrics) {
      unsubscribeMetrics();
    }
    d3.selectAll('.chart-tooltip').remove();
  });

  // 데이터 변경 시 차트 업데이트
  createEffect(() => {
    if (data().length > 0) {
      updateChart(data());
    }
  });

  return (
    <div class="realtime-chart bg-white rounded-lg shadow-md p-4">
      {isLoading() && (
        <div class="flex items-center justify-center h-full">
          <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
          <span class="ml-2">차트 로딩 중...</span>
        </div>
      )}
      
      {error() && (
        <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
          오류: {error()}
        </div>
      )}
      
      <div ref={chartContainer!} class={isLoading() || error() ? 'hidden' : ''}></div>
    </div>
  );
};

export default RealtimeChart;
