<script lang="ts">
  import PageHeader from "../PageHeader.svelte";
  import { api, parseError, type Proc } from "../api";
  import { bytes, heat, pct, rate } from "../format";
  import { live, poll } from "../live.svelte";

  type Key = "name" | "pid" | "user" | "cpu" | "memory" | "disk" | "status" | "nice";
  const columns: { key: Key; label: string; num?: boolean; width?: number }[] = [
    { key: "name", label: "Name" },
    { key: "pid", label: "PID", num: true, width: 84 },
    { key: "user", label: "User", width: 110 },
    { key: "cpu", label: "CPU", num: true, width: 84 },
    { key: "memory", label: "Memory", num: true, width: 100 },
    { key: "disk", label: "Disk", num: true, width: 100 },
    { key: "status", label: "Status", width: 100 },
    { key: "nice", label: "Priority", num: true, width: 90 },
  ];

  // Only the rows in view are drawn, so a thousand processes cost the same
  // as twenty. Rows have a fixed height for this.
  const ROW = 34;
  let scrollTop = $state(0);
  let viewHeight = $state(600);

  const priorities = [
    { nice: -15, label: "Highest" },
    { nice: -5, label: "High" },
    { nice: 0, label: "Normal" },
    { nice: 10, label: "Low" },
    { nice: 19, label: "Lowest" },
  ];

  let list = $state<Proc[]>([]);
  let query = $state("");
  let sortKey = $state<Key>("cpu");
  let descending = $state(true);
  let mineOnly = $state(false);
  let showKernel = $state(false);
  let selected = $state<number | null>(null);
  let message = $state<{ text: string; retry?: () => Promise<void>; bad?: boolean } | null>(null);
  let busy = $state(false);
  const me = $derived(live.info?.userName ?? "");

  poll(async () => (list = await api.processes()), () => live.settings.intervalMs);

  const value = (p: Proc, key: Key): string | number =>
    key === "disk" ? p.diskRead + p.diskWrite : key === "nice" ? (p.nice ?? 0) : p[key];

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const filtered = list.filter(
      (p) =>
        (!mineOnly || p.user === me) &&
        (showKernel || !p.kernel) &&
        (!q || p.name.toLowerCase().includes(q) || p.command.toLowerCase().includes(q) || String(p.pid) === q),
    );
    const dir = descending ? -1 : 1;
    return filtered.sort((a, b) => {
      const x = value(a, sortKey);
      const y = value(b, sortKey);
      const c = typeof x === "string" ? x.localeCompare(y as string) : x - (y as number);
      return c * dir || a.pid - b.pid;
    });
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 5));
  const visible = $derived(rows.slice(first, first + Math.ceil(viewHeight / ROW) + 10));

  const chosen = $derived(list.find((p) => p.pid === selected) ?? null);
  const totals = $derived({
    cpu: list.reduce((a, p) => a + p.cpu, 0),
    memory: list.reduce((a, p) => a + p.memory, 0),
    disk: list.reduce((a, p) => a + p.diskRead + p.diskWrite, 0),
  });
  const memTotal = $derived(live.sample?.memTotal ?? 1);

  function sortBy(key: Key) {
    if (sortKey === key) descending = !descending;
    else {
      sortKey = key;
      descending = !["name", "user", "status"].includes(key);
    }
  }

  function priorityLabel(nice: number | null) {
    if (nice == null) return "—";
    if (nice <= -10) return "Highest";
    if (nice < 0) return "High";
    if (nice === 0) return "Normal";
    if (nice < 15) return "Low";
    return "Lowest";
  }

  async function act(what: string, run: () => Promise<void>, asAdmin: () => Promise<void>) {
    if (!chosen) return;
    busy = true;
    message = null;
    try {
      await run();
      message = { text: what };
    } catch (e) {
      const err = parseError(e);
      if (err.kind === "needs-admin") {
        message = {
          text: err.message,
          bad: true,
          retry: async () => {
            try {
              await asAdmin();
              message = { text: what };
            } catch (e2) {
              message = { text: String(e2), bad: true };
            }
          },
        };
      } else {
        message = { text: err.message, bad: true };
      }
    } finally {
      busy = false;
    }
  }

  const endTask = (force: boolean) => {
    const p = chosen!;
    return act(
      `${force ? "Killed" : "Asked to end"} ${p.name} (${p.pid}).`,
      () => api.endProcess(p.pid, force),
      () => api.adminAction(p.pid, force ? "kill" : "end"),
    );
  };

  const setPriority = (nice: number) => {
    const p = chosen!;
    return act(
      `Set ${p.name} to ${priorityLabel(nice)} priority.`,
      () => api.setPriority(p.pid, nice),
      () => api.adminAction(p.pid, "priority", nice),
    );
  };

  function onkeydown(e: KeyboardEvent) {
    if (!rows.length) return;
    const i = rows.findIndex((p) => p.pid === selected);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = rows[Math.max(0, Math.min(rows.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))];
      selected = next.pid;
      const at = rows.indexOf(next) * ROW;
      const wrap = e.currentTarget as HTMLElement;
      const header = 46;
      if (at < wrap.scrollTop) wrap.scrollTop = at;
      else if (at + ROW > wrap.scrollTop + wrap.clientHeight - header) wrap.scrollTop = at + ROW - wrap.clientHeight + header;
    } else if (e.key === "Delete" && chosen) {
      e.preventDefault();
      endTask(e.shiftKey);
    }
  }
