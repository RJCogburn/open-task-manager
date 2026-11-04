<script lang="ts">
  import { getContext } from "svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, parseError, type SlowdownAction, type SlowdownReport } from "$lib/api";
  import { toolOn } from "$lib/settings.svelte";
  import { tools } from "../registry";

  const go = getContext<(id: string) => void>("otm.go");

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
    if (!problems.length) return "Nothing is slowing your system down right now.";
    const n = problems.length;
    return bad
      ? `${n} ${n === 1 ? "thing is" : "things are"} slowing your system down.`
      : `${n} ${n === 1 ? "thing" : "things"} could slow your system down.`;
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
  // A link only to pages that are switched on.
  const pageLabel = (id: string | null) => (id && toolOn(id) ? tools.find((t) => t.id === id)?.label : undefined);
</script>

<PageHeader title="Slowdown check" subtitle="What could be making your computer slow, checked just now">
  {#snippet actions()}
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

  {#if problems.length}
    <div class="list">
      {#each problems as c (key(c))}
        {@const isOpen = open[key(c)] ?? false}
        <section class="card item {c.status}">
          <button class="head" aria-expanded={isOpen} onclick={() => (open[key(c)] = !isOpen)}>
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
