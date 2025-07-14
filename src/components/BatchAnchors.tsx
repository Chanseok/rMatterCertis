import { Component, For } from 'solid-js';
import { Batch } from '../stores/crawlingProcessStore';

interface BatchAnchorsProps {
  batches: Batch[];
  activeBatchId: number | null;
}

/**
 * 배치 앵커 컴포넌트 - 배치 목록 표시
 * 
 * 좌측 사이드바에 배치들의 앵커를 표시하고,
 * 사용자가 특정 배치를 선택할 수 있도록 함
 */
export const BatchAnchors: Component<BatchAnchorsProps> = (props) => {
  const getStatusColor = (status: string) => {
    switch (status) {
      case 'active': return 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
      case 'completed': return 'bg-blue-500/20 text-blue-400 border-blue-500/30';
      case 'error': return 'bg-red-500/20 text-red-400 border-red-500/30';
      default: return 'bg-slate-600/20 text-slate-400 border-slate-600/30';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'active': return '🟢';
      case 'completed': return '✅';
      case 'error': return '❌';
      default: return '⏳';
    }
  };

  return (
    <div class="h-full flex flex-col">
      <div class="flex-shrink-0 mb-4">
        <h3 class="text-lg font-semibold text-white mb-2">배치 목록</h3>
        <div class="text-sm text-slate-400">
          총 {props.batches.length}개 배치
        </div>
      </div>

      <div class="flex-1 space-y-3 overflow-y-auto">
        <For each={props.batches}>
          {(batch) => (
            <div class={`
              relative p-4 rounded-lg border transition-all duration-200 cursor-pointer
              ${props.activeBatchId === batch.id 
                ? 'bg-slate-700/50 border-slate-500 shadow-lg' 
                : 'bg-slate-800/30 border-slate-700 hover:bg-slate-700/30'
              }
            `}>
              {/* 배치 헤더 */}
              <div class="flex items-center justify-between mb-3">
                <div class="flex items-center space-x-2">
                  <span class="text-lg">{getStatusIcon(batch.status)}</span>
                  <span class="font-medium text-white">배치 #{batch.id}</span>
                </div>
                <div class={`px-2 py-1 rounded-full text-xs font-medium border ${getStatusColor(batch.status)}`}>
                  {batch.status}
                </div>
              </div>

              {/* 진행률 바 */}
              <div class="mb-3">
                <div class="flex items-center justify-between text-sm mb-1">
                  <span class="text-slate-400">진행률</span>
                  <span class="text-white">{(batch.progress * 100).toFixed(1)}%</span>
                </div>
                <div class="w-full bg-slate-700 rounded-full h-2">
                  <div 
                    class="bg-gradient-to-r from-emerald-500 to-blue-500 h-2 rounded-full transition-all duration-300"
                    style={{ width: `${batch.progress * 100}%` }}
                  />
                </div>
              </div>

              {/* 배치 정보 */}
              <div class="grid grid-cols-2 gap-2 text-xs">
                <div>
                  <span class="text-slate-400">페이지 범위</span>
                  <div class="text-white font-medium">
                    {batch.pages_range[0]}-{batch.pages_range[1]}
                  </div>
                </div>
                <div>
                  <span class="text-slate-400">현재 페이지</span>
                  <div class="text-white font-medium">
                    {batch.current_page}
                  </div>
                </div>
                <div>
                  <span class="text-slate-400">완료/총합</span>
                  <div class="text-white font-medium">
                    {batch.items_completed}/{batch.items_total}
                  </div>
                </div>
                <div>
                  <span class="text-slate-400">스테이지</span>
                  <div class="text-white font-medium">
                    {batch.stages.listPage.status === 'completed' ? '✅' : '⏳'}
                    {batch.stages.detailPage.status === 'completed' ? '✅' : '⏳'}
                    {batch.stages.dbSave.status === 'completed' ? '✅' : '⏳'}
                  </div>
                </div>
              </div>

              {/* 활성 배치 인디케이터 */}
              {props.activeBatchId === batch.id && (
                <div class="absolute left-0 top-0 bottom-0 w-1 bg-gradient-to-b from-emerald-500 to-blue-500 rounded-l-lg" />
              )}
            </div>
          )}
        </For>

        {/* 배치가 없을 때 */}
        {props.batches.length === 0 && (
          <div class="text-center py-8">
            <div class="text-4xl mb-4">📦</div>
            <div class="text-slate-400">아직 배치가 없습니다</div>
            <div class="text-slate-500 text-sm mt-2">
              크롤링을 시작하면 배치가 생성됩니다
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
