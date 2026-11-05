<script lang="ts">
  import { getContext, onDestroy } from "svelte";
  import Chart, { type Cursor } from "$lib/Chart.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, parseError, type SlowdownAction, type SlowdownCheck, type SlowdownReport, type WatchReport } from "$lib/api";
  import { toolOn } from "$lib/settings.svelte";

  const go = getContext<(id: string) => void>("otm.go");
  // Through App rather than the tool registry: the registry imports this page.
  const toolLabel = getContext<(id: string) => string | undefined>("otm.label");

  let report = $state<SlowdownReport | null>(null);
  let running = $state(false);
  let error = $state("");
  // Which findings are open, by id and title.
  let open = $state<Record<string, boolean>>({});
  let done = $state<{ text: string; bad?: boolean } | null>(null);
  let fixing = $state(false);
  const key = (c: { id: string; title: string }) => c.id + c.title;

  async function fix(a: SlowdownAction) {
    const password = a.admin ? "\nIt will ask for your password." : "";
    if (!confirm(`${a.explain}\n\nIt runs: ${a.command}${password}`)) return;
    fixing = true;
    done = null;
    try {
      done = { text: await api.slowdownFix(a.fix, a.target) };
      await run();
    } catch (e) {
      done = { text: parseError(e).message, bad: true };
    } finally {
      fixing = false;
    }
  }

  async function run() {
    running = true;
    error = "";
    try {
      report = await api.slowdownCheck();
    } catch (e) {
      error = parseError(e).message;
    } finally {
      running = false;
    }
  }
  run();

  const problems = $derived(report?.checks.filter((c) => c.status === "bad" || c.status === "warn") ?? []);
  const fine = $derived(report?.checks.filter((c) => c.status === "ok" || c.status === "skipped") ?? []);
  const verdict = $derived.by(() => {
    if (!report) return "";
    const bad = problems.filter((c) => c.status === "bad").length;
    if (!problems.length) return "No performance degradation detected.";
    const n = problems.length;
    return bad
      ? `Performance degradation detected: ${n} ${n === 1 ? "issue" : "issues"} found.`
      : `${n} possible performance ${n === 1 ? "issue" : "issues"} found.`;
  });
  const pressure = $derived(
    report?.pressure
      ? [
          { label: "Waiting for the processor", value: report.pressure.cpu, color: "var(--cpu)" },
          { label: "Waiting for memory", value: report.pressure.memory, color: "var(--mem)" },
          { label: "Waiting for the disks", value: report.pressure.io, color: "var(--disk)" },
        ]
      : [],
  );
  // Watching for slowdowns that come and go.
  const WATCH = 30;
  let watching = $state(false);
  let watchStart = $state(0);
  let now = $state(Date.now());
  let watched = $state<WatchReport | null>(null);
  let watchError = $state("");
  let at = $state<number | null>(null);
  const timer = setInterval(() => (now = Date.now()), 250);
  onDestroy(() => {
    clearInterval(timer);
    // Leaving the page stops watching; nothing would show the result.
    if (watching) api.slowdownWatchStop();
  });

  async function watch() {
    watching = true;
    watchStart = Date.now();
    watchError = "";
    try {
      watched = await api.slowdownWatch(WATCH);
      at = null;
    } catch (e) {
      const message = parseError(e).message;
      if (message !== "Stopped.") watchError = message;
    } finally {
      watching = false;
    }
  }

  const left = $derived(Math.max(0, WATCH - Math.floor((now - watchStart) / 1000)));
  const clock = (i: number) => `${Math.floor((i + 1) / 60)}:${String((i + 1) % 60).padStart(2, "0")}`;
  const watchCursor: Cursor = {
    get ago() {
      return at;
    },
    move(ago) {
      at = ago;
    },
    label(ago) {
      const n = watched?.points.length ?? 0;
      return `${clock(n - 1 - ago)} into the watch`;
    },
  };
  const watchSeries = $derived(
    watched
      ? [
          { label: "Waiting for the processor", data: watched.points.map((p) => p.cpuWait ?? NaN), color: "var(--cpu)", fill: false },
          { label: "Waiting for memory", data: watched.points.map((p) => p.memoryWait ?? NaN), color: "var(--mem)", fill: false },
          { label: "Waiting for the disks", data: watched.points.map((p) => p.ioWait ?? NaN), color: "var(--disk)", fill: false },
          { label: "Processor busy", data: watched.points.map((p) => p.cpu), color: "var(--faint)", fill: true },
        ]
      : [],
  );

  // A link only to pages that are switched on.
  const pageLabel = (id: string | null) => (id && toolOn(id) ? toolLabel(id) : undefined);
</script>

