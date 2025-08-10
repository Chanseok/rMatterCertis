import { Component, createEffect, createMemo, createSignal, Show } from 'solid-js';

// Lightweight visual showing active detail concurrency vs limit, with animated downshift pulse.
// Props accept status.details (downshift metadata) and throughput metrics.
interface DetailStatusData {
  total: number;
  completed: number;
  failed: number;
  downshifted?: boolean;
  downshift_meta?: { old_limit?: number | null; new_limit?: number | null; trigger?: string | null; timestamp?: string | null } | null;
}
interface MetricsData { throughput_pages_per_min?: number; elapsed_ms?: number }
interface Props {
  details: DetailStatusData | null;
  metrics?: MetricsData | null;
}

// Utility easing for smooth bar transitions
function lerp(a:number,b:number,t:number){return a+(b-a)*t;}

export const DetailConcurrencyAnimation: Component<Props> = (props) => {
  const [animCompleted, setAnimCompleted] = createSignal(0);
  const [lastTick, setLastTick] = createSignal<number>(Date.now());
  const [pulse, setPulse] = createSignal(false);
  const [limitTransition, setLimitTransition] = createSignal<number | null>(null); // animated limit value

  // Derive progress ratio
  const progressRatio = createMemo(() => {
    const d = props.details; if(!d || !d.total) return 0;
    return Math.min(1, d.completed / d.total);
  });

  // Animate displayed completed count toward real completed
  createEffect(() => {
    const d = props.details; if(!d) return;
    let frame: number;
    const step = () => {
      const target = d.completed;
      const current = animCompleted();
      if (current !== target) {
        const delta = target - current;
        const next = Math.abs(delta) < 0.5 ? target : current + delta * 0.15;
        setAnimCompleted(next);
      }
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  });

  // Detect downshift & trigger pulse + limit animation
  createEffect(() => {
    const meta = props.details?.downshift_meta;
    if (props.details?.downshifted && meta?.new_limit) {
      setPulse(true);
      const start = meta.old_limit ?? meta.new_limit;
      const end = meta.new_limit;
      const startTime = Date.now();
      const duration = 1000;
      let raf:number;
      const animate = () => {
        const t = Math.min(1, (Date.now() - startTime)/duration);
        setLimitTransition(lerp(start, end, t));
        if (t < 1) raf = requestAnimationFrame(animate); else setTimeout(()=>setPulse(false), 800);
      };
      setLimitTransition(start);
      raf = requestAnimationFrame(animate);
      return () => cancelAnimationFrame(raf);
    }
  });

  const displayCompleted = createMemo(()=> Math.round(animCompleted()));

  // Bar style calculations
  const barWidth = () => (progressRatio()*100).toFixed(1)+'%';
  const limitLabel = () => {
    const meta = props.details?.downshift_meta;
    if (!props.details?.downshifted || !meta) return null;
    const val = Math.round(limitTransition() ?? meta.new_limit ?? 0);
    return `Limit ${val}`;
  };

  return (
    <div class="relative px-3 py-3 rounded-md bg-neutral-800/70 border border-neutral-700 overflow-hidden">
      <div class="flex items-center justify-between mb-1">
        <div class="text-xs uppercase tracking-wide text-neutral-400">Detail Progress</div>
        <div class="text-xs font-mono text-neutral-300">
          {displayCompleted()}/{props.details?.total ?? 0}{props.details && props.details.total>0 && <span class="text-neutral-500"> ({(progressRatio()*100).toFixed(1)}%)</span>}
        </div>
      </div>
      <div class="h-3 w-full rounded bg-neutral-700/50 relative">
        <div class="h-full rounded bg-gradient-to-r from-indigo-500 via-sky-500 to-emerald-400 transition-[width] duration-300 ease-out" style={{width: barWidth()}}></div>
        <Show when={props.details?.downshifted && limitLabel()}>
          <div class="absolute inset-y-0" style={{left: (progressRatio()*100).toFixed(1)+'%'}}>
            <div class="-translate-x-1/2 -translate-y-full text-[10px] px-1.5 py-0.5 rounded bg-red-700/70 text-white shadow">
              {limitLabel()}
            </div>
          </div>
        </Show>
      </div>
      <Show when={!props.details || (props.details && props.details.total===0)}>
        <div class="mt-2 text-[10px] text-neutral-500 italic">상세 수집 단계 대기 중… (Detail tasks not started yet)</div>
      </Show>
      <div class="mt-2 flex items-center gap-3 text-[10px] text-neutral-500">
        <Show when={props.details?.downshifted}>
          <span classList={{
            'px-2 py-0.5 rounded border text-red-300': true,
            'border-red-600 bg-red-900/40 animate-pulse': pulse(),
            'border-red-700 bg-red-900/30': !pulse()
          }}>DOWN {props.details?.downshift_meta?.trigger && <span class="text-red-400/60 ml-1">({props.details?.downshift_meta?.trigger})</span>}</span>
        </Show>
        {props.metrics?.throughput_pages_per_min !== undefined && (
          <span>Throughput: {props.metrics?.throughput_pages_per_min?.toFixed(1)} p/m</span>
        )}
      </div>
    </div>
  );
};
