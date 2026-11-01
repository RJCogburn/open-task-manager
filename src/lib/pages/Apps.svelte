<script lang="ts">
  import PageHeader from "../PageHeader.svelte";
  import { api, type App } from "../api";
  import { bytes, count } from "../format";
  import { external } from "../link";

  let apps = $state<App[]>([]);
  let sources = $state<string[]>([]);
  let loading = $state(true);
  let error = $state("");
  let query = $state("");
  let source = $state("all");
  let explicitOnly = $state(true);
  let sortBy = $state<"name" | "size">("size");

  api
    .installedApps()
    .then((r) => {
      apps = r.apps;
      sources = r.sources;
    })
    .catch((e) => (error = String(e)))
    .finally(() => (loading = false));

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = apps.filter(
      (a) =>
        (source === "all" || a.source === source) &&
        (!explicitOnly || a.explicit) &&
        (!q || a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q)),
    );
    return sortBy === "size" ? list.sort((a, b) => b.size - a.size) : list;
  });
  const biggest = $derived(Math.max(1, ...rows.map((a) => a.size)));
  const total = $derived(rows.reduce((a, r) => a + r.size, 0));
  const sourceNames: Record<string, string> = { pacman: "pacman", flatpak: "Flatpak", dpkg: "dpkg", rpm: "rpm" };
</script>

<PageHeader title="Installed Apps" subtitle="Everything your package managers have installed" sources={["pacman", "flatpak", "dpkg", "rpm"]} />

<div class="toolbar">
  <input class="input" type="search" placeholder="Search name or description" bind:value={query} style="width: 300px" aria-label="Search apps" />
  <div class="seg" role="group" aria-label="Package manager">
    <button aria-pressed={source === "all"} onclick={() => (source = "all")}>All</button>
    {#each sources as s (s)}
      <button aria-pressed={source === s} onclick={() => (source = s)}>{sourceNames[s] ?? s}</button>
    {/each}
  </div>
  <label class="row muted" style="font-size: 13px" title="Hide packages that were only installed because something else needed them">
    <input type="checkbox" bind:checked={explicitOnly} /> Only ones I installed
  </label>
  <div style="flex: 1"></div>
  <div class="seg" role="group" aria-label="Sort">
    <button aria-pressed={sortBy === "size"} onclick={() => (sortBy = "size")}>Largest</button>
    <button aria-pressed={sortBy === "name"} onclick={() => (sortBy = "name")}>A–Z</button>
  </div>
</div>

<p class="muted summary">{count(rows.length)} packages · {bytes(total)}</p>

{#if error}<div class="notice bad">{error}</div>{/if}

<div class="table-wrap">
  <table class="data">
    <thead>
      <tr>
        <th><span>Name</span></th>
        <th><span>Version</span></th>
        <th><span>From</span></th>
        <th style="width: 26%"><span>Size</span></th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as a (a.source + a.name)}
        <tr>
          <td class="app">
            <b>{a.name}</b>
            <span class="faint desc" title={a.description}>{a.description}</span>
          </td>
          <td class="muted mono">{a.version}</td>
          <td><span class="badge">{sourceNames[a.source]}</span></td>
          <td>
            <div class="row">
              <div class="bar" style="flex: 1; --c: var(--disk)"><i style:width="{(a.size / biggest) * 100}%"></i></div>
              <span class="size">{bytes(a.size)}</span>
            </div>
          </td>
          <td class="num">
            {#if a.url}<a class="btn link" href={a.url} use:external>Website</a>{/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if loading}<div class="empty"><span class="spinner"></span> Reading the package database…</div>{/if}
  {#if !loading && !rows.length}<div class="empty">No packages match.</div>{/if}
</div>

<style>
  .toolbar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }

  .summary {
    font-size: 13px;
    margin: 0 0 10px;
  }

  .app {
    max-width: 320px;
  }

  .app b {
    display: block;
  }

  .desc {
    display: block;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .size {
    width: 72px;
    text-align: right;
  }

  .btn.link {
    height: 26px;
    font-size: 12px;
    text-decoration: none;
    color: var(--text);
  }
</style>
