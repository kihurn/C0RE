<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type LoadedModel, type PublicState } from './lib/api'
  import Core from './lib/Core.svelte'
  import Keys from './routes/Keys.svelte'
  import Login from './routes/Login.svelte'
  import Models from './routes/Models.svelte'
  import Overview from './routes/Overview.svelte'
  import Settings from './routes/Settings.svelte'
  import Usage from './routes/Usage.svelte'

  const pages = [
    { id: 'overview', label: 'overview' },
    { id: 'keys', label: 'keys' },
    { id: 'usage', label: 'usage' },
    { id: 'models', label: 'models' },
    { id: 'settings', label: 'settings' }
  ] as const
  type Page = (typeof pages)[number]['id']

  const fromPath = (): Page => {
    const seg = location.pathname.replace(/^\/admin\/?/, '').split('/')[0] as Page
    return pages.some((p) => p.id === seg) ? seg : 'overview'
  }

  const host = location.hostname
  let auth = $state<'loading' | 'out' | 'in'>('loading')
  let passwordSet = $state(true)
  let page = $state<Page>(fromPath())
  let engineOnline = $state<boolean | null>(null)
  let loaded = $state<LoadedModel[]>([])
  let gpuLoad = $state<number | null>(null)
  let pub = $state<PublicState | null>(null)
  let loadError = $state('')

  async function checkAuth() {
    const r = await api.status()
    if (!r.ok) {
      loadError = r.message
      auth = 'out'
      return
    }
    passwordSet = r.data.password_set
    auth = r.data.logged_in ? 'in' : 'out'
  }

  function go(p: Page, e?: MouseEvent) {
    e?.preventDefault()
    page = p
    history.pushState({}, '', `/admin/${p === 'overview' ? '' : p}`)
    document.getElementById('content')?.focus()
  }

  // The system bar stays current on every page.
  async function pollStatus() {
    const [o, p] = await Promise.all([api.overview(), api.publicState()])
    if (o.ok) {
      engineOnline = o.data.engine.online
      loaded = o.data.engine.loaded
      gpuLoad = o.data.gpu?.util_percent ?? null
    }
    if (p.ok) pub = p.data
  }
  $effect(() => {
    if (auth !== 'in') return
    pollStatus()
    const t = setInterval(pollStatus, 15_000)
    return () => clearInterval(t)
  })

  async function logout() {
    await api.logout()
    auth = 'out'
  }

  function unauthorized() {
    auth = 'out'
  }

  onMount(() => {
    checkAuth()
    const onPop = () => (page = fromPath())
    addEventListener('popstate', onPop)
    return () => removeEventListener('popstate', onPop)
  })

  let model = $derived(loaded[0] ?? null)
</script>

