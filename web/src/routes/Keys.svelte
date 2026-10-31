<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { api, type Key, type Limits, type Model, type Reach, type Settings } from '../lib/api'
  import ReachControl from '../lib/ReachControl.svelte'
  import SecretCell from '../lib/SecretCell.svelte'
  import Slider from '../lib/Slider.svelte'
  import { limitSliders } from '../lib/limits'
  import { formatCount, relativeTime } from '../lib/format'

  let { unauthorized }: { unauthorized: () => void } = $props()

  let keys = $state<Key[] | null>(null)
  let models = $state<Model[]>([])
  let settings = $state<Settings | null>(null)
  let error = $state('')

  // Dialog state (create or edit).
  let dialog = $state<HTMLDialogElement | null>(null)
  let editing = $state<Key | null>(null)
  let project = $state('')
  let reach = $state<Reach>('private')
  let allModels = $state(true)
  let chosen = $state<string[]>([])
  let limits = $state<Limits>({ rate_per_min: 60, daily_cap: 2000, max_concurrent: 2, max_ctx: 8192, max_body_mb: 20 })
  let note = $state('')
  let formError = $state('')
  let busy = $state(false)
  let secret = $state<string | null>(null)
  let secretKept = $state(false)
  let copied = $state(false)
  let confirmDelete = $state<string | null>(null)

  const reachText: Record<Reach, string> = { private: 'Private', public: 'Public', off: 'Off' }
  const reachChip: Record<Reach, string> = { private: 'ok', public: 'warn', off: 'off' }

  function check<T>(r: { ok: true; data: T } | { ok: false; code: string; message: string }): T | null {
    if (r.ok) return r.data
    if (r.code === 'unauthorized') unauthorized()
    else error = r.message
    return null
  }

  async function load() {
    const [k, m, s] = await Promise.all([api.keys(), api.models(), api.settings()])
    keys = check(k)
    if (m.ok) models = m.data
    if (s.ok) settings = s.data
  }

  onMount(load)

  function defaultsFor(r: Reach): Limits {
    const d = r === 'public' ? settings?.public_defaults : settings?.private_defaults
    return { ...(d ?? limits) }
  }

  async function openCreate() {
    editing = null
    project = ''
    reach = 'private'
    allModels = true
    chosen = []
    limits = defaultsFor('private')
    note = ''
    formError = ''
    secret = null
    copied = false
    dialog?.showModal()
  }

  function openEdit(k: Key) {
    editing = k
    project = k.project
    reach = k.reach
    allModels = k.models.length === 0
    chosen = [...k.models]
    limits = { rate_per_min: k.rate_per_min, daily_cap: k.daily_cap, max_concurrent: k.max_concurrent, max_ctx: k.max_ctx, max_body_mb: k.max_body_mb }
    note = k.note
    formError = ''
    secret = null
    dialog?.showModal()
  }

  // New keys start from the defaults of the reach you pick.
  let lastReach: Reach = 'private'
  $effect(() => {
    if (!editing && reach !== lastReach) {
      if (reach !== 'off') limits = defaultsFor(reach)
      lastReach = reach
    }
  })

  async function save(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    formError = ''
    const m = allModels ? [] : chosen
    if (editing) {
      const r = await api.updateKey(editing.id, { project, reach, models: m, note, ...limits })
      busy = false
      if (!r.ok) return r.code === 'unauthorized' ? unauthorized() : (formError = r.message)
      dialog?.close()
    } else {
      const r = await api.createKey({ project, reach, models: m, limits, note })
      busy = false
      if (!r.ok) return r.code === 'unauthorized' ? unauthorized() : (formError = r.message)
      secret = r.data.secret
      secretKept = r.data.key.revealable
      await tick()
      document.getElementById('secret-value')?.focus()
    }
    await load()
  }

  async function copy() {
    if (!secret) return
    try {
      await navigator.clipboard.writeText(secret)
      copied = true
    } catch {
      document.getSelection()?.selectAllChildren(document.getElementById('secret-value')!)
    }
  }

  async function setReach(k: Key, r: Reach) {
    check(await api.updateKey(k.id, { reach: r }))
    await load()
  }

  async function remove(id: string) {
    const r = await api.deleteKey(id)
    confirmDelete = null
    check(r)
    await load()
  }
</script>

