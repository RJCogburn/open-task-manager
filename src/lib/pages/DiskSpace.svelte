<script lang="ts">
  import PageHeader from "../PageHeader.svelte";
  import ScanResults from "../ScanResults.svelte";
  import { api } from "../api";
  import { bytes, count } from "../format";
  import { live } from "../live.svelte";
  import { scan } from "../scan.svelte";

  const home = $derived(live.info?.homeDir ?? "/");
  $effect(() => {
    if (!scan.path && live.info) scan.path = live.info.homeDir;
  });

  const progress = $derived(scan.progress);
  const scanning = $derived(scan.running);
</script>

<PageHeader title="Disk Space" subtitle="Find what is filling up your drives" sources={["ncdu", "filelight"]} />

<section class="card">
  <form class="row" onsubmit={(e) => (e.preventDefault(), scan.start())}>
    <input class="input" style="flex: 1" bind:value={scan.path} placeholder="Folder to scan" aria-label="Folder to scan" />
    <button type="button" class="btn" onclick={() => (scan.path = home)}>Home</button>
    <button type="button" class="btn" onclick={() => (scan.path = "/")}>Whole system</button>
    {#if scanning}
      <button type="button" class="btn danger" onclick={() => api.scanCancel()}>Stop</button>
    {:else}
      <button type="submit" class="btn primary" disabled={!scan.path.trim()}>Scan</button>
    {/if}
  </form>
  {#if progress && (scanning || progress.done)}
    <div class="progress">
      {#if scanning}<span class="spinner" style:color="var(--accent)"></span>{/if}
      <span><b>{count(progress.files)}</b> files</span>
      <span><b>{bytes(progress.bytes)}</b> found</span>
      <span><b>{progress.seconds.toFixed(1)} s</b></span>
      {#if progress.errors}<span class="faint">{count(progress.errors)} folders could not be read</span>{/if}
      {#if progress.cancelled}<span class="badge warn">Stopped</span>{/if}
    </div>
  {/if}
  <p class="faint hint">
    Counts the space files really take on disk, stays on one drive, and counts hard-linked files once, the way ncdu does.
  </p>
</section>

{#if scan.error}<div class="notice bad" style="margin-top: 14px">{scan.error}</div>{/if}

<ScanResults />

<style>
  .progress {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-top: 14px;
    font-size: 13px;
  }

  .hint {
    font-size: 12px;
    margin: 12px 0 0;
  }
</style>
