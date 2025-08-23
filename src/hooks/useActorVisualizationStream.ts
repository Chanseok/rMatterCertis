import { createSignal, onCleanup, onMount } from 'solid-js';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type VisualizationEvent = {
  seq: number;
  ts: string;
  type: string; // session|phase|batch|stage|page|detail|progress|metrics|concurrency|report|shutdown
  rawName: string;
  sessionId?: string;
  batchId?: string;
  page?: number;
  detailId?: string;
  phase?: string;
  stageType?: string;
  progressPct?: number;
  currentStep?: number;
  totalSteps?: number;
  reportKind?: string;
  shutdownKind?: string;
  // concurrency snapshot fields (optional)
  activeBatches?: number;
  activePages?: number;
  activeDetails?: number;
};

// Map actor event_name to normalized type & field extraction
function normalize(raw: any): VisualizationEvent | null {
  if (!raw || typeof raw !== 'object') return null;
  const { event_name, variant } = raw;
  const base = {
    seq: raw.seq ?? 0,
    ts: raw.backend_ts || new Date().toISOString(),
    rawName: event_name || variant || 'unknown',
    sessionId: raw.session_id,
    batchId: raw.batch_id,
  } as Partial<VisualizationEvent>;

  switch (event_name) {
    case 'actor-session-started': return { ...base, type: 'session', } as VisualizationEvent;
    case 'actor-session-completed': return { ...base, type: 'session', } as VisualizationEvent;
    case 'actor-phase-started': return { ...base, type: 'phase', phase: raw.phase?.phase_type || raw.phase?.type } as VisualizationEvent;
    case 'actor-phase-completed': return { ...base, type: 'phase', phase: raw.phase?.phase_type || raw.phase?.type } as VisualizationEvent;
    case 'actor-batch-started': return { ...base, type: 'batch' } as VisualizationEvent;
    case 'actor-batch-completed': return { ...base, type: 'batch' } as VisualizationEvent;
    case 'actor-stage-started': return { ...base, type: 'stage', stageType: raw.stage_type } as VisualizationEvent;
    case 'actor-stage-completed': return { ...base, type: 'stage', stageType: raw.stage_type } as VisualizationEvent;
    case 'actor-page-task-started': return { ...base, type: 'page', page: raw.page } as VisualizationEvent;
    case 'actor-page-task-completed': return { ...base, type: 'page', page: raw.page } as VisualizationEvent;
    case 'actor-page-task-failed': return { ...base, type: 'page', page: raw.page } as VisualizationEvent;
    case 'actor-progress': return { ...base, type: 'progress', progressPct: raw.percentage, currentStep: raw.current_step, totalSteps: raw.total_steps } as VisualizationEvent;
  case 'actor-performance-metrics': return { ...base, type: 'metrics' } as VisualizationEvent;
  case 'actor-detail-concurrency-downshifted': return { ...base, type: 'concurrency' } as VisualizationEvent;
  case 'actor-batch-report': return { ...base, type: 'report', reportKind: 'batch' } as VisualizationEvent;
  case 'actor-session-report': return { ...base, type: 'report', reportKind: 'session' } as VisualizationEvent;
  case 'actor-shutdown-requested': return { ...base, type: 'shutdown', shutdownKind: 'requested' } as VisualizationEvent;
  case 'actor-shutdown-completed': return { ...base, type: 'shutdown', shutdownKind: 'completed' } as VisualizationEvent;
    default:
      return null;
  }
}

