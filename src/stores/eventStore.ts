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

function pushEvent(name: string, payload: any) {
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

    // Actor bridge: subscribe to standardized actor-* events
    try {
      const un = await tauriApi.subscribeToActorBridgeEvents((name, payload) => {
        pushEvent(name, payload);
      });
      unsubs.push(un);
    } catch (e) {
      console.warn('[eventStore] subscribeToActorBridgeEvents failed', e);
    }

    // Legacy/core streams used by SimpleEventDisplay
    const safe = async (p: Promise<any>) => {
      try { return await p; } catch (e) { console.warn('[eventStore] subscribe failed', e); return () => {}; }
    };

    unsubs.push(await safe(tauriApi.subscribeToProgress((p) => pushEvent('crawling-progress', p))));
    unsubs.push(await safe(tauriApi.subscribeToTaskStatus((p) => pushEvent('crawling-task-update', p))));
    unsubs.push(await safe(tauriApi.subscribeToStageChange((p) => pushEvent('crawling-stage-change', p))));
    unsubs.push(await safe(tauriApi.subscribeToErrors((p) => pushEvent('crawling-error', p))));
    unsubs.push(await safe(tauriApi.subscribeToDatabaseUpdates((p) => pushEvent('database-update', p))));
    unsubs.push(await safe(tauriApi.subscribeToCompletion((p) => pushEvent('crawling-completed', p))));
    unsubs.push(await safe(tauriApi.subscribeToDetailedCrawlingEvents((p) => pushEvent('detailed-crawling-event', p))));
    unsubs.push(await safe(tauriApi.subscribeToAtomicTaskUpdates((p) => pushEvent('atomic-task-update', p))));

    // Note: We intentionally do not expose unsubs; store is app-lifecycle long-lived.
  },
};
