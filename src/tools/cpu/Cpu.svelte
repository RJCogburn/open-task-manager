<script lang="ts">
  import Chart from "$lib/Chart.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { temp, duration, ghz, pct } from "$lib/format";
  import { live } from "$lib/live.svelte";

  const s = $derived(live.sample);
  const info = $derived(live.info);
  const freqs = $derived(s?.freqs.filter((f) => f > 0) ?? []);
  let view = $state<"cores" | "total">("cores");
</script>

<PageHeader title="CPU" subtitle={info?.cpuBrand} sources={["sysinfo", "linux"]}>
  {#snippet actions()}
    <div class="seg" role="group" aria-label="Graph view">
      <button aria-pressed={view === "cores"} onclick={() => (view = "cores")}>Per thread</button>
      <button aria-pressed={view === "total"} onclick={() => (view = "total")}>Total</button>
    </div>
  {/snippet}
</PageHeader>

{#if s}
  <section class="card">
    <div class="spread">
      <h3>Utilization over the last {(live.settings.intervalMs * live.settings.chart.history) / 1000} seconds</h3>
      <span class="big" style:color="var(--cpu)">{pct(s.cpu)}</span>
    </div>
    <div style="margin-top: 12px">
      <Chart
        label="CPU usage"
        series={[
          { label: "CPU", data: live.history("cpu"), color: "var(--cpu)" },
        ]}
        tick={live.tick}
        height={view === "total" ? 280 : 170}
        format={(v) => pct(v, 1)}
      />
    </div>
    <div class="stats">
      <div class="stat"><span>Utilization</span><b>{pct(s.cpu, 1)}</b></div>
      <div class="stat"><span>Average speed</span><b>{ghz(freqs.length ? freqs.reduce((a, b) => a + b, 0) / freqs.length : null)}</b></div>
      <div class="stat"><span>Fastest core</span><b>{ghz(freqs.length ? Math.max(...freqs) : null)}</b></div>
      <div class="stat"><span>Temperature</span><b>{temp(s.cpuTemp)}</b></div>
      <div class="stat"><span>Processes</span><b>{s.processes.toLocaleString()}</b></div>
      <div class="stat"><span>Threads</span><b>{s.threads.toLocaleString()}</b></div>
      <div class="stat"><span>Load (1, 5, 15 min)</span><b>{s.load.map((l) => l.toFixed(2)).join(" · ")}</b></div>
      <div class="stat"><span>Up time</span><b>{duration(s.uptime)}</b></div>
      <div class="stat"><span>Cores / threads</span><b>{info?.physicalCores} / {info?.logicalCores}</b></div>
    </div>
  </section>

  {#if view === "cores"}
    <div class="cores">
      {#each s.cores as usage, i (i)}
        <div class="core">
          <div class="spread">
            <span class="muted">CPU {i}</span>
            <span><b>{pct(usage)}</b> <span class="faint">{ghz(s.freqs[i])}</span></span>
          </div>
          <Chart
            label="CPU {i} usage"
            series={[{ label: `CPU ${i}`, data: live.history(`core:${i}`), color: "var(--cpu)" }]}
            tick={live.tick}
            height={54}
            grid={false}
            format={(v) => pct(v)}
          />
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .cores {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 10px;
    margin-top: 14px;
  }

  .core {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 9px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
  }
</style>
