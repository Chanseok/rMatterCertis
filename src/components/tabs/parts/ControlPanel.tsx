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
  handleAnalyzeMissing?: () => Promise<void> | void;
}

const ControlPanel: Component<ControlPanelProps> = (p) => {
  return (
    <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 mb-8 flex flex-wrap gap-4 items-end">
      {/* Sync Controls */}
      <div class="flex items-center gap-3">
        <button
          onClick={() => { p.startUnifiedAdvanced(); }}
          disabled={p.isRunning()}
          class={`px-6 py-3 rounded-xl font-semibold text-white ripple shadow-md hover:shadow-lg transition ${p.isRunning() ? 'bg-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-purple-600 to-indigo-600 hover:from-purple-700 hover:to-indigo-700'}`}
        >
          {p.isRunning() ? '통합 파이프라인 실행 중...' : '🎭 크롤링'}
        </button>

        <button
          onClick={p.calculateCrawlingRange}
          disabled={p.isRunning()}
          class="px-6 py-3 rounded-xl font-semibold text-blue-700 bg-white border border-blue-200 hover:bg-blue-50 disabled:opacity-50 ripple shadow"
        >
          📊 범위 다시 계산
        </button>

        {/* 🏃 Shallow Sync buttons */}
        {p.handleShallowSync && (
          <button
            onClick={() => p.handleShallowSync?.()}
            disabled={p.isRunning()}
            class="px-6 py-3 rounded-xl font-semibold text-white bg-gradient-to-r from-blue-500 to-cyan-500 hover:from-blue-600 hover:to-cyan-600 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow-md hover:shadow-lg transition"
            title="전체 페이지 좌표만 빠르게 동기화 (5-8분)"
          >
            🏃 빠른 동기화
          </button>
        )}

        {p.handleSmartSync && (
          <button
            onClick={() => p.handleSmartSync?.()}
            disabled={p.isRunning()}
            class="px-6 py-3 rounded-xl font-semibold text-white bg-gradient-to-r from-purple-500 to-pink-500 hover:from-purple-600 hover:to-pink-600 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow-md hover:shadow-lg transition"
            title="얕은 크롤링 + 진단 + 누락 보완 (8-12분)"
          >
            🧠 스마트 동기화
          </button>
        )}

        {p.handleAnalyzeMissing && (
          <button
            onClick={() => p.handleAnalyzeMissing?.()}
            disabled={p.isRunning()}
            class="px-5 py-2 rounded-lg font-medium text-gray-700 bg-white border-2 border-gray-300 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed ripple shadow"
            title="누락된 제품 분석"
          >
            📊 누락 분석
          </button>
        )}

        <div class="h-10 w-px bg-gray-300 dark:bg-gray-600" />

        <input
          type="text"
          class={`w-72 px-3 py-2 rounded-md text-sm bg-white/70 border border-white/40 focus:outline-none focus:ring-2 focus:ring-indigo-300 ${p.syncPulse() && p.effectsOn() ? 'flash-db' : ''}`}
          placeholder="Sync 범위 (예: 498-492,489,487-485)"
          value={p.syncRanges()}
          onInput={(e) => p.setSyncRanges(e.currentTarget.value)}
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
          title="기본 엔진으로 명시적 페이지 배열을 실행"
        >
          수동 크롤링
        </button>
      </div>

      <label class="flex items-center gap-2 text-sm text-gray-700 select-none">
        <input type="checkbox" checked={p.effectsOn()} onInput={(e) => p.setEffectsOn(e.currentTarget.checked)} />
        애니메이션 효과
      </label>
    </div>
  );
};

export default ControlPanel;
