<script lang="ts">
  import { board, card } from "$lib/arrange.svelte";
  import Chart from "$lib/Chart.svelte";
  import InstallPrompt from "$lib/InstallPrompt.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, missingProgram, parseError, type NetProcess, type Origin, type ProcessSource } from "$lib/api";
  import SpeedResult from "$lib/SpeedResult.svelte";
  import { benchmarks } from "$lib/benchmarks.svelte";
  import { netRate, bytes, count } from "$lib/format";
  import { live, poll } from "$lib/live.svelte";
  import { settings } from "$lib/settings.svelte";

  const s = $derived(live.sample);
  const nets = $derived(s?.nets.filter((n) => settings.showIdleInterfaces || n.rxTotal + n.txTotal > 0) ?? []);
  const hidden = $derived((s?.nets.length ?? 0) - nets.length);

  // The interface whose programs are listed: the busiest one until you pick.
  let picked = $state<string | null>(null);
  const busiest = $derived([...nets].sort((a, b) => b.rxTotal + b.txTotal - (a.rxTotal + a.txTotal))[0]?.name ?? null);
  const selected = $derived(picked && nets.some((n) => n.name === picked) ? picked : busiest);

  let procs = $state<NetProcess[]>([]);
  let error = $state("");
  let missing = $state<string | null>(null);
  async function loadActivity() {
    try {
      procs = await api.netActivity();
      error = "";
      missing = null;
    } catch (e) {
      error = parseError(e).message;
      missing = missingProgram(e);
    }
  }
  poll(loadActivity, () => Math.max(2000, settings.intervalMs));

  const using = $derived(
    selected
      ? procs
          .filter((p) => p.byInterface[selected])
          .map((p) => ({ p, u: p.byInterface[selected] }))
          .sort((a, b) => b.u.rx + b.u.tx - (a.u.rx + a.u.tx) || b.u.connections - a.u.connections)
      : [],
  );

  const known = new Set<Origin["kind"]>(["package", "flatpak", "snap", "hidden"]);
  const suspicious = $derived(procs.filter((p) => p.origin.suspicious));
  const unpackaged = $derived(procs.filter((p) => !p.origin.suspicious && !known.has(p.origin.kind)));

  const originLabel: Record<Origin["kind"], string> = {
    package: "Installed package",
    flatpak: "Flatpak",
    snap: "Snap",
    appimage: "AppImage",
    home: "Home folder",
    manual: "Installed by hand",
    elsewhere: "Not from a package",
    unpackaged: "Unknown system file",
    temporary: "Temporary folder",
    deleted: "Program file gone",
    hidden: "Another user",
    unchecked: "Not checked",
  };
  const originClass = (o: Origin) => (o.suspicious ? "bad" : known.has(o.kind) ? (o.kind === "hidden" ? "" : "ok") : "warn");

  // Details for the program you click on.
  let detail = $state<ProcessSource | null>(null);
  let detailFor = $state<NetProcess | null>(null);
  let detailError = $state("");

  async function show(p: NetProcess) {
    if (p.pid == null) return;
    detailFor = p;
    detail = null;
    detailError = "";
    try {
      detail = await api.processSource(p.pid);
    } catch (e) {
      detailError = parseError(e).message;
    }
  }

  function close() {
    detailFor = null;
    detail = null;
  }

  async function end() {
    if (!detail) return;
    if (settings.confirmEnd && !confirm(`End ${detail.name} (${detail.pid})? This closes all of its connections.`)) return;
    try {
      await api.endProcess(detail.pid, false);
      close();
    } catch (e) {
      detailError = parseError(e).message;
    }
  }

  const speedError = $derived(benchmarks.errors.network);
</script>

