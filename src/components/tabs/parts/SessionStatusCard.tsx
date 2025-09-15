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

const SessionStatusCard: Component<Props> = (props) => (
  <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 mb-8">
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-3xl font-bold mb-3 flex items-center gap-2">
        <span class="leading-none">🤖</span>
        <span class="bg-gradient-to-r from-blue-600 via-purple-600 to-indigo-600 bg-clip-text text-transparent">
          스마트 크롤링 엔진
        </span>
      </h2>
      <div class="flex items-center gap-2">
        <div class={`w-3 h-3 rounded-full ${props.isRunning() ? 'bg-green-400 animate-pulse' : 'bg-gray-300'}`}></div>
        <span class="text-sm font-medium text-gray-600">
          {props.isRunning() ? '실행 중' : '대기'}
        </span>
      </div>
    </div>
    <div
      class={`p-6 rounded-xl border-2 transition-all duration-300 ${
        props.isRunning()
          ? 'bg-gradient-to-r from-blue-50 to-indigo-50 border-blue-200 shadow-lg'
          : 'bg-gradient-to-r from-emerald-50 to-green-50 border-emerald-200 shadow-md'
      }`}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center space-x-4">
          <div class={`w-12 h-12 rounded-full flex items-center justify-center ${
            props.isRunning() ? 'bg-blue-500' : 'bg-emerald-500'
          }`}>
            <span class="text-2xl text-white">
              {props.isRunning() ? '🔄' : '✅'}
            </span>
          </div>
          <div>
            <h3 class="text-xl font-bold text-gray-800">{props.statusMessage()}</h3>
            <Show when={props.isRunning() && props.batchInfo().current > 0}>
              <p class="text-sm text-gray-600 mt-1">
                배치 진행: {props.batchInfo().current}
                {props.batchInfo().totalEstimated ? `/${props.batchInfo().totalEstimated}` : ''}
              </p>
            </Show>
          </div>
        </div>
        <Show when={props.isRunning() && props.batchInfo().batchId}>
          <div class="text-right">
            <div class="text-xs text-gray-500">세션 ID</div>
            <div class="text-sm font-mono text-gray-700 bg-white/50 px-2 py-1 rounded">
              {props.batchInfo().batchId}
            </div>
          </div>
        </Show>
      </div>
    </div>
  </div>
);

export default SessionStatusCard;
