import { createSignal, onCleanup, Accessor } from 'solid-js';
import { UnlistenFn } from '@tauri-apps/api/event';
import { tauriApi } from '../services/tauri-api';

// Simple throttle to batch rapid progress updates
function createThrottler(delayMs: number, fn: (...args: any[]) => void) {
  let last = 0; let timeout: any = null; let queued: any[] | null = null;
  return (...args: any[]) => {
    const now = Date.now();
    if (now - last >= delayMs) { last = now; fn(...args); }
    else {
      queued = args;
      if (!timeout) {
        const remaining = delayMs - (now - last);
        timeout = setTimeout(() => { last = Date.now(); if (queued) fn(...queued); queued = null; timeout = null; }, remaining);
      }
    }
  };
}

interface StatusLike {
  session_id: string;
  pages?: { processed: number; total: number; percent?: number; failed?: number; failed_rate?: number };
  details?: { total: number; completed: number; failed: number; downshifted?: boolean; downshift_meta?: any };
  [k: string]: any;
}

interface UseSessionEventStreamOptions { throttleMs?: number; liveWindowMs?: number; debug?: boolean; }

/**
 * Subscribe to backend crawling events and merge progress for current session.
 * Keeps polling as fallback; provides low-latency updates in between polls.
 */
export function useSessionEventStream(
  sessionId: Accessor<string | null>,
  mergeStatus: (updater: (prev: StatusLike | null) => StatusLike | null) => void,
  opts: UseSessionEventStreamOptions = {}
) {
  // Lower default throttle for snappier UI; allow override via env
  const envThrottle = Number((import.meta as any).env?.VITE_EVENT_THROTTLE_MS ?? '0');
  const throttleMs = opts.throttleMs ?? (envThrottle > 0 ? envThrottle : 80);
  const liveWindowMs = opts.liveWindowMs ?? 5000;
  const debug = opts.debug ?? false;
  const [lastEventTs, setLastEventTs] = createSignal<number | null>(null);
  const [eventCounts, setEventCounts] = createSignal<Record<string, number>>({});

  const applyProgress = createThrottler(throttleMs, (payload: any) => {
    const sid = sessionId(); if (!sid) return;
    let progress: any = null;
    if (payload && payload.type === 'ProgressUpdate') progress = payload.data;
    else if (payload && payload.session_id) progress = payload;
    if (!progress || progress.session_id !== sid) return;
    setLastEventTs(Date.now());
    mergeStatus(prev => {
      if (!prev) return prev;
      const pages = { ...(prev.pages || {}) };
      // Preferred structured page progress
      if (progress.overall_progress?.page_progress) {
        const pg = progress.overall_progress.page_progress;
        if (typeof pg.processed === 'number') pages.processed = pg.processed;
        if (typeof pg.total === 'number') pages.total = pg.total;
        if (typeof pg.failed === 'number') pages.failed = pg.failed;
        if (typeof pg.failed_rate === 'number') pages.failed_rate = pg.failed_rate;
        if (typeof pg.percent === 'number') pages.percent = pg.percent;
      } else {
        if (typeof progress.pages_processed === 'number') pages.processed = progress.pages_processed;
        if (typeof progress.pages_total === 'number') pages.total = progress.pages_total;
      }
      // Actor Progress fallback: map percentage/current_step/total_steps to pages
      if (typeof progress.percentage === 'number') pages.percent = progress.percentage;
      if (typeof progress.current_step === 'number') pages.processed = progress.current_step;
      if (typeof progress.total_steps === 'number') pages.total = progress.total_steps;
      return { ...prev, pages } as StatusLike;
    });
  });

  const incrementEvent = (name: string) => setEventCounts(c => ({ ...c, [name]: (c[name] ?? 0) + 1 }));

  const listeners: UnlistenFn[] = [];

  const markLive = (ev: string, payload: any) => {
    const now = Date.now();
    if (debug) {
      let backendTs: number | null = null;
      if (payload) {
        const tsStr = (payload.backend_ts || payload.timestamp);
        if (tsStr) {
          const parsed = Date.parse(tsStr);
            if (!isNaN(parsed)) backendTs = parsed;
        }
      }
      const latency = backendTs ? (now - backendTs) : null;
      // eslint-disable-next-line no-console
      console.debug('[event-stream]', ev, { latency_ms: latency, payload });
    }
    setLastEventTs(now);
  };

  // --- Minimal summary logger (always on, low-noise) ---
  let lastProgressBucket = -1;
  const summaryLog = (name: string, payload: any) => {
    try {
      // Allow user to silence via env flag
      if ((import.meta as any).env?.VITE_EVENT_SUMMARY_SILENT === 'true') return;
      const sid = payload?.session_id;
      // Basic seq gap detection (logs once per gap)
      if (typeof payload?.seq === 'number') {
        const last = (window as any).__lastSeq || 0;
        if (last && payload.seq !== last + 1) {
          if (!(window as any).__seqWarnedOnce) {
            console.warn('[crawl][seq-gap]', 'expected', last + 1, 'got', payload.seq);
            (window as any).__seqWarnedOnce = true;
          }
        }
        (window as any).__lastSeq = payload.seq;
      }
      switch (name) {
        case 'actor-session-started':
          console.info('[crawl]', 'session started', sid); break;
        case 'actor-session-completed':
          console.info('[crawl]', 'session completed', sid); break;
        case 'actor-session-failed':
          console.info('[crawl]', 'session failed', sid, 'error=', payload?.error); break;
        case 'actor-batch-started':
          console.info('[crawl]', 'batch start', payload?.batch_id, 'pages=', payload?.pages_count); break;
        case 'actor-batch-completed':
          console.info('[crawl]', 'batch done', payload?.batch_id, 'ok=', payload?.success_count, 'fail=', payload?.failed_count); break;
        case 'actor-detail-concurrency-downshifted':
          console.info('[crawl]', 'detail downshift', 'old', payload?.old_limit, '→', payload?.new_limit, 'trigger=', payload?.trigger); break;
        case 'actor-progress': {
          const pct = typeof payload?.percentage === 'number' ? payload.percentage : null;
          if (pct != null) {
            const bucket = Math.floor(pct / 10);
            if (bucket !== lastProgressBucket) {
              lastProgressBucket = bucket;
              console.info('[crawl]', 'progress', `${pct.toFixed(1)}%`, sid, `${payload?.current_step}/${payload?.total_steps}`);
            }
          }
          break;
        }
      }
    } catch { /* ignore */ }
  };

  // Unified-only subscription via Tauri API helper
  (async () => {
    try {
      // Progress updates directly from unified stream
      const unProgress = await tauriApi.subscribeToUnifiedActorEvents({
        variants: ['Progress'],
        onEvent: (payload) => {
          incrementEvent('actor-progress');
          markLive('actor-progress', payload);
          summaryLog('actor-progress', payload);
          applyProgress(payload);
        },
      });
      listeners.push(unProgress);

      // Detail/page lifecycle aggregated counters
      const unLifecycle = await tauriApi.subscribeToUnifiedActorEvents({
        variants: ['TaskLifecycle', 'ProductLifecycle', 'ProductLifecycleGroup', 'PageLifecycle', 'DetailConcurrencyDownshifted'],
        onEvent: (payload) => {
          const sid = sessionId();
          if (!sid || !payload || payload.session_id !== sid) return;
          const variant = payload?.variant as string | undefined;
          incrementEvent(variant || 'actor-event');
          markLive('actor-event', payload);
          summaryLog('actor-event', payload);
          if (variant === 'TaskLifecycle') {
            mergeStatus(prev => {
              if (!prev) return prev;
              const details = { ...(prev.details || { total: 0, completed: 0, failed: 0 }) } as any;
              const status = String(payload?.status || '').toLowerCase();
              if (payload?.task_kind === 'Product') {
                if (status === 'succeeded' || status === 'completed' || status === 'persisted') {
                  details.total = (details.total ?? 0) + 1;
                  details.completed = (details.completed ?? 0) + 1;
                } else if (status === 'failed') {
                  details.total = (details.total ?? 0) + 1;
                  details.failed = (details.failed ?? 0) + 1;
                }
              }
              return { ...prev, details } as StatusLike;
            });
          } else if (variant === 'ProductLifecycleGroup') {
            mergeStatus(prev => {
              if (!prev) return prev;
              const details = { ...(prev.details || { total: 0, completed: 0, failed: 0 }) } as any;
              if (payload?.phase === 'fetch') {
                const group = Number(payload?.group_size ?? payload?.started ?? 0) || 0;
                const succeeded = Number(payload?.succeeded ?? 0) || 0;
                const failed = Number(payload?.failed ?? 0) || 0;
                details.total = (typeof details.total === 'number' ? details.total : 0) + group;
                details.completed = (typeof details.completed === 'number' ? details.completed : 0) + succeeded;
                details.failed = (typeof details.failed === 'number' ? details.failed : 0) + failed;
              }
              return { ...prev, details } as StatusLike;
            });
          } else if (variant === 'ProductLifecycle') {
            mergeStatus(prev => {
              if (!prev) return prev;
              const details = { ...(prev.details || { total: 0, completed: 0, failed: 0 }) } as any;
              const status = String(payload?.status || '').toLowerCase();
              if (status === 'failed') { details.failed = (details.failed ?? 0) + 1; details.total = (details.total ?? 0) + 1; }
              if (status === 'product_inserted' || status === 'product_updated' || status === 'succeeded' || status === 'completed') {
                details.completed = (details.completed ?? 0) + 1; details.total = (details.total ?? 0) + 1;
              }
              return { ...prev, details } as StatusLike;
            });
          } else if (variant === 'DetailConcurrencyDownshifted') {
            mergeStatus(prev => {
              if (!prev) return prev;
              const details = { ...(prev.details || { total: 0, completed: 0, failed: 0 }) } as any;
              details.downshifted = true;
              details.downshift_meta = {
                timestamp: payload.timestamp,
                old_limit: payload.old_limit,
                new_limit: payload.new_limit,
                trigger: payload.trigger,
              };
              return { ...prev, details } as StatusLike;
            });
          }
        },
      });
      listeners.push(unLifecycle);
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('Failed to register unified actor-event listeners', err);
    }
  })();

  onCleanup(() => listeners.forEach(u => { try { u(); } catch {} }));

  const live = () => { const ts = lastEventTs(); return ts ? (Date.now() - ts) <= liveWindowMs : false; };

  return { lastEventTs, live, eventCounts };
}
