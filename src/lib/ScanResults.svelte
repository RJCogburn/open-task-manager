<!-- The result of the last disk space scan: browse it folder by folder, with
     the largest files and the space each file type takes beside it. -->
<script lang="ts">
  import { api } from "./api";
  import { bytes, count } from "./format";
  import { scan } from "./scan.svelte";

  const folder = $derived(scan.folder);
  const summary = $derived(scan.summary);
  const open = (id: number) => scan.open(id);

  const palette = ["var(--cpu)", "var(--mem)", "var(--disk)", "var(--net)", "var(--gpu)", "var(--temp)"];
  const typeTotal = $derived(summary?.types.reduce((a, t) => a + t.size, 0) ?? 1);
</script>

{#if folder}
  <div class="layout">
    <section class="card">
      <nav class="trail" aria-label="Folder path">
        {#each folder.trail as t, i (t.id)}
          {#if i > 0}<span class="faint">/</span>{/if}
          <button class="crumb" aria-current={t.id === folder.id ? "location" : undefined} onclick={() => open(t.id)}>
            {t.name}
          </button>
        {/each}
      </nav>
      <div class="spread" style="margin: 10px 0 12px">
        <span><b class="total">{bytes(folder.size)}</b> <span class="muted">in {count(folder.items)} items</span></span>
        <button class="btn" onclick={() => api.openFolder(folder.path)}>Open in file manager</button>
      </div>
      <ul class="entries">
        {#if folder.trail.length > 1}
          <li>
            <button class="entry" onclick={() => open(folder.trail[folder.trail.length - 2].id)}>
              <span class="ico">↰</span><span class="nm">..</span>
            </button>
          </li>
        {/if}
        {#each folder.children as c (c.id)}
          {@const share = folder.size ? c.size / folder.size : 0}
          <li>
            <button class="entry" disabled={!c.isDir} onclick={() => open(c.id)}>
              <span class="ico" class:dir={c.isDir}>{c.isDir ? "▸" : "·"}</span>
              <span class="nm" title={c.name}>{c.name}</span>
              <span class="bar" style="--c: {c.isDir ? 'var(--disk)' : 'var(--cpu)'}"><i style:width="{share * 100}%"></i></span>
              <span class="pc faint">{(share * 100).toFixed(1)}%</span>
              <b>{bytes(c.size)}</b>
            </button>
          </li>
        {/each}
      </ul>
      {#if folder.more}<p class="faint">…and {count(folder.more)} smaller items</p>{/if}
    </section>

    <div class="side">
      {#if summary}
        <section class="card">
          <h3>By file type</h3>
          <div class="stack" aria-hidden="true">
            {#each summary.types.slice(0, 6) as t, i (t.extension)}
              <i style:flex={t.size} style:background={palette[i]}></i>
            {/each}
          </div>
          <ul class="types">
            {#each summary.types.slice(0, 12) as t, i (t.extension)}
              <li>
                <i style:background={palette[i] ?? "var(--faint)"}></i>
                <span>.{t.extension}</span>
                <span class="faint">{count(t.count)}</span>
                <b>{bytes(t.size)}</b>
                <span class="faint pc">{((t.size / typeTotal) * 100).toFixed(0)}%</span>
              </li>
            {/each}
          </ul>
        </section>
        <section class="card">
          <h3>Largest files</h3>
          <ul class="largest">
            {#each summary.largest.slice(0, 20) as f (f.path)}
              {@const slash = f.path.lastIndexOf("/")}
              <li>
                <button class="file" title="Open the folder holding this file" onclick={() => api.openFolder(f.path.slice(0, slash) || "/")}>
                  <span class="fname">{f.path.slice(slash + 1)}</span>
                  <span class="fdir faint">{f.path.slice(0, slash)}</span>
                </button>
                <b>{bytes(f.size)}</b>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(280px, 380px);
    gap: 14px;
    margin-top: 14px;
    align-items: start;
  }

  @media (max-width: 1150px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

  .trail {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    font-size: 13px;
  }

  .crumb {
    border: 0;
    background: none;
    padding: 2px 5px;
    border-radius: 5px;
    cursor: pointer;
    color: var(--muted);
  }

  .crumb:hover {
    background: var(--panel-hover);
    color: var(--text);
  }

  .crumb[aria-current] {
    color: var(--text);
    font-weight: 650;
  }

  .total {
    font-size: 22px;
  }

  .entries,
  .types,
  .largest {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .entry {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) minmax(80px, 200px) 50px 80px;
    gap: 10px;
    align-items: center;
    width: 100%;
    padding: 6px 8px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    text-align: left;
    font-size: 13px;
    cursor: pointer;
  }

  .entry:disabled {
    cursor: default;
    color: var(--text);
  }

  .entry:hover:not(:disabled) {
    background: var(--panel-hover);
  }

  .entry b {
    text-align: right;
  }

  .ico {
    color: var(--faint);
    text-align: center;
  }

  .ico.dir {
    color: var(--disk);
  }

  .nm {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pc {
    text-align: right;
    font-size: 12px;
  }

  .stack {
    display: flex;
    height: 10px;
    border-radius: 5px;
    overflow: hidden;
    gap: 2px;
    margin: 12px 0;
  }

  .types li {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) auto 70px 34px;
    gap: 8px;
    align-items: center;
    font-size: 13px;
    padding: 3px 0;
  }

  .types li i {
    width: 9px;
    height: 9px;
    border-radius: 2px;
  }

  .types b {
    text-align: right;
  }

  .largest {
    margin-top: 8px;
  }

  .largest li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 0;
  }

  .file {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    border: 0;
    background: none;
    padding: 2px 4px;
    border-radius: 6px;
    text-align: left;
    cursor: pointer;
  }

  .file:hover {
    background: var(--panel-hover);
  }

  .fname,
  .fdir {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 13px;
  }

  .fdir {
    font-size: 11.5px;
  }

  .largest b {
    font-size: 13px;
  }
</style>
