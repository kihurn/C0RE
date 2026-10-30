<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from './lib/api'
  import Core from './lib/Core.svelte'
  import Keys from './routes/Keys.svelte'
  import Login from './routes/Login.svelte'
  import Models from './routes/Models.svelte'
  import Overview from './routes/Overview.svelte'
  import Settings from './routes/Settings.svelte'
  import Usage from './routes/Usage.svelte'

  const pages = [
    { id: 'overview', label: 'Overview' },
    { id: 'keys', label: 'Keys' },
    { id: 'usage', label: 'Usage' },
    { id: 'models', label: 'Models' },
    { id: 'settings', label: 'Settings' }
  ] as const
  type Page = (typeof pages)[number]['id']

  const fromPath = (): Page => {
    const seg = location.pathname.replace(/^\/admin\/?/, '').split('/')[0] as Page
    return pages.some((p) => p.id === seg) ? seg : 'overview'
  }

  let auth = $state<'loading' | 'out' | 'in'>('loading')
  let passwordSet = $state(true)
  let page = $state<Page>(fromPath())
  let engineOnline = $state<boolean | null>(null)
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

  // Keeps the brand's core ring honest on every page.
  async function pollEngine() {
    const r = await api.overview()
    if (r.ok) engineOnline = r.data.engine.online
  }
  $effect(() => {
    if (auth !== 'in') return
    pollEngine()
    const t = setInterval(pollEngine, 30_000)
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
</script>

{#if auth === 'loading'}
  <p class="loading muted">Loading…</p>
{:else if auth === 'out'}
  {#if loadError}<p class="message error top" role="alert">{loadError}</p>{/if}
  <Login {passwordSet} onlogin={() => (auth = 'in')} />
{:else}
  <div class="shell">
    <nav aria-label="Sections">
      <div class="brand" role="img" aria-label="C0RE"><span aria-hidden="true">C</span><Core online={engineOnline} size={17} /><span aria-hidden="true">RE</span></div>
      <ul>
        {#each pages as p (p.id)}
          <li>
            <a href="/admin/{p.id === 'overview' ? '' : p.id}" aria-current={page === p.id ? 'page' : undefined} onclick={(e) => go(p.id, e)}>{p.label}</a>
          </li>
        {/each}
      </ul>
      <button class="link logout" type="button" onclick={logout}>Log out</button>
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
  </div>
{/if}

<style>
  .loading,
  .top {
    margin: 16px;
  }
  .shell {
    display: grid;
    grid-template-columns: 196px minmax(0, 1fr);
    min-height: 100vh;
  }
  nav {
    border-right: 1px solid var(--line);
    padding: 16px 10px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    position: sticky;
    top: 0;
    height: 100vh;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 1px;
    padding: 0 8px;
    font-weight: 700;
    font-size: 15px;
    letter-spacing: 0.04em;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  nav a {
    display: block;
    padding: 6px 8px;
    min-height: 28px;
    border-radius: var(--radius-sm);
    color: var(--text);
    text-decoration: none;
  }
  nav a:hover {
    background: var(--surface-2);
  }
  nav a[aria-current='page'] {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .logout {
    margin-top: auto;
    align-self: flex-start;
    padding: 0 8px;
  }
  main {
    padding: 24px;
    max-width: 1008px;
    width: 100%;
    outline: none;
  }
  @media (max-width: 720px) {
    .shell {
      grid-template-columns: 1fr;
    }
    nav {
      position: sticky;
      height: auto;
      flex-direction: row;
      align-items: center;
      border-right: 0;
      border-bottom: 1px solid var(--line);
      padding: 8px 12px;
      gap: 10px;
      overflow-x: auto;
      background: var(--bg);
      z-index: 1;
    }
    ul {
      display: flex;
      gap: 2px;
    }
    nav a {
      white-space: nowrap;
    }
    .logout {
      margin: 0 0 0 auto;
      align-self: center;
      white-space: nowrap;
    }
    main {
      padding: 16px;
    }
  }
</style>
