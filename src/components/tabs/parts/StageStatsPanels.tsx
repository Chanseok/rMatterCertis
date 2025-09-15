import { Component, Show } from 'solid-js';
import CountUp from '../../common/CountUp';

export interface PageStats { started: number; completed: number; failed: number; retried: number; totalEstimated: number; inflight: number; }
export interface DetailStats { started: number; completed: number; failed: number; retried: number; inflight: number; }

interface Props {
  crawlingRange: () => any;
  preflight: () => { site_total_pages?: number } | null;
  pageStats: () => PageStats;
  detailStats: () => DetailStats;
  stage1Pulse: () => boolean;
  stage2Pulse: () => boolean;
  downshiftInfo: () => { newLimit?: number; reason?: string } | null;
  effectsOn: () => boolean;
}

const StageStatsPanels: Component<Props> = (p) => {
  const plannedPages = () => {
    const cr = p.crawlingRange();
    return (cr?.crawling_info?.pages_to_crawl ?? ((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1 || 0)) as number;
  };
  return (
    <div
      class={`grid grid-cols-1 md:grid-cols-2 gap-4 mb-8 ${p.stage1Pulse() ? 'pulse-once' : ''}`}
    >
      {/* Stage 1 */}
      <div class={`bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 ${p.stage1Pulse() ? 'pulse-once' : ''}`}>        
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-md font-semibold text-gray-800">Stage 1: 제품 목록 수집</h3>
          <span class="text-xs text-gray-500">
            {(() => {
              const pre = p.preflight();
              const siteTotal = Number(pre?.site_total_pages ?? 0) || 0;
              const planned = plannedPages();
              const batchEst = p.pageStats().totalEstimated || 0;
              const est = planned > 0 ? planned : (batchEst > 0 ? batchEst : siteTotal);
              return est > 0 ? `예상 ${est}p` : '';
            })()}
          </span>
        </div>
        <div class="grid grid-cols-5 gap-2 text-center">
          {statBox('시작', p.pageStats().started, 'bg-blue-50', 'text-blue-600', p.effectsOn())}
          {statBox('완료', p.pageStats().completed, 'bg-emerald-50', 'text-emerald-600', p.effectsOn())}
          {statBox('진행중', p.pageStats().inflight, 'bg-amber-50', 'text-amber-600', p.effectsOn())}
          {statBox('실패', p.pageStats().failed, 'bg-rose-50', 'text-rose-600', p.effectsOn())}
          {statBox('재시도', p.pageStats().retried, 'bg-violet-50', 'text-violet-600', p.effectsOn())}
        </div>
        <ProgressBar value={() => {
          const pre = p.preflight();
          const siteTotal = Number(pre?.site_total_pages ?? 0) || 0;
          const planned = plannedPages();
          const batchEst = p.pageStats().totalEstimated || 0;
          const denom = planned > 0 ? planned : (batchEst > 0 ? batchEst : siteTotal);
          return denom > 0 ? Math.min(100, (p.pageStats().completed / denom) * 100) : 0;
        }} />
      </div>
      {/* Stage 2 */}
      <div class={`bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 ${p.stage2Pulse() ? 'pulse-once' : ''}`}>        
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-md font-semibold text-gray-800">Stage 2: 세부 정보 수집</h3>
          <Show when={!!p.downshiftInfo()}>
            <span class="text-[10px] px-2 py-1 bg-yellow-100 text-yellow-700 rounded shake-x" title={p.downshiftInfo()?.reason || ''}>
              ↓ 제한 {p.downshiftInfo()?.newLimit ?? '-'}
            </span>
          </Show>
          <span class="text-xs text-gray-500">
            {(() => {
              const planned = plannedPages();
              const plannedProducts = planned > 0 ? planned * 12 : 0;
              const est = (p.crawlingRange()?.crawling_info?.estimated_new_products ?? 0) as number;
              const observed = Math.max(p.detailStats().started || 0, p.detailStats().completed || 0);
              const val = plannedProducts > 0 ? plannedProducts : (observed > 0 ? observed : (est > 0 ? est : 0));
              return val > 0 ? `예상 ${val}` : '';
            })()}
          </span>
        </div>
        <div class="grid grid-cols-5 gap-2 text-center">
          {statBox('시작', p.detailStats().started, 'bg-blue-50', 'text-blue-600', p.effectsOn())}
            {statBox('완료', p.detailStats().completed, 'bg-emerald-50', 'text-emerald-600', p.effectsOn())}
            {statBox('진행중', p.detailStats().inflight, 'bg-amber-50', 'text-amber-600', p.effectsOn())}
            {statBox('실패', p.detailStats().failed, 'bg-rose-50', 'text-rose-600', p.effectsOn())}
            {statBox('재시도', p.detailStats().retried, 'bg-violet-50', 'text-violet-600', p.effectsOn())}
        </div>
        <ProgressBar value={() => {
          const est = (p.crawlingRange()?.crawling_info?.estimated_new_products ?? 0) as number;
          const observed = Math.max(p.detailStats().started || 0, p.detailStats().completed || 0);
          const denom = observed > 0 ? observed : (est > 0 ? est : 0);
          return denom > 0 ? Math.min(100, (p.detailStats().completed / denom) * 100) : 0;
        }} />
      </div>
    </div>
  );
};

function statBox(label: string, value: number, bg: string, color: string, animated: boolean) {
  return (
    <div class={`${bg} rounded p-2`}>
      <div class={`text-xl font-bold ${color}`}>{animated ? <CountUp value={value} /> : value}</div>
      <div class="text-xs text-gray-600">{label}</div>
    </div>
  );
}

const ProgressBar: Component<{ value: () => number }> = (p) => (
  <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
    <div class="progress-fill rounded-full" style={{ width: `${p.value()}%` }}></div>
  </div>
);

export default StageStatsPanels;
