import { createSignal } from 'solid-js';
import { tauriApi } from '../services/tauri-api';

export type GlobalEvent = {
  id: string;
  ts: string; // ISO timestamp
  name: string; // event name e.g., actor-*, crawling-*
  payload: any;
};

const MAX_BUFFER = 20000;

let _initialized = false;
const [events, setEvents] = createSignal<GlobalEvent[]>([]);

// --- De-duplication helper: drop near-duplicates within a short window ---
type KeyTs = { ts: number };
const recentKeys: Map<string, KeyTs> = new Map();
const DUP_WINDOW_MS = 3000;

function cleanupRecentKeys() {
  const now = Date.now();
  for (const [k, v] of recentKeys) {
    if (now - v.ts > DUP_WINDOW_MS) recentKeys.delete(k);
  }
}

function makeEventKey(name: string, payload: any): string | null {
  if (!payload || typeof payload !== 'object') return null;
  if (typeof payload.seq === 'number') return `${name}#seq:${payload.seq}`;
  const variant = payload.variant || payload.type || '';
  const sid = payload.session_id || '';
  const bid = payload.batch_id || '';
  const page = payload.page || payload.page_number || '';
  const ts = payload.backend_ts || payload.timestamp || '';
  if (variant || sid || bid || page || ts) return `${name}|${variant}|${sid}|${bid}|${page}|${ts}`;
  return null;
}

function shouldDropDuplicate(name: string, payload: any): boolean {
  cleanupRecentKeys();
  const key = makeEventKey(name, payload);
  if (!key) return false;
  if (recentKeys.has(key)) return true;
  recentKeys.set(key, { ts: Date.now() });
  return false;
}

function pushEvent(name: string, payload: any) {
  if (shouldDropDuplicate(name, payload)) return;
  setEvents((prev) => {
    const item: GlobalEvent = {
      id: Math.random().toString(36).slice(2),
      ts: new Date().toISOString(),
      name,
      payload,
    };
    const next = [item, ...prev];
    return next.length > MAX_BUFFER ? next.slice(0, MAX_BUFFER) : next;
  });
}

export const eventStore = {
  events,
  clear: () => setEvents([]),
  initOnce: async () => {
    if (_initialized) return;
    _initialized = true;

    const unsubs: Array<() => void> = [];

    // Unified actor-event subscription
    try {
      const un = await tauriApi.subscribeToUnifiedActorEvents({
        onEvent: (payload) => {
          const name = payload?.event_name || 'actor-event';
          pushEvent(name, payload);
        },
      });
      unsubs.push(un);
    } catch (e) {
      console.warn('[eventStore] subscribeToUnifiedActorEvents failed', e);
    }

    // Keep atomic-task updates (non-legacy) if present
    try {
      const unAtomic = await tauriApi.subscribeToAtomicTaskUpdates((p) => pushEvent('atomic-task-update', p));
      unsubs.push(unAtomic);
    } catch (e) {
      console.warn('[eventStore] atomic-task subscription failed', e);
    }

    // Note: We intentionally do not expose unsubs; store is app-lifecycle long-lived.
  },
};
