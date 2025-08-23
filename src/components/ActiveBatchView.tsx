import { Component, For, createSignal, createEffect } from 'solid-js';
import { Batch, Stage, TaskItem } from '../stores/crawlingProcessStore';
import { AtomicTaskEvent } from '../types/events';

interface ActiveBatchViewProps {
  batch: Batch | undefined;
  recentCompletions: AtomicTaskEvent[];
}

/**
 * 활성 배치 뷰 - 선택된 배치의 상세 정보 표시
 * 
 * 배치의 각 스테이지와 작업 아이템들을 시각적으로 표현하고,
 * 실시간으로 변화하는 상태를 애니메이션으로 보여줌
 */
export const ActiveBatchView: Component<ActiveBatchViewProps> = (props) => {
  return (
    <div class="h-full flex flex-col">
      {props.batch ? (
        <>
          {/* 배치 헤더 */}
          <div class="flex-shrink-0 mb-6">
            <div class="flex items-center justify-between">
              <div>
                <h2 class="text-2xl font-bold text-white">배치 #{props.batch.id}</h2>
                <p class="text-slate-400">
                  페이지 {props.batch.pages_range[0]}-{props.batch.pages_range[1]} 
                  (현재: {props.batch.current_page})
                </p>
              </div>
              <div class="text-right">
                <div class="text-2xl font-bold text-emerald-400">
                  {(props.batch.progress * 100).toFixed(1)}%
                </div>
                <div class="text-sm text-slate-400">전체 진행률</div>
              </div>
            </div>
          </div>

          {/* 스테이지 파이프라인 */}
          <div class="flex-1 min-h-0">
            <div class="grid grid-cols-3 gap-6 h-full">
              <StageLane 
                stage={props.batch.stages.listPage} 
                title="목록 수집"
                icon="📋"
                color="emerald"
              />
              <StageLane 
                stage={props.batch.stages.detailPage} 
                title="상세 수집"
                icon="🔍"
                color="blue"
              />
              <StageLane 
                stage={props.batch.stages.dbSave} 
                title="저장"
                icon="💾"
                color="purple"
              />
            </div>
          </div>

          {/* 최근 완료 아이템 */}
          <div class="flex-shrink-0 mt-6">
            <RecentCompletions completions={props.recentCompletions} />
          </div>
        </>
      ) : (
        <div class="flex-1 flex items-center justify-center">
          <div class="text-center">
            <div class="text-6xl mb-4">🏭</div>
            <div class="text-xl text-slate-400 mb-2">배치를 선택하세요</div>
            <div class="text-slate-500">
              좌측에서 배치를 선택하면 상세 정보가 표시됩니다
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

interface StageLaneProps {
  stage: Stage;
  title: string;
  icon: string;
  color: 'emerald' | 'blue' | 'purple';
}

const StageLane: Component<StageLaneProps> = (props) => {
  const getColorClasses = (color: string) => {
    switch (color) {
      case 'emerald': return {
        bg: 'bg-emerald-500/10',
        border: 'border-emerald-500/30',
        text: 'text-emerald-400',
        progress: 'from-emerald-500 to-emerald-600'
      };
      case 'blue': return {
        bg: 'bg-blue-500/10',
        border: 'border-blue-500/30',
        text: 'text-blue-400',
        progress: 'from-blue-500 to-blue-600'
      };
      case 'purple': return {
        bg: 'bg-purple-500/10',
        border: 'border-purple-500/30',
        text: 'text-purple-400',
        progress: 'from-purple-500 to-purple-600'
      };
      default: return {
        bg: 'bg-slate-500/10',
        border: 'border-slate-500/30',
        text: 'text-slate-400',
        progress: 'from-slate-500 to-slate-600'
      };
    }
  };

  const colors = getColorClasses(props.color);

  return (
    <div class={`h-full rounded-xl border ${colors.bg} ${colors.border} p-4 flex flex-col`}>
      {/* 스테이지 헤더 */}
      <div class="flex-shrink-0 mb-4">
        <div class="flex items-center justify-between mb-2">
          <div class="flex items-center space-x-2">
            <span class="text-lg">{props.icon}</span>
            <span class={`font-semibold ${colors.text}`}>{props.title}</span>
          </div>
          <div class={`px-2 py-1 rounded-full text-xs font-medium border ${colors.border} ${colors.text}`}>
            {props.stage.status}
          </div>
        </div>
        
        {/* 진행률 */}
        <div class="mb-2">
          <div class="flex items-center justify-between text-sm mb-1">
            <span class="text-slate-400">진행률</span>
            <span class="text-white">{(props.stage.progress * 100).toFixed(1)}%</span>
          </div>
          <div class="w-full bg-slate-700 rounded-full h-2">
            <div 
              class={`bg-gradient-to-r ${colors.progress} h-2 rounded-full transition-all duration-300`}
              style={{ width: `${props.stage.progress * 100}%` }}
            />
          </div>
        </div>

        {/* 통계 */}
        <div class="grid grid-cols-2 gap-2 text-xs">
          <div>
            <span class="text-slate-400">완료</span>
            <div class="text-white font-medium">{props.stage.completed_items}</div>
          </div>
          <div>
            <span class="text-slate-400">활성</span>
            <div class="text-white font-medium">{props.stage.active_items}</div>
          </div>
          <div>
            <span class="text-slate-400">실패</span>
            <div class="text-white font-medium">{props.stage.failed_items}</div>
          </div>
          <div>
            <span class="text-slate-400">총합</span>
            <div class="text-white font-medium">{props.stage.total_items}</div>
          </div>
        </div>
      </div>

      {/* 작업 아이템 목록 */}
      <div class="flex-1 overflow-y-auto">
        <div class="space-y-2">
          <For each={props.stage.items}>
            {(item) => (
              <TaskItemCard item={item} />
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

interface TaskItemCardProps {
  item: TaskItem;
}

const TaskItemCard: Component<TaskItemCardProps> = (props) => {
  const [isAnimating, setIsAnimating] = createSignal(false);

  // 성공 상태 변화 감지 및 애니메이션
  createEffect(() => {
    if (props.item.status === 'Success') {
      setIsAnimating(true);
      setTimeout(() => setIsAnimating(false), 500);
    }
  });

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'Success': return 'bg-green-500/20 text-green-400 border-green-500/30';
      case 'Active': return 'bg-blue-500/20 text-blue-400 border-blue-500/30';
      case 'Error': return 'bg-red-500/20 text-red-400 border-red-500/30';
      case 'Retrying': return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30';
      default: return 'bg-slate-600/20 text-slate-400 border-slate-600/30';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'Success': return '✅';
      case 'Active': return '🔄';
      case 'Error': return '❌';
      case 'Retrying': return '🔁';
      default: return '⏳';
    }
  };

  return (
    <div class={`
      p-3 rounded-lg border transition-all duration-200 
  ${getStatusColor(props.item.status)}
  ${isAnimating() ? 'animate-pulse' : ''}
    `}>
      <div class="flex items-center justify-between">
        <div class="flex items-center space-x-2">
          <span class="text-sm">{getStatusIcon(props.item.status)}</span>
          <span class="text-sm font-medium text-white truncate">
            {props.item.id}
          </span>
        </div>
        {props.item.retryCount > 0 && (
          <span class="text-xs bg-yellow-500/20 text-yellow-400 px-2 py-1 rounded">
            재시도 {props.item.retryCount}
          </span>
        )}
      </div>
      {props.item.message && (
        <div class="text-xs text-slate-400 mt-1 truncate">
          {props.item.message}
        </div>
      )}
      {props.item.completedAt && (
        <div class="text-xs text-slate-500 mt-1">
          {new Date(props.item.completedAt).toLocaleTimeString()}
        </div>
      )}
    </div>
  );
};

interface RecentCompletionsProps {
  completions: AtomicTaskEvent[];
}

const RecentCompletions: Component<RecentCompletionsProps> = (props) => {
  return (
    <div class="bg-slate-800/50 backdrop-blur-sm rounded-xl p-4 border border-slate-700">
      <h3 class="text-lg font-semibold text-white mb-3">최근 완료 작업</h3>
      <div class="space-y-2 max-h-40 overflow-y-auto">
        <For each={props.completions}>
          {(completion) => (
            <div class="flex items-center justify-between p-2 bg-slate-700/30 rounded-lg">
              <div class="flex items-center space-x-2">
                <span class="text-sm">
                  {completion.status === 'Success' ? '✅' : '❌'}
                </span>
                <span class="text-sm text-white truncate">
                  {completion.task_id}
                </span>
                <span class="text-xs text-slate-400">
                  ({(completion as any).stage_type ?? (completion as any).stage_name})
                </span>
              </div>
              <span class="text-xs text-slate-500">
                {new Date(completion.timestamp).toLocaleTimeString()}
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  );
};
