/**
 * CrawlingMetricsChart - 실시간 크롤링 메트릭 차트 컴포넌트
 * 처리량, 오류율, 큐 상태 등을 실시간으로 시각화
 */

import { Component, createSignal, createMemo, For, Show, onMount, onCleanup } from 'solid-js';
import type { CrawlingProgress } from '../../types/crawling';

export interface CrawlingMetricsChartProps {
  progress: CrawlingProgress | null;
  isRunning: boolean;
  timeRange: number; // minutes
}

interface MetricDataPoint {
  timestamp: number;
  value: number;
}

interface MetricSeries {
  name: string;
  color: string;
  data: MetricDataPoint[];
  unit: string;
}

interface ChartConfig {
  width: number;
  height: number;
  margin: { top: number; right: number; bottom: number; left: number };
  showGrid: boolean;
  showLegend: boolean;
  animationDuration: number;
}

export const CrawlingMetricsChart: Component<CrawlingMetricsChartProps> = (props) => {
  const [metrics, setMetrics] = createSignal<MetricSeries[]>([
    {
      name: 'Pages/min',
      color: '#3B82F6',
      data: [],
      unit: 'pages/min'
    },
    {
      name: 'Products/min',
      color: '#10B981',
      data: [],
      unit: 'products/min'
    },
    {
      name: 'Error Rate',
      color: '#EF4444',
      data: [],
      unit: '%'
    },
    {
      name: 'Queue Size',
      color: '#F59E0B',
      data: [],
      unit: 'items'
    }
  ]);

  const [chartConfig] = createSignal<ChartConfig>({
    width: 800,
    height: 400,
    margin: { top: 20, right: 80, bottom: 40, left: 60 },
    showGrid: true,
    showLegend: true,
    animationDuration: 300
  });

  const [selectedMetric, setSelectedMetric] = createSignal<string | null>(null);
  const [hoverPoint, setHoverPoint] = createSignal<{ x: number; y: number; data: any } | null>(null);

  let updateInterval: number;

  onMount(() => {
    // 초기 데이터 생성
    generateInitialData();
    
    // 실시간 업데이트 시작
    updateInterval = setInterval(() => {
      if (props.isRunning) {
        updateMetrics();
      }
    }, 1000);
  });

  onCleanup(() => {
    if (updateInterval) {
      clearInterval(updateInterval);
    }
  });

  const generateInitialData = () => {
    const now = Date.now();
    const initialData = Array.from({ length: 60 }, (_, i) => ({
      timestamp: now - (59 - i) * 1000,
      value: Math.random() * 100
    }));

    setMetrics(prev => prev.map(metric => ({
      ...metric,
      data: initialData.map(point => ({
        ...point,
        value: getSimulatedValue(metric.name, point.value)
      }))
    })));
  };

  const getSimulatedValue = (metricName: string, baseValue: number) => {
    switch (metricName) {
      case 'Pages/min':
        return Math.max(0, baseValue * 2 + Math.random() * 20);
      case 'Products/min':
        return Math.max(0, baseValue * 8 + Math.random() * 50);
      case 'Error Rate':
        return Math.max(0, Math.min(100, baseValue * 0.1 + Math.random() * 5));
      case 'Queue Size':
        return Math.max(0, baseValue * 1.5 + Math.random() * 30);
      default:
        return baseValue;
    }
  };

  const updateMetrics = () => {
    const now = Date.now();
    const maxDataPoints = props.timeRange * 60; // timeRange is in minutes
    
    setMetrics(prev => prev.map(metric => {
      const newValue = getSimulatedValue(metric.name, metric.data[metric.data.length - 1]?.value || 0);
      const newData = [
        ...metric.data.slice(-(maxDataPoints - 1)),
        { timestamp: now, value: newValue }
      ];
      
      return { ...metric, data: newData };
    }));
  };

  const chartDimensions = createMemo(() => {
    const config = chartConfig();
    return {
      innerWidth: config.width - config.margin.left - config.margin.right,
      innerHeight: config.height - config.margin.top - config.margin.bottom
    };
  });

  const getScales = createMemo(() => {
    const allMetrics = metrics();
    const dimensions = chartDimensions();
    
    if (allMetrics.length === 0 || allMetrics[0].data.length === 0) {
      return { xScale: (x: number) => x, yScale: (y: number) => y };
    }

    const allData = allMetrics.flatMap(m => m.data);
    const xMin = Math.min(...allData.map(d => d.timestamp));
    const xMax = Math.max(...allData.map(d => d.timestamp));
    const yMin = 0;
    const yMax = Math.max(...allData.map(d => d.value)) * 1.1;

    const xScale = (x: number) => ((x - xMin) / (xMax - xMin)) * dimensions.innerWidth;
    const yScale = (y: number) => dimensions.innerHeight - ((y - yMin) / (yMax - yMin)) * dimensions.innerHeight;

    return { xScale, yScale, xMin, xMax, yMin, yMax };
  });

  const generatePath = (data: MetricDataPoint[]) => {
    const { xScale, yScale } = getScales();
    if (data.length === 0) return '';

    const pathData = data
      .map((point, index) => {
        const x = xScale(point.timestamp);
        const y = yScale(point.value);
        return `${index === 0 ? 'M' : 'L'} ${x} ${y}`;
      })
      .join(' ');

    return pathData;
  };

  const generateAreaPath = (data: MetricDataPoint[]) => {
    const { xScale, yScale } = getScales();
    if (data.length === 0) return '';

    const pathData = data
      .map((point, index) => {
        const x = xScale(point.timestamp);
        const y = yScale(point.value);
        return `${index === 0 ? 'M' : 'L'} ${x} ${y}`;
      })
      .join(' ');

    const dimensions = chartDimensions();
    const bottomY = dimensions.innerHeight;
    const firstX = xScale(data[0].timestamp);
    const lastX = xScale(data[data.length - 1].timestamp);

    return `${pathData} L ${lastX} ${bottomY} L ${firstX} ${bottomY} Z`;
  };

  const formatTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString('ko-KR', { 
      hour: '2-digit', 
      minute: '2-digit', 
      second: '2-digit' 
    });
  };

  const formatValue = (value: number, unit: string) => {
    return `${value.toFixed(unit === '%' ? 1 : 0)}${unit}`;
  };

  const handleMouseMove = (event: MouseEvent) => {
    const rect = (event.currentTarget as SVGElement).getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;
    
    // 마우스 위치에 해당하는 데이터 포인트 찾기
    const { xScale, yScale } = getScales();
    const allMetrics = metrics();
    const config = chartConfig();
    
    if (allMetrics.length > 0 && allMetrics[0].data.length > 0) {
      const mouseX = x - config.margin.left;
      const mouseY = y - config.margin.top;
      
      // 가장 가까운 데이터 포인트 찾기
      const closestData = allMetrics.map(metric => {
        const closestPoint = metric.data.reduce((closest, point) => {
          const pointX = xScale(point.timestamp);
          const pointY = yScale(point.value);
          const distance = Math.sqrt(Math.pow(mouseX - pointX, 2) + Math.pow(mouseY - pointY, 2));
          
          if (!closest || distance < closest.distance) {
            return { point, distance, metric };
          }
          return closest;
        }, null as any);
        
        return closestPoint;
      }).filter(Boolean);
      
      if (closestData.length > 0) {
        const closest = closestData.reduce((min, curr) => 
          curr.distance < min.distance ? curr : min
        );
        
        if (closest.distance < 20) { // 20px 이내의 포인트만 선택
          setHoverPoint({
            x: x,
            y: y,
            data: {
              timestamp: closest.point.timestamp,
              value: closest.point.value,
              metric: closest.metric.name,
              unit: closest.metric.unit,
              color: closest.metric.color
            }
          });
        } else {
          setHoverPoint(null);
        }
      }
    }
  };

  return (
    <div class="w-full bg-white rounded-xl shadow-lg p-6">
      <div class="flex justify-between items-center mb-6">
        <h2 class="text-xl font-bold text-gray-800">📊 실시간 성능 메트릭</h2>
        <div class="flex gap-2">
          <button class="px-3 py-1 text-sm bg-blue-100 text-blue-700 rounded-md hover:bg-blue-200">
            1분
          </button>
          <button class="px-3 py-1 text-sm bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200">
            5분
          </button>
          <button class="px-3 py-1 text-sm bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200">
            15분
          </button>
        </div>
      </div>

      {/* 범례 */}
      <Show when={chartConfig().showLegend}>
        <div class="flex flex-wrap gap-4 mb-4">
          <For each={metrics()}>
            {(metric) => (
              <div 
                class={`flex items-center gap-2 cursor-pointer transition-opacity ${
                  selectedMetric() && selectedMetric() !== metric.name ? 'opacity-50' : 'opacity-100'
                }`}
                onClick={() => setSelectedMetric(selectedMetric() === metric.name ? null : metric.name)}
              >
                <div 
                  class="w-4 h-4 rounded-full"
                  style={{ 'background-color': metric.color }}
                />
                <span class="text-sm font-medium text-gray-700">{metric.name}</span>
                <span class="text-xs text-gray-500">
                  {metric.data.length > 0 ? formatValue(metric.data[metric.data.length - 1].value, metric.unit) : '0'}
                </span>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* 차트 */}
      <div class="relative">
        <svg
          width={chartConfig().width}
          height={chartConfig().height}
          class="border border-gray-200 rounded-lg"
          onMouseMove={handleMouseMove}
          onMouseLeave={() => {
            setHoverPoint(null);
          }}
        >
          {/* 그리드 */}
          <Show when={chartConfig().showGrid}>
            <defs>
              <pattern id="grid-pattern" width="40" height="40" patternUnits="userSpaceOnUse">
                <path d="M 40 0 L 0 0 0 40" fill="none" stroke="#f3f4f6" stroke-width="1"/>
              </pattern>
            </defs>
            <rect
              x={chartConfig().margin.left}
              y={chartConfig().margin.top}
              width={chartDimensions().innerWidth}
              height={chartDimensions().innerHeight}
              fill="url(#grid-pattern)"
            />
          </Show>

          {/* Y축 라벨 */}
          <g transform={`translate(${chartConfig().margin.left - 10}, ${chartConfig().margin.top})`}>
            <For each={Array.from({ length: 6 }, (_, i) => i)}>
              {(i) => {
                const { yMax } = getScales();
                const value = (yMax || 100) * (1 - i / 5);
                const y = (chartDimensions().innerHeight / 5) * i;
                
                return (
                  <text
                    x="0"
                    y={y + 5}
                    text-anchor="end"
                    class="text-xs fill-gray-500"
                  >
                    {value.toFixed(0)}
                  </text>
                );
              }}
            </For>
          </g>

          {/* X축 라벨 */}
          <g transform={`translate(${chartConfig().margin.left}, ${chartConfig().height - chartConfig().margin.bottom + 20})`}>
            <For each={Array.from({ length: 6 }, (_, i) => i)}>
              {(i) => {
                const { xMin, xMax } = getScales();
                const timestamp = (xMin || 0) + ((xMax || 0) - (xMin || 0)) * (i / 5);
                const x = (chartDimensions().innerWidth / 5) * i;
                
                return (
                  <text
                    x={x}
                    y="0"
                    text-anchor="middle"
                    class="text-xs fill-gray-500"
                  >
                    {formatTime(timestamp)}
                  </text>
                );
              }}
            </For>
          </g>

          {/* 메트릭 라인들 */}
          <g transform={`translate(${chartConfig().margin.left}, ${chartConfig().margin.top})`}>
            <For each={metrics()}>
              {(metric) => (
                <Show when={!selectedMetric() || selectedMetric() === metric.name}>
                  {/* 영역 채우기 */}
                  <path
                    d={generateAreaPath(metric.data)}
                    fill={metric.color}
                    fill-opacity="0.1"
                    class="transition-all duration-300"
                  />
                  
                  {/* 라인 */}
                  <path
                    d={generatePath(metric.data)}
                    fill="none"
                    stroke={metric.color}
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    class="transition-all duration-300"
                  />
                  
                  {/* 데이터 포인트 */}
                  <For each={metric.data.slice(-10)}>
                    {(point) => {
                      const { xScale, yScale } = getScales();
                      return (
                        <circle
                          cx={xScale(point.timestamp)}
                          cy={yScale(point.value)}
                          r="3"
                          fill={metric.color}
                          class="transition-all duration-300 hover:r-5"
                        />
                      );
                    }}
                  </For>
                </Show>
              )}
            </For>
          </g>
        </svg>

        {/* 호버 툴팁 */}
        <Show when={hoverPoint()}>
          {(point) => (
            <div
              class="absolute bg-gray-800 text-white px-3 py-2 rounded-lg shadow-lg text-sm pointer-events-none z-10"
              style={{
                left: `${point().x + 10}px`,
                top: `${point().y - 10}px`,
                transform: 'translateY(-100%)'
              }}
            >
              <div class="font-medium">{point().data.metric}</div>
              <div class="text-gray-300">
                {formatValue(point().data.value, point().data.unit)}
              </div>
              <div class="text-xs text-gray-400">
                {formatTime(point().data.timestamp)}
              </div>
            </div>
          )}
        </Show>
      </div>

      {/* 실시간 통계 요약 */}
      <div class="mt-6 grid grid-cols-2 md:grid-cols-4 gap-4">
        <For each={metrics()}>
          {(metric) => {
            const latestValue = metric.data[metric.data.length - 1]?.value || 0;
            const previousValue = metric.data[metric.data.length - 2]?.value || 0;
            const change = latestValue - previousValue;
            const changePercent = previousValue > 0 ? (change / previousValue) * 100 : 0;
            
            return (
              <div class="bg-gray-50 rounded-lg p-4 text-center">
                <div class="text-sm text-gray-600 mb-1">{metric.name}</div>
                <div class="text-2xl font-bold text-gray-800 mb-1">
                  {formatValue(latestValue, metric.unit)}
                </div>
                <div class={`text-sm flex items-center justify-center gap-1 ${
                  change > 0 ? 'text-green-600' : change < 0 ? 'text-red-600' : 'text-gray-500'
                }`}>
                  <span>{change > 0 ? '↗' : change < 0 ? '↘' : '→'}</span>
                  <span>{Math.abs(changePercent).toFixed(1)}%</span>
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
};
