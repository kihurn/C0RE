<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Overview, type PublicState } from '../lib/api'
  import Bars from '../lib/Bars.svelte'
  import Gauge from '../lib/Gauge.svelte'
  import Ticker, { type Tick } from '../lib/Ticker.svelte'
  import { clockTime, formatBytes, formatCount, formatDuration } from '../lib/format'

  let {
    unauthorized,
    onengine,
    onnavigate
  }: {
    unauthorized: () => void
    onengine: (online: boolean) => void
    onnavigate: (p: 'keys' | 'usage' | 'settings') => void
  } = $props()

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

  // The feed refreshes every 5 seconds while this page is open and visible.
  onMount(() => {
    load()
    const t = setInterval(() => document.visibilityState === 'visible' && load(), 5_000)
    return () => clearInterval(t)
  })

  const reason = (r: string | null) => (r ? r.replaceAll('_', ' ') : '')
  const tokensOf = (r: { prompt_tokens: number | null; output_tokens: number | null }) =>
    r.prompt_tokens == null && r.output_tokens == null ? null : (r.prompt_tokens ?? 0) + (r.output_tokens ?? 0)
  let model = $derived(data?.engine.loaded[0] ?? null)

  // Boot-log lines: what C0RE checked, padded with dots to a status, like a startup screen.
  type Line = { label: string; status: string; bad?: boolean }
  let log = $derived.by((): Line[] => {
    if (!data) return []
    const e = data.engine
    const lines: Line[] = [
      e.online ? { label: `engine ollama ${e.version ?? ''}`, status: 'OK' } : { label: 'engine ollama', status: 'FAIL: not running', bad: true }
    ]
    if (e.online) {
      lines.push(model ? { label: `load ${model.name}`, status: 'OK' } : { label: 'load model', status: 'IDLE' })
      if (model)
        lines.push(
          model.gpu_percent >= 100
            ? { label: 'gpu share', status: '100%' }
            : { label: 'gpu share', status: `WARN: ${model.gpu_percent}%, rest on cpu`, bad: true }
        )
    }
    lines.push({ label: 'public address', status: pub?.state === 'on' ? 'ON' : 'OFF' })
    lines.push({ label: 'keys issued', status: String(data.keys) })
    return lines
  })
  const pad = (s: string) => `${s} `.padEnd(Math.max(28, s.length + 5), '.')

  let ticks = $derived.by((): Tick[] => {
    if (!data) return []
    const t: Tick[] = [
      { text: data.engine.online ? 'engine online' : 'engine offline', kind: data.engine.online ? 'inv' : 'red' },
      { text: `${formatCount(data.today.requests)} req today`, kind: 'big' },
      { text: model ? model.name : 'no model loaded' },
      { text: '░▒▓█▓▒░', kind: 'blocks' },
      { text: model ? `${model.gpu_percent}% gpu` : 'gpu idle', kind: model && model.gpu_percent < 100 ? 'red' : undefined },
      { text: 'private / public / off', kind: 'ghost' },
      { text: `${formatCount(data.today.blocked)} blocked`, kind: data.today.blocked ? 'red' : undefined },
      { text: `${formatCount(data.today.projects)} projects`, kind: 'big' },
      { text: `${formatCount(data.entrances.private.requests)} via tailscale` },
      { text: pub?.state === 'on' ? 'public address on' : 'public address off', kind: pub?.state === 'on' ? 'inv' : undefined }
    ]
    return t
  })
</script>

