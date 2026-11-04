<!--
  Recorded history: the CSV logs written while recording is on (Settings →
  Recording), read back as graphs for one day, or part of one, with each
  line's average and peak. Another day can be put beside it to compare,
  such as before and after a change.
-->
<script lang="ts">
  import { getContext } from "svelte";
  import Chart, { type Cursor } from "$lib/Chart.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, type LogConfig, type LogDay, type LogGroup, type LogSeries, type LogView } from "$lib/api";
  import { bytes, ghz, netRate, pct, rate, temp, watts } from "$lib/format";

  const go = getContext<(id: string) => void>("otm.go");

  const SLICES = 360;
  const COLORS = ["var(--cpu)", "var(--mem)", "var(--disk)", "var(--net)", "var(--gpu)", "var(--temp)", "var(--accent)", "var(--warn)"];

  let days = $state<LogDay[] | null>(null);
  let config = $state<LogConfig | null>(null);
  let folder = $state("");
  let day = $state("");
  let other = $state("");
  let from = $state("00:00");
  let to = $state("23:59");
  let view = $state<LogView | null>(null);
  let compare = $state<LogView | null>(null);
  let error = $state("");
  let loading = $state(false);
  let tick = $state(0);
  let at = $state<number | null>(null);

  async function refresh() {
    const status = await api.loggingGet();
    config = status.config;
    folder = status.folder;
    days = await api.logDays();
    if (!days.some((d) => d.day === day)) day = days[0]?.day ?? "";
    if (other && !days.some((d) => d.day === other)) other = "";
  }
  refresh();

  /** "14:05" into seconds after midnight; `end` counts the whole minute. */
  function secs(clock: string, end = false): number {
    const [h, m] = clock.split(":").map(Number);
    return Math.min(86_400, (h || 0) * 3600 + (m || 0) * 60 + (end ? 60 : 0));
  }

  function clock(s: number, seconds = false): string {
    const h = Math.floor(s / 3600) % 24;
    const m = Math.floor((s % 3600) / 60);
    const text = `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
    return seconds ? `${text}:${String(Math.floor(s % 60)).padStart(2, "0")}` : text;
  }

  // Reload whenever the day, the window or the day to compare changes.
  $effect(() => {
    const d = day;
    const o = other;
    const f = secs(from);
    const t = Math.max(f + 60, secs(to, true));
    if (!d) return;
    let stale = false;
    loading = true;
    error = "";
    Promise.all([api.logRead(d, f, t, SLICES), o ? api.logRead(o, f, t, SLICES).catch(() => null) : null])
      .then(([v, c]) => {
        if (stale) return;
        view = v;
        compare = c;
        at = null;
        tick++;
      })
      .catch((e) => {
        if (stale) return;
        view = null;
        error = String(e);
      })
      .finally(() => {
        if (!stale) loading = false;
      });
    return () => (stale = true);
  });

  const cursor: Cursor = {
    get ago() {
      return at;
    },
    move(ago) {
      at = ago;
    },
    label(ago) {
      if (!view) return "";
      const n = view.groups[0]?.series[0]?.values.length ?? 1;
      const start = view.from + (n - 1 - ago) * view.slice;
      return view.slice >= 60 ? `${clock(start)}–${clock(start + view.slice)}, average` : clock(start, true);
    },
  };

  function format(unit: LogGroup["unit"], id: string): (v: number) => string {
    switch (unit) {
      case "percent":
        return (v) => pct(v, 0);
      case "mhz":
        return ghz;
      case "load":
        return (v) => v.toFixed(2);
      case "bytes":
        return id === "network" ? netRate : rate;
      case "celsius":
        return temp;
      case "watts":
        return watts;
    }
  }

  /** The same line on the day being compared. */
  function match(g: LogGroup, s: LogSeries): LogSeries | null {
    return compare?.groups.find((x) => x.id === g.id)?.series.find((x) => x.label === s.label) ?? null;
  }

  /** How the shown day differs from the compared one: points for percentages, a share for the rest. */
  function change(unit: LogGroup["unit"], a: number | null, b: number | null): { text: string; up: boolean } | null {
    if (a == null || b == null) return null;
    if (unit === "percent") {
      const d = a - b;
      return Math.abs(d) < 0.05 ? { text: "same", up: false } : { text: `${d > 0 ? "+" : "−"}${Math.abs(d).toFixed(1)} pts`, up: d > 0 };
    }
    if (b === 0) return null;
    const d = ((a - b) / Math.abs(b)) * 100;
    return Math.abs(d) < 0.5 ? { text: "same", up: false } : { text: `${d > 0 ? "+" : "−"}${Math.abs(d).toFixed(0)}%`, up: d > 0 };
  }

  /** Narrows the window to half an hour around a moment. */
  function around(s: number) {
    from = clock(Math.max(0, s - 900));
    to = clock(Math.min(86_340, s + 900));
  }

  function wholeDay() {
    from = "00:00";
    to = "23:59";
  }

  const zoomed = $derived(from !== "00:00" || to !== "23:59");

  async function startRecording() {
    if (!config) return;
    await api.loggingSet({ ...config, enabled: true });
    await refresh();
  }

  const longDay = (d: string) =>
    new Date(`${d}T12:00:00`).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric", year: "numeric" });
  const kinds = $derived(days?.find((d) => d.day === day)?.kinds ?? []);
</script>

<PageHeader title="History" subtitle="Readings recorded while you weren't watching, read back as graphs." />

{#if days == null}
  <p class="muted">Looking for recordings…</p>
{:else if !days.length}
  <section class="card empty">
    <h3>Nothing recorded yet</h3>
    {#if config?.enabled}
      <p>Recording is on. The first readings are written within {config.every} seconds; check back in a few minutes for something to look at.</p>
      <div class="row"><button class="btn primary" onclick={refresh}>Look again</button></div>
    {:else}
      <p>
        Open Task Manager can record the processor, memory, disks, network, graphics and temperatures to files on this
        computer every few seconds, even while its window is closed to the tray. This page reads them back, so you can see
        what happened while you weren't watching, or compare two days, such as before and after a change.
      </p>
      <p class="faint">Nothing leaves your computer. Old recordings are deleted after {config?.keepDays ?? 14} days.</p>
      <div class="row">
        <button class="btn primary" onclick={startRecording}>Start recording</button>
        <button class="btn" onclick={() => go("settings")}>Recording settings</button>
      </div>
    {/if}
  </section>
{:else}
  <section class="card controls">
    <label>
      <span>Day</span>
      <select class="input" bind:value={day}>
        {#each days as d (d.day)}<option value={d.day}>{longDay(d.day)}</option>{/each}
      </select>
    </label>
    <label>
      <span>From</span>
      <input class="input" type="time" bind:value={from} />
    </label>
    <label>
      <span>To</span>
      <input class="input" type="time" bind:value={to} />
    </label>
    {#if zoomed}<button class="btn" onclick={wholeDay}>Whole day</button>{/if}
    <label>
      <span>Compare with</span>
      <select class="input" bind:value={other}>
        <option value="">Nothing</option>
        {#each days.filter((d) => d.day !== day) as d (d.day)}<option value={d.day}>{longDay(d.day)}</option>{/each}
      </select>
    </label>
    <div class="status faint">
      {#if view}
        {view.rows.toLocaleString()} readings{view.every ? `, one every ${view.every} s` : ""}{view.slice > (view.every ?? 0)
          ? ` · each point is the average of ${view.slice >= 60 ? `${Math.round(view.slice / 60)} min` : `${view.slice} s`}`
          : ""}
      {/if}
      {#if loading}· loading…{/if}
      <br />
      {#if config?.enabled}Recording every {config.every} s{:else}Recording is off{/if} · kept {config?.keepDays} days ·
      <button class="link" onclick={() => api.openFolder(folder)}>Open folder</button> ·
      <button class="link" onclick={() => go("settings")}>Settings</button>
    </div>
  </section>

  {#if error}
    <section class="card"><p class="muted">{error}</p></section>
  {:else if view}
    {#if !view.groups.length}
      <section class="card"><p class="muted">Nothing was recorded between {from} and {to} on {longDay(view.day)}.</p></section>
    {/if}
    {#if compare === null && other}
      <p class="faint">Nothing was recorded between {from} and {to} on {longDay(other)} to compare with.</p>
    {/if}
    {#each view.groups as g (g.id)}
      {@const fmt = format(g.unit, g.id)}
      <section class="card">
        <h3>{g.title}</h3>
        <Chart
          label="History: {g.title}"
          series={g.series.map((s, i) => ({
            label: s.label,
            data: s.values.map((v) => v ?? NaN),
            color: COLORS[i % COLORS.length],
            fill: g.series.length < 3,
          }))}
          slots={g.series[0].values.length}
          {cursor}
          {tick}
          height={180}
          max={g.unit === "percent" ? 100 : "auto"}
          format={fmt}
        />
        <div class="axis faint" aria-hidden="true">
          <span>{clock(view.from)}</span><span>{clock((view.from + view.to) / 2)}</span><span>{clock(view.to)}</span>
        </div>
        <table class="data">
          <thead>
            <tr>
              <th><span>Line</span></th>
              <th class="num"><span>Average</span></th>
              <th class="num"><span>Peak</span></th>
              <th><span>Peak at</span></th>
              {#if compare}
                <th class="num"><span>Average, {longDay(compare.day)}</span></th>
                <th class="num"><span>Peak, {longDay(compare.day)}</span></th>
                <th class="num"><span>Change</span></th>
              {/if}
            </tr>
          </thead>
          <tbody>
            {#each g.series as s, i (s.label)}
              {@const m = match(g, s)}
              {@const c = change(g.unit, s.avg, m?.avg ?? null)}
              <tr>
                <td><i class="swatch" style:background={COLORS[i % COLORS.length]}></i>{s.label}</td>
                <td class="num">{s.avg == null ? "—" : fmt(s.avg)}</td>
                <td class="num">{s.max == null ? "—" : fmt(s.max)}</td>
                <td>
                  {#if s.maxAt != null}
                    <button class="link" title="Show half an hour around this" onclick={() => around(s.maxAt!)}>{clock(s.maxAt, true)}</button>
                  {/if}
                </td>
                {#if compare}
                  <td class="num">{m?.avg == null ? "—" : fmt(m.avg)}</td>
                  <td class="num">{m?.max == null ? "—" : fmt(m.max)}</td>
                  <td class="num" class:up={c?.up} class:down={c && !c.up && c.text !== "same"}>{c?.text ?? "—"}</td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/each}
    {#if view.groups.length && kinds.length < 6}
      <p class="faint">
        Only {kinds.join(", ")} {kinds.length === 1 ? "was" : "were"} recorded on this day. Choose what to record in
        <button class="link" onclick={() => go("settings")}>Settings</button>.
      </p>
    {/if}
  {/if}
{/if}

<style>
  .card {
    margin-bottom: 16px;
  }

  .empty p {
    max-width: 70ch;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 12px 16px;
  }

  .controls label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }

  .status {
    flex: 1 1 260px;
    text-align: right;
    font-size: 12px;
    line-height: 1.6;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .axis {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    margin: 4px 2px 10px;
  }

  .swatch {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 8px;
  }

  td.up {
    color: var(--warn);
  }

  td.down {
    color: var(--ok);
  }
</style>
