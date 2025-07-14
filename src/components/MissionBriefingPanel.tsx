import { Component } from 'solid-js';
import { MacroState } from '../stores/crawlingProcessStore';

interface MissionBriefingPanelProps {
  macroState: MacroState;
}

/**
 * 미션 브리핑 패널 - 거시적 정보 표시
 * 
 * 전체 데이터 수집 현황과 현재 진행 중인 세션의 목표,
 * 예상 완료 시간을 한눈에 파악할 수 있는 대시보드
 */
export const MissionBriefingPanel: Component<MissionBriefingPanelProps> = (props) => {
  const formatTime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hours > 0) {
      return `${hours}시간 ${minutes}분`;
    } else if (minutes > 0) {
      return `${minutes}분 ${secs}초`;
    } else {
      return `${secs}초`;
    }
  };

  return (
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* 전체 현황 */}
      <div class="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-white">전체 현황</h3>
          <div class="text-2xl">📊</div>
        </div>
        <div class="grid grid-cols-2 gap-4">
          <div class="text-center">
            <div class="text-2xl font-bold text-emerald-400">
              {props.macroState.totalKnownItems.toLocaleString()}
            </div>
            <div class="text-sm text-slate-400">총 수집 가능</div>
          </div>
          <div class="text-center">
            <div class="text-2xl font-bold text-blue-400">
              {props.macroState.itemsCollectedTotal.toLocaleString()}
            </div>
            <div class="text-sm text-slate-400">수집 완료</div>
          </div>
        </div>
        <div class="mt-4">
          <div class="text-sm text-slate-400 mb-2">전체 진행률</div>
          <div class="w-full bg-slate-700 rounded-full h-2">
            <div 
              class="bg-gradient-to-r from-emerald-500 to-blue-500 h-2 rounded-full transition-all duration-300"
              style={{
                width: `${props.macroState.totalKnownItems > 0 
                  ? (props.macroState.itemsCollectedTotal / props.macroState.totalKnownItems) * 100 
                  : 0}%`
              }}
            />
          </div>
        </div>
      </div>

      {/* 현재 세션 */}
      <div class="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-white">현재 세션</h3>
          <div class="text-2xl">🎯</div>
        </div>
        <div class="grid grid-cols-2 gap-4">
          <div class="text-center">
            <div class="text-2xl font-bold text-yellow-400">
              {props.macroState.sessionTargetItems.toLocaleString()}
            </div>
            <div class="text-sm text-slate-400">목표 수집</div>
          </div>
          <div class="text-center">
            <div class="text-2xl font-bold text-green-400">
              {props.macroState.sessionCollectedItems.toLocaleString()}
            </div>
            <div class="text-sm text-slate-400">현재 수집</div>
          </div>
        </div>
        <div class="mt-4">
          <div class="text-sm text-slate-400 mb-2">세션 진행률</div>
          <div class="w-full bg-slate-700 rounded-full h-2">
            <div 
              class="bg-gradient-to-r from-yellow-500 to-green-500 h-2 rounded-full transition-all duration-300"
              style={{
                width: `${props.macroState.sessionTargetItems > 0 
                  ? (props.macroState.sessionCollectedItems / props.macroState.sessionTargetItems) * 100 
                  : 0}%`
              }}
            />
          </div>
        </div>
      </div>

      {/* 성능 지표 */}
      <div class="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-white">성능 지표</h3>
          <div class="text-2xl">⚡</div>
        </div>
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <span class="text-slate-400">처리 속도</span>
            <span class="text-emerald-400 font-medium">
              {props.macroState.itemsPerMinute.toFixed(1)} items/min
            </span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-slate-400">예상 완료</span>
            <span class="text-blue-400 font-medium">
              {formatTime(props.macroState.sessionETASeconds)}
            </span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-slate-400">현재 단계</span>
            <span class="text-yellow-400 font-medium">
              {props.macroState.currentStage || '대기 중'}
            </span>
          </div>
        </div>
        
        {/* 사이트 정보 */}
        <div class="mt-4 pt-4 border-t border-slate-700">
          <div class="flex items-center justify-between text-sm">
            <span class="text-slate-400">총 페이지</span>
            <span class="text-slate-300">{props.macroState.totalPages}</span>
          </div>
          {props.macroState.lastDbCursor && (
            <div class="flex items-center justify-between text-sm mt-2">
              <span class="text-slate-400">마지막 위치</span>
              <span class="text-slate-300">
                페이지 {props.macroState.lastDbCursor.page}, 
                인덱스 {props.macroState.lastDbCursor.index}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
