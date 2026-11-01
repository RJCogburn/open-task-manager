<script lang="ts">
  import Chart from "../Chart.svelte";
  import PageHeader from "../PageHeader.svelte";
  import { api, type Mount } from "../api";
  import { bytes, pct, rate } from "../format";
  import { live, poll } from "../live.svelte";

  const s = $derived(live.sample);
  let mounts = $state<Mount[]>([]);
  poll(async () => (mounts = await api.mounts()), () => 10_000);
</script>

<PageHeader title="Disks" subtitle="Read and write speed, and how busy each drive is" sources={["linux", "sysinfo"]} />

{#if s}
  <div class="grid" style="--min: 420px">
    {#each s.disks as d (d.name)}
      <section class="card">
        <div class="spread">
          <div>
            <h3>{d.name}</h3>
            <div class="title">{d.label}</div>
          </div>
          <span class="badge" class:warn={d.busy > 80}>{pct(d.busy)} busy</span>
        </div>
        <div style="margin-top: 12px">
          <Chart
            label="{d.label} read and write speed"
            series={[
              { label: "Read", data: live.history(`disk:${d.name}:read`), color: "var(--disk)" },
              { label: "Write", data: live.history(`disk:${d.name}:write`), color: "var(--cpu)" },
            ]}
            tick={live.tick}
            height={150}
            max="auto"
            format={rate}
          />
        </div>
        <div class="stats">
          <div class="stat"><span><i style:background="var(--disk)"></i>Read</span><b>{rate(d.read)}</b></div>
          <div class="stat"><span><i style:background="var(--cpu)"></i>Write</span><b>{rate(d.write)}</b></div>
          <div class="stat"><span>Active time</span><b>{pct(d.busy)}</b></div>
        </div>
      </section>
    {/each}
  </div>
{/if}

<h2 class="section">Space on each drive</h2>
<div class="table-wrap">
  <table class="data">
    <thead>
      <tr>
        <th><span>Mounted at</span></th>
        <th><span>Device</span></th>
        <th><span>Type</span></th>
        <th style="width: 34%"><span>Used</span></th>
        <th class="num"><span>Free</span></th>
        <th class="num"><span>Size</span></th>
      </tr>
    </thead>
    <tbody>
      {#each mounts as m (m.mountPoint)}
        {@const used = m.total ? ((m.total - m.available) / m.total) * 100 : 0}
        <tr>
          <td><b>{m.mountPoint}</b> {#if m.removable}<span class="badge">removable</span>{/if}</td>
          <td class="muted">{m.device}</td>
          <td class="muted">{m.fileSystem}</td>
          <td>
            <div class="row">
              <div class="bar" style="flex: 1; --c: {used > 90 ? 'var(--bad)' : used > 75 ? 'var(--warn)' : 'var(--disk)'}">
                <i style:width="{used}%"></i>
              </div>
              <span style="width: 44px; text-align: right">{pct(used)}</span>
            </div>
          </td>
          <td class="num">{bytes(m.available)}</td>
          <td class="num">{bytes(m.total)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .title {
    font-size: 16px;
    font-weight: 600;
  }

  .stat i {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 5px;
  }

  .section {
    font-size: 15px;
    margin: 26px 0 10px;
  }
</style>
