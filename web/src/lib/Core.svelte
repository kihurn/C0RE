<script lang="ts">
  // The "0" of C0RE: lit when the engine is online, hollow when it is not.
  let { online, size = 18 }: { online: boolean | null; size?: number } = $props()
</script>

<svg width={size} height={size} viewBox="0 0 20 20" aria-hidden="true" class:lit={online === true} class:unknown={online === null}>
  <circle cx="10" cy="10" r="7" class="ring" />
  <circle cx="10" cy="10" r="3" class="dot" />
</svg>

<style>
  svg {
    flex: none;
  }
  .ring {
    fill: none;
    stroke: var(--text-2);
    stroke-width: 3;
  }
  .dot {
    fill: transparent;
  }
  .lit .ring {
    stroke: var(--accent);
  }
  .lit .dot {
    fill: var(--accent);
  }
  .unknown .ring {
    stroke-dasharray: 3 3;
  }
</style>
