<!-- What approving a plugin means, what it may run, and the button to approve it. -->
<script lang="ts">
  import { parseError, type PluginInfo } from "./api";
  import { approvePlugin } from "./plugins.svelte";

  let { plugin }: { plugin: PluginInfo } = $props();

  let error = $state("");
  const commands = $derived(Object.entries(plugin.commands));

  async function approve() {
    error = "";
    try {
      await approvePlugin(plugin, true);
    } catch (e) {
      error = parseError(e).message;
    }
  }
</script>

<section class="card approval">
  <h3>Approve {plugin.name}?</h3>
  <p>
    <b>{plugin.name}</b>{plugin.version ? ` ${plugin.version}` : ""}{plugin.author ? ` by ${plugin.author}` : ""} is a
    plugin. Its code runs inside Open Task Manager, with the same access the app has to your system. Only approve
    plugins from people you trust.
  </p>
  {#if commands.length}
    <p>It may run these programs:</p>
    <ul>
      {#each commands as [name, argv] (name)}
        <li><code>{argv.join(" ")}</code> <span class="faint">({name})</span></li>
      {/each}
    </ul>
  {:else}
    <p class="muted">It runs no programs; it only shows readings the app already has.</p>
  {/if}
  <p class="faint">If the plugin changes, it will ask again.</p>
  {#if error}<div class="notice bad">{error}</div>{/if}
  <button class="btn primary" onclick={approve}>Approve and load</button>
</section>

<style>
  .approval {
    max-width: 640px;
  }

  .approval p {
    margin: 10px 0;
  }

  ul {
    margin: 6px 0 10px;
    padding-left: 20px;
  }

  code {
    font-size: 12.5px;
  }
</style>
