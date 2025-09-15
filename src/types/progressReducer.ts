// Utility to maintain ProductLifecycleGroup progress state.
// Aggregates partial snapshots (fetch/persist phases) keyed by (session_id,batch_id,phase).

import type { AppEvent } from './generated/AppEvent';

export interface GroupProgressState {
  key: string;
  session_id: string;
  batch_id: string | null;
  phase: string; // 'fetch' | 'persist' | etc.
  group_size: number;
  done: number;
  succeeded: number;
  failed: number;
  duplicates: number;
  progress: number; // 0..1
  final: boolean;
  lastTimestamp: number; // epoch ms
}

export interface ProgressStore {
  groups: Record<string, GroupProgressState>;
}

export const createEmptyProgressStore = (): ProgressStore => ({ groups: {} });

function computeKey(session_id: string, batch_id: string | null, phase: string) {
  return `${session_id}:${batch_id ?? 'none'}:${phase}`;
}

export function applyAppEvent(store: ProgressStore, evt: AppEvent): ProgressStore {
  if ('ProductLifecycleGroup' in evt) {
    const g = evt.ProductLifecycleGroup;
    const key = computeKey(g.session_id, g.batch_id, g.phase);
    const done = (g as any).done ?? (g.succeeded + g.failed + g.duplicates);
    const progress = g.group_size > 0 ? Math.min(1, done / g.group_size) : 0;
    const final = (!g.partial && done === g.group_size);
    const ts = Date.parse(g.timestamp);
    const prev = store.groups[key];
    if (prev && prev.lastTimestamp > ts) {
      return store; // ignore out-of-order
    }
    store.groups[key] = {
      key,
      session_id: g.session_id,
      batch_id: g.batch_id,
      phase: g.phase,
      group_size: g.group_size,
      done,
      succeeded: g.succeeded,
      failed: g.failed,
      duplicates: g.duplicates,
      progress,
      final,
      lastTimestamp: ts,
    };
  }
  return store;
}

export function summarize(store: ProgressStore) {
  const phases = Object.values(store.groups).reduce<Record<string, GroupProgressState>>((acc, v) => {
    acc[v.phase] = v;
    return acc;
  }, {});
  return { phases };
}
