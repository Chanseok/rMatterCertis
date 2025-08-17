import { createSignal } from 'solid-js';

/**
 * One-shot pulse helper.
 * Returns [active, trigger] where trigger(ms) sets active=true then resets after ms (default 300ms).
 */
export function usePulse(defaultMs = 300): [() => boolean, (ms?: number) => void] {
  const [active, setActive] = createSignal(false);
  let timer: number | undefined;
  const trigger = (ms?: number) => {
    if (timer) clearTimeout(timer);
    setActive(true);
    timer = window.setTimeout(() => setActive(false), ms ?? defaultMs);
  };
  return [active, trigger];
}
