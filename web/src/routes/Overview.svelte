<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Overview, type PublicState } from '../lib/api'
  import Bars from '../lib/Bars.svelte'
  import Core from '../lib/Core.svelte'
  import { clockTime, formatBytes, formatCount, formatDuration } from '../lib/format'

  let {
    unauthorized,
    onengine,
    onnavigate
  }: { unauthorized: () => void; onengine: (online: boolean) => void; onnavigate: (p: 'keys' | 'settings') => void } = $props()

  let data = $state<Overview | null>(null)
  let pub = $state<PublicState | null>(null)
  let error = $state('')

  async function load() {
    const [o, p] = await Promise.all([api.overview(), api.publicState()])
    if (!o.ok) {
      if (o.code === 'unauthorized') return unauthorized()
      error = o.message
      return
    }
    error = ''
    data = o.data
    onengine(o.data.engine.online)
    if (p.ok) pub = p.data
  }

  onMount(() => {
    load()
    const t = setInterval(load, 10_000)
    return () => clearInterval(t)
  })

  const reason = (r: string | null) => (r ? r.replaceAll('_', ' ') : '')
</script>

<div class="stack">
  <h1>Overview</h1>
  {#if error}<p class="message error" role="alert">{error}</p>{/if}

  {#if data}
    <section class="panel door-map" aria-label="How requests reach your AI">
      <div class="doors">
        <div class="door">
          <div class="row">
            <strong>Private entrance</strong>
            <span class="chip ok">Open</span>
          </div>
          <p class="muted">Your Tailscale devices</p>
          <p class="num">
            {formatCount(data.entrances.private.requests)} today{#if data.entrances.private.blocked}{' · '}<span class="blocked">{formatCount(data.entrances.private.blocked)} blocked</span>{/if}
          </p>
        </div>
        <div class="door">
          <div class="row">
            <strong>Public entrance</strong>
            {#if pub?.state === 'on'}
              <span class="chip warn">On</span>
            {:else if pub?.state === 'unavailable'}
              <span class="chip off">Unavailable</span>
            {:else}
              <span class="chip off">Off</span>
            {/if}
          </div>
          <p class="muted">
            {#if pub?.state === 'on'}{pub.url}{:else}Internet, public keys only{/if}
          </p>
          <p class="num">
            {formatCount(data.entrances.public.requests)} today{#if data.entrances.public.blocked}{' · '}<span class="blocked">{formatCount(data.entrances.public.blocked)} blocked</span>{/if}
          </p>
        </div>
      </div>
      <div class="pipe" aria-hidden="true"></div>
      <div class="engine">
        <Core online={data.engine.online} size={44} />
        <div>
          <strong>{data.engine.online ? 'Engine online' : 'Engine offline'}</strong>
          <p class="muted">
            {#if data.engine.online}Ollama {data.engine.version}{:else}Open Ollama on the PC{/if}
          </p>
        </div>
      </div>
    </section>

    <section class="panel">
      <h2>Loaded on the GPU</h2>
      {#if data.engine.loaded.length === 0}
        <p class="muted">{data.engine.online ? 'No model loaded. The first request will load one.' : 'Engine offline.'}</p>
      {:else}
        <ul class="loaded">
          {#each data.engine.loaded as m (m.name)}
            <li class="row">
              <span class="mono">{m.name}</span>
              <span class="spacer"></span>
              <span class="num muted">{formatBytes(m.size)}</span>
              {#if m.gpu_percent >= 100}
                <span class="chip ok">100% GPU</span>
              {:else}
                <span class="chip warn">{m.gpu_percent}% GPU — partly on CPU</span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="panel">
      <h2>Activity</h2>
      <Bars data={data.per_hour} label="Last 24 hours" />
    </section>

    <section class="panel">
      <div class="row"><h2>Recent requests</h2><span class="spacer"></span></div>
      {#if data.recent.length === 0}
        <p class="muted">
          Nothing yet. <button class="link" type="button" onclick={() => onnavigate('keys')}>Create a key</button> and point a project at
          this PC.
        </p>
      {:else}
        <div class="table-wrap">
          <table>
            <thead>
              <tr><th>Time</th><th>Project</th><th class="wide">Entrance</th><th class="wide">Model</th><th class="r">Time taken</th><th>Result</th></tr>
            </thead>
            <tbody>
              {#each data.recent as r, i (i)}
                <tr>
                  <td class="num">{clockTime(r.ts)}</td>
                  <td>{r.project ?? '—'}</td>
                  <td class="muted wide">{r.entrance}</td>
                  <td class="mono wide">{r.model ?? '—'}</td>
                  <td class="r num">{formatDuration(r.latency_ms)}</td>
                  <td>
                    {#if r.block_reason}
                      <span class="chip danger">{reason(r.block_reason)}</span>
                    {:else if r.status >= 400}
                      <span class="chip warn">Error {r.status}</span>
                    {:else}
                      <span class="chip ok">OK</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>
  {:else if !error}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .door-map {
    display: grid;
    grid-template-columns: minmax(0, 400px) 56px minmax(0, 1fr);
    align-items: center;
  }
  .doors {
    display: grid;
    gap: 12px;
  }
  .door {
    display: grid;
    gap: 2px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .door p {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .blocked {
    color: var(--danger);
  }
  /* A bracket joining both doors, then one line into the engine. */
  .pipe {
    position: relative;
    align-self: stretch;
  }
  .pipe::before {
    content: '';
    position: absolute;
    top: 25%;
    bottom: 25%;
    left: 0;
    width: 50%;
    border: 1px solid var(--line-strong);
    border-left: 0;
    border-radius: 0 6px 6px 0;
  }
  .pipe::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    right: 0;
    border-top: 1px solid var(--line-strong);
  }
  .engine {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-left: 10px;
  }
  .engine strong {
    font-size: 15px;
  }
  .loaded {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  @media (max-width: 560px) {
    .wide {
      display: none;
    }
  }
  @media (max-width: 720px) {
    .door-map {
      grid-template-columns: 1fr;
      gap: 12px;
    }
    .pipe {
      display: none;
    }
    .engine {
      padding: 0;
    }
  }
</style>