// Normalize ConcurrencyEvent emitted on channel 'concurrency-event'
function normalizeConcurrency(raw: any): VisualizationEvent[] {
  // Expect shape: { type: 'SessionEvent' | 'BatchEvent' | 'StageEvent' | 'ConcurrencySnapshot' | 'ConcurrencyInsight', payload: {...} }
  if (!raw || typeof raw !== 'object') return [];
  const t = raw.type;
  const p = (raw as any).payload || {};
  const ts = p.timestamp || new Date().toISOString();
  switch (t) {
    case 'SessionEvent': {
      const ev: VisualizationEvent = {
        seq: 0,
        ts,
        type: 'concurrency',
        rawName: `concurrency-session-${(p.event_type || 'unknown').toString().toLowerCase()}`,
        sessionId: p.session_id,
      };
      return [ev];
    }
    case 'BatchEvent': {
      const ev: VisualizationEvent = {
        seq: 0,
        ts,
        type: 'concurrency',
        rawName: `concurrency-batch-${(p.event_type || 'unknown').toString().toLowerCase()}`,
        sessionId: p.session_id,
        batchId: p.batch_id,
      };
      return [ev];
    }
    case 'StageEvent': {
      const ev: VisualizationEvent = {
        seq: 0,
        ts,
        type: 'concurrency',
        rawName: `concurrency-stage-${(p.event_type || 'unknown').toString().toLowerCase()}`,
        sessionId: p.session_id,
        batchId: p.batch_id,
        stageType: p.stage_type,
      };
      return [ev];
    }
    case 'ConcurrencySnapshot': {
      const snapshot = p; // has concurrency metrics
      const ev: VisualizationEvent = {
        seq: 0,
        ts,
        type: 'concurrency',
        rawName: 'concurrency-snapshot',
        sessionId: snapshot.session_id,
        activeBatches: snapshot.active_batches,
        activePages: snapshot.active_page_tasks,
        activeDetails: snapshot.active_detail_tasks,
      };
      return [ev];
    }
    case 'ConcurrencyInsight': {
      const ev: VisualizationEvent = {
        seq: 0,
        ts,
        type: 'concurrency',
        rawName: 'concurrency-insight',
        sessionId: p.session_id,
      };
      return [ev];
    }
    default:
      return [];
  }
}

export function useActorVisualizationStream(limit: number = 500) {
  const [events, setEvents] = createSignal<VisualizationEvent[]>([]);
  let unlisteners: UnlistenFn[] = [];
  let lastSeq = 0;

  const eventNames = [
    'actor-session-started','actor-session-completed','actor-session-failed','actor-session-paused','actor-session-resumed','actor-session-timeout',
    'actor-phase-started','actor-phase-completed','actor-phase-aborted',
    'actor-batch-started','actor-batch-completed','actor-batch-failed',
    'actor-stage-started','actor-stage-completed','actor-stage-failed',
    'actor-page-task-started','actor-page-task-completed','actor-page-task-failed',
  'actor-product-lifecycle','actor-product-lifecycle-group',
  'actor-progress','actor-performance-metrics','actor-detail-concurrency-downshifted',
    // newly added report / shutdown events
    'actor-batch-report','actor-session-report','actor-shutdown-requested','actor-shutdown-completed'
  ];

  async function setup() {
    for (const name of eventNames) {
      const un = await listen<any>(name, (e) => {
        const ev = normalize(e.payload);
        if (!ev) return;
        // seq gap detection
        if (ev.seq && lastSeq && ev.seq !== lastSeq + 1) {
          // Insert synthetic gap event (optional for visualization layering)
          // Could push a marker event later
        }
        lastSeq = ev.seq || lastSeq;
        setEvents(prev => {
          const next = [...prev, ev];
            if (next.length > limit) next.splice(0, next.length - limit);
            return next;
        });
      });
      unlisteners.push(un);
    }
    // concurrency-event (separate schema)
    const unConcurrency = await listen<any>('concurrency-event', (e) => {
      const list = normalizeConcurrency(e.payload);
      if (!list.length) return;
      setEvents(prev => {
        const next = [...prev, ...list];
        if (next.length > limit) next.splice(0, next.length - limit);
        return next;
      });
    });
    unlisteners.push(unConcurrency);
  }

  onMount(setup);
  onCleanup(() => { unlisteners.forEach(u => u()); unlisteners = []; });

  return { events };
}
