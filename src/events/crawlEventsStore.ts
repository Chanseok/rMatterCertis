// Basic reactive-ish store for crawl events (framework agnostic minimal version)
// SolidJS 사용자라면 createSignal 래핑 가능. 여기서는 POJO + subscribe 패턴.

import { CrawlEvent } from '../types/crawlEvents';
import { getCrawlUpdatesListener } from './crawlUpdatesListener';

export interface CrawlSessionState {
  sessionId: string;
  startedAt?: string;
  completedAt?: string;
  failed?: boolean;
  failureMessage?: string;
  totalStages?: number;
  currentStageIndex?: number;
  overallProgressPct?: number;
  lastSequence?: number;
  lastEventType?: string;
  stageStats: Record<string, {
    started: boolean;
    completedItems: number;
    failedItems: number;
    retries: number;
    lastProgressPct?: number;
  }>;
}

export interface CrawlEventsStoreSnapshot {
  activeSession?: CrawlSessionState;
  gapWarnings: Array<{ expected: number; got: number; atSequence: number | null }>; 
  versionMismatches: number;
  invalidPayloads: number;
}

export type StoreSubscriber = (snap: CrawlEventsStoreSnapshot) => void;

class CrawlEventsStore {
  private snapshot: CrawlEventsStoreSnapshot = {
    activeSession: undefined,
    gapWarnings: [],
    versionMismatches: 0,
    invalidPayloads: 0,
  };
  private subs = new Set<StoreSubscriber>();

  subscribe(fn: StoreSubscriber) { this.subs.add(fn); fn(this.snapshot); return () => this.subs.delete(fn); }
  getSnapshot() { return this.snapshot; }

  private notify() { for (const fn of this.subs) { try { fn(this.snapshot); } catch { /* ignore */ } } }

  integrate(ev: CrawlEvent) {
    let s = this.snapshot.activeSession;
    if (!s) {
      if (ev.event_type === 'CrawlSessionStarted') {
        s = {
          sessionId: ev.session_id,
          startedAt: ev.timestamp,
          totalStages: ev.total_stages,
          stageStats: {},
        };
        this.snapshot.activeSession = s;
      } else {
        // ignore events before session start
        return;
      }
    }
    s.lastSequence = ev.sequence;
    s.lastEventType = ev.event_type;
    switch (ev.event_type) {
      case 'CrawlSessionStarted':
        s.totalStages = ev.total_stages;
        break;
      case 'CrawlSessionCompleted':
        s.completedAt = ev.timestamp;
        break;
      case 'CrawlSessionFailed':
        s.failed = true;
        s.failureMessage = ev.error_message;
        s.completedAt = ev.timestamp;
        break;
      case 'OverallProgressUpdate':
        s.currentStageIndex = ev.current_stage_index;
        s.overallProgressPct = ev.overall_progress_percentage;
        break;
      case 'StageStarted': {
        const st = s.stageStats[ev.stage_name] || { started: false, completedItems: 0, failedItems: 0, retries: 0 };
        st.started = true; s.stageStats[ev.stage_name] = st; break; }
      case 'StageItemCompleted': {
        const st = s.stageStats[ev.stage_name] || { started: false, completedItems: 0, failedItems: 0, retries: 0 };
        st.completedItems++; s.stageStats[ev.stage_name] = st; break; }
      case 'StageItemFailed': {
        const st = s.stageStats[ev.stage_name] || { started: false, completedItems: 0, failedItems: 0, retries: 0 };
        st.failedItems++; s.stageStats[ev.stage_name] = st; break; }
      case 'StageItemRetrying': {
        const st = s.stageStats[ev.stage_name] || { started: false, completedItems: 0, failedItems: 0, retries: 0 };
        st.retries++; s.stageStats[ev.stage_name] = st; break; }
      case 'StageProgress': {
        const st = s.stageStats[ev.stage_name] || { started: false, completedItems: 0, failedItems: 0, retries: 0 };
        st.lastProgressPct = ev.progress_percentage; s.stageStats[ev.stage_name] = st; break; }
    }
    this.notify();
  }

  recordGap(expected: number, got: number, lastSequence: number | null) {
    this.snapshot.gapWarnings.push({ expected, got, atSequence: lastSequence });
    this.notify();
  }
  recordVersionMismatch() { this.snapshot.versionMismatches++; this.notify(); }
  recordInvalidPayload() { this.snapshot.invalidPayloads++; this.notify(); }
}

let _store: CrawlEventsStore | null = null;
export function getCrawlEventsStore() {
  if (!_store) _store = new CrawlEventsStore();
  return _store;
}

// Wiring helper
export async function bootstrapCrawlEventsLayer() {
  const store = getCrawlEventsStore();
  const listener = getCrawlUpdatesListener({
    onGapDetected: (expected, got) => store.recordGap(expected, got, store.getSnapshot().activeSession?.lastSequence ?? null),
    onVersionMismatch: () => store.recordVersionMismatch(),
    onInvalidPayload: () => store.recordInvalidPayload(),
  });
  await listener.start();
  listener.subscribe(ev => store.integrate(ev));
  return { store, listener };
}
