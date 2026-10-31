<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type PublicState, type Settings } from '../lib/api'
  import Slider from '../lib/Slider.svelte'
  import { dbSizeStops, limitSliders, retentionStops } from '../lib/limits'

  let { unauthorized }: { unauthorized: () => void } = $props()
  let pub = $state<PublicState | null>(null)
  let pubBusy = $state(false)
  let pubError = $state('')
  let settings = $state<Settings | null>(null)
  let saved = $state('')
  let saveError = $state('')
  let current = $state('')
  let next = $state('')
  let pwMessage = $state('')
  let pwError = $state('')

  onMount(async () => {
    const [p, s] = await Promise.all([api.publicState(), api.settings()])
    if (!s.ok && s.code === 'unauthorized') return unauthorized()
    if (p.ok) pub = p.data
    if (s.ok) settings = s.data
  })

  async function togglePublic() {
    if (!pub || pub.state === 'unavailable') return
    pubBusy = true
    pubError = ''
    const r = await api.setPublic(pub.state !== 'on')
    pubBusy = false
    if (r.ok) pub = r.data
    else if (r.code === 'unauthorized') unauthorized()
    else pubError = r.message
  }

  async function save(e: SubmitEvent) {
    e.preventDefault()
    if (!settings) return
    saved = ''
    saveError = ''
    const r = await api.saveSettings(settings)
    if (r.ok) saved = 'Saved. New keys use these defaults; existing keys keep their own limits.'
    else if (r.code === 'unauthorized') unauthorized()
    else saveError = r.message
  }

  async function changePassword(e: SubmitEvent) {
    e.preventDefault()
    pwMessage = ''
    pwError = ''
    const r = await api.changePassword(current, next)
    if (r.ok) {
      current = next = ''
      pwMessage = 'Password changed. You will need to log in again.'
      setTimeout(unauthorized, 1500)
    } else pwError = r.message
  }

</script>

<div class="stack">
  <h1>Settings</h1>

  <section class="panel stack">
    <div class="row">
      <div class="grow">
        <h2 id="pub-label">Public address</h2>
        <p class="muted">Lets projects outside your Tailscale network reach C0RE over the internet. Only keys with Public reach work there.</p>
      </div>
      <button
        class="toggle"
        type="button"
        role="switch"
        aria-labelledby="pub-label"
        aria-checked={pub?.state === 'on'}
        disabled={!pub || pub.state === 'unavailable' || pubBusy}
        onclick={togglePublic}
      ></button>
    </div>
    {#if pub?.state === 'on'}
      <p>On at <span class="mono">{pub.url}</span></p>
    {:else if pub?.state === 'unavailable'}
      <p class="message">Unavailable: {pub.reason}</p>
    {:else if pub}
      <p class="muted">Off. Nothing outside your Tailscale network can reach C0RE.</p>
    {/if}
    {#if pubError}<p class="message error" role="alert">{pubError}</p>{/if}
  </section>

  {#if settings}
    <form class="panel stack" onsubmit={save}>
      <h2>Default limits for new keys</h2>
      {#each [['private_defaults', 'Private keys'], ['public_defaults', 'Public keys']] as const as [group, title] (group)}
        <fieldset>
          <legend>{title}</legend>
          <div class="sliders">
            {#each limitSliders as f (f.key)}
              <Slider label={f.label} unit={f.unit} min={f.min} max={f.max} stops={f.stops} bind:value={settings[group][f.key]} />
            {/each}
          </div>
        </fieldset>
      {/each}
      <h2>Usage history</h2>
      <div class="sliders">
        <Slider label="Keep for" unit="days" stops={retentionStops} bind:value={settings.retention_days} />
        <Slider label="Max size" unit="MB" stops={dbSizeStops} bind:value={settings.max_db_mb} />
      </div>
      {#if saved}<p class="message" role="status">{saved}</p>{/if}
      {#if saveError}<p class="message error" role="alert">{saveError}</p>{/if}
      <div class="row"><span class="spacer"></span><button class="btn primary" type="submit">Save settings</button></div>
    </form>
  {/if}

  <form class="panel stack" onsubmit={changePassword}>
    <h2>Admin password</h2>
    <div class="grid-2">
      <label class="field">Current password<input type="password" bind:value={current} autocomplete="current-password" required /></label>
      <label class="field">New password (10+ characters)<input type="password" bind:value={next} autocomplete="new-password" minlength="10" required /></label>
    </div>
    {#if pwMessage}<p class="message" role="status">{pwMessage}</p>{/if}
    {#if pwError}<p class="message error" role="alert">{pwError}</p>{/if}
    <div class="row"><span class="spacer"></span><button class="btn" type="submit">Change password</button></div>
  </form>
</div>

<style>
  .grow {
    flex: 1;
    min-width: 220px;
  }
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
  }
  legend {
    margin-bottom: 12px;
  }
  .sliders {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 22px 32px;
  }
</style>