<PageHeader title="Network" subtitle="Traffic on each network interface, and the programs behind it">
  {#snippet actions()}
    {#if hidden > 0 || settings.showIdleInterfaces}
      <label class="row muted" style="font-size: 13px">
        <input type="checkbox" bind:checked={settings.showIdleInterfaces} /> Show unused interfaces
      </label>
    {/if}
    <button class="btn primary" disabled={benchmarks.running !== null} onclick={benchmarks.runNetwork}>
      {#if benchmarks.running === "network"}<span class="spinner"></span> Testing…{:else}Test internet speed{/if}
    </button>
  {/snippet}
</PageHeader>

<section class="card speed">
  <h3>Internet speed</h3>
  {#if benchmarks.running === "network"}
    <div class="row">
      <span class="spinner" style:color="var(--accent)"></span>
      <span class="muted">Measuring latency, download and upload. This takes about 25 seconds.</span>
    </div>
  {:else if speedError?.missing}
    <InstallPrompt program={speedError.missing} compact oninstalled={benchmarks.runNetwork} />
  {:else if speedError}
    <div class="notice bad">{speedError.message}</div>
  {/if}
  <SpeedResult />
</section>

{#if s}
  <div use:board={{ group: "network", min: 420 }}>
    {#each nets as n (n.name)}
      <button class="card iface" use:card={{ group: "network", id: n.name }} aria-pressed={n.name === selected} onclick={() => (picked = n.name)}>
        <div class="spread">
          <div>
            <h3>Interface</h3>
            <div class="title">{n.name}</div>
          </div>
          <div class="now">
            <span style:color="var(--net)">↓ {netRate(n.rx)}</span>
            <span style:color="var(--gpu)">↑ {netRate(n.tx)}</span>
          </div>
        </div>
        <div style="margin-top: 12px">
          <Chart
            label="{n.name} traffic"
            series={[
              { label: "Download", data: live.history(`net:${n.name}:rx`), color: "var(--net)" },
              { label: "Upload", data: live.history(`net:${n.name}:tx`), color: "var(--gpu)" },
            ]}
            tick={live.tick}
            height={150}
            max="auto"
            format={netRate}
          />
        </div>
        <div class="stats">
          <div class="stat"><span>Received since boot</span><b>{bytes(n.rxTotal)}</b></div>
          <div class="stat"><span>Sent since boot</span><b>{bytes(n.txTotal)}</b></div>
        </div>
      </button>
    {:else}
      <div class="empty">No network traffic yet.</div>
    {/each}
  </div>
{/if}

{#if missing}
  <div style="margin-top: 14px">
    <InstallPrompt program={missing} name="iproute2" why="Which programs use the network comes from ss, part of iproute2." compact oninstalled={loadActivity} />
  </div>
{:else if error}<div class="notice bad" style="margin-top: 14px">{error}</div>{/if}

{#if suspicious.length}
  <h2 class="section">Needs a look</h2>
  <div class="notice bad" style="margin-bottom: 10px">
    These programs are using the network and don't come from anything installed on this system the usual way. That
    isn't proof of harm, but check that you recognise them.
  </div>
  <div class="table-wrap">
    <table class="data">
      <tbody>
        {#each suspicious as p (p.pid)}
          <tr class="click" onclick={() => show(p)}>
            <td><b>{p.name}</b> <span class="faint">· {p.pid}</span></td>
            <td><span class="badge bad">{originLabel[p.origin.kind]}</span></td>
            <td class="muted">{p.origin.detail}</td>
            <td class="mono path" title={p.exe}>{p.exe}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

{#if selected}
  <h2 class="section">Using {selected}</h2>
  <div class="table-wrap">
    <table class="data">
      <thead>
        <tr>
          <th><span>Program</span></th>
          <th><span>Comes from</span></th>
          <th class="num"><span>Download</span></th>
          <th class="num"><span>Upload</span></th>
          <th class="num"><span>Connections</span></th>
          <th><span>Talking to</span></th>
        </tr>
      </thead>
      <tbody>
        {#each using as { p, u } (p.pid ?? -1)}
          <tr class:click={p.pid != null} onclick={() => show(p)} title={p.pid != null ? "Where does this program come from?" : ""}>
            <td>
              {#if p.pid != null}<b>{p.name}</b> <span class="faint">· {p.pid}</span>{:else}<span class="faint">{p.name}</span>{/if}
            </td>
            <td><span class="badge {originClass(p.origin)}" title={p.origin.detail}>{originLabel[p.origin.kind]}</span></td>
            <td class="num">{netRate(u.rx)}</td>
            <td class="num">{netRate(u.tx)}</td>
            <td class="num">{count(u.connections)}</td>
            <td class="mono muted remotes">{p.remotes.slice(0, 3).join(", ")}{p.remotes.length > 3 ? " …" : ""}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if !using.length}<div class="empty">Nothing is connected through {selected} right now.</div>{/if}
  </div>
  <p class="faint hint">
    Speeds are measured for TCP connections. UDP connections, which browsers use for some traffic, are counted but
    can't be measured. Click a program to see where it is installed.
  </p>
{/if}

{#if unpackaged.length}
  <h2 class="section">Not from an installed package</h2>
  <p class="faint hint" style="margin-top: -4px">
    Programs on the network that the package manager didn't install: things in your home folder, AppImages, or
    programs put on another drive by hand. Usually fine, worth knowing.
  </p>
  <div class="chips">
    {#each unpackaged as p (p.pid)}
      <button class="chip" onclick={() => show(p)} title={p.exe}>
        <b>{p.name}</b><span class="badge warn">{originLabel[p.origin.kind]}</span>
      </button>
    {/each}
  </div>
{/if}

{#if detailFor}
  <div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="card dialog" role="dialog" aria-modal="true" aria-label="Where {detailFor.name} comes from">
      <div class="spread">
        <div>
          <h3>Program</h3>
          <div class="title">{detailFor.name} <span class="faint">· {detailFor.pid}</span></div>
        </div>
        <button class="btn" onclick={close}>Close</button>
      </div>
      {#if detailError}<div class="notice bad" style="margin-top: 12px">{detailError}</div>{/if}
      {#if detail}
        <div class="notice" class:bad={detail.origin.suspicious} class:info={!detail.origin.suspicious} style="margin-top: 14px">
          <span><b>{originLabel[detail.origin.kind]}.</b> {detail.origin.detail}</span>
        </div>
        <dl class="facts">
          <dt>Program file</dt>
          <dd class="mono">{detail.exe ?? "Hidden: it runs as another user"}</dd>
          <dt>Command</dt>
          <dd class="mono">{detail.command || "—"}</dd>
          <dt>Working folder</dt>
          <dd class="mono">{detail.cwd ?? "—"}</dd>
          <dt>User</dt>
          <dd>{detail.user || "—"}</dd>
          <dt>Started by</dt>
          <dd>{detail.parent ?? "—"}</dd>
          <dt>Started</dt>
          <dd>{new Date(detail.started * 1000).toLocaleString()}</dd>
          <dt>Talking to</dt>
          <dd class="mono">{detailFor.remotes.join(", ") || "Only this computer"}</dd>
        </dl>
        <div class="row" style="margin-top: 16px; justify-content: flex-end">
          {#if detail.folder}
            <button class="btn" onclick={() => api.openFolder(detail!.folder!)}>Open its folder</button>
          {/if}
          <button class="btn danger" onclick={end}>End process</button>
        </div>
      {:else if !detailError}
        <div class="row" style="margin-top: 14px"><span class="spinner"></span><span class="muted">Checking…</span></div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .title {
    font-size: 16px;
    font-weight: 600;
  }

  .now {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-weight: 650;
    font-size: 15px;
  }

  .iface {
    text-align: left;
    cursor: pointer;
    font: inherit;
    color: inherit;
    transition: border-color 0.12s;
  }

  .iface:hover {
    border-color: var(--border-strong);
  }

  .iface[aria-pressed="true"] {
    border-color: var(--net);
    box-shadow: 0 0 0 1px var(--net), var(--shadow);
  }

  .speed {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 14px;
  }

  .section {
    font-size: 15px;
    margin: 26px 0 10px;
  }

  .hint {
    font-size: 12px;
    margin: 8px 0 0;
  }

  tr.click {
    cursor: pointer;
  }

  td.remotes,
  td.path {
    max-width: 280px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 6px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--panel);
    cursor: pointer;
    font-size: 13px;
  }

  .chip:hover {
    background: var(--panel-hover);
  }

  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 30px;
    background: rgba(0, 0, 0, 0.45);
  }

  .dialog {
    width: min(680px, 100%);
    max-height: 100%;
    overflow-y: auto;
  }

  .facts {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 8px 18px;
    margin: 16px 0 0;
    font-size: 13px;
  }

  .facts dt {
    color: var(--muted);
  }

  .facts dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
