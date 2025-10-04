import { Show, Component } from "solid-js";

interface BatchInfo {
  current: number;
  totalEstimated?: number;
  batchId?: string;
}

interface Props {
  isRunning: () => boolean;
  statusMessage: () => string;
  batchInfo: () => BatchInfo;
}

const SessionStatusCard: Component<Props> = (props) => {
  // 실행 중인 작업 유형 감지
  const getTaskType = () => {
    const msg = props.statusMessage();
    if (msg.includes('스마트 동기화')) return 'smart';
    if (msg.includes('빠른 동기화')) return 'fast';
    if (msg.includes('통합 파이프라인')) return 'crawl';
    if (msg.includes('수동 크롤링')) return 'manual';
    return 'default';
  };

  const getIconAndColor = () => {
    if (!props.isRunning()) {
      return { icon: '✅', bg: 'bg-emerald-500', text: 'text-emerald-700' };
    }
    
    const type = getTaskType();
    switch(type) {
      case 'smart':
        return { icon: '🧠', bg: 'bg-gradient-to-r from-purple-500 to-pink-500', text: 'text-purple-700' };
      case 'fast':
        return { icon: '🏃', bg: 'bg-gradient-to-r from-blue-500 to-cyan-500', text: 'text-blue-700' };
      case 'crawl':
        return { icon: '🎭', bg: 'bg-gradient-to-r from-purple-600 to-indigo-600', text: 'text-purple-700' };
      case 'manual':
        return { icon: '🎯', bg: 'bg-gradient-to-r from-indigo-500 to-purple-500', text: 'text-indigo-700' };
      default:
        return { icon: '🔄', bg: 'bg-blue-500', text: 'text-blue-700' };
    }
  };

  return (
    <div class="bg-white/90 backdrop-blur-sm rounded-xl shadow-lg border border-white/20 p-4 mb-6">
      <div class="flex items-center justify-between">
        {/* 상태 표시 영역 */}
        <div class="flex items-center gap-3">
          <div class={`w-10 h-10 rounded-full flex items-center justify-center ${getIconAndColor().bg} ${props.isRunning() ? 'animate-pulse' : ''}`}>
            <span class="text-xl text-white">
              {getIconAndColor().icon}
            </span>
          </div>
          <div>
            <div class={`text-lg font-bold ${getIconAndColor().text}`}>{props.statusMessage()}</div>
            <Show when={props.isRunning() && props.batchInfo().current > 0}>
              <div class="text-xs text-gray-600 mt-0.5">
                배치 {props.batchInfo().current}
                <Show when={props.batchInfo().totalEstimated}>
                  {' '}/ {props.batchInfo().totalEstimated}
                </Show>
                <Show when={props.batchInfo().batchId}>
                  {' '}(ID: {props.batchInfo().batchId})
                </Show>
              </div>
            </Show>
          </div>
        </div>

        {/* 상태 인디케이터 */}
        <div class="flex items-center gap-2">
          <div class={`w-2 h-2 rounded-full ${props.isRunning() ? 'bg-blue-400 animate-pulse' : 'bg-gray-300'}`}></div>
          <span class="text-xs font-medium text-gray-500">
            {props.isRunning() ? '실행 중' : '대기'}
          </span>
        </div>
      </div>
    </div>
  );
};

export default SessionStatusCard;
