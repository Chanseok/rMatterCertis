/**
 * CrawlingDashboard - 게임 스타일 크롤링 대시보드 메인 컴포넌트
 * 도시 성장 시뮬레이션과 실시간 메트릭을 통합한 종합 대시보드
 */

import { Component, createSignal, createMemo, For, Show, onMount, onCleanup } from 'solid-js';
import type { CrawlingProgress } from '../types/crawling';
import { CrawlingCityDashboard } from './visualization/CrawlingCityDashboard';
import { CrawlingCity3D } from './visualization/CrawlingCity3D';
import { CrawlingMetricsChart } from './visualization/CrawlingMetricsChart';

export interface CrawlingDashboardProps {
  progress: CrawlingProgress | null;
  isRunning: boolean;
  onToggleRunning: () => void;
  onPauseResume: () => void;
  onStop: () => void;
}

type ViewMode = 'city' | '3d' | 'metrics' | 'combined';

export const CrawlingDashboard: Component<CrawlingDashboardProps> = (props) => {
  const [viewMode, setViewMode] = createSignal<ViewMode>('city');
  const [selectedBuilding, setSelectedBuilding] = createSignal<string | null>(null);
  const [showSettings, setShowSettings] = createSignal(false);
  const [autoSwitchViews, setAutoSwitchViews] = createSignal(false);
  const [metricsTimeRange, setMetricsTimeRange] = createSignal(5); // minutes

  let viewSwitchInterval: number;

  onMount(() => {
    // 자동 뷰 전환 설정
    if (autoSwitchViews()) {
      startAutoViewSwitch();
    }
  });

  onCleanup(() => {
    if (viewSwitchInterval) {
      clearInterval(viewSwitchInterval);
    }
  });

  const startAutoViewSwitch = () => {
    const views: ViewMode[] = ['city', '3d', 'metrics'];
    let currentIndex = 0;
    
    viewSwitchInterval = setInterval(() => {
      currentIndex = (currentIndex + 1) % views.length;
      setViewMode(views[currentIndex]);
    }, 10000); // 10초마다 전환
  };

  const stopAutoViewSwitch = () => {
    if (viewSwitchInterval) {
      clearInterval(viewSwitchInterval);
      viewSwitchInterval = 0;
    }
  };

  const handleAutoSwitchToggle = () => {
    const newValue = !autoSwitchViews();
    setAutoSwitchViews(newValue);
    
    if (newValue) {
      startAutoViewSwitch();
    } else {
      stopAutoViewSwitch();
    }
  };

  const handleBuildingClick = (buildingId: string) => {
    setSelectedBuilding(buildingId);
    console.log('Building clicked:', buildingId);
  };

  const getViewModeIcon = (mode: ViewMode) => {
    switch (mode) {
      case 'city': return '🏙️';
      case '3d': return '🎮';
      case 'metrics': return '📊';
      case 'combined': return '📱';
      default: return '🏙️';
    }
  };

  const getViewModeLabel = (mode: ViewMode) => {
    switch (mode) {
      case 'city': return 'City View';
      case '3d': return '3D View';
      case 'metrics': return 'Metrics';
      case 'combined': return 'Combined';
      default: return 'City View';
    }
  };

  const currentStats = createMemo(() => {
    if (!props.progress) return null;
    
    return {
      totalPages: props.progress.total || 0,
      completedPages: props.progress.current || 0,
      percentage: props.progress.percentage || 0,
      status: props.progress.status || 'Idle',
      stage: props.progress.current_stage || 'Idle',
      message: props.progress.message || 'Ready to start',
      elapsedTime: props.progress.elapsed_time || 0,
      newItems: props.progress.new_items || 0
    };
  });

  return (
    <div class="w-full h-screen bg-gradient-to-br from-slate-50 to-slate-100 flex flex-col">
      {/* 헤더 */}
      <div class="bg-white border-b border-gray-200 px-6 py-4">
        <div class="flex justify-between items-center">
          <div class="flex items-center gap-4">
            <h1 class="text-2xl font-bold text-gray-800">
              🎮 Crawling Game Dashboard
            </h1>
            <div class="flex items-center gap-2 text-sm text-gray-600">
              <div class={`w-2 h-2 rounded-full ${props.isRunning ? 'bg-green-500 animate-pulse' : 'bg-gray-400'}`} />
              <span>{props.isRunning ? 'Running' : 'Stopped'}</span>
            </div>
          </div>
          
          <div class="flex items-center gap-2">
            {/* 뷰 모드 선택 */}
            <div class="flex bg-gray-100 rounded-lg p-1">
              <For each={['city', '3d', 'metrics', 'combined'] as ViewMode[]}>
                {(mode) => (
                  <button
                    onClick={() => setViewMode(mode)}
                    class={`px-3 py-1 rounded-md text-sm font-medium transition-all ${
                      viewMode() === mode
                        ? 'bg-white text-blue-600 shadow-sm'
                        : 'text-gray-600 hover:text-gray-800'
                    }`}
                  >
                    {getViewModeIcon(mode)} {getViewModeLabel(mode)}
                  </button>
                )}
              </For>
            </div>

            {/* 설정 버튼 */}
            <button
              onClick={() => setShowSettings(!showSettings())}
              class="p-2 text-gray-600 hover:text-gray-800 rounded-lg hover:bg-gray-100"
            >
              ⚙️
            </button>
          </div>
        </div>

        {/* 상태 표시줄 */}
        <Show when={currentStats()}>
          {(stats) => (
            <div class="mt-4 grid grid-cols-2 md:grid-cols-6 gap-4">
              <div class="bg-blue-50 rounded-lg p-3 text-center">
                <div class="text-lg font-bold text-blue-600">{stats().completedPages}</div>
                <div class="text-sm text-gray-600">Completed Pages</div>
              </div>
              <div class="bg-green-50 rounded-lg p-3 text-center">
                <div class="text-lg font-bold text-green-600">{stats().percentage.toFixed(1)}%</div>
                <div class="text-sm text-gray-600">Progress</div>
              </div>
              <div class="bg-purple-50 rounded-lg p-3 text-center">
                <div class="text-lg font-bold text-purple-600">{stats().newItems}</div>
                <div class="text-sm text-gray-600">New Items</div>
              </div>
              <div class="bg-orange-50 rounded-lg p-3 text-center">
                <div class="text-lg font-bold text-orange-600">{Math.floor(stats().elapsedTime / 60)}m</div>
                <div class="text-sm text-gray-600">Elapsed Time</div>
              </div>
              <div class="bg-red-50 rounded-lg p-3 text-center">
                <div class="text-lg font-bold text-red-600">{stats().stage}</div>
                <div class="text-sm text-gray-600">Current Stage</div>
              </div>
              <div class="bg-gray-50 rounded-lg p-3 text-center">
                <div class="text-lg font-bold text-gray-600">{stats().status}</div>
                <div class="text-sm text-gray-600">Status</div>
              </div>
            </div>
          )}
        </Show>
      </div>

      {/* 설정 패널 */}
      <Show when={showSettings()}>
        <div class="bg-white border-b border-gray-200 px-6 py-4">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">🎛️ Dashboard Settings</h3>
          
          <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div class="flex items-center gap-3">
              <label class="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={autoSwitchViews()}
                  onChange={handleAutoSwitchToggle}
                  class="rounded border-gray-300"
                />
                <span class="text-sm text-gray-700">Auto Switch Views</span>
              </label>
            </div>
            
            <div class="flex items-center gap-3">
              <label class="text-sm text-gray-700">Metrics Time Range:</label>
              <select
                value={metricsTimeRange()}
                onChange={(e) => setMetricsTimeRange(Number(e.target.value))}
                class="rounded border-gray-300 text-sm"
              >
                <option value={1}>1 minute</option>
                <option value={5}>5 minutes</option>
                <option value={15}>15 minutes</option>
                <option value={30}>30 minutes</option>
              </select>
            </div>
            
            <div class="flex items-center gap-2">
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
        </div>
      </Show>

      {/* 메인 콘텐츠 */}
      <div class="flex-1 overflow-hidden">
        <Show when={viewMode() === 'city'}>
          <CrawlingCityDashboard
            progress={props.progress}
            isRunning={props.isRunning}
            onToggleRunning={props.onToggleRunning}
            onPauseResume={props.onPauseResume}
            onStop={props.onStop}
          />
        </Show>

        <Show when={viewMode() === '3d'}>
          <CrawlingCity3D
            progress={props.progress}
            isRunning={props.isRunning}
            onBuildingClick={handleBuildingClick}
          />
        </Show>

        <Show when={viewMode() === 'metrics'}>
          <div class="p-6 h-full overflow-auto">
            <CrawlingMetricsChart
              progress={props.progress}
              isRunning={props.isRunning}
              timeRange={metricsTimeRange()}
            />
          </div>
        </Show>

        <Show when={viewMode() === 'combined'}>
          <div class="grid grid-cols-1 lg:grid-cols-2 h-full">
            <div class="border-r border-gray-200">
              <CrawlingCity3D
                progress={props.progress}
                isRunning={props.isRunning}
                onBuildingClick={handleBuildingClick}
              />
            </div>
            <div class="p-4 overflow-auto">
              <CrawlingMetricsChart
                progress={props.progress}
                isRunning={props.isRunning}
                timeRange={metricsTimeRange()}
              />
            </div>
          </div>
        </Show>
      </div>

      {/* 하단 상태 바 */}
      <div class="bg-white border-t border-gray-200 px-6 py-2">
        <div class="flex justify-between items-center text-sm text-gray-600">
          <div class="flex items-center gap-4">
            <span>View: {getViewModeLabel(viewMode())}</span>
            <Show when={selectedBuilding()}>
              <span>Selected: {selectedBuilding()}</span>
            </Show>
          </div>
          <div class="flex items-center gap-4">
            <span>Last Updated: {new Date().toLocaleTimeString()}</span>
            <div class="flex items-center gap-1">
              <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
              <span>Live</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};