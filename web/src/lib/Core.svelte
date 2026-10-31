<script lang="ts">
  // The C0RE wordmark. Its 0 is drawn in outline: red while the engine is online, grey and
  // dashed-looking (dimmed) when it is not, so the brand doubles as the status light.
  let { online, size = 22 }: { online: boolean | null; size?: number } = $props()
</script>

<span class="mark" role="img" aria-label="C0RE" style:font-size="{size}px"
  ><span aria-hidden="true">C</span><span aria-hidden="true" class="zero" class:on={online === true} class:down={online === false}>0</span
  ><span aria-hidden="true">RE</span></span
>

<style>
  .mark {
    font-family: var(--display);
    line-height: 1;
    letter-spacing: 0.04em;
    color: var(--text);
    white-space: nowrap;
  }
  .zero {
    color: transparent;
    -webkit-text-stroke: 0.06em var(--text-3);
  }
  .zero.on {
    -webkit-text-stroke-color: var(--red);
  }
  .zero.down {
    -webkit-text-stroke-color: var(--text-3);
    animation: blink 1.2s steps(1) infinite;
  }
</style>
