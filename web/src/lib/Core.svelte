<script lang="ts">
  // The C0RE wordmark, set in the headline face.
  let { size = 22 }: { size?: number } = $props()
</script>

<span class="mark" style:font-size="{size}px">C0RE</span>

<style>
  .mark {
    font-family: var(--display);
    line-height: 1;
    letter-spacing: 0.04em;
    color: var(--text);
    white-space: nowrap;
  }
</style>
