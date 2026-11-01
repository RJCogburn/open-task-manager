<script lang="ts">
  import { card } from "$lib/arrange.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, parseError, type Connection } from "$lib/api";
  import { poll } from "$lib/live.svelte";
  import { settings } from "$lib/settings.svelte";

  let list = $state<Connection[]>([]);
  let error = $state("");
  let filter = $state<"all" | "listening" | "active">("all");
  let query = $state("");
  let paused = $state(false);

  poll(async () => {
    if (paused) return;
    try {
      list = await api.connections();
      error = "";
    } catch (e) {
      error = parseError(e).message;
    }
  }, () => 3000);

  const listening = (c: Connection) => c.state === "LISTEN" || c.state === "UNCONN";
  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return list
      .filter((c) => (filter === "listening" ? listening(c) : filter === "active" ? !listening(c) : true))
      .filter(
        (c) =>
          !q ||
          [c.process ?? "", c.localAddress, c.localPort, c.peerAddress, c.peerPort, String(c.pid ?? "")].some((v) =>
            v.toLowerCase().includes(q),
          ),
      )
      .sort((a, b) => order(a) - order(b) || (a.process ?? "~").localeCompare(b.process ?? "~"));
  });
  const order = (c: Connection) => (c.exposure === "network" ? 0 : c.exposure === "local" ? 1 : 2);

  const exposed = $derived(list.filter((c) => c.exposure === "network"));
  const unknownOwners = $derived(list.filter((c) => !c.process).length);
  const byProcess = $derived.by(() => {
    const m = new Map<string, number>();
    for (const c of list) if (!listening(c)) m.set(c.process ?? "(system)", (m.get(c.process ?? "(system)") ?? 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
  });

  const exposureText = { network: "Open to network", local: "This computer only", outbound: "Outgoing" };
  const exposureClass = { network: "warn", local: "ok", outbound: "" };

  async function end(c: Connection) {
    if (c.pid == null) return;
    if (settings.confirmEnd && !confirm(`End ${c.process} (${c.pid})? This closes all of its connections.`)) return;
    try {
      await api.endProcess(c.pid, false);
    } catch (e) {
      error = parseError(e).message;
    }
  }
</script>

<PageHeader title="Connections" subtitle="Which programs are talking on the network, and who can reach them">
  {#snippet actions()}
    <button class="btn" onclick={() => (paused = !paused)} aria-pressed={paused}>{paused ? "Resume" : "Pause"}</button>
  {/snippet}
</PageHeader>

<div class="grid" style="--min: 220px; margin-bottom: 14px">
  <section class="card" use:card={{ group: "connections", id: "exposed" }}>
    <h3>Open to the network</h3>
    <div class="big" style:color={exposed.length ? "var(--warn)" : "var(--ok)"}>{exposed.length}</div>
    <div class="faint small">Listening sockets other computers can reach</div>
  </section>
  <section class="card" use:card={{ group: "connections", id: "active" }}>
    <h3>Active connections</h3>
    <div class="big">{list.filter((c) => !listening(c)).length}</div>
    <div class="faint small">{list.filter(listening).length} listening sockets</div>
  </section>
  <section class="card most" use:card={{ group: "connections", id: "most" }}>
    <h3>Most connections</h3>
    <ol>
      {#each byProcess as [name, n] (name)}
        <li><span>{name}</span><b>{n}</b></li>
      {/each}
    </ol>
  </section>
</div>

{#if error}<div class="notice bad" style="margin-bottom: 12px">{error}</div>{/if}
{#if unknownOwners > 0}
  <div class="notice info" style="margin-bottom: 12px">
    {unknownOwners} sockets belong to system services or other users. Linux only shows their owner to administrators.
  </div>
{/if}

<div class="toolbar">
  <div class="seg" role="group" aria-label="Show">
    <button aria-pressed={filter === "all"} onclick={() => (filter = "all")}>All</button>
    <button aria-pressed={filter === "listening"} onclick={() => (filter = "listening")}>Listening</button>
    <button aria-pressed={filter === "active"} onclick={() => (filter = "active")}>Connected</button>
  </div>
  <input class="input" type="search" placeholder="Search program, address or port" bind:value={query} style="width: 300px" aria-label="Search connections" />
</div>

<div class="table-wrap">
  <table class="data">
    <thead>
      <tr>
        <th><span>Program</span></th>
        <th><span>Type</span></th>
        <th><span>Local address</span></th>
        <th><span>Remote address</span></th>
        <th><span>Reach</span></th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as c, i (i)}
        <tr>
          <td>
            {#if c.process}<b>{c.process}</b> <span class="faint">· {c.pid}</span>{:else}<span class="faint">system or another user</span>{/if}
          </td>
          <td class="muted">{c.protocol.toUpperCase()} · {c.state.toLowerCase()}</td>
          <td class="mono addr" title="{c.localAddress}:{c.localPort}">{c.localAddress}<span class="faint">:</span>{c.localPort}</td>
          <td class="mono addr" title={listening(c) ? "" : `${c.peerAddress}:${c.peerPort}`}>{#if listening(c)}<span class="faint">—</span>{:else}{c.peerAddress}<span class="faint">:</span>{c.peerPort}{/if}</td>
          <td><span class="badge {exposureClass[c.exposure]}">{exposureText[c.exposure]}</span></td>
          <td class="num">
            {#if c.pid != null}<button class="btn small" onclick={() => end(c)} title="End {c.process}">End</button>{/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if !rows.length}<div class="empty">No connections match.</div>{/if}
</div>

<style>
  .small {
    font-size: 12px;
  }

  .most ol {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    font-size: 12.5px;
    columns: 2;
    column-gap: 18px;
  }

  .most li {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .most span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  td.addr {
    max-width: 230px;
  }

  .toolbar {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 12px;
  }

  .btn.small {
    height: 26px;
    font-size: 12px;
    padding: 0 9px;
  }
</style>
