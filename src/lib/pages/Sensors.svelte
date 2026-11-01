<script lang="ts">
  import Chart from "../Chart.svelte";
  import PageHeader from "../PageHeader.svelte";
  import { api, parseError, type Reading, type SensorReport } from "../api";
  import { temp, ghz, watts } from "../format";
  import { live, poll } from "../live.svelte";

  const s = $derived(live.sample);
  let report = $state<SensorReport | null>(null);
  let error = $state("");

  poll(async () => {
    try {
      report = await api.sensors();
      error = "";
    } catch (e) {
      error = parseError(e).message;
    }
  }, () => Math.max(1500, live.settings.intervalMs));

  const kindOrder: Reading["kind"][] = ["temperature", "power", "fan", "pwm", "frequency", "voltage", "current", "humidity"];
  const kindNames: Record<string, string> = {
    temperature: "Temperatures",
    power: "Power",
    fan: "Fans",
    pwm: "Fan duty",
    frequency: "Clocks",
    voltage: "Voltages",
    current: "Current",
    humidity: "Humidity",
  };

  function display(r: Reading): string {
    switch (r.kind) {
      case "temperature":
        return temp(r.value);
      case "power":
        return watts(r.value);
      case "fan":
        return `${r.value.toFixed(0)} RPM`;
      case "voltage":
        return `${r.value.toFixed(3)} V`;
      case "frequency":
        return `${r.value.toFixed(0)} MHz`;
      default:
        return `${r.value.toFixed(r.value < 10 ? 2 : 0)} ${r.unit}`;
    }
  }

  /** How close a temperature is to its limit, for colouring. */
  function tempLevel(r: Reading): number {
    const limit = r.crit ?? r.max ?? 100;
    return Math.max(0, Math.min(1, r.value / limit));
  }

  const tempColor = (f: number) => (f > 0.85 ? "var(--bad)" : f > 0.7 ? "var(--warn)" : "var(--ok)");
  const avgFreq = $derived(live.history("freq").at(-1));
  const gpuPower = $derived(s?.gpus.reduce((a, g) => a + (g.power ?? 0), 0) ?? 0);
</script>

<PageHeader
  title="Power & Thermals"
  subtitle="Temperatures, fans, voltages, clocks and power draw"
  sources={["lmSensors", "linux", "nvidiaSmi"]}
/>

{#if s}
  <div class="grid" style="--min: 300px">
    <section class="card">
      <div class="spread"><h3>CPU temperature</h3><b class="v" style:color="var(--temp)">{temp(s.cpuTemp)}</b></div>
      <div style="margin-top: 10px">
        <Chart label="CPU temperature" series={[{ label: "CPU", data: live.history("cpuTemp"), color: "var(--temp)" }]} tick={live.tick} height={110} format={(v) => temp(v)} />
      </div>
    </section>
    <section class="card">
      <div class="spread"><h3>CPU clock (average)</h3><b class="v" style:color="var(--cpu)">{ghz(avgFreq)}</b></div>
      <div style="margin-top: 10px">
        <Chart label="CPU clock" series={[{ label: "Clock", data: live.history("freq"), color: "var(--cpu)" }]} tick={live.tick} height={110} max="auto" format={(v) => ghz(v)} />
      </div>
    </section>
    <section class="card">
      <div class="spread">
        <h3>Measured power draw</h3>
        <b class="v" style:color="var(--net)">{watts((report?.cpuPower ?? 0) + gpuPower)}</b>
      </div>
      <div class="stats">
        <div class="stat"><span>CPU package</span><b>{watts(report?.cpuPower)}</b></div>
        {#each s.gpus as g (g.id)}
          <div class="stat"><span title={g.name}>{g.vendor} GPU</span><b>{watts(g.power)}</b></div>
        {/each}
      </div>
      {#if report?.cpuPowerNote}<p class="faint note">{report.cpuPowerNote}</p>{/if}
    </section>
  </div>
{/if}

{#if error}
  <div class="notice" style="margin-top: 16px">{error}</div>
{/if}

{#if report}
  <div class="chips">
    {#each report.chips as chip (chip.id)}
      <section class="card chip">
        <h3>{chip.name.split(" · ")[0]}</h3>
        <div class="faint id">{chip.id} · {chip.adapter}</div>
        {#each kindOrder as kind (kind)}
          {@const readings = chip.readings.filter((r) => r.kind === kind)}
          {#if readings.length}
            <div class="kind faint">{kindNames[kind]}</div>
            {#each readings as r (r.label)}
              <div class="reading">
                <span class="label" title={r.label}>{r.label}</span>
                {#if kind === "temperature"}
                  {@const f = tempLevel(r)}
                  <span class="bar" style="--c: {tempColor(f)}"><i style:width="{f * 100}%"></i></span>
                {/if}
                <b>{display(r)}</b>
              </div>
            {/each}
          {/if}
        {/each}
      </section>
    {/each}
  </div>
{/if}

<style>
  .v {
    font-size: 22px;
    font-weight: 650;
  }

  .note {
    font-size: 12px;
    margin: 12px 0 0;
  }

  .chips {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 14px;
    align-items: start;
    margin-top: 14px;
  }

  .id {
    font-size: 11.5px;
    margin-top: 1px;
  }

  .kind {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 12px 0 4px;
    font-weight: 600;
  }

  .reading {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
    font-size: 13px;
  }

  .reading .bar {
    width: 70px;
  }

  .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .reading b {
    font-weight: 600;
    min-width: 64px;
    text-align: right;
  }
</style>
