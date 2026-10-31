<script lang="ts">
  // A key's secret, hidden until asked for. Fetched from the PC only when shown, and
  // hidden again after a minute so it isn't left on screen.
  import { onDestroy } from 'svelte'
  import { api, type Key } from './api'

  let { k, unauthorized }: { k: Key; unauthorized: () => void } = $props()

  const HIDE_AFTER_MS = 60_000
  let secret = $state<string | null>(null)
  let shown = $state(false)
  let busy = $state(false)
  let problem = $state('')
  let copied = $state(false)
  let timer: ReturnType<typeof setTimeout> | undefined

  async function fetchSecret(): Promise<string | null> {
    if (secret) return secret
    busy = true
    const r = await api.revealKey(k.id)
    busy = false
    if (r.ok) return (secret = r.data.secret)
    if (r.code === 'unauthorized') unauthorized()
    else problem = r.message
    return null
  }

  function hide() {
    shown = false
    secret = null
    clearTimeout(timer)
  }

  async function toggle() {
    if (shown) return hide()
    if (!(await fetchSecret())) return
    shown = true
    clearTimeout(timer)
    timer = setTimeout(hide, HIDE_AFTER_MS)
  }

  async function copy() {
    const s = await fetchSecret()
    if (!s) return
    try {
      await navigator.clipboard.writeText(s)
      copied = true
      setTimeout(() => (copied = false), 1600)
    } catch {
      // Browsers only allow the clipboard on https or localhost: show it to copy by hand.
      shown = true
      clearTimeout(timer)
      timer = setTimeout(hide, HIDE_AFTER_MS)
      problem = 'Copying is blocked on this address. Select the key and copy it.'
    }
  }

  onDestroy(() => clearTimeout(timer))
</script>

{#if !k.revealable}
  <span class="muted small" title="Made before C0RE 0.2. Delete it and make a new key if you need to see it.">[ can't be shown ]</span>
{:else}
  <span class="secret">
    <code class="mono" class:masked={!shown}>{shown && secret ? secret : 'c0re_••••••••••••'}</code>
    <button
      class="btn icon"
      type="button"
      aria-pressed={shown}
      aria-label="{shown ? 'Hide' : 'Show'} {k.project}'s key"
      title={shown ? 'Hide' : 'Show'}
      disabled={busy}
      onclick={toggle}
    >
      {#if shown}
        <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"
          ><path d="M3 3l18 18M10.6 5.1A9.9 9.9 0 0 1 12 5c5.5 0 9 5.5 9 7 0 .7-.8 2.2-2.2 3.6M6.4 6.4C4.3 7.8 3 10.9 3 12c0 1.5 3.5 7 9 7 1.9 0 3.6-.6 5-1.5M9.9 9.9a3 3 0 0 0 4.2 4.2" /></svg
        >
      {:else}
        <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"
          ><path d="M3 12c0-1.5 3.5-7 9-7s9 5.5 9 7-3.5 7-9 7-9-5.5-9-7z" /><circle cx="12" cy="12" r="3" /></svg
        >
      {/if}
    </button>
    <button class="btn icon" type="button" aria-label="Copy {k.project}'s key" title="Copy" disabled={busy} onclick={copy}>
      {#if copied}
        <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
      {:else}
        <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"
          ><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></svg
        >
      {/if}
    </button>
    <span class="sr-only" aria-live="polite">{copied ? 'Copied' : ''}</span>
  </span>
  {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
{/if}

<style>
  .secret {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
  }
  code {
    padding: 7px 10px;
    background: var(--bg);
    border: 1px solid var(--line-strong);
    font-size: 12px;
    color: var(--white);
    user-select: all;
    word-break: break-all;
    min-width: 0;
  }
  code.masked {
    color: var(--text-3);
    user-select: none;
  }
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: square;
    stroke-linejoin: miter;
  }
  .small {
    font-size: 12px;
  }
  .problem {
    margin-top: 4px;
    font-size: 12px;
    color: var(--warn);
    max-width: 280px;
    white-space: normal;
  }
</style>
