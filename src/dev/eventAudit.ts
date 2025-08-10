import { listen, UnlistenFn } from '@tauri-apps/api/event';
// Lightweight in-memory event audit. (FS write removed for portability / missing fs plugin)

const EVENT_NAMES = [
  'actor-session-started','actor-session-completed','actor-session-failed','actor-session-report',
  'actor-batch-started','actor-batch-completed','actor-batch-failed','actor-batch-report',
  'actor-page-task-started','actor-page-task-completed','actor-page-task-failed',
  'actor-detail-task-started','actor-detail-task-completed','actor-detail-task-failed','actor-detail-concurrency-downshifted',
  'actor-progress','actor-phase-started','actor-phase-completed','actor-phase-aborted',
  'crawling-progress'
];

interface AuditRecord {
  recv_ts: string;
  name: string;
  seq?: number;
  backend_ts?: string;
  latency_ms?: number | null;
  payload: any;
}

const BUFFER: AuditRecord[] = [];
let initialized = false;
let unsubs: UnlistenFn[] = [];

function scheduleFlush() {/* no-op now */}

export async function enableEventAudit(): Promise<() => Promise<void>> {
  if (initialized) return async () => disableEventAudit();
  initialized = true;
  (window as any).__eventAudit = BUFFER; // expose globally for manual export
  console.info('[event-audit] in-memory audit enabled: window.__eventAudit');
  for (const name of EVENT_NAMES) {
    const un = await listen(name, (evt) => {
      const payload: any = evt.payload;
      const backend = payload?.backend_ts ? Date.parse(payload.backend_ts) : null;
      const recv = Date.now();
      const rec: AuditRecord = {
        recv_ts: new Date(recv).toISOString(),
        name,
        seq: payload?.seq,
        backend_ts: payload?.backend_ts,
        latency_ms: backend ? (recv - backend) : null,
        payload
      };
      BUFFER.push(rec);
  // simple pacing: no file flush, just trimming by interval; keep placeholder
  scheduleFlush();
    });
    unsubs.push(un);
  }
  // periodic trim to avoid unbounded growth
  const interval = setInterval(() => { if (BUFFER.length > 5000) BUFFER.splice(0, BUFFER.length - 5000); }, 5000);
  unsubs.push(() => { clearInterval(interval); });
  return async () => disableEventAudit();
}

export async function disableEventAudit() {
  for (const u of unsubs) { try { await u(); } catch {} }
  unsubs = [];
  initialized = false;
}

export async function readAuditTail(lines = 100): Promise<string[]> {
  return BUFFER.slice(-lines).map(r => JSON.stringify(r));
}
