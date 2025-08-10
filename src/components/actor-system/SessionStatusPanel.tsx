import { Component, Show } from 'solid-js';
import { useLatestActorSession } from '../../hooks/useLatestActorSession';
// Using path alias to avoid potential relative resolution issues under bundler mode
import { useSessionEventStream } from '@/hooks/useSessionEventStream';
import { DetailConcurrencyAnimation } from './DetailConcurrencyAnimation';
import type { DetailStatus } from '../../types/events';
import { invoke } from '@tauri-apps/api/core';

export const SessionStatusPanel: Component = () => {
  const { sessionId, status, setStatus, loading, error, refresh, sessions, setSessionId, auto, setAuto } = useLatestActorSession(2500);
  const debugEvents = (import.meta as any).env?.VITE_EVENT_DEBUG === 'true';
  const { live, eventCounts } = useSessionEventStream(
    sessionId,
    (updater: any) => setStatus((prev: any) => updater(prev as any) as any),
    { debug: debugEvents }
  );
  const noSessions = () => !loading() && sessions().length === 0;
  return (
    <div class="p-4 rounded border text-sm space-y-3 transition-colors backdrop-blur-sm" classList={{
      'border-neutral-300 bg-white/70 text-neutral-800': true,
      'dark:border-neutral-700 dark:bg-neutral-800/60 dark:text-neutral-200': true,
      'border-red-600 shadow-[0_0_0_1px_rgba(220,38,38,0.4)]': (() => { const s = status(); if(!s) return false; const fr = s.pages?.failed_rate||0; return fr>0.25; })(),
      'border-amber-600': (() => { const s = status(); if(!s) return false; const fr = s.pages?.failed_rate||0; return fr>0.15 && fr<=0.25; })(),
    }}>
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <div class="flex items-center gap-2">
          <div class="font-semibold flex items-center gap-2">Latest Session {live() && <span class="animate-pulse text-xs px-2 py-0.5 rounded bg-emerald-600/20 text-emerald-400 border border-emerald-500/40">Live</span>}</div>
          <select class="bg-neutral-700 text-xs px-2 py-1 rounded outline-none" value={sessionId() ?? ''} onChange={(e) => setSessionId(e.currentTarget.value)}>
            <option value="" disabled>Select...</option>
            {sessions().map(id => <option value={id}>{id.substring(0,20)}{id.length>20?'…':''}</option>)}
          </select>
          <label class="flex items-center gap-1 text-xs cursor-pointer">
            <input type="checkbox" checked={auto()} onChange={e => setAuto(e.currentTarget.checked)} /> Auto
          </label>
        </div>
        <div class="flex items-center gap-2">
          <button class="text-xs px-2 py-1 rounded bg-neutral-700 hover:bg-neutral-600" onClick={refresh}>↻ Manual</button>
          <Show when={status() && (status()!.status === 'Failed' || status()!.status === 'Paused') && status()!.resume_token}>
            <button class="text-xs px-2 py-1 rounded bg-indigo-600 hover:bg-indigo-500" onClick={async () => {
              try {
                const token = status()!.resume_token;
                await invoke('resume_from_token', { resume_token: token });
                refresh();
              } catch (e) { console.error('Resume failed', e); }
            }}>Resume</button>
          </Show>
        </div>
        <div class="text-[10px] text-neutral-500 flex gap-2">
          {Object.entries(eventCounts()).slice(0,4).map(([k,v]) => <span>{(k as string).replace('crawling-','').replace('-event','')}: {v as number}</span>)}
        </div>
      </div>
      <Show when={!loading()} fallback={<div class="text-neutral-500 text-xs">Loading latest sessions…</div>}>
        <Show when={!noSessions()} fallback={
          <div class="text-neutral-600 dark:text-neutral-400 text-xs flex flex-col gap-2">
            <div>No actor sessions yet.</div>
            <button
              class="self-start px-2 py-1 rounded bg-indigo-600 hover:bg-indigo-500 text-white text-[11px]"
              onClick={async () => {
                try {
                  // Fire a minimal actor system crawling request (backend picks optimal range itself)
                  await invoke('start_actor_system_crawling', { request: { start_page: 0, end_page: 0, concurrency: 64, batch_size: 3, delay_ms: 100 } });
                  await refresh();
                } catch (e) { console.error('Start actor session failed', e); }
              }}
            >Start Actor Session</button>
            <div class="text-[10px] text-neutral-500">Press the button above or use controls below ("진짜 Actor 시스템…") to create a session. The animation appears once detail tasks begin.</div>
          </div>
        }>
        <Show when={!error()} fallback={<div class="text-red-400">{error()}</div>}>
          <Show when={status()}>{(sAcc) => {
            const s = sAcc();
            const pages = s.pages;
            const details: DetailStatus | undefined = (s as any).details;
            const pctPages = pages.percent?.toFixed(1) ?? '0.0';
            const detailPct = details && details.total>0 ? ((details.completed / details.total)*100).toFixed(1) : null;
            return (
              <div class="space-y-2">
                <div class="text-xs text-neutral-400">Session ID</div>
    <div class="font-mono text-xs break-all text-neutral-700 dark:text-neutral-200">{sessionId()}</div>
                <div class="grid grid-cols-2 gap-3 pt-2">
                  <div class="space-y-1">
                    <div class="text-xs uppercase tracking-wide text-neutral-400">Pages</div>
                    <div class="text-lg font-semibold">{pages.processed}/{pages.total} <span class="text-xs text-neutral-400">({pctPages}%)</span></div>
                    <div class="text-xs" classList={{'text-red-400': pages.failed_rate>0.25,'text-amber-400': pages.failed_rate>0.15 && pages.failed_rate<=0.25,'text-neutral-400': pages.failed_rate<=0.15}}>Failed {pages.failed} (rate {(pages.failed_rate*100).toFixed(1)}%)</div>
                  </div>
                  <div class="space-y-1">
                    <div class="text-xs uppercase tracking-wide text-neutral-400">Details</div>
                    {details ? (
                      <div class="text-lg font-semibold">{details.completed}/{details.total} {detailPct && <span class="text-xs text-neutral-400">({detailPct}%)</span>}</div>
                    ) : <div class="text-neutral-500 text-xs">(pending phase)</div>}
                    {details && details.downshifted && (
                      <div class="text-xs text-red-400 flex items-center gap-1">
                        <span class="inline-flex items-center px-2 py-0.5 rounded bg-red-900/40 border border-red-700/50">DOWN</span>
                        <span>→ {details.downshift_meta?.new_limit ?? '?'} <span class="text-neutral-400">({details.downshift_meta?.trigger})</span></span>
                      </div>
                    )}
                  </div>
                </div>
                <div class="flex gap-4 text-xs pt-1 text-neutral-300">
                  <div>Status: <span class="font-semibold">{s.status}</span></div>
                  {s.metrics && <div>ETA: {s.metrics.eta_ms>0 ? Math.round(s.metrics.eta_ms/1000)+'s' : '—'}</div>}
                  {s.metrics && <div>Throughput: {s.metrics.throughput_pages_per_min.toFixed(1)} p/m</div>}
                </div>
                <div class="pt-3">
                  <DetailConcurrencyAnimation details={details||null} metrics={s.metrics||null} />
                </div>
              </div>
            );
          }}</Show>
        </Show>
  </Show>
      </Show>
    </div>
  );
};