{#snippet head(n: string, title: string, link?: { text: string; to: 'keys' | 'usage' | 'settings' })}
  <div class="sec-head">
    <span class="sec-n" aria-hidden="true">[{n}]</span>
    <h2>{title}</h2>
    <span class="rule" aria-hidden="true"></span>
    {#if link}<button class="cmd" type="button" onclick={() => onnavigate(link.to)}>{link.text} &gt;&gt;</button>{/if}
  </div>
{/snippet}

<h1 class="sr-only">Overview</h1>
{#if error}<p class="message error" role="alert">{error}</p>{/if}

{#if data}
  <section class="hero" aria-label="Engine">
    <div class="hero-text">
      <ol class="boot">
        {#each log as l, i (i)}
          <li style:animation-delay="{i * 90}ms">
            <span class="caret" aria-hidden="true">&gt;</span>
            {pad(l.label)} <b class:bad={l.bad}>{l.status}</b>
          </li>
        {/each}
      </ol>
      <p class="title" aria-label={data.engine.online ? 'Engine online' : 'Engine offline'}>
        <span aria-hidden="true" class="solid">Engine</span>
        <span aria-hidden="true" class="outline" class:down={!data.engine.online}>{data.engine.online ? 'Online' : 'Offline'}</span>
      </p>
      {#if !data.engine.online}
        <p class="lede">Open Ollama on the PC. C0RE reconnects by itself; until then every request gets <b>engine offline</b>.</p>
      {:else if model}
        <dl class="specs">
          <div><dt>model</dt><dd>{model.name}</dd></div>
          <div><dt>memory</dt><dd>{formatBytes(model.size)}</dd></div>
          <div><dt>on gpu</dt><dd>{formatBytes(model.size_vram)}</dd></div>
          {#if model.context}<div><dt>context</dt><dd>{formatCount(model.context)}</dd></div>{/if}
        </dl>
        {#if model.gpu_percent < 100}
          <p class="lede bad">Part of this model is running on the CPU, so answers are slower. Close GPU-heavy apps or lower the context.</p>
        {/if}
        {#if data.engine.loaded.length > 1}
          <p class="lede">Also loaded: {data.engine.loaded.slice(1).map((m) => m.name).join(', ')}</p>
        {/if}
      {:else}
        <p class="lede">Ollama {data.engine.version} is running. The first request loads a model onto the GPU.</p>
      {/if}
    </div>
    <div class="gauge">
      <Gauge online={data.engine.online} percent={model?.gpu_percent ?? null} />
      <div class="readout">
        {#if !data.engine.online}
          <span class="big bad">ERR</span><span class="cap">engine offline</span>
        {:else if model}
          <span class="big">{model.gpu_percent}%</span><span class="cap">on gpu</span>
        {:else}
          <span class="big dim">IDLE</span><span class="cap">nothing loaded</span>
        {/if}
      </div>
    </div>
  </section>

  <Ticker items={ticks} />

  <section class="sec" aria-label="Today">
    {@render head('01', 'Today', { text: 'cat ./usage', to: 'usage' })}
    <div class="stats">
      <div class="stat"><span class="value num">{formatCount(data.today.requests)}</span><span class="label">requests</span></div>
      <div class="stat">
        <span class="value num" class:bad={data.today.blocked > 0}>{formatCount(data.today.blocked)}</span><span class="label">blocked</span>
      </div>
      <div class="stat"><span class="value num">{formatCount(data.today.projects)}</span><span class="label">projects active</span></div>
      <div class="stat"><span class="value num">{formatCount(data.keys)}</span><span class="label">keys issued</span></div>
    </div>
  </section>

  <section class="sec" aria-label="Entrances">
    {@render head('02', 'Entrances', { text: 'cd ./settings', to: 'settings' })}
    <div class="panel striped doors" data-tab="TRAFFIC_TODAY">
      <div class="door">
        <span class="chip">private</span>
        <span class="muted desc">your tailscale devices</span>
        <span class="spacer"></span>
        <span class="count num">{formatCount(data.entrances.private.requests)}</span>
        {#if data.entrances.private.blocked}<span class="num bad">{formatCount(data.entrances.private.blocked)} blocked</span>{/if}
      </div>
      <div class="door">
        {#if pub?.state === 'on'}
          <span class="chip warn">public</span>
          <span class="desc url">{pub.url}</span>
        {:else}
          <span class="chip off">public</span>
          <span class="muted desc">{pub?.state === 'unavailable' ? 'unavailable on this pc' : 'switched off'}</span>
        {/if}
        <span class="spacer"></span>
        <span class="count num">{formatCount(data.entrances.public.requests)}</span>
        {#if data.entrances.public.blocked}<span class="num bad">{formatCount(data.entrances.public.blocked)} blocked</span>{/if}
      </div>
    </div>
  </section>

  <section class="sec" aria-label="Last 24 hours">
    {@render head('03', 'Last 24 hours')}
    <div class="panel"><Bars data={data.per_hour} label="In total" /></div>
  </section>

  <section class="sec" aria-label="Live requests">
    {@render head('04', 'Live requests', { text: 'ls ./keys', to: 'keys' })}
    <div class="panel feed">
      <p class="tail" aria-hidden="true"><span class="rec"></span>tail -f access.log</p>
      {#if data.recent.length === 0}
        <p class="muted">
          No requests yet. <button class="link" type="button" onclick={() => onnavigate('keys')}>Create a key</button> and point a project at
          this PC.
        </p>
      {:else}
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>time</th><th>project</th><th class="wide">model</th><th class="r wide">tokens</th><th class="r">took</th><th>result</th>
              </tr>
            </thead>
            <tbody>
              {#each data.recent as r, i (`${r.ts}-${i}`)}
                <tr class:blocked-row={!!r.block_reason}>
                  <td class="num muted">{clockTime(r.ts)}</td>
                  <td>
                    {r.project ?? 'unknown key'}
                    {#if r.entrance === 'public'}<span class="muted small">public</span>{/if}
                  </td>
                  <td class="wide muted">{r.model ?? ''}</td>
                  <td class="r num wide muted">{tokensOf(r) == null ? '' : formatCount(tokensOf(r)!)}</td>
                  <td class="r num">{formatDuration(r.latency_ms)}</td>
                  <td>
                    {#if r.block_reason}
                      <span class="chip danger">{reason(r.block_reason)}</span>
                    {:else if r.status >= 400}
                      <span class="chip danger">error {r.status}</span>
                    {:else}
                      <span class="chip">ok</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </section>
{:else if !error}
  <p class="muted">loading…</p>
{/if}

<style>
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 40px;
    align-items: center;
    padding: 8px 0 40px;
  }
  .hero-text {
    min-width: 0;
    display: grid;
    gap: 22px;
  }
  .boot {
    list-style: none;
    margin: 0;
    padding: 0;
    font: 400 19px/1.35 var(--term);
    color: var(--text-2);
    white-space: pre;
    overflow: hidden;
  }
  /* Lines appear one after another; if the animation doesn't run they are simply shown. */
  .boot li {
    animation: type-in 0.01s steps(1) backwards;
  }
  .boot b {
    font-weight: 400;
    color: var(--white);
  }
  .boot b.bad {
    color: var(--red);
  }
  .caret {
    color: var(--text-3);
  }
  @keyframes type-in {
    from {
      opacity: 0;
    }
  }
  .title {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    column-gap: 0.25em;
    font: 400 clamp(56px, 9vw, 112px) / 0.95 var(--display);
    text-transform: uppercase;
    letter-spacing: 0.015em;
    color: var(--text);
  }
  /* Scan lines through the letters, like type on an old monitor. */
  .solid {
    background: repeating-linear-gradient(var(--text) 0 3px, #9a9a9a 3px 4px);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }
  .outline {
    -webkit-text-fill-color: transparent;
    -webkit-text-stroke: 1.5px var(--text);
  }
  .outline.down {
    -webkit-text-stroke-color: var(--red);
  }
  .specs {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 10px 32px;
  }
  .specs dt {
    font-size: 10px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .specs dd {
    margin: 0;
    font-size: 15px;
    color: var(--white);
  }
  .lede {
    max-width: 60ch;
    color: var(--text-2);
  }
  .lede b {
    color: var(--white);
    font-weight: 400;
  }
  .bad {
    color: var(--red);
  }
  .gauge {
    position: relative;
  }
  .readout {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
  }
  .big {
    font: 400 52px/1 var(--term);
    color: var(--white);
  }
  .big.dim {
    color: var(--text-2);
  }
  .big.bad {
    color: var(--red);
  }
  .cap {
    font-size: 10px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .sec {
    margin-top: 52px;
  }
  .sec-head {
    display: flex;
    align-items: flex-end;
    gap: 16px;
    margin-bottom: 18px;
  }
  .sec-n {
    font-size: 12px;
    color: var(--text-3);
    padding-bottom: 6px;
  }
  .sec-head h2 {
    margin: 0;
    font-size: 44px;
    line-height: 0.9;
  }
  .rule {
    flex: 1;
    border-bottom: 1px dashed var(--line-strong);
    margin-bottom: 8px;
  }
  .cmd {
    background: none;
    border: 0;
    padding: 0 0 4px;
    color: var(--text);
    cursor: pointer;
    font-size: 13px;
    white-space: nowrap;
  }
  .cmd:hover {
    text-decoration: underline;
    text-underline-offset: 4px;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    border: 1px solid var(--line);
    background: var(--surface);
  }
  .stat {
    display: grid;
    gap: 4px;
    padding: 20px 22px;
  }
  .stat + .stat {
    border-left: 1px solid var(--line);
  }
  .value {
    font: 400 56px/1 var(--display);
    color: var(--white);
  }
  .doors {
    display: grid;
    padding-bottom: 8px;
  }
  .door {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 52px;
    flex-wrap: wrap;
  }
  .door + .door {
    border-top: 1px dashed var(--line);
  }
  .count {
    font: 400 30px/1 var(--term);
    color: var(--white);
  }
  .url {
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .feed {
    padding-top: 14px;
  }
  .tail {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    font-size: 12px;
    color: var(--text-3);
  }
  .rec {
    width: 7px;
    height: 7px;
    background: var(--red);
    animation: blink 1.4s steps(1) infinite;
  }
  .small {
    font-size: 11px;
    margin-left: 6px;
  }
  .blocked-row td:first-child {
    box-shadow: inset 2px 0 0 var(--red);
  }
  @media (max-width: 900px) {
    .hero {
      grid-template-columns: 1fr;
    }
    .gauge {
      justify-self: center;
      order: -1;
    }
    .stats {
      grid-template-columns: 1fr 1fr;
    }
    .stat:nth-child(3) {
      border-left: 0;
    }
    .stat:nth-child(n + 3) {
      border-top: 1px solid var(--line);
    }
  }
  @media (max-width: 560px) {
    .boot {
      font-size: 16px;
    }
    .sec-head h2 {
      font-size: 32px;
    }
    .value {
      font-size: 42px;
    }
    .wide {
      display: none;
    }
    .door {
      padding: 10px 0;
      row-gap: 6px;
    }
    .desc {
      order: 3;
      flex-basis: 100%;
    }
  }
</style>
