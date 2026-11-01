<script lang="ts">
  import { board, card } from "$lib/arrange.svelte";
  import Chart from "$lib/Chart.svelte";
  import Gauge from "$lib/Gauge.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, type Proc } from "$lib/api";
  import { bytes, temp, duration, ghz, pct, rate, netRate, watts } from "$lib/format";
  import { live, poll } from "$lib/live.svelte";

  const s = $derived(live.sample);
  const info = $derived(live.info);
  const freq = $derived(live.history("freq").at(-1));
  const nets = $derived(s?.nets.filter((n) => n.rxTotal + n.txTotal > 0) ?? []);

  let top = $state<Proc[]>([]);
  poll(async () => {
    const list = await api.processes();
    top = list.sort((a, b) => b.cpu - a.cpu).slice(0, 6);
  }, () => Math.max(2000, live.settings.intervalMs));
</script>

<PageHeader
  title="Overview"
  subtitle={info ? `${info.os} · Linux ${info.kernel} · up ${duration(s?.uptime ?? 0)}` : ""}
/>

{#if !s}
  <div class="empty"><span class="spinner"></span> Collecting the first readings…</div>
{:else}
  <div use:board={{ group: "overview", min: 330 }}>
    <section class="card tile" use:card={{ group: "overview", id: "cpu" }}>
      <div class="head">
        <Gauge value={s.cpu} color="var(--cpu)" label="CPU usage" />
        <div>
          <h3>CPU</h3>
          <div class="sub" title={info?.cpuBrand}>{info?.cpuBrand}</div>
          <div class="facts">
            <span>{ghz(freq)}</span><span>{temp(s.cpuTemp)}</span><span>{s.cores.length} threads</span>
          </div>
        </div>
      </div>
      <Chart label="CPU usage" series={[{ label: "CPU", data: live.history("cpu"), color: "var(--cpu)" }]} tick={live.tick} height={90} format={(v) => pct(v)} />
    </section>

    <section class="card tile" use:card={{ group: "overview", id: "memory" }}>
      <div class="head">
        <Gauge value={(s.memUsed / s.memTotal) * 100} color="var(--mem)" label="Memory in use" />
        <div>
          <h3>Memory</h3>
          <div class="sub">{bytes(s.memUsed)} of {bytes(s.memTotal)}</div>
          <div class="facts">
            <span>Swap {bytes(s.swapUsed)} / {bytes(s.swapTotal)}</span>
          </div>
        </div>
      </div>
      <Chart label="Memory in use" series={[{ label: "Memory", data: live.history("mem"), color: "var(--mem)" }]} tick={live.tick} height={90} format={(v) => pct(v)} />
    </section>

    {#each s.gpus as g (g.id)}
      <section class="card tile" use:card={{ group: "overview", id: `gpu:${g.id}` }}>
        <div class="head">
          <Gauge value={g.util} color="var(--gpu)" label="{g.name} load" />
          <div>
            <h3>GPU · {g.vendor}</h3>
            <div class="sub" title={g.name}>{g.name}</div>
            <div class="facts">
              <span>{bytes(g.memUsed)} / {bytes(g.memTotal)}</span><span>{temp(g.temp)}</span><span>{watts(g.power)}</span>
            </div>
          </div>
        </div>
        <Chart label="{g.name} load" series={[{ label: "Load", data: live.history(`gpu:${g.id}:util`), color: "var(--gpu)" }]} tick={live.tick} height={90} format={(v) => pct(v)} />
      </section>
    {/each}

    {#each s.disks as d (d.name)}
      <section class="card tile" use:card={{ group: "overview", id: `disk:${d.name}` }}>
        <div class="head">
          <Gauge value={d.busy} color="var(--disk)" label="{d.label} busy time" />
          <div>
            <h3>Disk</h3>
            <div class="sub" title={d.label}>{d.label}</div>
            <div class="facts"><span>R {rate(d.read)}</span><span>W {rate(d.write)}</span></div>
          </div>
        </div>
        <Chart
          label="{d.label} throughput"
          series={[
            { label: "Read", data: live.history(`disk:${d.name}:read`), color: "var(--disk)" },
            { label: "Write", data: live.history(`disk:${d.name}:write`), color: "var(--cpu)", fill: false },
          ]}
          tick={live.tick}
          height={90}
          max="auto"
          format={rate}
        />
      </section>
    {/each}

    {#each nets as n (n.name)}
      <section class="card tile" use:card={{ group: "overview", id: `net:${n.name}` }}>
        <div class="head simple">
          <div>
            <h3>Network · {n.name}</h3>
            <div class="facts big-facts">
              <span><i style:background="var(--net)"></i>↓ {netRate(n.rx)}</span>
              <span><i style:background="var(--gpu)"></i>↑ {netRate(n.tx)}</span>
            </div>
          </div>
        </div>
        <Chart
          label="{n.name} traffic"
          series={[
            { label: "Down", data: live.history(`net:${n.name}:rx`), color: "var(--net)" },
            { label: "Up", data: live.history(`net:${n.name}:tx`), color: "var(--gpu)", fill: false },
          ]}
          tick={live.tick}
          height={90}
          max="auto"
          format={netRate}
        />
      </section>
    {/each}

    <section class="card" use:card={{ group: "overview", id: "processes" }}>
      <h3>Busiest processes</h3>
      <ol class="top">
        {#each top as p (p.pid)}
          <li>
            <span class="name" title={p.command}>{p.name}</span>
            <span class="bar"><i style:width="{Math.min(100, p.cpu)}%"></i></span>
            <b>{pct(p.cpu, 1)}</b>
            <span class="faint">{bytes(p.memory)}</span>
          </li>
        {/each}
      </ol>
      <div class="stats">
        <div class="stat"><span>Processes</span><b>{s.processes.toLocaleString()}</b></div>
        <div class="stat"><span>Threads</span><b>{s.threads.toLocaleString()}</b></div>
        <div class="stat"><span>Load average</span><b>{s.load.map((l) => l.toFixed(2)).join(" ")}</b></div>
      </div>
    </section>
  </div>
{/if}

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 16px;
    min-width: 0;
  }

  .head > div {
    min-width: 0;
  }

  .sub {
    font-size: 15px;
    font-weight: 600;
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 12.5px;
    color: var(--muted);
    margin-top: 4px;
  }

  .big-facts {
    font-size: 18px;
    font-weight: 650;
    color: var(--text);
  }

  .big-facts i {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 6px;
    vertical-align: middle;
  }

  .top {
    list-style: none;
    padding: 0;
    margin: 12px 0 0;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .top li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 70px 52px 64px;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }

  .top .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 550;
  }

  .top b {
    text-align: right;
    font-weight: 600;
  }

  .top .faint {
    text-align: right;
    font-size: 12px;
  }
</style>
