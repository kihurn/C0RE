<script lang="ts">
  import { api } from '../lib/api'
  import Core from '../lib/Core.svelte'

  let { passwordSet, onlogin }: { passwordSet: boolean; onlogin: () => void } = $props()
  let password = $state('')
  let error = $state('')
  let busy = $state(false)

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = ''
    const r = await api.login(password)
    busy = false
    if (r.ok) onlogin()
    else error = r.message
  }
</script>

<main class="login">
  <form class="panel" data-tab="RESTRICTED_ACCESS" onsubmit={submit}>
    <h1><Core size={84} /></h1>
    <ol class="boot" aria-hidden="true">
      <li>&gt; connect {location.host} ....... <b>OK</b></li>
      <li>&gt; private ai gateway ......... <b>OK</b></li>
      <li>&gt; {passwordSet ? 'auth required.' : 'no admin password set.'}</li>
    </ol>
    {#if passwordSet}
      <label class="field">
        admin password
        <!-- svelte-ignore a11y_autofocus -->
        <input type="password" bind:value={password} autocomplete="current-password" required autofocus />
      </label>
      {#if error}<p class="message error" role="alert">{error}</p>{/if}
      <button class="btn primary" type="submit" disabled={busy || !password}>{busy ? 'logging in…' : 'log in'}</button>
    {:else}
      <p>No admin password is set yet. On the PC, open PowerShell and run:</p>
      <pre>c0re admin set-password</pre>
      <button class="btn" type="button" onclick={() => location.reload()}>reload</button>
    {/if}
  </form>
</main>

<style>
  .login {
    min-height: 100vh;
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 16px;
  }
  form {
    width: min(420px, 100%);
    display: grid;
    gap: 20px;
    padding: 44px 30px 30px;
    box-shadow: 8px 8px 0 var(--line);
  }
  /* Above the page's scanline overlay, so the wordmark stays crisp. */
  h1 {
    line-height: 1;
    position: relative;
    z-index: 60;
  }
  .boot {
    list-style: none;
    margin: 0;
    padding: 0;
    font: 400 18px/1.35 var(--term);
    color: var(--text-2);
  }
  .boot b {
    font-weight: 400;
    color: var(--white);
  }
  pre {
    margin: 0;
    padding: 10px 12px;
    background: var(--bg);
    border: 1px solid var(--line-strong);
  }
  .btn {
    justify-self: start;
  }
</style>
