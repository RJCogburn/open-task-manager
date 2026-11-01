<script lang="ts">
  import Chart from "$lib/Chart.svelte";
  import Gauge from "$lib/Gauge.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { bytes, temp, pct, watts } from "$lib/format";
  import { live } from "$lib/live.svelte";

  const s = $derived(live.sample);
</script>

<PageHeader
  title="GPU"
  subtitle="Load, video memory, temperature and power"
  sources={["nvidiaSmi", "linux", "pciutils"]}
/>

{#if s && s.gpus.length === 0}
  <div class="notice info">
    No graphics card readings found. NVIDIA cards need the NVIDIA driver (which provides nvidia-smi); AMD cards are
    read from the amdgpu driver.
  </div>
{/if}

{#each s?.gpus ?? [] as g (g.id)}
  <section class="card gpu">
    <div class="head">
      <Gauge value={g.util} color="var(--gpu)" size={104} label="{g.name} load" />
      <div class="title">
        <h3>{g.vendor} · read with {g.source}</h3>
        <div class="name">{g.name}</div>
        <div class="stats">
          <div class="stat"><span>Load</span><b>{pct(g.util)}</b></div>
          <div class="stat"><span>Video memory</span><b>{bytes(g.memUsed)} / {bytes(g.memTotal)}</b></div>
          <div class="stat"><span>Temperature</span><b>{temp(g.temp)}</b></div>
          <div class="stat"><span>Power</span><b>{watts(g.power)}{g.powerLimit ? ` / ${watts(g.powerLimit)}` : ""}</b></div>
          <div class="stat"><span>Clock</span><b>{g.clockMhz != null ? `${g.clockMhz.toFixed(0)} MHz` : "—"}</b></div>
          {#if g.fan != null}<div class="stat"><span>Fan</span><b>{pct(g.fan)}</b></div>{/if}
        </div>
      </div>
    </div>
    <div class="charts">
      <div>
        <div class="label muted">Load and video memory</div>
        <Chart
          label="{g.name} load and memory"
          series={[
            { label: "Load", data: live.history(`gpu:${g.id}:util`), color: "var(--gpu)" },
            { label: "Memory", data: live.history(`gpu:${g.id}:mem`), color: "var(--mem)", fill: false },
          ]}
          tick={live.tick}
          height={150}
          format={(v) => pct(v)}
        />
      </div>
      <div>
        <div class="label muted">Temperature</div>
        <Chart
          label="{g.name} temperature"
          series={[{ label: "Temperature", data: live.history(`gpu:${g.id}:temp`), color: "var(--temp)" }]}
          tick={live.tick}
          height={150}
          max={100}
          format={(v) => temp(v)}
        />
      </div>
      <div>
        <div class="label muted">Power</div>
        <Chart
          label="{g.name} power"
          series={[{ label: "Power", data: live.history(`gpu:${g.id}:power`), color: "var(--net)" }]}
          tick={live.tick}
          height={150}
          max={g.powerLimit ?? "auto"}
          format={(v) => watts(v)}
        />
      </div>
    </div>
  </section>
{/each}

<style>
  .gpu + .gpu {
    margin-top: 14px;
  }

  .head {
    display: flex;
    gap: 22px;
    align-items: center;
  }

  .title {
    flex: 1;
    min-width: 0;
  }

  .name {
    font-size: 19px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .head .stats {
    margin-top: 8px;
  }

  .charts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
    margin-top: 18px;
  }

  .label {
    font-size: 12px;
    margin-bottom: 6px;
  }
</style>
