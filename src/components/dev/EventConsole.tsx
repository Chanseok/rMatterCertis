import { Component, createSignal, onMount, For, Show, createMemo } from 'solid-js';
import { eventStore } from '../../stores/eventStore';

export const EventConsole: Component = () => {
  const [filter, setFilter] = createSignal('');
  const [autoScroll, setAutoScroll] = createSignal(true);

  let containerRef: HTMLDivElement | undefined;

  onMount(async () => {
    await eventStore.initOnce();
  });

  const exportJson = () => {
    const data = JSON.stringify(eventStore.events(), null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `events_${new Date().toISOString()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const filtered = createMemo(() => {
    const q = filter().toLowerCase().trim();
    const list = eventStore.events();
    if (!q) return list;
    return list.filter((l) =>
      l.name.toLowerCase().includes(q) || JSON.stringify(l.payload).toLowerCase().includes(q)
    );
  });

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
  <button class="px-2 py-1 border rounded" onClick={() => eventStore.clear()}>Clear</button>
        <button class="px-2 py-1 border rounded" onClick={exportJson}>Export JSON</button>
      </div>
      <div ref={containerRef} class="h-[50vh] overflow-auto border rounded">
    <For each={filtered()}>{(item) => (
          <div class="border-b px-2 py-1">
            <div class="text-xs text-gray-500">{item.ts}</div>
      <div class="font-mono font-semibold">{item.name}</div>
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
