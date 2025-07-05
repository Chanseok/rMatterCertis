/**
 * CrawlingProgressDisplay - 크롤링 실시간 진행 상황 표시 컴포넌트
 * 스크린샷의 세부 진행 상황을 구현
 */

import { Component, createSignal, createMemo, For, Show } from 'solid-js';
import type { CrawlingProgress } from '../../types/crawling';

export interface CrawlingProgressDisplayProps {
  progress: CrawlingProgress | null;
  isRunning: boolean;
}

export const CrawlingProgressDisplay: Component<CrawlingProgressDisplayProps> = (props) => {
  // 시뮬레이션을 위한 단계 상태
  const [simulatedStage, setSimulatedStage] = createSignal<string>('ProductList');
  
  // 단계별 정보 정의
  const stages = [
    { key: 'Idle', name: '시작', icon: '🚀' },
    { key: 'ProductList', name: '1단계: 목록', icon: '📋' },
    { key: 'Verification', name: '2단계: 검증', icon: '✓' },
    { key: 'ProductDetails', name: '3단계: 상세', icon: '📊' },
    { key: 'Completed', name: '완료', icon: '🎉' }
  ];

  // 현재 단계 인덱스 계산
  const currentStageIndex = createMemo(() => {
    const currentStage = getSimulatedStage();
    const index = stages.findIndex(stage => stage.key === currentStage);
    return index >= 0 ? index : 0;
  });

  // 회차 정보 (예: "총 2회 중 1회차 진행 중")
  const sessionInfo = createMemo(() => {
    return "총 2회 중 1회차 진행 중";
  });

  // 현재 진행 정보 (실제 또는 시뮬레이션)
  const currentProgress = createMemo(() => getSimulatedProgress());

  // 페이지별 상태 (예시 데이터 - 실제로는 백엔드에서 받아옴)
  const [pageStates] = createSignal([
    { page: 476, status: 'completed' },
    { page: 477, status: 'completed' },
    { page: 478, status: 'completed' },
    { page: 479, status: 'completed' },
    { page: 480, status: 'processing' }
  ]);

  // 시뮬레이션된 크롤링 상태 (실제 데이터가 없을 때 사용)
  const getSimulatedStage = () => {
    if (props.isRunning && props.progress?.current_stage) {
      // 크롤링이 실행 중일 때는 실제 상태 사용
      return props.progress.current_stage;
    }
    // 크롤링이 실행 중이 아닐 때는 시뮬레이션 상태
    return simulatedStage();
  };

  const getSimulatedProgress = () => {
    if (props.progress) return props.progress;
    
    // 시뮬레이션된 진행 정보
    const stage = simulatedStage();
    if (stage === 'ProductDetails') {
      return {
        current: 0,
        total: 1,
        percentage: 0.0,
        current_stage: 'ProductDetails',
        current_step: '3단계: 상세정보 수집',
        elapsed_time: 37, // 37초
        remaining_time: 26, // 26초
        errors: []
      };
    }
    
    return {
      current: 1,
      total: 5,
      percentage: 20.0,
      current_stage: 'ProductList',
      current_step: '1단계: 목록 수집',
      elapsed_time: 6, // 6초
      remaining_time: 62, // 1분 2초
      errors: []
    };
  };

  // 배치 정보
  const batchInfo = createMemo(() => {
    const progress = currentProgress();
    return {
      current: 1,
      total: 2,
      currentPageBatch: 1,
      totalPageBatch: 5,
      percentage: progress.percentage
    };
  });

  const getPageStatusColor = (status: string) => {
    switch (status) {
      case 'completed': return 'bg-green-500';
      case 'processing': return 'bg-blue-500 animate-pulse';
      case 'error': return 'bg-red-500';
      default: return 'bg-gray-300';
    }
  };

  const getPageStatusIcon = (status: string) => {
    switch (status) {
      case 'completed': return '✓';
      case 'processing': return '●';
      case 'error': return '✗';
      default: return '○';
    }
  };

  return (
    <div class="space-y-6">
      {/* 시뮬레이션 제어 버튼 (개발용) */}
      <Show when={!props.isRunning}>
        <div class="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg border-2 border-dashed border-gray-300 dark:border-gray-600">
          <div class="text-sm text-gray-600 dark:text-gray-400 mb-2">UI 시뮬레이션 (스크린샷 시연용):</div>
          <div class="flex space-x-2">
            <button 
              class="px-3 py-1 bg-blue-500 text-white rounded text-sm hover:bg-blue-600"
              onClick={() => setSimulatedStage('ProductList')}
            >
              1단계: 목록 수집
            </button>
            <button 
              class="px-3 py-1 bg-green-500 text-white rounded text-sm hover:bg-green-600"
              onClick={() => setSimulatedStage('ProductDetails')}
            >
              3단계: 상세정보 수집
            </button>
          </div>
        </div>
      </Show>
      {/* 현재 상태 헤더 */}
      <div class="bg-blue-50 dark:bg-blue-900/20 p-4 rounded-lg border border-blue-200 dark:border-blue-800">
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-lg font-semibold text-blue-900 dark:text-blue-100">
            현재 상태: <span class="bg-blue-600 text-white px-2 py-1 rounded text-sm">{stages[currentStageIndex()]?.name || '대기'}</span>
          </h3>
          <span class="text-sm text-blue-700 dark:text-blue-300">
            {sessionInfo()}
          </span>
        </div>
        
        <Show when={currentProgress()}>
          <div class="text-sm text-blue-800 dark:text-blue-200">
            {currentProgress().current_step || '제품 목록 수집'}
          </div>
        </Show>

        {/* 일시정지 버튼 */}
        <Show when={props.isRunning}>
          <div class="flex justify-center mt-4">
            <button class="bg-orange-500 text-white px-4 py-2 rounded flex items-center space-x-2 hover:bg-orange-600">
              <span>⏸️</span>
              <span>일시 정지</span>
            </button>
          </div>
        </Show>

        {/* 예상 완료 시간 */}
        <Show when={currentProgress().remaining_time}>
          <div class="mt-3 text-center text-sm text-gray-600 dark:text-gray-400">
            예상 완료: 오후 {new Date(Date.now() + (currentProgress().remaining_time || 0) * 1000).toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}
          </div>
        </Show>
      </div>

      {/* 시간 정보 */}
      <div class="grid grid-cols-2 gap-4">
        <div class="text-center bg-white dark:bg-gray-800 p-4 rounded-lg shadow-sm">
          <div class="text-sm text-gray-600 dark:text-gray-400">소요시간</div>
          <div class="text-2xl font-mono font-bold text-gray-800 dark:text-gray-200">
            {formatTime(currentProgress().elapsed_time)}
          </div>
        </div>
        <div class="text-center bg-white dark:bg-gray-800 p-4 rounded-lg shadow-sm">
          <div class="text-sm text-gray-600 dark:text-gray-400">예상 남은 시간</div>
          <div class="text-2xl font-mono font-bold text-blue-600 dark:text-blue-400">
            {formatTime(currentProgress().remaining_time || 0)}
          </div>
        </div>
      </div>

      {/* 진행 단계 표시 */}
      <div class="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
        <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-4">전체 배치 진행률</h4>
        
        {/* 단계별 진행바 */}
        <div class="relative mb-6">
          <div class="flex justify-between">
            <For each={stages}>
              {(stage, index) => (
                <div class="flex flex-col items-center flex-1">
                  <div class={`w-8 h-8 rounded-full flex items-center justify-center text-white text-sm font-bold ${
                    index() < currentStageIndex() ? 'bg-green-500' : 
                    index() === currentStageIndex() ? 'bg-blue-500' : 'bg-gray-300'
                  }`}>
                    {index() < currentStageIndex() ? '✓' : stage.icon}
                  </div>
                  <div class="text-xs mt-1 text-center">
                    {stage.name}
                  </div>
                </div>
              )}
            </For>
          </div>
          
          {/* 진행바 선 */}
          <div class="absolute top-4 left-4 right-4 h-0.5 bg-gray-300 -z-10">
            <div 
              class="h-full bg-blue-500 transition-all duration-500"
              style={`width: ${(currentStageIndex() / (stages.length - 1)) * 100}%`}
            ></div>
          </div>
        </div>
        
        <div class="text-center text-sm text-blue-600 dark:text-blue-400 font-medium">
          {stages[currentStageIndex()]?.name || '대기 중'}
        </div>
      </div>

      {/* 세부 진행 정보 (1단계: 목록 수집시만 표시) */}
      <Show when={getSimulatedStage() === 'ProductList'}>
        <div class="bg-yellow-50 dark:bg-yellow-900/20 p-4 rounded-lg border border-yellow-200 dark:border-yellow-800">
          <div class="flex items-center justify-between mb-3">
            <h4 class="text-lg font-semibold text-yellow-900 dark:text-yellow-100">
              총 2회 중 1회차 진행 중
            </h4>
            <span class="text-sm text-yellow-700 dark:text-yellow-300">
              전체 배치 진행률: 1 / 2
            </span>
          </div>
          
          {/* 전체 진행바 */}
          <div class="mb-4">
            <div class="w-full bg-yellow-200 dark:bg-yellow-800 rounded-full h-3">
              <div 
                class="bg-yellow-500 h-3 rounded-full transition-all duration-300"
                style="width: 40%"
              ></div>
            </div>
          </div>

          {/* 현재 단계 정보 */}
          <div class="bg-white dark:bg-gray-800 p-3 rounded mb-4">
            <div class="text-lg font-semibold text-blue-600 dark:text-blue-400 mb-2">
              1단계: 목록
            </div>
            <h5 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-3">
              1단계: 제품 목록 페이지 읽기
            </h5>
            
            {/* 페이지 진행 상황 */}
            <div class="mb-4">
              <div class="flex justify-between text-sm text-gray-700 dark:text-gray-300 mb-2">
                <span>페이지 진행 상황:</span>
                <span>{pageStates().filter(p => p.status === 'completed').length} / {pageStates().length} 페이지</span>
              </div>
              
              <div class="flex space-x-2 mb-3 justify-center">
                <For each={pageStates()}>
                  {(pageState) => (
                    <div class={`relative w-12 h-12 ${getPageStatusColor(pageState.status)} rounded border-2 border-yellow-400 flex flex-col items-center justify-center text-white text-xs font-bold`}>
                      <div class="text-xs">p.{pageState.page}</div>
                      <div class="text-sm">{getPageStatusIcon(pageState.status)}</div>
                    </div>
                  )}
                </For>
              </div>
            </div>

            {/* 진행 정보 */}
            <div class="space-y-2 text-sm text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-700 p-3 rounded">
              <div>• 총 페이지 수: {pageStates().length}페이지</div>
              <div>• 완료된 성공한 페이지: {pageStates().filter(p => p.status === 'completed').length}페이지</div>
              <div>• 성공된 저장도 횟수: 위</div>
            </div>

            {/* 배치 진행률 */}
            <div class="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 rounded border border-blue-200 dark:border-blue-800">
              <div class="text-sm font-medium text-blue-800 dark:text-blue-200 mb-2">
                배치 {batchInfo().current}/{batchInfo().total} - 제품 목록 페이지 {batchInfo().currentPageBatch}/{batchInfo().totalPageBatch} 처리 중 ({batchInfo().percentage.toFixed(1)}%) (목록 페이지: {pageStates().filter(p => p.status === 'completed').length}페이지)
              </div>
              <div class="w-full bg-blue-200 dark:bg-blue-700 rounded-full h-2">
                <div 
                  class="bg-blue-500 h-2 rounded-full transition-all duration-300"
                  style={`width: ${batchInfo().percentage}%`}
                ></div>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* 3단계: 상세정보 수집 */}
      <Show when={getSimulatedStage() === 'ProductDetails'}>
        <div class="bg-green-50 dark:bg-green-900/20 p-4 rounded-lg border border-green-200 dark:border-green-800">
          <h4 class="text-lg font-semibold text-green-900 dark:text-green-100 mb-3">
            3단계: 제품 상세정보 수집
          </h4>
          
          <div class="bg-white dark:bg-gray-800 p-3 rounded mb-3">
            <div class="flex justify-between items-center mb-3">
              <span class="text-sm text-green-700 dark:text-green-300">진행률</span>
              <span class="text-lg font-bold text-green-600 dark:text-green-400">
                {currentProgress().percentage.toFixed(1)}% 완료
              </span>
            </div>
            
            <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3 mb-3">
              <div 
                class="bg-green-500 h-3 rounded-full transition-all duration-300"
                style={`width: ${currentProgress().percentage}%`}
              ></div>
            </div>
            
            <div class="text-sm text-green-700 dark:text-green-300">
              진행률: {currentProgress().current} / {currentProgress().total}
            </div>
          </div>
          
          <Show when={currentProgress().remaining_time}>
            <div class="text-center text-sm text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20 p-2 rounded">
              예상 완료 시간: {new Date(Date.now() + (currentProgress().remaining_time || 0) * 1000).toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}
            </div>
          </Show>

          {/* 상세 정보 수집 시작 시각 */}
          <div class="mt-3 text-sm text-blue-600 dark:text-blue-400">
            3단계: 제품 상세 정보 수집 시작 시각 (0/12)
          </div>
        </div>
      </Show>
    </div>
  );
};

// 시간 포맷팅 헬퍼 함수
function formatTime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}
