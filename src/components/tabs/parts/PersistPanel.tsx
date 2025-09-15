import { Component, Show } from "solid-js";
import CountUp from "../../common/CountUp";

interface PersistStats {
  attempted: number;
  inserted: number;
  updated: number;
  duplicates: number;
  unchanged: number;
  failedTrue: number;
  successRate?: number;
  mode?: string; // group-only | mixed | unknown
}

interface Props {
  stats: () => PersistStats | undefined;
  lastBatch: () => string | undefined;
  flash: () => boolean;
  effectsOn: () => boolean;
}

const PersistPanel: Component<Props> = (p) => (
  <div class={`bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 ${p.flash() && p.effectsOn() ? 'flash-persist' : ''}`}>
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-md font-semibold text-gray-800">Stage 5: 저장 요약</h3>
      <Show when={p.stats()?.mode}>
        {(m) => (
          <span class="text-[10px] px-2 py-0.5 rounded-full bg-gray-100 text-gray-600 border border-gray-200 tracking-wide uppercase">{m()}</span>
        )}
      </Show>
    </div>

    <Show when={p.stats()} fallback={<div class="text-sm text-gray-500">아직 저장 통계 없음</div>}>
      {(s) => (
        <div class="space-y-3">
          <div class="grid grid-cols-2 md:grid-cols-3 gap-2">
            <div class="bg-indigo-50 rounded p-2 text-center">
              <div class="text-xl font-bold text-indigo-600">
                {p.effectsOn() ? <CountUp value={s().attempted} /> : s().attempted}
              </div>
              <div class="text-xs text-gray-600">시도</div>
            </div>
            <div class="bg-emerald-50 rounded p-2 text-center">
              <div class="text-xl font-bold text-emerald-600">
                {p.effectsOn() ? <CountUp value={s().inserted + s().updated} /> : (s().inserted + s().updated)}
              </div>
              <div class="text-xs text-gray-600">성공 (삽입+갱신)</div>
            </div>
            <div class="bg-rose-50 rounded p-2 text-center">
              <div class="text-xl font-bold text-rose-600">
                {p.effectsOn() ? <CountUp value={s().failedTrue} /> : s().failedTrue}
              </div>
              <div class="text-xs text-gray-600">실패</div>
            </div>
            <div class="bg-yellow-50 rounded p-2 text-center">
              <div class="text-xl font-bold text-yellow-600">
                {p.effectsOn() ? <CountUp value={s().duplicates} /> : s().duplicates}
              </div>
              <div class="text-xs text-gray-600">중복</div>
            </div>
            <div class="bg-slate-50 rounded p-2 text-center">
              <div class="text-xl font-bold text-slate-600">
                {p.effectsOn() ? <CountUp value={s().unchanged} /> : s().unchanged}
              </div>
              <div class="text-xs text-gray-600">무변경</div>
            </div>
            <div class="bg-lime-50 rounded p-2 text-center">
              <div class="text-xl font-bold text-lime-600">
                {p.effectsOn() ? <CountUp value={+(s().successRate ?? 0).toFixed(2)} /> : (s().successRate ?? 0).toFixed(2)}%
              </div>
              <div class="text-xs text-gray-600">성공률</div>
            </div>
          </div>
          <Show when={p.lastBatch()}>
            {(b) => (
              <div class="text-[11px] text-gray-500 font-mono break-all">
                마지막 배치 ID: {b()}
              </div>
            )}
          </Show>
        </div>
      )}
    </Show>
  </div>
);

export default PersistPanel;
