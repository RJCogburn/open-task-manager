<script lang="ts">
  import { board, card } from "$lib/arrange.svelte";
  import Chart from "$lib/Chart.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, type Proc } from "$lib/api";
  import { bytes, pct } from "$lib/format";
  import { live, poll } from "$lib/live.svelte";

  const s = $derived(live.sample);
  const used = $derived(s ? (s.memUsed / s.memTotal) * 100 : 0);

  let top = $state<Proc[]>([]);
  poll(async () => {
    top = (await api.processes()).sort((a, b) => b.memory - a.memory).slice(0, 10);
  }, () => Math.max(2000, live.settings.intervalMs));
</script>

<PageHeader title="Memory" subtitle={s ? `${bytes(s.memTotal)} installed` : ""} />

{#if s}
  <div use:board={{ group: "memory", min: 900 }}>
  <section class="card" use:card={{ group: "memory", id: "usage" }}>
    <div class="spread">
      <h3>Memory in use</h3>
      <span class="big" style:color="var(--mem)">{bytes(s.memUsed)}</span>
    </div>
    <div style="margin-top: 12px">
      <Chart
        label="Memory and swap use"
        series={[
          { label: "Memory", data: live.history("mem"), color: "var(--mem)" },
          { label: "Swap", data: live.history("swap"), color: "var(--net)", fill: false },
        ]}
        tick={live.tick}
        height={240}
        format={(v) => pct(v, 1)}
      />
    </div>

    <div class="composition" aria-label="Memory composition">
      <div class="part" style:flex={s.memUsed} style:background="var(--mem)"></div>
      <div class="part" style:flex={s.memTotal - s.memUsed} style:background="var(--grid)"></div>
    </div>
    <div class="legend">
      <span><i style:background="var(--mem)"></i>In use {bytes(s.memUsed)}</span>
      <span><i style:background="var(--grid)"></i>Available {bytes(s.memTotal - s.memUsed)}</span>
    </div>

    <div class="stats">
      <div class="stat"><span>In use</span><b>{pct(used, 1)}</b></div>
      <div class="stat"><span>Available</span><b>{bytes(s.memTotal - s.memUsed)}</b></div>
      <div class="stat"><span>Total</span><b>{bytes(s.memTotal)}</b></div>
      <div class="stat"><span>Swap in use</span><b>{bytes(s.swapUsed)} / {bytes(s.swapTotal)}</b></div>
    </div>
  </section>

  <section class="card" use:card={{ group: "memory", id: "top" }}>
    <h3>Using the most memory</h3>
    <ol class="list">
      {#each top as p (p.pid)}
        <li>
          <span class="name" title={p.command}>{p.name} <span class="faint">· {p.pid}</span></span>
          <span class="bar" style="--c: var(--mem)"><i style:width="{(p.memory / s.memTotal) * 100 * 4}%"></i></span>
          <b>{bytes(p.memory)}</b>
        </li>
      {/each}
    </ol>
  </section>
  </div>
{/if}

<style>
  .composition {
    display: flex;
    height: 12px;
    border-radius: 6px;
    overflow: hidden;
    margin-top: 16px;
    gap: 2px;
  }

  .legend {
    display: flex;
    gap: 18px;
    font-size: 12.5px;
    color: var(--muted);
    margin-top: 8px;
  }

  .legend i {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 6px;
  }

  .list {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .list li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 160px 80px;
    gap: 12px;
    align-items: center;
    font-size: 13px;
  }

  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 550;
  }

  .list b {
    text-align: right;
  }
</style>
