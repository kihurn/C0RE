<script lang="ts">
  import type { Reach } from './api'

  let { value = $bindable(), label = 'Reach' }: { value: Reach; label?: string } = $props()
  const options: { v: Reach; label: string }[] = [
    { v: 'private', label: 'Private' },
    { v: 'public', label: 'Public' },
    { v: 'off', label: 'Off' }
  ]

  function onkey(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.v === value)
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') value = options[(i + 1) % options.length]!.v
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') value = options[(i + options.length - 1) % options.length]!.v
    else return
    e.preventDefault()
    ;(e.currentTarget as HTMLElement).querySelector<HTMLButtonElement>(`[data-v="${value}"]`)?.focus()
  }
</script>

<div class="segmented" role="radiogroup" aria-label={label} tabindex="-1" onkeydown={onkey}>
  {#each options as o (o.v)}
    <button
      type="button"
      role="radio"
      data-v={o.v}
      aria-checked={value === o.v}
      tabindex={value === o.v ? 0 : -1}
      onclick={() => (value = o.v)}>{o.label}</button
    >
  {/each}
</div>