<div class="stack">
  <div class="row">
    <h1>Keys</h1>
    <span class="spacer"></span>
    <button class="btn primary" type="button" onclick={openCreate}>New key</button>
  </div>
  <p class="muted">Each project gets its own key. Private keys only work from your Tailscale devices; public keys also work on the public address.</p>
  {#if error}<p class="message error" role="alert">{error}</p>{/if}

  {#if keys === null}
    <p class="muted">Loading…</p>
  {:else if keys.length === 0}
    <section class="panel empty">
      <p>No keys yet.</p>
      <p class="muted">Create one for each project that should use your AI, starting with Lenz.</p>
      <button class="btn primary" type="button" onclick={openCreate}>Create your first key</button>
    </section>
  {:else}
    <section class="panel list" aria-label="Keys">
      {#each keys as k (k.id)}
        <article class="key" class:off={k.reach === 'off'}>
          <div class="head">
            <h2>{k.project}</h2>
            <span class="mono muted id">{k.id}</span>
            <span class="chip {reachChip[k.reach]}">{reachText[k.reach]}</span>
            <span class="spacer"></span>
            {#if confirmDelete === k.id}
              <span class="row confirm">
                <span>Delete {k.project}'s key? Apps using it stop working.</span>
                <button class="btn danger" type="button" onclick={() => remove(k.id)}>Delete</button>
                <button class="btn" type="button" onclick={() => (confirmDelete = null)}>Cancel</button>
              </span>
            {:else}
              <span class="row actions">
                {#if k.reach === 'off'}
                  <button class="btn" type="button" onclick={() => setReach(k, 'private')}>Switch on</button>
                {:else}
                  <button class="btn" type="button" onclick={() => setReach(k, 'off')}>Switch off</button>
                {/if}
                <button class="btn" type="button" onclick={() => openEdit(k)}>Edit</button>
                <button class="btn danger" type="button" aria-label="Delete {k.project}'s key" onclick={() => (confirmDelete = k.id)}
                  >Delete</button
                >
              </span>
            {/if}
          </div>
          <dl>
            <div class="wide-cell"><dt>Key</dt><dd><SecretCell {k} {unauthorized} /></dd></div>
            <div><dt>Models</dt><dd>{k.models.length ? k.models.join(', ') : 'All models'}</dd></div>
            <div>
              <dt>Limits</dt>
              <dd><span class="num">{formatCount(k.rate_per_min)}</span> a minute, <span class="num">{formatCount(k.daily_cap)}</span> a day</dd>
            </div>
            <div><dt>Last used</dt><dd>{relativeTime(k.last_used_at)}</dd></div>
          </dl>
        </article>
      {/each}
    </section>
  {/if}
</div>

<dialog bind:this={dialog} aria-labelledby="key-dialog-title">
  {#if secret}
    <div class="stack">
      <h2 id="key-dialog-title">Key created for {project}</h2>
      <p>Copy this key now and paste it into {project}'s settings.</p>
      <div class="secret">
        <code id="secret-value" class="mono" tabindex="-1">{secret}</code>
        <button class="btn primary" type="button" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>
      </div>
      {#if secretKept}
        <p class="muted">You can see it again any time with the eye button on the Keys page.</p>
      {:else}
        <p class="warn-text">You won't see this key again. If you lose it, delete it and create a new one.</p>
      {/if}
      <div class="row"><span class="spacer"></span><button class="btn" type="button" onclick={() => dialog?.close()}>Done</button></div>
    </div>
  {:else}
    <form class="stack" onsubmit={save}>
      <h2 id="key-dialog-title">{editing ? `Edit ${editing.project}` : 'New key'}</h2>
      <label class="field">
        Project name
        <input type="text" bind:value={project} maxlength="40" required placeholder="e.g. lenz" />
      </label>
      <div class="field">
        <span id="reach-label">Reach</span>
        <ReachControl bind:value={reach} label="Reach" />
        <span class="muted hint">
          {#if reach === 'private'}Works only from your Tailscale devices.
          {:else if reach === 'public'}Also works on the public internet address, when it's on.
          {:else}Switched off: every request is refused.{/if}
        </span>
      </div>
      <fieldset>
        <legend>Models</legend>
        <label class="check"><input type="checkbox" bind:checked={allModels} /> All installed models</label>
        {#if !allModels}
          {#if models.length}
            {#each models as m (m.name)}
              <label class="check"><input type="checkbox" value={m.name} bind:group={chosen} /> <span class="mono">{m.name}</span></label>
            {/each}
          {:else}
            <p class="muted">The engine is offline, so models can't be listed. Choose "All installed models" or try again later.</p>
          {/if}
        {/if}
      </fieldset>
      <fieldset>
        <legend>Limits</legend>
        <div class="sliders">
          {#each limitSliders as f (f.key)}
            <Slider label={f.label} unit={f.unit} min={f.min} max={f.max} stops={f.stops} bind:value={limits[f.key]} />
          {/each}
        </div>
      </fieldset>
      <label class="field">Note (optional)<input type="text" bind:value={note} maxlength="200" /></label>
      {#if formError}<p class="message error" role="alert">{formError}</p>{/if}
      <div class="row">
        <span class="spacer"></span>
        <button class="btn" type="button" onclick={() => dialog?.close()}>Cancel</button>
        <button class="btn primary" type="submit" disabled={busy || !project.trim() || (!allModels && chosen.length === 0)}>
          {editing ? 'Save changes' : 'Create key'}
        </button>
      </div>
    </form>
  {/if}
</dialog>

<style>
  .empty {
    display: grid;
    gap: 8px;
    justify-items: start;
  }
  .list {
    padding: 0;
  }
  .key {
    padding: 22px 24px;
    display: grid;
    gap: 14px;
  }
  .key + .key {
    border-top: 1px dashed var(--line-strong);
  }
  .key.off h2,
  .key.off dd {
    color: var(--text-2);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .head h2 {
    margin: 0;
    font-size: 30px;
    line-height: 1;
  }
  .id {
    font-size: 12px;
  }
  .actions,
  .confirm {
    flex-wrap: wrap;
  }
  .key dl {
    margin: 0;
    display: grid;
    grid-template-columns: minmax(440px, 2fr) repeat(3, minmax(120px, 1fr));
    gap: 12px 24px;
  }
  .key dt {
    font-size: 10px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--text-3);
    margin-bottom: 6px;
  }
  .key dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  @media (max-width: 1100px) {
    .key dl {
      grid-template-columns: 1fr 1fr;
    }
    .wide-cell {
      grid-column: 1 / -1;
    }
  }
  dialog {
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 22px;
  }
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  legend {
    font-size: 12px;
    color: var(--text-2);
    margin-bottom: 4px;
  }
  .sliders {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 20px 28px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
  }
  .hint {
    font-size: 12px;
  }
  .field > span:first-child {
    font-size: 12px;
    color: var(--text-2);
  }
  .secret {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .secret code {
    flex: 1;
    word-break: break-all;
    user-select: all;
  }
  .warn-text {
    color: var(--warn);
  }
</style>
