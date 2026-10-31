<script lang="ts">
  import type { HourUsage } from './api'
  import { formatCount } from './format'

  // Requests per hour over the last `hours` hours, blocked part stacked on top.
  let { data, hours = 24, label }: { data: HourUsage[]; hours?: number; label: string } = $props()

  const now = Math.floor(Date.now() / 3600_000) * 3600
  let slots = $derived(
    Array.from({ length: hours }, (_, i) => {
      const hour = now - (hours - 1 - i) * 3600
      const d = data.find((x) => x.hour === hour)
      return { hour, requests: d?.requests ?? 0, blocked: d?.blocked ?? 0 }
    })
  )
  let max = $derived(Math.max(1, ...slots.map((s) => s.requests)))
  let total = $derived(slots.reduce((a, s) => a + s.requests, 0))
  let blocked = $derived(slots.reduce((a, s) => a + s.blocked, 0))
  const hh = (t: number) => new Date(t * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
</script>

<figure>
  <figcaption class="muted">
    {label}: {formatCount(total)} requests{#if blocked}, {formatCount(blocked)} blocked{/if}
  </figcaption>
  <div class="bars" aria-hidden="true">
    {#each slots as s (s.hour)}
      <div class="col" title="{hh(s.hour)} · {s.requests} requests{s.blocked ? `, ${s.blocked} blocked` : ''}">
        <div class="ok" style:height="{((s.requests - s.blocked) / max) * 100}%"></div>
        <div class="bad" style:height="{(s.blocked / max) * 100}%"></div>
      </div>
    {/each}
  </div>
  <div class="axis muted" aria-hidden="true">
    <span>{hh(slots[0]!.hour)}</span><span>now</span>
  </div>
  <table class="sr-only">
    <caption>{label}</caption>
    <thead><tr><th>Hour</th><th>Requests</th><th>Blocked</th></tr></thead>
    <tbody>
      {#each slots.filter((s) => s.requests) as s (s.hour)}
        <tr><td>{hh(s.hour)}</td><td>{s.requests}</td><td>{s.blocked}</td></tr>
      {/each}
    </tbody>
  </table>
</figure>

<style>
  figure {
    margin: 0;
  }
  figcaption {
    margin-bottom: 8px;
  }
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 132px;
    border-bottom: 1px solid var(--line-strong);
    /* Quarter lines, like graph paper. */
    background: repeating-linear-gradient(to top, transparent 0 32px, var(--line) 32px 33px);
  }
  .col {
    flex: 1;
    height: 100%;
    display: flex;
    flex-direction: column-reverse;
    min-width: 3px;
  }
  .ok {
    background: var(--text);
    min-height: 0;
  }
  @media (hover: hover) and (pointer: fine) {
    .col:hover .ok {
      background: var(--white);
      box-shadow: 0 0 0 1px var(--white);
    }
  }
  .bad {
    background: var(--red);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    margin-top: 6px;
  }
  figcaption {
    font-size: 12px;
  }
</style>
