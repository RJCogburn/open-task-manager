<script lang="ts">
  import Chart from "$lib/Chart.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, parseError, type Autostart, type LogStatus, type TrayConfig } from "$lib/api";
  import { benchmarks } from "$lib/benchmarks.svelte";
  import { bytes, pct } from "$lib/format";
  import { live } from "$lib/live.svelte";
  import { defaults, resetSettings, settings, type ChartStyle } from "$lib/settings.svelte";

  let { pages }: { pages: { id: string; label: string }[] } = $props();

  let autostart = $state<Autostart | null>(null);
  let logging = $state<LogStatus | null>(null);
  let error = $state("");
  let saved = $state("");

  let tray = $state<TrayConfig | null>(null);
  api.autostartGet().then((a) => (autostart = a));
  api.trayGet().then((t) => (tray = t));

  async function saveTray() {
    if (!tray) return;
    error = "";
    try {
      await api.traySet(tray);
    } catch (e) {
      error = parseError(e).message;
    }
  }
  const loadLogging = () => api.loggingGet().then((l) => (logging = l));
  loadLogging();

  async function setAutostart(enabled: boolean, minimized: boolean) {
    error = "";
    try {
      autostart = await api.autostartSet(enabled, minimized);
    } catch (e) {
      error = parseError(e).message;
    }
  }

  async function saveLogging() {
    if (!logging) return;
    error = "";
    try {
      await api.loggingSet(logging.config);
      await loadLogging();
    } catch (e) {
      error = parseError(e).message;
    }
  }

  const styles: { id: ChartStyle; label: string }[] = [
    { id: "smooth", label: "Smooth" },
    { id: "straight", label: "Straight" },
    { id: "step", label: "Steps" },
    { id: "bars", label: "Bars" },
  ];
  const kinds = [
    { key: "cpu", label: "Processor", hint: "usage, clock speed, load" },
    { key: "memory", label: "Memory", hint: "RAM and swap in use" },
    { key: "disks", label: "Disks", hint: "read and write speed, busy time" },
    { key: "network", label: "Network", hint: "download and upload per interface" },
    { key: "gpu", label: "Graphics", hint: "usage, memory, temperature, power" },
    { key: "temperatures", label: "Temperatures", hint: "CPU, graphics and drives" },
  ] as const;

  function clearBenchmarks() {
    benchmarks.clear();
    saved = "Saved benchmark results cleared.";
  }

  function reset() {
    if (!confirm("Put every setting on this page back the way it started? Logging and start at login are left as they are.")) return;
    resetSettings();
    live.applyTheme();
    api.setInterval(settings.intervalMs);
    saved = "Settings reset.";
  }
</script>

<PageHeader title="Settings" subtitle="Make Open Task Manager work the way you like" />