</script>

<PageHeader title="Processes" subtitle="{rows.length.toLocaleString()} shown of {list.length.toLocaleString()} running" sources={["sysinfo", "polkit"]} />

<div class="toolbar">
  <input class="input search" type="search" placeholder="Search by name, command or PID" bind:value={query} aria-label="Search processes" />
  <label class="row muted toggle">
    <input type="checkbox" bind:checked={mineOnly} /> Only mine
  </label>
  <label class="row muted toggle" title="kworker, ksoftirqd and other threads the kernel runs for itself">
    <input type="checkbox" bind:checked={showKernel} /> Kernel threads
  </label>
  <div class="spacer"></div>
  {#if chosen}
    <span class="sel" title={chosen.command}>{chosen.name} <span class="faint">· {chosen.pid}</span></span>
    <select
      class="input"
      aria-label="Priority"
      disabled={busy}
      value=""
      onchange={(e) => {
        const v = e.currentTarget.value;
        e.currentTarget.value = "";
        if (v !== "") setPriority(Number(v));
      }}
    >
      <option value="" disabled>Priority: {priorityLabel(chosen.nice)}</option>
      {#each priorities as p (p.nice)}
        <option value={p.nice}>{p.label} (nice {p.nice})</option>
      {/each}
    </select>
    <button class="btn" disabled={busy} onclick={() => endTask(false)} title="Ask the process to close (SIGTERM). Delete">End task</button>
    <button class="btn danger" disabled={busy} onclick={() => endTask(true)} title="Stop it immediately (SIGKILL). Shift+Delete">Kill</button>
  {/if}
</div>

{#if message}
  <div class="notice" class:info={!message.bad} class:bad={message.bad} style="margin-bottom: 12px">
    <span style="flex: 1">{message.text}</span>
    {#if message.retry}
      <button class="btn" onclick={message.retry}>Try as administrator</button>
    {/if}
    <button class="btn" onclick={() => (message = null)} aria-label="Dismiss">✕</button>
  </div>
{/if}

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="table-wrap"
  tabindex="0"
  role="region"
  aria-label="Process list"
  {onkeydown}
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  bind:clientHeight={viewHeight}
>
  <table class="data" aria-rowcount={rows.length}>
    <colgroup>
      {#each columns as c (c.key)}<col style:width={c.width ? `${c.width}px` : undefined} />{/each}
    </colgroup>
    <thead>
      <tr>
        {#each columns as c (c.key)}
          <th class:num={c.num} aria-sort={sortKey === c.key ? (descending ? "descending" : "ascending") : undefined}>
            <button onclick={() => sortBy(c.key)}>
              <span class="col">
                {c.label}
                {#if c.key === "cpu"}<small>{pct(totals.cpu)}</small>{/if}
                {#if c.key === "memory"}<small>{bytes(totals.memory)}</small>{/if}
                {#if c.key === "disk"}<small>{rate(totals.disk)}</small>{/if}
              </span>
              {#if sortKey === c.key}<span aria-hidden="true">{descending ? "↓" : "↑"}</span>{/if}
            </button>
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#if first > 0}<tr class="spacer" style:height="{first * ROW}px" aria-hidden="true"></tr>{/if}
      {#each visible as p, i (p.pid)}
        <tr aria-rowindex={first + i + 1} aria-selected={selected === p.pid} onclick={() => (selected = p.pid)}>
          <td class="name" title={p.command || p.name}>{p.name}</td>
          <td class="num faint">{p.pid}</td>
          <td class="muted">{p.user}</td>
          <td class="num" style:background={heat(p.cpu / 25)}>{pct(p.cpu, 1)}</td>
          <td class="num" style:background={heat((p.memory / memTotal) * 8, "var(--mem)")}>{bytes(p.memory)}</td>
          <td class="num" style:background={heat((p.diskRead + p.diskWrite) / 50e6, "var(--disk)")}>
            {p.diskRead + p.diskWrite > 0 ? rate(p.diskRead + p.diskWrite) : "0"}
          </td>
          <td class="muted">{p.status}</td>
          <td class="num muted">{priorityLabel(p.nice)}</td>
        </tr>
      {/each}
      {#if first + visible.length < rows.length}
        <tr class="spacer" style:height="{(rows.length - first - visible.length) * ROW}px" aria-hidden="true"></tr>
      {/if}
    </tbody>
  </table>
  {#if !rows.length}<div class="empty">No processes match.</div>{/if}
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
    flex-wrap: wrap;
  }

  .search {
    width: 320px;
  }

  .toggle {
    font-size: 13px;
  }

  .spacer {
    flex: 1;
  }

  .sel {
    font-weight: 600;
    max-width: 240px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .table-wrap {
    max-height: calc(100vh - 210px);
  }

  .table-wrap:focus-visible {
    outline: 2px solid var(--accent);
  }

  .col {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }

  th.num .col {
    align-items: flex-end;
  }

  small {
    font-size: 11px;
    font-weight: 500;
    color: var(--faint);
  }

  table {
    table-layout: fixed;
  }

  tbody tr {
    height: 34px;
  }

  tbody td {
    padding-top: 0;
    padding-bottom: 0;
    height: 34px;
    max-width: none;
  }

  tr.spacer {
    padding: 0;
    border: 0;
  }

  td.name {
    font-weight: 550;
  }
</style>
