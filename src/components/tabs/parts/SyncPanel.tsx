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
  // 시작 시간 추적
  let startTime: number | null = null;
  
  // 시간 추정 계산
  const getTimeEstimate = () => {
    const processed = props.syncLive().pagesProcessed || 0;
    const total = props.syncLive().planned || 0;
    
    if (!total || processed === 0) {
      return { remaining: 0, eta: null, avgTime: 0 };
    }
    
    // 시작 시간 설정 (첫 페이지 처리 시)
    if (processed === 1 && !startTime) {
      startTime = Date.now();
    }
    
    if (!startTime || processed < 2) {
      return { remaining: 0, eta: null, avgTime: 0 };
    }
    
    const elapsed = Date.now() - startTime;
    const avgTimePerPage = elapsed / processed; // ms per page
    const remainingPages = total - processed;
    const remainingMs = avgTimePerPage * remainingPages;
    const eta = new Date(Date.now() + remainingMs);
    
    return {
      remaining: remainingMs,
      eta,
      avgTime: avgTimePerPage,
    };
  };
  
  // 시간 포맷팅 헬퍼
  const formatDuration = (ms: number) => {
    if (ms < 1000) return `${Math.round(ms)}ms`;
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    
    if (hours > 0) {
      return `${hours}시간 ${minutes % 60}분`;
    } else if (minutes > 0) {
      return `${minutes}분 ${seconds % 60}초`;
    } else {
      return `${seconds}초`;
    }
  };
  
  const formatETA = (date: Date | null) => {
    if (!date) return '-';
    const hours = date.getHours().toString().padStart(2, '0');
    const minutes = date.getMinutes().toString().padStart(2, '0');
    return `${hours}:${minutes}`;
  };
  
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
              const timeEst = getTimeEstimate();
              
              return (
                <>
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
                  {/* 시간 추정 표시 */}
                  <Show when={timeEst.remaining > 0}>
                    <div class="mt-2 flex items-center justify-between text-xs text-white/90">
                      <div class="flex items-center gap-4">
                        <span>⏱️ 남은 시간: <b>{formatDuration(timeEst.remaining)}</b></span>
                        <span>📅 완료 예정: <b>{formatETA(timeEst.eta)}</b></span>
                      </div>
                      <span class="text-white/70">평균: {formatDuration(timeEst.avgTime)}/페이지</span>
                    </div>
                  </Show>
                </>
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