<PageHeader title="Slowdown check" subtitle="What could be making your computer slow, checked just now">
  {#snippet actions()}
    {#if watching}
      <button class="btn" onclick={() => api.slowdownWatchStop()}>Stop watching</button>
    {:else}
      <button class="btn" onclick={watch} disabled={running} title="Catch slowdowns that come and go: samples every second for {WATCH} seconds">
        Watch for {WATCH} s
      </button>
    {/if}
    <button class="btn primary" onclick={run} disabled={running}>{running ? "Checking…" : "Check again"}</button>
  {/snippet}
</PageHeader>

{#if error}<div class="notice bad">{error}</div>{/if}
{#if done}<div class="notice {done.bad ? 'bad' : 'info'}" style="margin-bottom: 14px">{done.text}</div>{/if}

{#if !report}
  <div class="empty"><span class="spinner"></span> Checking. This takes a second…</div>
{:else}
  <section class="card verdict" class:clear={!problems.length} class:bad={problems.some((c) => c.status === "bad")}>
    <div class="big">{verdict}</div>
    {#if pressure.length}
      <p class="muted">How much of the last minute programs spent waiting:</p>
      <div class="pressure">
        {#each pressure as p (p.label)}
          <div>
            <div class="spread"><span class="muted">{p.label}</span><b>{p.value.toFixed(0)}%</b></div>
            <div class="bar"><i style:width="{Math.min(100, p.value)}%" style:background={p.color}></i></div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  {#snippet findings(list: SlowdownCheck[], prefix: string)}
    <div class="list">
      {#each list as c (prefix + key(c))}
        {@const isOpen = open[prefix + key(c)] ?? false}
        <section class="card item {c.status}">
          <button class="head" aria-expanded={isOpen} onclick={() => (open[prefix + key(c)] = !isOpen)}>
            <span class="dot" aria-hidden="true"></span>
            <span class="body">
              <h3>{c.title}</h3>
              <span class="detail">{c.detail}</span>
            </span>
            <span class="chev" aria-hidden="true">{isOpen ? "▾" : "▸"}</span>
          </button>
          {#if isOpen}
            <div class="more">
              {#each c.more as para, i (i)}<p>{para}</p>{/each}
              {#each c.actions as a (a.fix + a.target)}
                <div class="action">
                  <button class="btn primary" disabled={fixing} onclick={() => fix(a)}>{a.label}</button>
                  <div>
                    <div class="muted">{a.explain}</div>
                    <code>{a.admin ? "pkexec " : ""}{a.command}</code>
                    {#if a.admin}<span class="faint">· asks for your password</span>{/if}
                  </div>
                </div>
              {/each}
              {#if pageLabel(c.page)}
                <button class="btn" onclick={() => go(c.page!)}>Open {pageLabel(c.page)}</button>
              {/if}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  {/snippet}

  {#if problems.length}
    {@render findings(problems, "")}
  {/if}

  {#if watching}
    <section class="card watching">
      <span class="spinner"></span>
      <div>
        <b>Watching… {left} s left</b>
        <p class="muted">Use your computer as you were when it felt slow; whatever comes and goes in these {WATCH} seconds is caught.</p>
      </div>
    </section>
  {:else if watchError}
    <div class="notice bad">{watchError}</div>
  {/if}

  {#if watched && !watching}
    <section class="card watch">
      <div class="spread">
        <h3>Over {watched.points.length} seconds</h3>
        <span class="faint">from {new Date(watched.started).toLocaleTimeString()}</span>
      </div>
      <Chart
        label="Slowdown watch"
        series={watchSeries}
        slots={watched.points.length}
        cursor={watchCursor}
        tick={watched.started}
        height={150}
        format={(v) => `${v.toFixed(0)}%`}
      />
      <div class="legend faint">
        {#each watchSeries as w (w.label)}<span><i style:background={w.color}></i>{w.label}</span>{/each}
      </div>
    </section>
    {@render findings(watched.checks, "w:")}
  {/if}

  <section class="card">
    <h3>Checked and fine</h3>
    <ul class="fine">
      {#each fine as c (c.id + c.title)}
        <li class={c.status}>
          <span class="dot" aria-hidden="true"></span>
          <div><b>{c.title}</b> <span class="faint">{c.detail}</span></div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .verdict {
    margin-bottom: 14px;
    border-left: 3px solid var(--warn);
  }

  .verdict.bad {
    border-left-color: var(--bad);
  }

  .verdict.clear {
    border-left-color: var(--ok);
  }

  .verdict .big {
    font-size: 22px;
  }

  .verdict p {
    margin: 14px 0 8px;
  }

  .pressure {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 10px 22px;
  }

  .bar {
    height: 6px;
    margin-top: 5px;
    border-radius: 3px;
    background: var(--grid);
    overflow: hidden;
  }

  .bar i {
    display: block;
    height: 100%;
    min-width: 2px;
  }

  .list {
    display: grid;
    gap: 10px;
    margin-bottom: 14px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .head .body {
    flex: 1;
    min-width: 0;
  }

  .item h3 {
    color: var(--text);
    font-size: 14px;
  }

  .detail {
    display: block;
    margin-top: 4px;
    color: var(--muted);
  }

  .chev {
    color: var(--faint);
  }

  .more {
    margin: 12px 0 0 22px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .more p {
    margin: 0 0 10px;
    line-height: 1.55;
  }

  .action {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin: 12px 0;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }

  .action .btn {
    flex: none;
  }

  .action code {
    display: inline-block;
    margin-top: 4px;
    font-size: 12px;
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    margin-top: 5px;
    border-radius: 50%;
    background: var(--faint);
  }

  .bad .dot {
    background: var(--bad);
  }

  .warn .dot {
    background: var(--warn);
  }

  .ok .dot {
    background: var(--ok);
  }

  .watching {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    margin-bottom: 14px;
  }

  .watching p {
    margin: 4px 0 0;
  }

  .watch {
    margin-bottom: 10px;
  }

  .watch h3 {
    margin-bottom: 10px;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin-top: 8px;
    font-size: 12px;
  }

  .legend i {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 6px;
  }

  .fine {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }

  .fine li {
    display: flex;
    gap: 10px;
  }

  .fine li.skipped {
    opacity: 0.7;
  }
</style>
