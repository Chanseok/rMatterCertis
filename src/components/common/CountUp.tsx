import { createEffect, createSignal, onCleanup } from 'solid-js';

interface CountUpProps {
  value: number;
  durationMs?: number;
  class?: string;
}

export default function CountUp(props: CountUpProps) {
  const duration = () => props.durationMs ?? 200;
  const [display, setDisplay] = createSignal<number>(props.value ?? 0);
  let raf = 0 as number | undefined;

  createEffect(() => {
    const startVal = display();
    const endVal = props.value ?? 0;
    if (startVal === endVal) return;

    const start = performance.now();

    const step = (now: number) => {
      const t = Math.min(1, (now - start) / Math.max(1, duration()));
      const eased = 1 - Math.pow(1 - t, 3); // easeOutCubic
      const next = Math.round(startVal + (endVal - startVal) * eased);
      setDisplay(next);
      if (t < 1) {
        raf = requestAnimationFrame(step);
      }
    };

    raf = requestAnimationFrame(step);
  });

  onCleanup(() => {
    if (raf) cancelAnimationFrame(raf);
  });

  return <span class={props.class}>{display()}</span>;
}
