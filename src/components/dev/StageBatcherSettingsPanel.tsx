import { Component, Show } from 'solid-js';
import { useStageBatcherSettings } from '../../hooks/useStageBatcherSettings';

const Row = (props: { label: string; value: string | number | boolean | bigint | null | undefined }) => (
  <div class="flex items-center justify-between py-1">
    <div class="text-gray-600">{props.label}</div>
    <div class="font-mono">{String(props.value)}</div>
  </div>
);

export const StageBatcherSettingsPanel: Component = () => {
  const { settings, loading, error } = useStageBatcherSettings();

  return (
    <div class="p-3 border rounded">
      <div class="font-semibold mb-2">StageBatcher Settings (read-only)</div>
      <Show when={loading()}>
        <div class="text-gray-500">Loading…</div>
      </Show>
      <Show when={error()}>
        <div class="text-red-600">{error()}</div>
      </Show>
      <Show when={!loading() && !error()}>
        <Row label="max_chunk_size" value={settings()?.max_chunk_size} />
        <Row label="prefer_reverse_order" value={settings()?.prefer_reverse_order} />
        <Row label="enforce_timeout_ms" value={settings()?.enforce_timeout_ms ?? null} />
      </Show>
    </div>
  );
};

export default StageBatcherSettingsPanel;
