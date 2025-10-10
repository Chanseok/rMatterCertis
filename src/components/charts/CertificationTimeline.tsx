import { Component, createSignal, createEffect, Show, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { Chart, registerables } from 'chart.js';
import 'chartjs-adapter-date-fns'; // time scale을 위한 어댑터

// Register Chart.js components
Chart.register(...registerables);

interface TimelinePoint {
  date: string;
  count: number;
}

interface TimelineResponse {
  data: TimelinePoint[];
  aggregation: string;
  date_span_days: number;
}

interface CertificationTimelineProps {
  filter: string | null;
  startDate: string;
  endDate: string;
}

export const CertificationTimeline: Component<CertificationTimelineProps> = (props) => {
  const [timelineData, setTimelineData] = createSignal<TimelineResponse | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [canvasElement, setCanvasElement] = createSignal<HTMLCanvasElement | undefined>(undefined);
  let chartInstance: any = null; // Chart.js with time scale uses different data structure

  // Load timeline data when props change
  createEffect(() => {
    loadTimeline();
  });

  const loadTimeline = async () => {
    setLoading(true);
    setError(null);
    
    try {
      console.log('[Timeline] Loading with params:', {
        filter: props.filter || null,
        startDate: props.startDate || null,
        endDate: props.endDate || null,
      });
      
      const result = await invoke<TimelineResponse>('get_certification_timeline', {
        filter: props.filter || null,
        startDate: props.startDate || null,
        endDate: props.endDate || null,
      });
      
      console.log('[Timeline] Loaded data:', result);
      console.log('[Timeline] Data points:', result?.data?.length || 0);
      
      if (!result || !result.data || result.data.length === 0) {
        console.warn('[Timeline] No data returned from API');
        setError('데이터가 없습니다');
      } else {
        setTimelineData(result);
      }
    } catch (err) {
      console.error('[Timeline] Failed to load:', err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  // Get aggregation label in Korean
  const getAggregationLabel = () => {
    const agg = timelineData()?.aggregation || '';
    switch (agg) {
      case 'daily': return '일별';
      case 'weekly': return '주별';
      case 'monthly': return '월별';
      case 'quarterly': return '분기별';
      case 'half-yearly': return '반기별';
      default: return agg;
    }
  };

  // Create or update chart when both data and canvas are ready
  createEffect(() => {
    const data = timelineData();
    const canvas = canvasElement();
    
    console.log('[Timeline Chart] createEffect triggered');
    console.log('[Timeline Chart] Data:', data);
    console.log('[Timeline Chart] Canvas element:', canvas);
    
    // Wait for both data and canvas to be available
    if (!data) {
      console.log('[Timeline Chart] No data yet, skipping render');
      return;
    }

    if (!canvas) {
      console.log('[Timeline Chart] Canvas not mounted yet, will retry when available');
      return;
    }

    if (!data.data || data.data.length === 0) {
      console.warn('[Timeline Chart] No data points to render');
      return;
    }

    // Destroy existing chart and clean up Chart.js registry
    if (chartInstance) {
      console.log('[Timeline Chart] Destroying existing chart');
      try {
        chartInstance.destroy();
      } catch (e) {
        console.warn('[Timeline Chart] Error destroying chart:', e);
      }
      chartInstance = null;
    }
    
    // Clean up any orphaned Chart.js instances attached to this canvas
    try {
      const chartId = (canvas as any)['data-chart-id'];
      if (chartId !== undefined) {
        console.log('[Timeline Chart] Cleaning up orphaned chart ID:', chartId);
        delete (canvas as any)['data-chart-id'];
      }
    } catch (e) {
      console.warn('[Timeline Chart] Error cleaning canvas:', e);
    }

    const ctx = canvas.getContext('2d');
    if (!ctx) {
      console.error('[Timeline Chart] Failed to get canvas context');
      return;
    }

    console.log('[Timeline Chart] Creating new chart with', data.data.length, 'points');
    
    // x축을 time scale로 사용하기 위해 {x, y} 객체 배열로 변환
    // 날짜 형식을 Chart.js가 인식할 수 있는 형식으로 변환
    const chartData = data.data.map(p => {
      let dateValue = p.date;
      
      // 월별 집계 (YYYY-MM) -> YYYY-MM-01로 변환
      if (/^\d{4}-\d{2}$/.test(dateValue)) {
        dateValue = `${dateValue}-01`;
      }
      // 주별 집계 (YYYY-W##) -> 해당 주의 월요일로 변환
      else if (/^\d{4}-W\d{2}$/.test(dateValue)) {
        const [year, week] = dateValue.split('-W');
        const date = new Date(parseInt(year), 0, 1 + (parseInt(week) - 1) * 7);
        dateValue = date.toISOString().split('T')[0];
      }
      // 분기별 (YYYY-Q#) -> 해당 분기 첫날로 변환
      else if (/^\d{4}-Q\d$/.test(dateValue)) {
        const [year, quarter] = dateValue.split('-Q');
        const month = (parseInt(quarter) - 1) * 3;
        dateValue = `${year}-${String(month + 1).padStart(2, '0')}-01`;
      }
      // 반기별 (YYYY-H#) -> 해당 반기 첫날로 변환  
      else if (/^\d{4}-H\d$/.test(dateValue)) {
        const [year, half] = dateValue.split('-H');
        const month = parseInt(half) === 1 ? '01' : '07';
        dateValue = `${year}-${month}-01`;
      }
      
      console.log('[Timeline Chart] Date conversion:', p.date, '->', dateValue);
      
      return {
        x: dateValue,
        y: p.count,
      };
    });

    console.log('[Timeline Chart] Chart data:', chartData);

    try {
      chartInstance = new Chart(ctx, {
        type: 'bar',
        data: {
          datasets: [
            {
              type: 'bar',
              label: '인증 건수',
              data: chartData,
              backgroundColor: 'rgba(59, 130, 246, 0.5)',
              borderColor: 'rgba(59, 130, 246, 1)',
              borderWidth: 1,
              borderRadius: 6,
              hoverBackgroundColor: 'rgba(59, 130, 246, 0.7)',
              order: 2,
            },
            {
              type: 'line',
              label: '추세선',
            data: chartData, // counts 대신 chartData 사용
            borderColor: 'rgba(168, 85, 247, 1)',
            backgroundColor: 'rgba(168, 85, 247, 0.1)',
            borderWidth: 3,
            fill: true,
            tension: 0.4, // 곡선 형태
            pointRadius: 5,
            pointHoverRadius: 8,
            pointBackgroundColor: 'rgba(168, 85, 247, 1)',
            pointBorderColor: '#fff',
            pointBorderWidth: 2,
            pointHoverBackgroundColor: 'rgba(168, 85, 247, 1)',
            pointHoverBorderColor: '#fff',
            pointHoverBorderWidth: 3,
            order: 1,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: {
          mode: 'index',
          intersect: false,
        },
        plugins: {
          legend: {
            display: true,
            position: 'top',
            labels: {
              font: {
                size: 12,
                family: "'Inter', sans-serif",
              },
              padding: 15,
              usePointStyle: true,
              pointStyle: 'circle',
            },
          },
          tooltip: {
            enabled: true,
            backgroundColor: 'rgba(0, 0, 0, 0.8)',
            titleFont: {
              size: 14,
              weight: 'bold',
            },
            bodyFont: {
              size: 13,
            },
            padding: 12,
            cornerRadius: 8,
            displayColors: true,
            callbacks: {
              title: (context) => {
                return context[0].label;
              },
              label: (context) => {
                const label = context.dataset.label || '';
                const value = context.parsed.y.toLocaleString();
                return ` ${label}: ${value}개`;
              },
              footer: () => {
                return `집계: ${getAggregationLabel()}`;
              },
            },
          },
        },
        scales: {
          x: {
            type: 'time', // time scale 사용
            time: {
              // 집계 방식에 따라 동적으로 unit 설정
              unit: data.aggregation === 'daily' ? 'day' :
                    data.aggregation === 'weekly' ? 'week' :
                    data.aggregation === 'monthly' ? 'month' :
                    data.aggregation === 'quarterly' ? 'quarter' :
                    'year', // half-yearly는 year로 표시
              displayFormats: {
                day: 'MM/dd',
                week: 'yyyy-MM-dd',
                month: 'yyyy-MM',
                quarter: 'yyyy-[Q]Q',
                year: 'yyyy',
              },
              tooltipFormat: 'yyyy-MM-dd',
            },
            display: true,
            title: {
              display: true,
              text: `인증 날짜 (${getAggregationLabel()})`,
              font: {
                size: 13,
                weight: 'bold',
              },
              color: '#1e40af',
            },
            grid: {
              display: false,
            },
            ticks: {
              font: {
                size: 11,
              },
              maxRotation: 45,
              minRotation: 45,
            },
          },
          y: {
            display: true,
            beginAtZero: true,
            title: {
              display: true,
              text: '인증 건수 (기간별)',
              font: {
                size: 13,
                weight: 'bold',
              },
              color: '#1e40af',
            },
            grid: {
              color: 'rgba(0, 0, 0, 0.05)',
            },
            ticks: {
              font: {
                size: 11,
              },
              callback: function(value) {
                return Number(value).toLocaleString();
              },
            },
          },
        },
        animation: {
          duration: 750,
          easing: 'easeInOutQuart',
        },
      }, // options 끝
    }); // new Chart() 끝
      
      console.log('[Timeline Chart] Chart created successfully');
    } catch (error) {
      console.error('[Timeline Chart] Failed to create chart:', error);
      // Canvas가 이미 사용 중인 경우, 강제로 정리하고 재시도
      if (error instanceof Error && error.message.includes('Canvas is already in use')) {
        console.warn('[Timeline Chart] Canvas in use, attempting recovery...');
        // Chart.js의 모든 인스턴스에서 이 canvas 제거
        const charts = (Chart as any).instances;
        if (charts) {
          for (const chart of Object.values(charts)) {
            if ((chart as any).canvas === canvas) {
              console.log('[Timeline Chart] Found and destroying orphaned chart');
              (chart as any).destroy();
            }
          }
        }
      }
      setError(error instanceof Error ? error.message : String(error));
    }
  });

  // Cleanup when component unmounts
  onCleanup(() => {
    if (chartInstance) {
      console.log('[Timeline Chart] Component cleanup: destroying chart');
      chartInstance.destroy();
      chartInstance = null;
    }
  });

  return (
    <div class="bg-gradient-to-br from-blue-50 to-indigo-50 border-2 border-blue-200 rounded-xl p-6 shadow-sm transition-all duration-300 hover:shadow-md">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-3">
          <span class="text-3xl">📈</span>
          <div>
            <h3 class="text-lg font-bold text-blue-900">인증 추세 (기간별 인증 건수)</h3>
            <Show when={!loading() && timelineData()}>
              <p class="text-xs text-blue-600 transition-opacity duration-300">
                {getAggregationLabel()} 집계 · {timelineData()!.data.length}개 데이터 포인트
              </p>
            </Show>
          </div>
        </div>
        <Show when={loading()}>
          <div class="flex items-center gap-2 text-blue-500">
            <div class="animate-spin rounded-full h-5 w-5 border-b-2 border-blue-500"></div>
            <span class="text-xs font-medium">업데이트 중...</span>
          </div>
        </Show>
      </div>

      <Show
        when={!loading() && !error() && timelineData()}
        fallback={
          <div class="flex flex-col items-center justify-center h-[400px] bg-white rounded-lg border border-blue-100 transition-all duration-300 gap-4">
            <Show when={loading()} fallback={
              <div class="flex flex-col items-center gap-2">
                <p class="text-red-500 font-semibold">⚠️ 데이터 로드 실패</p>
                <p class="text-sm text-gray-600">{error() || '알 수 없는 오류'}</p>
                <button 
                  class="mt-2 px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                  onClick={() => loadTimeline()}
                >
                  다시 시도
                </button>
              </div>
            }>
              <div class="flex flex-col items-center gap-2">
                <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
                <p class="text-sm text-gray-500">차트 로딩 중...</p>
                <p class="text-xs text-gray-400">
                  필터: {props.filter || '없음'} | 날짜: {props.startDate} ~ {props.endDate}
                </p>
              </div>
            </Show>
          </div>
        }
      >
        <div class="bg-white rounded-lg border border-blue-100 p-4">
          <div class="relative h-[400px]">
            <canvas ref={setCanvasElement}></canvas>
          </div>
          
          {/* Stats summary */}
          <div class="mt-6 flex justify-around text-center border-t border-blue-100 pt-4">
            <div class="transform transition-all hover:scale-105">
              <p class="text-xs text-gray-500">기간별 합계</p>
              <p class="text-lg font-bold text-blue-600">
                {timelineData()!.data.reduce((sum, p) => sum + p.count, 0).toLocaleString()}
              </p>
              <p class="text-[10px] text-gray-400">
                ({timelineData()!.aggregation === 'daily' ? '일별' : 
                  timelineData()!.aggregation === 'weekly' ? '주별' : 
                  timelineData()!.aggregation === 'monthly' ? '월별' : 
                  timelineData()!.aggregation === 'quarterly' ? '분기별' : '반기별'} 집계)
              </p>
            </div>
            <div class="transform transition-all hover:scale-105">
              <p class="text-xs text-gray-500">기간별 평균</p>
              <p class="text-lg font-bold text-blue-600">
                {Math.round(timelineData()!.data.reduce((sum, p) => sum + p.count, 0) / Math.max(timelineData()!.data.length, 1)).toLocaleString()}
              </p>
              <p class="text-[10px] text-gray-400">{timelineData()!.data.length}개 기간</p>
            </div>
            <div class="transform transition-all hover:scale-105">
              <p class="text-xs text-gray-500">최대 (단일 기간)</p>
              <p class="text-lg font-bold text-blue-600">
                {Math.max(...timelineData()!.data.map(p => p.count), 0).toLocaleString()}
              </p>
              <p class="text-[10px] text-gray-400">피크 기간</p>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};
