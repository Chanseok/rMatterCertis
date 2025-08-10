import { createSignal, onCleanup, Accessor } from 'solid-js';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

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
  const throttleMs = opts.throttleMs ?? 250;
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
      return { ...prev, pages } as StatusLike;
    });
  });

  const incrementEvent = (name: string) => setEventCounts(c => ({ ...c, [name]: (c[name] ?? 0) + 1 }));

  const listeners: UnlistenFn[] = [];
  // Legacy + new actor-system event names we care about.
  const eventNames = [
    // === Legacy events (keep until fully deprecated) ===
    'crawling-progress',
    'session-event',
    'batch-event',
    'product-list-page-event',
    'product-detail-event',
    'crawling-completed',
    'crawling-error',
    // === Actor Session lifecycle ===
    'actor-session-started',
    'actor-session-paused',
    'actor-session-resumed',
    'actor-session-completed',
    'actor-session-failed',
    'actor-session-timeout',
    // === Phase & Stage ===
    'actor-phase-started',
    'actor-phase-completed',
    'actor-phase-aborted',
    'actor-stage-started',
    'actor-stage-completed',
    'actor-stage-failed',
    // === Batch ===
    'actor-batch-started',
    'actor-batch-completed',
    'actor-batch-failed',
    // === Progress & metrics ===
    'actor-progress',
    'actor-performance-metrics',
    'actor-batch-report',
    'actor-session-report',
    // === Page task granularity ===
    'actor-page-task-started',
    'actor-page-task-completed',
    'actor-page-task-failed',
    // === Detail task granularity ===
    'actor-detail-task-started',
    'actor-detail-task-completed',
    'actor-detail-task-failed',
    'actor-detail-concurrency-downshifted',
  ];

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

  Promise.all(eventNames.map(ev => listen(ev, (evt) => {
    const payload: any = (evt as any).payload;
    incrementEvent(ev);
    markLive(ev, payload);
    summaryLog(ev, payload);
    if (ev === 'crawling-progress') {
      applyProgress(payload);
      return;
    }
    // Actor detail task events → merge into details progress in status
    if (ev.startsWith('actor-detail-task-') || ev === 'actor-detail-concurrency-downshifted') {
      const payload: any = (evt as any).payload;
      const sid = sessionId();
      if (!sid || !payload || payload.session_id !== sid) return;
      mergeStatus(prev => {
        if (!prev) return prev;
        const details = { ...(prev.details || { total: 0, completed: 0, failed: 0 }) } as any;
        // total stays as originally planned; if zero we cannot infer reliably here.
  if (ev === 'actor-detail-task-completed') {
          details.completed = (details.completed || 0) + 1;
        } else if (ev === 'actor-detail-task-failed') {
          details.failed = (details.failed || 0) + 1;
        }
        if (ev === 'actor-detail-concurrency-downshifted') {
          details.downshifted = true;
          details.downshift_meta = {
            timestamp: payload.timestamp,
            old_limit: payload.old_limit,
            new_limit: payload.new_limit,
            trigger: payload.trigger
          };
        }
        return { ...prev, details } as StatusLike;
      });
      return;
    }
  }).then(un => listeners.push(un))))
    .catch(err => console.error('Failed to register event listeners', err));

  onCleanup(() => listeners.forEach(u => { try { u(); } catch {} }));

  const live = () => { const ts = lastEventTs(); return ts ? (Date.now() - ts) <= liveWindowMs : false; };

  return { lastEventTs, live, eventCounts };
}
