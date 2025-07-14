import { Component, createEffect, onMount, onCleanup } from 'solid-js';
import { listen } from '@tauri-apps/api/event';
import { 
  sessionStore, 
  updateSystemState, 
  handleAtomicTaskEvent, 
  createNewBatch, 
  resetCrawlingSession 
} from '../stores/crawlingProcessStore';
import { SystemStatePayload, AtomicTaskEvent, LiveSystemState } from '../types/events';
import { MissionBriefingPanel } from './MissionBriefingPanel';
import { BatchAnchors } from './BatchAnchors';
import { ActiveBatchView } from './ActiveBatchView';

/**
 * 크롤링 프로세스 대시보드 - "살아있는 생산 라인" UI
 * 
 * 이 컴포넌트는 백엔드의 실시간 이벤트를 수신하여
 * 크롤링 과정을 시각적으로 표현하는 메인 대시보드입니다.
 */
export const CrawlingProcessDashboard: Component = () => {
  let systemStateUnlisten: (() => void) | null = null;
  let atomicTaskUnlisten: (() => void) | null = null;
  let liveStateUnlisten: (() => void) | null = null;

  onMount(async () => {
    console.log('🚀 CrawlingProcessDashboard mounted, setting up event listeners...');
    
    try {
      // 1. 거시적 상태 업데이트 리스너 (시스템 상태 스냅샷)
      systemStateUnlisten = await listen<SystemStatePayload>('system-state-update', (event) => {
        console.log('📊 System state update received:', event.payload);
        updateSystemState(event.payload);
      });

      // 2. 미시적 상태 업데이트 리스너 (원자적 작업 이벤트)
      atomicTaskUnlisten = await listen<AtomicTaskEvent>('atomic-task-update', (event) => {
        console.log('⚡ Atomic task event received:', event.payload);
        handleAtomicTaskEvent(event.payload);
      });

      // 3. Live Production Line 상태 업데이트 리스너
      liveStateUnlisten = await listen<LiveSystemState>('live-state-update', (event) => {
        console.log('🏭 Live state update received:', event.payload);
        
        // 기본 상태 업데이트
        updateSystemState(event.payload.basic_state);
        
        // 현재 배치 정보 업데이트
        if (event.payload.current_batch) {
          // 기존 배치가 없으면 새로 생성
          const existingBatch = sessionStore.batches.find(b => b.id === event.payload.current_batch!.id);
          if (!existingBatch) {
            createNewBatch(event.payload.current_batch);
          }
        }
        
        // 최근 완료 이벤트들 처리
        event.payload.recent_completions.forEach(completion => {
          handleAtomicTaskEvent(completion);
        });
      });

      console.log('✅ All event listeners set up successfully');
    } catch (error) {
      console.error('❌ Failed to set up event listeners:', error);
    }
  });

  onCleanup(() => {
    console.log('🧹 CrawlingProcessDashboard cleanup, removing event listeners...');
    systemStateUnlisten?.();
    atomicTaskUnlisten?.();
    liveStateUnlisten?.();
  });

  // 크롤링 상태 변화 감지
  createEffect(() => {
    console.log('🔄 Crawling state changed:', {
      isRunning: sessionStore.isRunning,
      activeBatchId: sessionStore.activeBatchId,
      batchesCount: sessionStore.batches.length,
      lastUpdated: sessionStore.lastUpdated
    });
  });

  return (
    <div class="h-full bg-gradient-to-br from-slate-900 to-slate-800 text-white overflow-hidden">
      {/* 헤더 */}
      <div class="bg-slate-800/50 backdrop-blur-sm border-b border-slate-700 px-6 py-4">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-2xl font-bold text-emerald-400">Live Production Line</h1>
            <p class="text-slate-400 text-sm">실시간 크롤링 공정 모니터링</p>
          </div>
          <div class="flex items-center space-x-4">
            <div class={`px-3 py-1 rounded-full text-sm font-medium ${
              sessionStore.isRunning 
                ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' 
                : 'bg-slate-600/20 text-slate-400 border border-slate-600/30'
            }`}>
              {sessionStore.isRunning ? '🟢 운영 중' : '🔴 정지'}
            </div>
            <button 
              onClick={resetCrawlingSession}
              class="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
            >
              초기화
            </button>
          </div>
        </div>
      </div>

      {/* 메인 콘텐츠 */}
      <div class="h-full flex flex-col">
        {/* 거시적 정보 패널 */}
        <div class="flex-shrink-0 p-6">
          <MissionBriefingPanel macroState={sessionStore.macroState} />
        </div>

        {/* 배치 앵커 및 활성 배치 뷰 */}
        <div class="flex-1 flex min-h-0">
          {/* 배치 앵커 (왼쪽) */}
          <div class="flex-shrink-0 w-64 border-r border-slate-700 p-4">
            <BatchAnchors 
              batches={sessionStore.batches}
              activeBatchId={sessionStore.activeBatchId}
            />
          </div>

          {/* 활성 배치 상세 뷰 (오른쪽) */}
          <div class="flex-1 p-6">
            <ActiveBatchView 
              batch={sessionStore.batches.find(b => b.id === sessionStore.activeBatchId)}
              recentCompletions={sessionStore.recentCompletions}
            />
          </div>
        </div>

        {/* 하단 상태 바 */}
        <div class="flex-shrink-0 bg-slate-800/30 border-t border-slate-700 px-6 py-3">
          <div class="flex items-center justify-between text-sm">
            <div class="flex items-center space-x-6">
              <span class="text-slate-400">
                마지막 업데이트: {new Date(sessionStore.lastUpdated).toLocaleTimeString()}
              </span>
              <span class="text-slate-400">
                총 배치: {sessionStore.batches.length}
              </span>
              <span class="text-slate-400">
                현재 단계: {sessionStore.macroState.currentStage || '대기 중'}
              </span>
            </div>
            <div class="flex items-center space-x-4">
              <span class="text-emerald-400">
                {sessionStore.macroState.itemsPerMinute.toFixed(1)} items/min
              </span>
              <span class="text-blue-400">
                ETA: {Math.floor(sessionStore.macroState.sessionETASeconds / 60)}분
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
