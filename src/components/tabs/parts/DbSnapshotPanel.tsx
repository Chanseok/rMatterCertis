import { Component } from "solid-js";
import CountUp from "../../common/CountUp";

interface DbSnapshot {
  total?: number;
  minPage?: number | null;
  maxPage?: number | null;
  inserted?: number;
  updated?: number;
}

interface Props {
  snapshot: () => DbSnapshot;
  flash: () => boolean;
  effectsOn: () => boolean;
}

const DbSnapshotPanel: Component<Props> = (p) => (
  <div class={`bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 ${p.flash() && p.effectsOn() ? 'flash-db' : ''}`}>
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-md font-semibold text-gray-800">Stage 4: DB 저장 스냅샷</h3>
      <span class="text-xs text-gray-500">최근 보고 기준</span>
    </div>
    <div class="grid grid-cols-2 md:grid-cols-5 gap-2 text-center">
      <div class="bg-sky-50 rounded p-2">
        <div class="text-xl font-bold text-sky-600">
          {p.effectsOn() && typeof p.snapshot().total === 'number' ? <CountUp value={p.snapshot().total as number} /> : (p.snapshot().total ?? '-')}
        </div>
        <div class="text-xs text-gray-600">총 상세 수</div>
      </div>
      <div class="bg-purple-50 rounded p-2">
        <div class="text-xl font-bold text-purple-600">
          {p.effectsOn() && typeof p.snapshot().minPage === 'number' ? <CountUp value={p.snapshot().minPage as number} /> : (p.snapshot().minPage ?? '-')}
        </div>
        <div class="text-xs text-gray-600">DB 최소 페이지</div>
      </div>
      <div class="bg-purple-50 rounded p-2">
        <div class="text-xl font-bold text-purple-600">
          {p.effectsOn() && typeof p.snapshot().maxPage === 'number' ? <CountUp value={p.snapshot().maxPage as number} /> : (p.snapshot().maxPage ?? '-')}
        </div>
        <div class="text-xs text-gray-600">DB 최대 페이지</div>
      </div>
      <div class="bg-emerald-50 rounded p-2 col-span-2 md:col-span-2">
        <div class="text-xl font-bold text-emerald-600">
          {p.effectsOn() ? <CountUp value={p.snapshot().inserted ?? 0} /> : (p.snapshot().inserted ?? 0)}
          /
          {p.effectsOn() ? <CountUp value={p.snapshot().updated ?? 0} /> : (p.snapshot().updated ?? 0)}
        </div>
        <div class="text-xs text-gray-600">삽입/업데이트(세션)</div>
      </div>
    </div>
  </div>
);

export default DbSnapshotPanel;
