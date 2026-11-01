<script lang="ts">
  import Chart from "../Chart.svelte";
  import PageHeader from "../PageHeader.svelte";
  import { bits, bytes } from "../format";
  import { live } from "../live.svelte";

  const s = $derived(live.sample);
  let showIdle = $state(false);
  const nets = $derived(s?.nets.filter((n) => showIdle || n.rxTotal + n.txTotal > 0) ?? []);
  const hidden = $derived((s?.nets.length ?? 0) - nets.length);
</script>

<PageHeader title="Network" subtitle="Traffic on each network interface" sources={["sysinfo", "linux"]}>
  {#snippet actions()}
    {#if hidden > 0 || showIdle}
      <label class="row muted" style="font-size: 13px">
        <input type="checkbox" bind:checked={showIdle} /> Show unused interfaces
      </label>
    {/if}
  {/snippet}
</PageHeader>

{#if s}
  <div class="grid" style="--min: 420px">
    {#each nets as n (n.name)}
      <section class="card">
        <div class="spread">
          <div>
            <h3>Interface</h3>
            <div class="title">{n.name}</div>
          </div>
          <div class="now">
            <span style:color="var(--net)">↓ {bits(n.rx)}</span>
            <span style:color="var(--gpu)">↑ {bits(n.tx)}</span>
          </div>
        </div>
        <div style="margin-top: 12px">
          <Chart
            label="{n.name} traffic"
            series={[
              { label: "Download", data: live.history(`net:${n.name}:rx`), color: "var(--net)" },
              { label: "Upload", data: live.history(`net:${n.name}:tx`), color: "var(--gpu)" },
            ]}
            tick={live.tick}
            height={150}
            max="auto"
            format={bits}
          />
        </div>
        <div class="stats">
          <div class="stat"><span>Received since boot</span><b>{bytes(n.rxTotal)}</b></div>
          <div class="stat"><span>Sent since boot</span><b>{bytes(n.txTotal)}</b></div>
        </div>
      </section>
    {:else}
      <div class="empty">No network traffic yet.</div>
    {/each}
  </div>
{/if}

<style>
  .title {
    font-size: 16px;
    font-weight: 600;
  }

  .now {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-weight: 650;
    font-size: 15px;
  }
</style>
