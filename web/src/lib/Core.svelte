<script lang="ts">
  // The C0RE wordmark, set in the headline face, with a solid red 0.
  let { size = 22 }: { size?: number } = $props()
</script>

<span class="mark" role="img" aria-label="C0RE" style:font-size="{size}px"
  ><span aria-hidden="true">C</span><span aria-hidden="true" class="zero">0</span><span aria-hidden="true">RE</span></span
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
    color: var(--red);
  }
</style>