{#if auth === 'loading'}
  <p class="loading muted">loading…</p>
{:else if auth === 'out'}
  {#if loadError}<p class="message error top" role="alert">{loadError}</p>{/if}
  <Login {passwordSet} onlogin={() => (auth = 'in')} />
{:else}
  <header class="sysbar" aria-label="System status">
    <span class="sys-left">c0re_gateway // {host}</span>
    <span class="signal" class:on={engineOnline === true} class:down={engineOnline === false}>
      <span class="led" aria-hidden="true"></span>
      {engineOnline === null ? 'checking engine' : engineOnline ? 'engine online' : 'engine offline'}
      {#if engineOnline && gpuLoad != null}<span class="sys-gpu" class:spill={!!model && model.gpu_percent < 100}>:: gpu load {gpuLoad}%</span>
      {:else if model}<span class="sys-gpu" class:spill={model.gpu_percent < 100}>:: {model.gpu_percent}% on gpu</span>{/if}
    </span>
    <span class="sys-right" class:pub-on={pub?.state === 'on'}>public_address :: {pub?.state === 'on' ? 'on' : 'off'}</span>
  </header>

  <nav class="nav" aria-label="Sections">
    <div class="nav-inner">
      <a class="prompt" href="/admin/" onclick={(e) => go('overview', e)} aria-label="C0RE overview">
        <Core size={26} />
        <span class="path" aria-hidden="true">~$<span class="cursor"></span></span>
      </a>
      <ul>
        {#each pages as p (p.id)}
          <li>
            <a href="/admin/{p.id === 'overview' ? '' : p.id}" aria-current={page === p.id ? 'page' : undefined} onclick={(e) => go(p.id, e)}
              >{p.label}</a
            >
          </li>
        {/each}
        <li><button class="logout" type="button" onclick={logout}>logout</button></li>
      </ul>
    </div>
  </nav>

  <main id="content" tabindex="-1">
    {#if page === 'overview'}
      <Overview {unauthorized} onengine={(o: boolean) => (engineOnline = o)} onnavigate={(p: Page) => go(p)} />
    {:else if page === 'keys'}
      <Keys {unauthorized} />
    {:else if page === 'usage'}
      <Usage {unauthorized} />
    {:else if page === 'models'}
      <Models {unauthorized} />
    {:else}
      <Settings {unauthorized} />
    {/if}
  </main>

  <footer class="foot">
    <span>c0re {__C0RE_VERSION__} // private gateway</span>
    <span class="blocks" aria-hidden="true">░▒▓█▓▒░</span>
    <span>{host}:{location.port || '80'}</span>
  </footer>
{/if}

<style>
  .loading,
  .top {
    margin: 16px;
  }
  .sysbar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 16px;
    padding: 6px 24px;
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--text-2);
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .sys-right {
    justify-self: end;
  }
  .pub-on {
    color: var(--white);
  }
  .signal {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-weight: 700;
  }
  .signal.on {
    color: var(--white);
  }
  .signal.down {
    color: var(--red);
  }
  .led {
    width: 6px;
    height: 6px;
    background: var(--text-3);
  }
  .on .led {
    background: var(--white);
  }
  .down .led {
    background: var(--red);
    animation: blink 1s steps(1) infinite;
  }
  .sys-gpu.spill {
    color: var(--red);
  }
  /* Above the page's scanline overlay, so the wordmark stays crisp. */
  .nav {
    position: sticky;
    top: 0;
    z-index: 60;
    background: rgba(7, 7, 7, 0.94);
    border-bottom: 1px solid var(--line);
  }
  .nav-inner {
    max-width: 1180px;
    margin: 0 auto;
    padding: 12px 24px;
    display: flex;
    align-items: center;
    gap: 24px;
  }
  .prompt {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    text-decoration: none;
    color: var(--text);
  }
  .path {
    font-size: 15px;
    color: var(--text-2);
  }
  .cursor {
    display: inline-block;
    width: 8px;
    height: 15px;
    margin-left: 3px;
    vertical-align: -2px;
    background: var(--white);
    animation: blink 1.1s steps(1) infinite;
  }
  ul {
    list-style: none;
    margin: 0 0 0 auto;
    padding: 0;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  ul a,
  .logout {
    display: block;
    padding: 5px 12px;
    font-size: 13px;
    color: var(--text-2);
    text-decoration: none;
    border: 1px solid transparent;
    background: none;
    cursor: pointer;
  }
  ul a::before,
  .logout::before {
    content: './';
    opacity: 0.5;
  }
  ul a[aria-current='page'] {
    background: var(--white);
    color: var(--bg);
  }
  ul a[aria-current='page']::before {
    opacity: 0.8;
  }
  /* Hover only with a mouse; on a phone a tap would leave it stuck. */
  @media (hover: hover) and (pointer: fine) {
    ul a:not([aria-current='page']):hover,
    .logout:hover {
      color: var(--white);
      border-color: var(--white);
    }
    .logout:hover {
      color: var(--red);
    }
  }
  main {
    max-width: 1180px;
    margin: 0 auto;
    padding: 36px 24px 56px;
    outline: none;
  }
  .foot {
    max-width: 1180px;
    margin: 0 auto;
    padding: 18px 24px 28px;
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--text-3);
    border-top: 1px dashed var(--line);
  }
  .blocks {
    letter-spacing: 0;
    color: var(--line-strong);
  }

  @media (max-width: 760px) {
    .sysbar {
      grid-template-columns: 1fr;
      justify-items: center;
      padding: 6px 16px;
    }
    .sys-left,
    .sys-right {
      display: none;
    }
    .nav-inner {
      flex-direction: column;
      align-items: stretch;
      gap: 10px;
      padding: 10px 16px;
    }
    ul {
      margin: 0;
      flex-wrap: nowrap;
      overflow-x: auto;
      scrollbar-width: none;
    }
    /* Thumb-sized targets. */
    ul a,
    .logout {
      white-space: nowrap;
      display: flex;
      align-items: center;
      min-height: 44px;
      padding: 0 12px;
    }
    main {
      padding: 24px 16px 40px;
    }
    .foot {
      padding: 16px;
    }
    .foot .blocks {
      display: none;
    }
  }
</style>
