import { Component, createSignal, onCleanup, onMount, For, Show } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';

type AnyPayload = Record<string, any> | string | number | null;

interface LogItem {
  id: string;
  ts: string;
  evt: string;
  payload: AnyPayload;
}

const MAX_LOGS = 1000;

export const EventConsole: Component = () => {
  const [logs, setLogs] = createSignal<LogItem[]>([]);
  const [filter, setFilter] = createSignal('');
  const [autoScroll, setAutoScroll] = createSignal(true);

  let containerRef: HTMLDivElement | undefined;
  const add = (evt: string, payload: AnyPayload) => {
    setLogs(prev => {
      const item: LogItem = {
        id: Math.random().toString(36).slice(2),
        ts: new Date().toISOString(),
        evt,
        payload,
      };
      const next = [item, ...prev];
      return next.length > MAX_LOGS ? next.slice(0, MAX_LOGS) : next;
    });
  };

  onMount(async () => {
    const unsubs: (() => void)[] = [];

  // Core events
    unsubs.push(await tauriApi.subscribeToProgress((p) => add('crawling-progress', p)));
    unsubs.push(await tauriApi.subscribeToTaskStatus((p) => add('crawling-task-update', p)));
    unsubs.push(await tauriApi.subscribeToStageChange((p) => add('crawling-stage-change', p)));
    unsubs.push(await tauriApi.subscribeToErrors((p) => add('crawling-error', p)));
    unsubs.push(await tauriApi.subscribeToDatabaseUpdates((p) => add('database-update', p)));
    unsubs.push(await tauriApi.subscribeToCompletion((p) => add('crawling-completed', p)));
    unsubs.push(await tauriApi.subscribeToDetailedCrawlingEvents((p) => add('detailed-crawling-event', p)));
    unsubs.push(await tauriApi.subscribeToAtomicTaskUpdates((p) => add('atomic-task-update', p)));

    // Extended streams
    unsubs.push(await tauriApi.subscribeToConcurrency((p) => add('concurrency-event', p)));
    unsubs.push(await tauriApi.subscribeToValidation((p) => add('validation-event', p)));
    unsubs.push(await tauriApi.subscribeToDbSave((p) => add('db-save-event', p)));

  // Actor bridge (covers Stage2/3 and more with consistent names)
  unsubs.push(await tauriApi.subscribeToActorBridgeEvents((name, payload) => add(name, payload)));

    onCleanup(() => unsubs.forEach((u) => u()));
  });

  const exportJson = () => {
    const data = JSON.stringify(logs(), null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `events_${new Date().toISOString()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const filtered = () => {
    const q = filter().toLowerCase().trim();
    if (!q) return logs();
    return logs().filter((l) =>
      l.evt.toLowerCase().includes(q) || JSON.stringify(l.payload).toLowerCase().includes(q)
    );
  };

  return (
    <div class="p-3 text-sm">
      <div class="mb-2 flex gap-2 items-center">
        <input
          class="border rounded px-2 py-1 flex-1"
          placeholder="필터 (이벤트명/내용 포함)"
          value={filter()}
          onInput={(e) => setFilter(e.currentTarget.value)}
        />
        <label class="flex items-center gap-1">
          <input type="checkbox" checked={autoScroll()} onChange={(e) => setAutoScroll(e.currentTarget.checked)} />
          자동 스크롤
        </label>
        <button class="px-2 py-1 border rounded" onClick={() => setLogs([])}>Clear</button>
        <button class="px-2 py-1 border rounded" onClick={exportJson}>Export JSON</button>
      </div>
      <div ref={containerRef} class="h-[50vh] overflow-auto border rounded">
        <For each={filtered()}>{(item) => (
          <div class="border-b px-2 py-1">
            <div class="text-xs text-gray-500">{item.ts}</div>
            <div class="font-mono font-semibold">{item.evt}</div>
            <pre class="whitespace-pre-wrap text-[11px]">{JSON.stringify(item.payload, null, 2)}</pre>
          </div>
        )}</For>
        <Show when={filtered().length === 0}>
          <div class="p-4 text-gray-500">이벤트가 없습니다. 크롤링을 시작해 보세요.</div>
        </Show>
      </div>
    </div>
  );
};

export default EventConsole;
