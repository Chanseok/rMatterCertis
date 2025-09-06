import { createSignal, onCleanup } from 'solid-js';
import { tauriApi } from '../services/tauri-api';
import type { StageBatcherSettings } from '../types/generated/StageBatcherSettings';

/**
 * Load StageBatcherSettings once on mount and expose loading/error state.
 */
export function useStageBatcherSettings() {
  const [settings, setSettings] = createSignal<StageBatcherSettings | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  let disposed = false;
  (async () => {
    try {
      const s = await tauriApi.getStageBatcherSettings();
      if (!disposed) setSettings(s);
    } catch (e: any) {
      if (!disposed) setError(String(e?.message ?? e));
    } finally {
      if (!disposed) setLoading(false);
    }
  })();

  onCleanup(() => { disposed = true; });

  return { settings, loading, error };
}
