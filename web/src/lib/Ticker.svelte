<script lang="ts">
  // A strip of live readings that scrolls past, like a departures board. Pauses on hover;
  // stands still for people who prefer reduced motion.
  export type Tick = { text: string; kind?: 'big' | 'inv' | 'ghost' | 'red' | 'blocks' }
  let { items }: { items: Tick[] } = $props()
</script>

<div class="ticker" aria-hidden="true">
  <div class="track">
    {#each [0, 1] as copy (copy)}
      {#each items as t, i (`${copy}-${i}`)}
        <span class="tk {t.kind ?? ''}">{t.text}</span>
      {/each}
    {/each}
  </div>
</div>

<style>
  .ticker {
    position: relative;
    overflow: hidden;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    background: var(--bg);
  }
  .ticker::before,
  .ticker::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    width: 70px;
    z-index: 1;
    pointer-events: none;
  }
  .ticker::before {
    left: 0;
    background: linear-gradient(to right, var(--bg), transparent);
  }
  .ticker::after {
    right: 0;
    background: linear-gradient(to left, var(--bg), transparent);
  }
  .track {
    display: flex;
    width: max-content;
    animation: ticker 48s linear infinite;
  }
  .ticker:hover .track {
    animation-play-state: paused;
  }
  .tk {
    display: flex;
    align-items: center;
    padding: 12px 26px;
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-2);
    border-right: 1px solid var(--line);
    white-space: nowrap;
  }
  .tk.big {
    font: 400 26px/1 var(--term);
    letter-spacing: 0.04em;
    color: var(--text);
    padding-block: 8px;
  }
  .tk.inv {
    background: var(--white);
    color: var(--bg);
  }
  .tk.red {
    color: var(--red);
  }
  .tk.ghost {
    font: 400 22px/1 var(--display);
    letter-spacing: 0.08em;
    color: transparent;
    -webkit-text-stroke: 1px var(--text-2);
  }
  .tk.blocks {
    color: var(--line-strong);
    letter-spacing: 0;
  }
  @keyframes ticker {
    to {
      transform: translateX(-50%);
    }
  }
</style>