{#if error}<div class="notice bad" style="margin-bottom: 14px">{error}</div>{/if}
{#if saved}<div class="notice info" style="margin-bottom: 14px">{saved}</div>{/if}

<div class="cols">
  <section class="card">
    <h3>Appearance</h3>
    <div class="field">
      <span>Theme</span>
      <div class="seg" role="group" aria-label="Theme">
        {#each [["system", "System"], ["dark", "Dark"], ["light", "Light"]] as const as [id, label] (id)}
          <button aria-pressed={settings.theme === id} onclick={() => live.setTheme(id)}>{label}</button>
        {/each}
      </div>
    </div>
    <div class="field">
      <span>Graph style</span>
      <div class="seg" role="group" aria-label="Graph style">
        {#each styles as st (st.id)}
          <button aria-pressed={settings.chart.style === st.id} onclick={() => (settings.chart.style = st.id)}>{st.label}</button>
        {/each}
      </div>
    </div>
    <div class="preview">
      <Chart
        label="Preview: processor usage"
        series={[
          { label: "CPU", data: live.history("cpu"), color: "var(--cpu)" },
          { label: "Memory", data: live.history("mem"), color: "var(--mem)" },
        ]}
        tick={live.tick}
        height={110}
        format={(v) => pct(v)}
      />
    </div>
    <label class="field">
      <span>Shade under lines</span>
      <input type="checkbox" bind:checked={settings.chart.fill} disabled={settings.chart.style === "bars"} />
    </label>
    <label class="field">
      <span>Grid lines</span>
      <input type="checkbox" bind:checked={settings.chart.grid} />
    </label>
    <label class="field">
      <span>Line thickness</span>
      <span class="row">
        <input type="range" min="1" max="4" step="0.25" bind:value={settings.chart.thickness} disabled={settings.chart.style === "bars"} />
        <span class="faint num">{settings.chart.thickness.toFixed(2)}</span>
      </span>
    </label>
    <label class="field">
      <span>Graphs show the last</span>
      <select class="input" bind:value={settings.chart.history}>
        {#each [60, 120, 300, 600] as n (n)}
          <option value={n}>{n} samples ({Math.round((n * settings.intervalMs) / 1000 / 60 * 10) / 10} min)</option>
        {/each}
      </select>
    </label>
  </section>

  <section class="card">
    <h3>Behaviour</h3>
    <label class="field">
      <span>Update every</span>
      <select class="input" value={settings.intervalMs} onchange={(e) => live.setIntervalMs(Number(e.currentTarget.value))}>
        <option value={500}>0.5 s</option>
        <option value={1000}>1 s</option>
        <option value={2000}>2 s</option>
        <option value={5000}>5 s</option>
      </select>
    </label>
    <label class="field">
      <span>Open on</span>
      <select class="input" bind:value={settings.startPage}>
        <option value="last">The page I was on last</option>
        {#each pages as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
    </label>
    <div class="field">
      <span>Temperatures in</span>
      <div class="seg" role="group" aria-label="Temperature unit">
        <button aria-pressed={settings.temperature === "c"} onclick={() => (settings.temperature = "c")}>°C</button>
        <button aria-pressed={settings.temperature === "f"} onclick={() => (settings.temperature = "f")}>°F</button>
      </div>
    </div>
    <div class="field">
      <span>Network speeds in</span>
      <div class="seg" role="group" aria-label="Network speed unit">
        <button aria-pressed={settings.networkUnit === "bits"} onclick={() => (settings.networkUnit = "bits")}>Mbps</button>
        <button aria-pressed={settings.networkUnit === "bytes"} onclick={() => (settings.networkUnit = "bytes")}>MB/s</button>
      </div>
    </div>
    <label class="field">
      <span>Ask before ending a process</span>
      <input type="checkbox" bind:checked={settings.confirmEnd} />
    </label>
    <label class="field">
      <span>Show unused network interfaces</span>
      <input type="checkbox" bind:checked={settings.showIdleInterfaces} />
    </label>

    <h3 class="sub">System tray</h3>
    {#if tray}
      <label class="field">
        <span>Show an icon in the system tray</span>
        <input type="checkbox" bind:checked={tray.enabled} onchange={saveTray} />
      </label>
      <label class="field">
        <span>Closing the window keeps it running in the tray</span>
        <input type="checkbox" bind:checked={tray.closeToTray} disabled={!tray.enabled} onchange={saveTray} />
      </label>
      <label class="field">
        <span>Show CPU usage next to the tray icon</span>
        <input type="checkbox" bind:checked={tray.showUsage} disabled={!tray.enabled} onchange={saveTray} />
      </label>
      <p class="faint hint">
        In the tray, it keeps logging and uses little power: graphs stop drawing until you open the window again. Quit
        from the tray icon's menu.
      </p>
    {/if}

    <h3 class="sub">Start up</h3>
    {#if autostart}
      <label class="field">
        <span>Start when I log in</span>
        <input type="checkbox" checked={autostart.enabled} onchange={(e) => setAutostart(e.currentTarget.checked, autostart!.minimized)} />
      </label>
      <label class="field">
        <span>{tray?.enabled ? "Start in the tray, without opening the window" : "Start minimized"}</span>
        <input
          type="checkbox"
          checked={autostart.minimized}
          disabled={!autostart.enabled}
          onchange={(e) => setAutostart(true, e.currentTarget.checked)}
        />
      </label>
      {#if autostart.enabled}<p class="faint hint">Starts <code>{autostart.program}</code> from <code>~/.config/autostart</code>.</p>{/if}
    {/if}

    <h3 class="sub">Benchmarks</h3>
    <div class="field">
      <span>Internet speed test</span>
      <div class="seg" role="group" aria-label="Internet speed test service">
        <button aria-pressed={settings.speedTest === "auto"} onclick={() => (settings.speedTest = "auto")}>Automatic</button>
        <button aria-pressed={settings.speedTest === "ookla"} onclick={() => (settings.speedTest = "ookla")}>Ookla</button>
        <button aria-pressed={settings.speedTest === "cloudflare"} onclick={() => (settings.speedTest = "cloudflare")}>Cloudflare</button>
      </div>
    </div>
    <p class="faint hint">Automatic uses Ookla's speedtest command (speedtest.net) when it is installed, and Cloudflare otherwise.</p>
    <label class="field">
      <span>Remember the last results</span>
      <input type="checkbox" bind:checked={settings.rememberBenchmarks} onchange={() => (settings.rememberBenchmarks ? benchmarks.save() : benchmarks.forgetSaved())} />
    </label>
    <div class="field">
      <span class="faint">Results are kept on this computer only.</span>
      <button class="btn" onclick={clearBenchmarks}>Clear saved results</button>
    </div>
  </section>

  <section class="card wide">
    <div class="spread">
      <h3>Hardware logging</h3>
      {#if logging}
        <label class="row">
          <span class="muted" style="font-size: 13px">Record readings to files</span>
          <input type="checkbox" bind:checked={logging.config.enabled} onchange={saveLogging} />
        </label>
      {/if}
    </div>
    <p class="faint hint">
      Writes readings to CSV files you can open in a spreadsheet, one file per kind of hardware per day. Logging runs
      while Open Task Manager is open, minimized or not.
    </p>
    {#if logging}
      <div class="kinds" class:off={!logging.config.enabled}>
        {#each kinds as k (k.key)}
          <label class="kind">
            <input type="checkbox" bind:checked={logging.config[k.key]} onchange={saveLogging} />
            <span><b>{k.label}</b><span class="faint">{k.hint}</span></span>
          </label>
        {/each}
      </div>
      <div class="row logrow">
        <label class="row">
          <span class="muted">Record every</span>
          <select class="input" bind:value={logging.config.every} onchange={saveLogging}>
            {#each [1, 5, 10, 30, 60, 300] as n (n)}
              <option value={n}>{n < 60 ? `${n} s` : `${n / 60} min`}</option>
            {/each}
          </select>
        </label>
        <label class="row">
          <span class="muted">Keep files for</span>
          <select class="input" bind:value={logging.config.keepDays} onchange={saveLogging}>
            {#each [1, 7, 14, 30, 90, 365] as n (n)}
              <option value={n}>{n === 1 ? "1 day" : `${n} days`}</option>
            {/each}
          </select>
        </label>
        <span class="faint" style="margin-left: auto">{logging.files} files · {bytes(logging.size)}</span>
        <button class="btn" onclick={() => api.openFolder(logging!.folder)} disabled={!logging.files}>Open log folder</button>
      </div>
      <p class="faint hint mono">{logging.folder}</p>
    {/if}
  </section>
</div>

<div class="row" style="margin-top: 18px; justify-content: flex-end">
  <button class="btn" onclick={reset} disabled={JSON.stringify({ ...settings, lastPage: "" }) === JSON.stringify({ ...defaults, lastPage: "" })}>
    Reset to defaults
  </button>
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
    gap: 14px;
    align-items: start;
  }

  .wide {
    grid-column: 1 / -1;
  }

  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    min-height: 34px;
    margin-top: 8px;
    font-size: 13px;
  }

  .field select {
    height: 30px;
  }

  .sub {
    margin-top: 22px !important;
  }

  .preview {
    margin-top: 10px;
  }

  .num {
    width: 32px;
    text-align: right;
    font-size: 12px;
  }

  .hint {
    font-size: 12px;
    margin: 8px 0 0;
  }

  .kinds {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 8px;
    margin-top: 14px;
  }

  .kinds.off {
    opacity: 0.55;
  }

  .kind {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: 13px;
  }

  .kind span {
    display: flex;
    flex-direction: column;
  }

  .kind .faint {
    font-size: 12px;
  }

  .logrow {
    flex-wrap: wrap;
    margin-top: 14px;
    font-size: 13px;
  }

  .logrow select {
    height: 30px;
  }
</style>
