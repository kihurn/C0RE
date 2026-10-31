<script lang="ts">
  // The engine as a pixel ring filled to `percent` (live GPU load). Lit cells turn red when
  // `alarm` is set (part of a model spilled onto the CPU). Unlit cells stay dim; sparse red
  // when the engine is offline. It lights up cell by cell once, when the page opens; after
  // that it follows the live value at once, so the number and the ring never disagree.
  import { onMount } from 'svelte'

  let {
    online,
    percent,
    alarm = false,
    size = 220
  }: { online: boolean; percent: number | null; alarm?: boolean; size?: number } = $props()

  const N = 23
  const C = (N - 1) / 2
  // Ring cells, ordered clockwise from 12 o'clock.
  const cells = (() => {
    const out: { x: number; y: number; a: number }[] = []
    for (let y = 0; y < N; y++)
      for (let x = 0; x < N; x++) {
        const d = Math.hypot(x - C, y - C)
        if (d >= C - 3.2 && d <= C + 0.4) out.push({ x, y, a: (Math.atan2(x - C, C - y) + 2 * Math.PI) % (2 * Math.PI) })
      }
    return out.sort((p, q) => p.a - q.a)
  })()

  const STAGGER_MS = 5
  let ready = $state(false)
  let swept = $state(false)
  onMount(() => {
    const t = requestAnimationFrame(() => (ready = true))
    const done = setTimeout(() => (swept = true), cells.length * STAGGER_MS + 300)
    return () => {
      cancelAnimationFrame(t)
      clearTimeout(done)
    }
  })

  let lit = $derived(online && percent != null ? Math.round((Math.max(0, Math.min(100, percent)) / 100) * cells.length) : 0)
  function cellClass(i: number): string {
    if (!ready) return 'dim'
    if (!online) return i % 7 === 0 ? 'fault' : 'dim'
    if (percent == null || i >= lit) return 'dim'
    return alarm ? 'alarm' : 'lit'
  }
</script>

<svg width={size} height={size} viewBox="0 0 {N * 10} {N * 10}" aria-hidden="true">
  {#each cells as c, i (i)}
    <rect x={c.x * 10 + 1} y={c.y * 10 + 1} width="8" height="8" class={cellClass(i)} class:swept style:transition-delay="{ready && !swept ? i * STAGGER_MS : 0}ms" />
  {/each}
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
  rect {
    transition: fill 200ms steps(2);
  }
  rect.swept {
    transition-duration: 120ms;
  }
  .dim {
    fill: var(--line);
  }
  .lit {
    fill: var(--text);
  }
  .alarm {
    fill: var(--red);
  }
  .fault {
    fill: var(--red);
    opacity: 0.7;
  }
</style>
