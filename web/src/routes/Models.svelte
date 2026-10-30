<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Model } from '../lib/api'
  import { formatBytes } from '../lib/format'

  let { unauthorized }: { unauthorized: () => void } = $props()
  let models = $state<Model[] | null>(null)
  let error = $state('')

  onMount(async () => {
    const r = await api.models()
    if (r.ok) models = r.data
    else if (r.code === 'unauthorized') unauthorized()
    else error = r.message
  })
</script>

<div class="stack">
  <h1>Models</h1>
  <p class="muted">Models installed on the PC. To add one, run <span class="mono">ollama pull &lt;name&gt;</span> on the PC.</p>
  {#if error}<p class="message error" role="alert">{error}</p>{/if}
  {#if models}
    <section class="panel table-wrap">
      {#if models.length === 0}
        <p class="muted">No models installed yet.</p>
      {:else}
        <table>
          <thead><tr><th>Model</th><th class="r">Size</th><th>Can</th><th>Status</th></tr></thead>
          <tbody>
            {#each models as m (m.name)}
              <tr>
                <td class="mono">{m.name}</td>
                <td class="r num">{m.size ? formatBytes(m.size) : '—'}</td>
                <td>
                  <span class="caps">
                    <span>Text</span>
                    {#if m.vision}<span>Images</span>{/if}
                    {#if m.thinking}<span>Thinking</span>{/if}
                  </span>
                </td>
                <td>{#if m.loaded}<span class="chip ok">Loaded</span>{:else}<span class="chip off">On disk</span>{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {:else if !error}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .caps {
    display: inline-flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .caps span {
    font-size: 12px;
    padding: 0 6px;
    border-radius: 4px;
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
</style>
