/**
 * CrawlingCityDashboard - 도시 성장 게임 스타일의 크롤링 시각화 대시보드
 * 각 작업 단계를 도시의 건물과 인프라로 표현
 */

import { Component, createSignal, createMemo, For, Show, onMount, onCleanup } from 'solid-js';
import type { CrawlingProgress } from '../../types/crawling';

export interface CrawlingCityDashboardProps {
  progress: CrawlingProgress | null;
  isRunning: boolean;
  onToggleRunning: () => void;
  onPauseResume: () => void;
  onStop: () => void;
}

interface BuildingState {
  type: 'factory' | 'parser' | 'warehouse' | 'database' | 'control_tower';
  name: string;
  icon: string;
  workers: number;
  maxWorkers: number;
  queueSize: number;
  maxQueueSize: number;
  throughput: number;
  status: 'idle' | 'working' | 'busy' | 'error';
  animation: 'none' | 'working' | 'completed' | 'error';
}

export const CrawlingCityDashboard: Component<CrawlingCityDashboardProps> = (props) => {
  // 시뮬레이션 데이터
  const [buildings, setBuildings] = createSignal<BuildingState[]>([
    {
      type: 'control_tower',
      name: 'Control Tower',
      icon: '🗼',
      workers: 1,
      maxWorkers: 1,
      queueSize: 0,
      maxQueueSize: 0,
      throughput: 0,
      status: 'idle',
      animation: 'none'
    },
    {
      type: 'factory',
      name: 'Page Fetcher',
      icon: '🏭',
      workers: 3,
      maxWorkers: 8,
      queueSize: 12,
      maxQueueSize: 100,
      throughput: 24,
      status: 'working',
      animation: 'working'
    },
    {
      type: 'parser',
      name: 'HTML Parser',
      icon: '🔧',
      workers: 2,
      maxWorkers: 4,
      queueSize: 8,
      maxQueueSize: 50,
      throughput: 18,
      status: 'working',
      animation: 'working'
    },
    {
      type: 'warehouse',
      name: 'Data Processor',
      icon: '🏪',
      workers: 4,
      maxWorkers: 6,
      queueSize: 15,
      maxQueueSize: 80,
      throughput: 32,
      status: 'busy',
      animation: 'working'
    },
    {
      type: 'database',
      name: 'Database Saver',
      icon: '🏦',
      workers: 2,
      maxWorkers: 3,
      queueSize: 3,
      maxQueueSize: 20,
      throughput: 16,
      status: 'working',
      animation: 'working'
    }
  ]);

  // 도시 전체 통계
  const [cityStats, setCityStats] = createSignal({
    totalPages: 1250,
    completedPages: 847,
    productsFound: 12840,
    productsSaved: 11290,
    errors: 23,
    avgSpeed: 185, // pages per hour
    estimatedCompletion: '2시간 15분'
  });

  // 애니메이션 틱 - 사용하지 않는 변수 제거
  let animationInterval: number;

  onMount(() => {
    // 애니메이션 틱 - 1초마다 업데이트
    animationInterval = setInterval(() => {
      // 시뮬레이션 데이터 업데이트
      if (props.isRunning) {
        updateSimulationData();
      }
    }, 1000);
  });

  onCleanup(() => {
    if (animationInterval) {
      clearInterval(animationInterval);
    }
  });

  const updateSimulationData = () => {
    setBuildings(prev => prev.map(building => ({
      ...building,
      queueSize: Math.max(0, building.queueSize + (Math.random() - 0.7) * 3),
      throughput: building.throughput + (Math.random() - 0.5) * 2
    })));

    setCityStats(prev => ({
      ...prev,
      completedPages: Math.min(prev.totalPages, prev.completedPages + Math.floor(Math.random() * 5)),
      productsFound: prev.productsFound + Math.floor(Math.random() * 20),
      productsSaved: prev.productsSaved + Math.floor(Math.random() * 15)
    }));
  };

  const progressPercentage = createMemo(() => {
    const stats = cityStats();
    return (stats.completedPages / stats.totalPages) * 100;
  });

  const getStatusColor = (status: BuildingState['status']) => {
    switch (status) {
      case 'idle': return 'text-gray-500';
      case 'working': return 'text-green-500';
      case 'busy': return 'text-yellow-500';
      case 'error': return 'text-red-500';
      default: return 'text-gray-500';
    }
  };

  const getQueueFillPercentage = (queueSize: number, maxQueueSize: number) => {
    return Math.min(100, (queueSize / maxQueueSize) * 100);
  };

  const getWorkerUtilization = (workers: number, maxWorkers: number) => {
    return (workers / maxWorkers) * 100;
  };

  return (
    <div class="w-full h-full bg-gradient-to-br from-blue-50 to-indigo-100 p-6 overflow-auto">
      {/* 도시 헤더 */}
      <div class="mb-6">
        <div class="flex items-center justify-between mb-4">
          <h1 class="text-3xl font-bold text-gray-800 flex items-center gap-2">
            🏙️ Crawling City Dashboard
          </h1>
          <div class="flex gap-2">
            <button 
              onClick={props.onToggleRunning}
              class={`px-4 py-2 rounded-lg font-medium transition-colors ${
                props.isRunning 
                  ? 'bg-red-500 hover:bg-red-600 text-white' 
                  : 'bg-green-500 hover:bg-green-600 text-white'
              }`}
            >
              {props.isRunning ? '⏹️ Stop' : '▶️ Start'}
            </button>
            <button 
              onClick={props.onPauseResume}
              class="px-4 py-2 bg-yellow-500 hover:bg-yellow-600 text-white rounded-lg font-medium transition-colors"
            >
              ⏸️ Pause
            </button>
          </div>
        </div>

        {/* 전체 진행률 */}
        <div class="bg-white rounded-xl shadow-lg p-6 mb-6">
          <div class="flex justify-between items-center mb-2">
            <span class="text-lg font-semibold text-gray-700">전체 진행률</span>
            <span class="text-xl font-bold text-blue-600">{progressPercentage().toFixed(1)}%</span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-4 mb-4">
            <div 
              class="bg-gradient-to-r from-blue-500 to-purple-500 h-4 rounded-full transition-all duration-1000"
              style={{ width: `${progressPercentage()}%` }}
            />
          </div>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div class="text-center">
              <div class="text-2xl font-bold text-blue-600">{cityStats().completedPages}</div>
              <div class="text-gray-500">완료된 페이지</div>
            </div>
            <div class="text-center">
              <div class="text-2xl font-bold text-green-600">{cityStats().productsFound}</div>
              <div class="text-gray-500">발견된 상품</div>
            </div>
            <div class="text-center">
              <div class="text-2xl font-bold text-purple-600">{cityStats().productsSaved}</div>
              <div class="text-gray-500">저장된 상품</div>
            </div>
            <div class="text-center">
              <div class="text-2xl font-bold text-orange-600">{cityStats().avgSpeed}</div>
              <div class="text-gray-500">시간당 페이지</div>
            </div>
          </div>
        </div>
      </div>

      {/* 도시 건물들 */}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        <For each={buildings()}>
          {(building) => (
            <div class="bg-white rounded-xl shadow-lg p-6 hover:shadow-xl transition-shadow">
              {/* 건물 헤더 */}
              <div class="flex items-center justify-between mb-4">
                <div class="flex items-center gap-3">
                  <div class={`text-4xl ${building.animation === 'working' ? 'animate-pulse' : ''}`}>
                    {building.icon}
                  </div>
                  <div>
                    <h3 class="font-bold text-gray-800">{building.name}</h3>
                    <span class={`text-sm font-medium ${getStatusColor(building.status)}`}>
                      {building.status.toUpperCase()}
                    </span>
                  </div>
                </div>
                <div class="text-right">
                  <div class="text-lg font-bold text-blue-600">{building.throughput.toFixed(1)}</div>
                  <div class="text-xs text-gray-500">per/min</div>
                </div>
              </div>

              {/* 작업자 현황 */}
              <div class="mb-4">
                <div class="flex justify-between items-center mb-2">
                  <span class="text-sm font-medium text-gray-600">작업자</span>
                  <span class="text-sm text-gray-500">{building.workers}/{building.maxWorkers}</span>
                </div>
                <div class="w-full bg-gray-200 rounded-full h-2">
                  <div 
                    class="bg-green-500 h-2 rounded-full transition-all duration-500"
                    style={{ width: `${getWorkerUtilization(building.workers, building.maxWorkers)}%` }}
                  />
                </div>
              </div>

              {/* 큐 현황 */}
              <Show when={building.maxQueueSize > 0}>
                <div class="mb-4">
                  <div class="flex justify-between items-center mb-2">
                    <span class="text-sm font-medium text-gray-600">작업 대기열</span>
                    <span class="text-sm text-gray-500">{building.queueSize}/{building.maxQueueSize}</span>
                  </div>
                  <div class="w-full bg-gray-200 rounded-full h-2">
                    <div 
                      class={`h-2 rounded-full transition-all duration-500 ${
                        getQueueFillPercentage(building.queueSize, building.maxQueueSize) > 80 
                          ? 'bg-red-500' 
                          : getQueueFillPercentage(building.queueSize, building.maxQueueSize) > 60 
                            ? 'bg-yellow-500' 
                            : 'bg-blue-500'
                      }`}
                      style={{ width: `${getQueueFillPercentage(building.queueSize, building.maxQueueSize)}%` }}
                    />
                  </div>
                </div>
              </Show>

              {/* 상태 표시기 */}
              <div class="flex items-center gap-2 mt-4">
                <div class={`w-3 h-3 rounded-full ${
                  building.status === 'working' ? 'bg-green-500 animate-pulse' :
                  building.status === 'busy' ? 'bg-yellow-500' :
                  building.status === 'error' ? 'bg-red-500' : 'bg-gray-400'
                }`} />
                <span class="text-xs text-gray-500">
                  {building.status === 'working' ? '활발히 작업 중' :
                   building.status === 'busy' ? '과부하 상태' :
                   building.status === 'error' ? '오류 발생' : '대기 중'}
                </span>
              </div>
            </div>
          )}
        </For>
      </div>

      {/* 도시 하단 정보 */}
      <div class="mt-8 bg-white rounded-xl shadow-lg p-6">
        <h3 class="text-lg font-bold text-gray-800 mb-4">🌆 도시 현황</h3>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div class="text-center p-4 bg-blue-50 rounded-lg">
            <div class="text-2xl mb-2">⚡</div>
            <div class="text-sm text-gray-600">시스템 상태</div>
            <div class="text-lg font-bold text-blue-600">
              {props.isRunning ? '가동 중' : '중지'}
            </div>
          </div>
          <div class="text-center p-4 bg-green-50 rounded-lg">
            <div class="text-2xl mb-2">🎯</div>
            <div class="text-sm text-gray-600">예상 완료 시간</div>
            <div class="text-lg font-bold text-green-600">{cityStats().estimatedCompletion}</div>
          </div>
          <div class="text-center p-4 bg-red-50 rounded-lg">
            <div class="text-2xl mb-2">⚠️</div>
            <div class="text-sm text-gray-600">오류 발생</div>
            <div class="text-lg font-bold text-red-600">{cityStats().errors}</div>
          </div>
        </div>
      </div>

      {/* 데이터 플로우 시각화 */}
      <div class="mt-8">
        <h3 class="text-lg font-bold text-gray-800 mb-4">🔄 데이터 플로우</h3>
        <div class="bg-white rounded-xl shadow-lg p-6">
          <div class="flex items-center justify-between">
            <For each={buildings().filter(b => b.type !== 'control_tower')}>
              {(building, index) => (
                <>
                  <div class="flex flex-col items-center">
                    <div class={`text-3xl mb-2 ${building.animation === 'working' ? 'animate-bounce' : ''}`}>
                      {building.icon}
                    </div>
                    <div class="text-sm text-gray-600 text-center">
                      <div class="font-medium">{building.name}</div>
                      <div class="text-xs text-blue-600">{building.throughput.toFixed(1)}/min</div>
                    </div>
                  </div>
                  <Show when={index() < buildings().filter(b => b.type !== 'control_tower').length - 1}>
                    <div class="flex-1 mx-4">
                      <div class="relative">
                        <div class="h-0.5 bg-gray-300 w-full"></div>
                        <div class="absolute top-0 left-0 h-0.5 bg-blue-500 animate-pulse" 
                             style={{ width: props.isRunning ? '100%' : '0%' }}></div>
                        <div class="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 
                                  text-blue-500 text-xs animate-pulse">
                          {props.isRunning ? '→' : ''}
                        </div>
                      </div>
                    </div>
                  </Show>
                </>
              )}
            </For>
          </div>
        </div>
      </div>
    </div>
  );
};
