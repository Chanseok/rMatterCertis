import { Component, Show } from "solid-js";
import CountUp from "../../common/CountUp";

interface ValidationStats {
  targetPages: number;
  pagesScanned: number;
  divergences: number;
  anomalies: number;
  lastPage: number | null;
  lastAssignedStart: number | null;
  lastAssignedEnd: number | null;
  started: boolean;
  completed: boolean;
}

interface Props {
  stats: () => ValidationStats;
  effectsOn: () => boolean;
}

const ValidationPanel: Component<Props> = (p) => (
  <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
    <div class="flex items-center justify-between mb-2">
  <h3 class="text-md font-semibold text-gray-800">Stage 3: 검증</h3>
      <span class="text-xs text-gray-500">
        {p.stats().started ? (p.stats().completed ? '완료' : '진행 중') : '대기'}
      </span>
    </div>
    <div class="grid grid-cols-4 gap-2 text-center">
      <div class="bg-indigo-50 rounded p-2">
        <div class="text-xl font-bold text-indigo-600">
          {p.effectsOn() ? <CountUp value={p.stats().targetPages} /> : p.stats().targetPages}
        </div>
  <div class="text-xs text-gray-600">검증 예정</div>
      </div>
      <div class="bg-emerald-50 rounded p-2">
        <div class="text-xl font-bold text-emerald-600">
          {p.effectsOn() ? <CountUp value={p.stats().pagesScanned} /> : p.stats().pagesScanned}
        </div>
  <div class="text-xs text-gray-600">검증 완료</div>
      </div>
      <div class="bg-amber-50 rounded p-2">
        <div class="text-xl font-bold text-amber-600">
          {p.effectsOn() ? <CountUp value={p.stats().divergences} /> : p.stats().divergences}
        </div>
        <div class="text-xs text-gray-600">불일치</div>
      </div>
      <div class="bg-rose-50 rounded p-2">
        <div class="text-xl font-bold text-rose-600">
          {p.effectsOn() ? <CountUp value={p.stats().anomalies} /> : p.stats().anomalies}
        </div>
        <div class="text-xs text-gray-600">이상</div>
      </div>
    </div>
    <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
      <div
        class="h-2 rounded-full bg-indigo-500 transition-all"
        style={{
          width: `${(() => {
            const t = p.stats().targetPages || 0;
            const s = p.stats().pagesScanned || 0;
            return t > 0 ? Math.min(100, (s / t) * 100) : 0;
          })()}%`
        }}
      />
    </div>
    <Show when={p.stats().lastPage != null}>
      <div class="mt-2 text-[11px] text-gray-500">
        최근 스캔: 페이지 {p.stats().lastPage} (오프셋 {p.stats().lastAssignedStart ?? '-'}–{p.stats().lastAssignedEnd ?? '-'})
      </div>
    </Show>
  </div>
);

export default ValidationPanel;
