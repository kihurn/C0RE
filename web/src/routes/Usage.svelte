<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Summary } from '../lib/api'
  import Bars from '../lib/Bars.svelte'
  import { formatCount, formatDuration } from '../lib/format'

  let { unauthorized }: { unauthorized: () => void } = $props()
  let days = $state(7)
  let sum = $state<Summary | null>(null)
  let error = $state('')

  const reasons: Record<string, string> = {
    missing_key: 'No key sent',
    invalid_key: 'Wrong key',
    key_disabled: 'Key switched off',
    key_not_public: 'Private key used publicly',
    model_not_allowed: 'Model not allowed',
    rate_limited: 'Too many per minute',
    daily_cap: 'Daily limit reached',
    too_many_concurrent: 'Too many at once',
    too_many_failures: 'Blocked after failed keys',
    body_too_large: 'Request too large',
    bad_request: 'Malformed request',
    engine_offline: 'Engine offline',
    model_missing: 'Model not installed',
    upstream_dropped: 'Engine stopped mid-answer'
  }

  async function load() {
    const r = await api.usage(days)
    if (r.ok) {
      sum = r.data.summary
      error = ''
    } else if (r.code === 'unauthorized') unauthorized()
    else error = r.message
  }

  onMount(load)
  const pick = (d: number) => {
    days = d
    load()
  }
</script>

<div class="stack">
  <div class="row">
    <h1>Usage</h1>
    <span class="spacer"></span>
    <div class="segmented" role="radiogroup" aria-label="Period">
      {#each [1, 7, 30, 90] as d (d)}
        <button type="button" role="radio" aria-checked={days === d} onclick={() => pick(d)}>{d === 1 ? '24 h' : `${d} days`}</button>
      {/each}
    </div>
  </div>
  {#if error}<p class="message error" role="alert">{error}</p>{/if}

  {#if sum}
    <section class="totals">
      <div class="panel"><p class="muted">Requests</p><p class="big num">{formatCount(sum.total)}</p></div>
      <div class="panel"><p class="muted">Blocked</p><p class="big num">{formatCount(sum.blocked)}</p></div>
      <div class="panel"><p class="muted">Projects</p><p class="big num">{sum.per_key.length}</p></div>
    </section>

    {#if days === 1}
      <section class="panel"><Bars data={sum.per_hour} label="Last 24 hours" /></section>
    {/if}

    <section class="panel table-wrap">
      <h2>By project</h2>
      {#if sum.per_key.length === 0}
        <p class="muted">No requests in this period.</p>
      {:else}
        <table>
          <thead>
            <tr><th>Project</th><th class="r">Requests</th><th class="r">Blocked</th><th class="r">Average time</th><th class="r">Tokens in</th><th class="r">Tokens out</th></tr>
          </thead>
          <tbody>
            {#each sum.per_key as k (k.key_id)}
              <tr>
                <td>{k.project} <span class="mono muted">{k.key_id}</span></td>
                <td class="r num">{formatCount(k.requests)}</td>
                <td class="r num">{formatCount(k.blocked)}</td>
                <td class="r num">{formatDuration(k.avg_latency_ms)}</td>
                <td class="r num">{formatCount(k.prompt_tokens)}</td>
                <td class="r num">{formatCount(k.output_tokens)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>

    {#if sum.by_reason.length}
      <section class="panel">
        <h2>Why requests were blocked</h2>
        <table>
          <tbody>
            {#each sum.by_reason as [code, n] (code)}
              <tr><td>{reasons[code] ?? code}</td><td class="r num">{formatCount(n)}</td></tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}
  {:else if !error}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .totals {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }
  .big {
    font-size: 22px;
    line-height: 28px;
    font-weight: 600;
    margin-top: 2px;
  }
  @media (max-width: 520px) {
    .totals {
      grid-template-columns: 1fr;
    }
  }
</style>
