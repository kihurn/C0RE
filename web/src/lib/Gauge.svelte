<script lang="ts">
  // The engine as a pixel ring: each lit cell is a share of the loaded model. White cells
  // sit on the GPU; red cells spilled onto the CPU (slow). Dim when idle, sparse red when
  // the engine is offline. Lights up cell by cell once, when the page opens.
  import { onMount } from 'svelte'

  let { online, percent, size = 220 }: { online: boolean; percent: number | null; size?: number } = $props()

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

  let ready = $state(false)
  onMount(() => {
    const t = requestAnimationFrame(() => (ready = true))
    return () => cancelAnimationFrame(t)
  })

  let lit = $derived(online && percent != null ? Math.round((Math.max(0, Math.min(100, percent)) / 100) * cells.length) : 0)
  function cellClass(i: number): string {
    if (!ready) return 'dim'
    if (!online) return i % 7 === 0 ? 'fault' : 'dim'
    if (percent == null) return 'dim'
    return i < lit ? 'gpu' : 'cpu'
  }
</script>

<svg width={size} height={size} viewBox="0 0 {N * 10} {N * 10}" aria-hidden="true">
  {#each cells as c, i (i)}
    <rect x={c.x * 10 + 1} y={c.y * 10 + 1} width="8" height="8" class={cellClass(i)} style:transition-delay="{ready ? i * 6 : 0}ms" />
  {/each}
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
  rect {
    transition: fill 0.25s steps(2);
  }
  .dim {
    fill: var(--line);
  }
  .gpu {
    fill: var(--text);
  }
  .cpu {
    fill: var(--red);
  }
  .fault {
    fill: var(--red);
    opacity: 0.7;
  }
</style>
