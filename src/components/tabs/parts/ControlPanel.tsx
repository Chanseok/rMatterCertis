import { Component } from "solid-js";

interface ControlPanelProps {
  isRunning: () => boolean;
  isSyncing: () => boolean;
  effectsOn: () => boolean;
  syncPulse: () => boolean;
  syncRanges: () => string;
  startUnifiedAdvanced: () => Promise<void> | void;
  calculateCrawlingRange: () => Promise<void> | void;
  deriveRangesFromDiagnostics: () => string | null;
  setSyncRanges: (v: string) => void;
  setSyncPulse: (b: boolean) => void; // (kept for future parity)
  setEffectsOn: (b: boolean) => void;
  setIsSyncing: (b: boolean) => void;
  setCrawlingRange: (updater: any) => void;
  addLog: (msg: string) => void;
  tauriApi: any;
  // 🏃 Shallow Sync handlers
  handleShallowSync?: () => Promise<void> | void;
  handleSmartSync?: () => Promise<void> | void;
  handleComplementCrawl?: () => Promise<void> | void;
  // 도움말 패널
  onHelpClick?: () => void;
  // 정지 기능
  onStop?: () => Promise<void> | void;
}

const ControlPanel: Component<ControlPanelProps> = (p) => {
  return (
    <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 mb-8">
      {/* 상단 헤더 with 도움말 버튼 */}
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-sm font-semibold text-gray-600">크롤링 컨트롤</h3>
        {p.onHelpClick && (
          <button
            onClick={p.onHelpClick}
            class="flex items-center gap-1 px-3 py-1.5 text-xs font-medium text-blue-700 bg-blue-50 hover:bg-blue-100 rounded-lg transition-colors"
            title="버튼 사용 가이드"
          >
            <span>❓</span>
            <span>사용 가이드</span>
          </button>
        )}
      </div>

      {/* 버튼 그룹 */}
      <div class="flex flex-wrap gap-3 items-end">
        {/* 정지 버튼 (실행 중일 때만 표시) */}
        {p.isRunning() && p.onStop && (
          <button
            onClick={() => p.onStop?.()}
            class="px-6 py-3 rounded-xl font-semibold text-white bg-gradient-to-r from-red-500 to-rose-500 hover:from-red-600 hover:to-rose-600 ripple shadow-md hover:shadow-lg transition animate-pulse"
            title="현재 실행 중인 작업 중지"
          >
            🛑 정지
          </button>
        )}

        <button
          onClick={() => { p.startUnifiedAdvanced(); }}
          disabled={p.isRunning()}
          class={`px-6 py-3 rounded-xl font-semibold text-white ripple shadow-md hover:shadow-lg transition ${p.isRunning() ? 'bg-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-purple-600 to-indigo-600 hover:from-purple-700 hover:to-indigo-700'}`}
          title="전체 통합 파이프라인 실행 (Stage 1~5)"
        >
          {p.isRunning() ? '통합 파이프라인 실행 중...' : '🎭 크롤링'}
        </button>

        <button
          onClick={p.calculateCrawlingRange}
          disabled={p.isRunning()}
          class="px-6 py-3 rounded-xl font-semibold text-blue-700 bg-white border border-blue-200 hover:bg-blue-50 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow"
          title="사이트 상태를 다시 분석하여 크롤링 범위 재계산"
        >
          📊 범위 다시 계산
        </button>

        {/* 🏃 Shallow Sync buttons */}
        {p.handleShallowSync && (
          <button
            onClick={() => p.handleShallowSync?.()}
            disabled={p.isRunning() || p.isSyncing()}
            class="px-6 py-3 rounded-xl font-semibold text-white bg-gradient-to-r from-blue-500 to-cyan-500 hover:from-blue-600 hover:to-cyan-600 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow-md hover:shadow-lg transition"
            title="전체 페이지의 좌표만 빠르게 동기화 (상세 정보 제외, 5-8분)"
          >
            🏃 빠른 동기화
          </button>
        )}

        {p.handleSmartSync && (
          <button
            onClick={() => p.handleSmartSync?.()}
            disabled={p.isRunning() || p.isSyncing()}
            class="px-6 py-3 rounded-xl font-semibold text-white bg-gradient-to-r from-purple-500 to-pink-500 hover:from-purple-600 hover:to-pink-600 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow-md hover:shadow-lg transition"
            title="좌표 동기화 + 누락 분석 + 자동 보완 (8-12분)"
          >
            🧠 스마트 동기화
          </button>
        )}

        {p.handleComplementCrawl && (
          <button
            onClick={() => p.handleComplementCrawl?.()}
            disabled={p.isRunning() || p.isSyncing()}
            class="px-6 py-3 rounded-xl font-semibold text-white bg-gradient-to-r from-green-500 to-teal-500 hover:from-green-600 hover:to-teal-600 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow-md hover:shadow-lg transition"
            title="certification_date가 누락된 제품만 재크롤링하여 정보를 업데이트합니다. 스마트 동기화보다 빠르고 가볍습니다."
          >
            🔧 제품 보완 동기화
          </button>
        )}

        <div class="h-10 w-px bg-gray-300 dark:bg-gray-600" />

        <input
          type="text"
          class={`w-72 px-3 py-2 rounded-md text-sm bg-white/70 border border-white/40 focus:outline-none focus:ring-2 focus:ring-indigo-300 ${p.syncPulse() && p.effectsOn() ? 'flash-db' : ''}`}
          placeholder="Sync 범위 (예: 498-492,489,487-485)"
          value={p.syncRanges()}
          onInput={(e) => p.setSyncRanges(e.currentTarget.value)}
          title="페이지 범위 입력: 498-492 (범위), 489 (단일), 498-492,489 (복합)"
        />

        <button
          onClick={async () => {
            if (p.isSyncing()) return;
            let ranges = (p.syncRanges() || '').trim();
            if (!ranges) {
              const auto = p.deriveRangesFromDiagnostics();
              if (auto) {
                p.setSyncRanges(auto);
                p.addLog(`🔁 Diagnostics 기반 범위 자동설정: ${auto}`);
                ranges = auto;
              } else {
                p.addLog('⚠️ 먼저 Sync 범위를 입력하거나, 진단을 실행해 주세요. 예: 498-492,489');
                return;
              }
            }
            const norm = ranges
              .replace(/\s+/g, '')
              .replace(/[–—−﹣－]/g, '-')
              .replace(/[〜～]/g, '~');
            const tokens = norm.split(',').map(t => t.trim()).filter(Boolean);
            const pages: number[] = [];
            for (const tk of tokens) {
              if (tk.includes('-') || tk.includes('~')) {
                const sep = tk.includes('~') ? '~' : '-';
                const [a, b] = tk.split(sep);
                let s = parseInt(a, 10), e = parseInt(b, 10);
                if (!Number.isFinite(s) || !Number.isFinite(e)) continue;
                if (e > s) { const tmp = s; s = e; e = tmp; }
                for (let pnum = s; pnum >= e; pnum--) pages.push(pnum);
              } else {
                const v = parseInt(tk, 10); if (Number.isFinite(v)) pages.push(v);
              }
            }
            const seen = new Set<number>();
            const uniquePages = pages.filter(pn => seen.has(pn) ? false : (seen.add(pn), true));
            if (uniquePages.length === 0) { p.addLog('⚠️ 유효한 페이지가 없습니다. 예: 498-492,489'); return; }
            p.setIsSyncing(true);
            p.addLog(`🧑‍💻 수동 크롤링(Actor) 실행: [${uniquePages.join(', ')}]`);
            try {
              const res = await p.tauriApi.startManualCrawlPagesActor(uniquePages, true);
              p.addLog(`✅ 수동 크롤링 세션 시작: ${JSON.stringify(res)}`);
              p.setCrawlingRange((prev: any) => {
                const pagesCt = uniquePages.length;
                const estimated_new_products = pagesCt * 12;
                return {
                  ...(prev || {}),
                  crawling_info: { ...((prev as any)?.crawling_info || {}), pages_to_crawl: pagesCt, estimated_new_products },
                  range: [Math.max(...uniquePages), Math.min(...uniquePages)],
                } as any;
              });
              if (res?.session_id) p.addLog(`🆔 세션 ID: ${res.session_id}`);
            } catch (e) {
              p.addLog(`❌ 수동 크롤링(Actor) 실패: ${e}`);
            } finally {
              p.setIsSyncing(false);
            }
          }}
          disabled={p.isSyncing()}
          class={`px-5 py-2.5 rounded-xl font-semibold text-white ripple shadow-md hover:shadow-lg transition ${p.isSyncing() ? 'bg-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-purple-600 to-indigo-600 hover:from-purple-700 hover:to-indigo-700'}`}
          title="특정 페이지 범위를 직접 지정하여 크롤링 (진단 결과 기반 자동 범위 지원)"
        >
          수동 크롤링
        </button>

        <label class="flex items-center gap-2 text-sm text-gray-700 select-none">
          <input type="checkbox" checked={p.effectsOn()} onInput={(e) => p.setEffectsOn(e.currentTarget.checked)} />
          애니메이션 효과
        </label>
      </div>
    </div>
  );
};

export default ControlPanel;
