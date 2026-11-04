<!--
  A plugin's page. Until the plugin is approved it asks for approval;
  afterwards it loads the plugin's code and lets it draw.
-->
<script lang="ts">
  import PageHeader from "./PageHeader.svelte";
  import PluginApproval from "./PluginApproval.svelte";
  import type { PluginInfo } from "./api";
  import { mountPlugin } from "./plugins.svelte";

  let { plugin }: { plugin: PluginInfo } = $props();

  let el = $state<HTMLElement>();
  let error = $state("");

  $effect(() => {
    if (!el || !plugin.approved) return;
    const target = el;
    let cleanup: (() => void) | null = null;
    let gone = false;
    error = "";
    mountPlugin(plugin, target)
      .then((c) => (gone ? c() : (cleanup = c)))
      .catch((e) => (error = e instanceof Error ? e.message : String(e)));
    return () => {
      gone = true;
      cleanup?.();
    };
  });
</script>

<PageHeader title={plugin.name} subtitle={plugin.description || `A plugin by ${plugin.author || "someone"}`} />

{#if !plugin.approved}
  <PluginApproval {plugin} />
{:else}
  {#if error}<div class="notice bad">{plugin.name} couldn't start: {error}</div>{/if}
  <div class="plugin" bind:this={el}></div>
{/if}
