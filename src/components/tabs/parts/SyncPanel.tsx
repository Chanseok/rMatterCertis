import { Show, Component } from "solid-js";

export interface SyncLiveState {
  active: boolean;
  planned?: number | null | undefined; // made optional
  pagesProcessed: number;
  inserted: number;
  updated: number;
  skipped: number;
  failed: number;
  lastPage?: number | null;
  lastWarn?: string | null;
  durationMs?: number | undefined;
}

interface Props {
  syncLive: () => SyncLiveState;
}

const SyncPanel: Component<Props> = (props) => {
  return (
    <Show when={props.syncLive().active || props.syncLive().pagesProcessed > 0}>
      <div class="bg-gradient-to-r from-teal-500 to-cyan-500 rounded-2xl p-6 mb-8 text-white shadow-2xl">
        <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-3">
              <div class="w-3 h-3 bg-white rounded-full animate-pulse"></div>
              <h3 class="text-xl font-bold">실시간 동기화</h3>
            </div>
            <div class="bg-white/20 backdrop-blur-sm rounded-full px-4 py-2">
              <span class="text-sm font-medium">
                {props.syncLive().planned ? `${props.syncLive().planned}페이지 계획` : "계획 수립 중"}
              </span>
            </div>
          </div>
          <div class="bg-white/10 rounded-xl p-1 mb-4">
            {(() => {
              const processed = props.syncLive().pagesProcessed || 0;
              const total = props.syncLive().planned || processed || 1;
              const pct = Math.min(100, (processed / Math.max(1, total)) * 100);
              return (
                <div class="relative">
                  <div class="h-3 bg-white/20 rounded-lg overflow-hidden">
                    <div
                      class="h-full bg-gradient-to-r from-white to-yellow-200 rounded-lg transition-all duration-500 ease-out"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                  <div class="absolute inset-0 flex items-center justify-center">
                    <span class="text-xs font-semibold text-white drop-shadow-lg">
                      {pct.toFixed(1)}%
                    </span>
                  </div>
                </div>
              );
            })()}
          </div>
          <div class="grid grid-cols-2 md:grid-cols-5 gap-4">
            <Stat label="처리 페이지" value={props.syncLive().pagesProcessed} />
            <Stat label="신규 추가" value={props.syncLive().inserted} bg="bg-emerald-400/20" />
            <Stat label="업데이트" value={props.syncLive().updated} bg="bg-blue-400/20" />
            <Stat label="건너뜀" value={props.syncLive().skipped} bg="bg-yellow-400/20" />
            <Stat label="실패" value={props.syncLive().failed} bg="bg-red-400/20" />
          </div>
          <Show when={props.syncLive().lastWarn}>
            <div class="mt-4 bg-red-500/20 backdrop-blur-sm border border-red-300/30 rounded-xl px-4 py-3">
              <div class="flex items-start gap-2">
                <span class="text-red-200 text-sm">⚠️</span>
                <div class="text-sm text-red-100">
                  <strong>최근 경고:</strong> {props.syncLive().lastWarn}
                </div>
              </div>
            </div>
          </Show>
      </div>
    </Show>
  );
};

const Stat: Component<{ label: string; value: number; bg?: string }> = (p) => (
  <div class={`${p.bg || 'bg-white/10'} backdrop-blur-sm rounded-xl p-3 text-center`}>
    <div class="text-2xl font-bold text-white">{p.value}</div>
    <div class="text-xs text-white/80">{p.label}</div>
  </div>
);

export default SyncPanel;
