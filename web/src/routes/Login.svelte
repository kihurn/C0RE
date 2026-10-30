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
  <form class="panel stack" onsubmit={submit}>
    <h1 class="brand" aria-label="C0RE"><span aria-hidden="true">C</span><Core online={true} size={22} /><span aria-hidden="true">RE</span></h1>
    {#if passwordSet}
      <label class="field">
        Admin password
        <!-- svelte-ignore a11y_autofocus -->
        <input type="password" bind:value={password} autocomplete="current-password" required autofocus />
      </label>
      {#if error}<p class="message error" role="alert">{error}</p>{/if}
      <button class="btn primary" type="submit" disabled={busy || !password}>{busy ? 'Logging in…' : 'Log in'}</button>
    {:else}
      <p>No admin password is set yet.</p>
      <p class="muted">On the PC, open PowerShell and run:</p>
      <pre class="mono">c0re admin set-password</pre>
      <button class="btn" type="button" onclick={() => location.reload()}>I've set it — reload</button>
    {/if}
  </form>
</main>

<style>
  .login {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 16px;
  }
  form {
    width: min(360px, 100%);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 1px;
    letter-spacing: 0.04em;
  }
  pre {
    margin: 0;
    padding: 8px 10px;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }
</style>
