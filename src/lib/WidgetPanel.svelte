<!-- The desktop widget panel: tiles for the readings chosen in Settings,
     in a small see-through window. Drag it anywhere; hover for buttons to
     open the app, rearrange the tiles, or hide the panel. -->
<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import Chart from "./Chart.svelte";
  import { api, type WidgetConfig, type WidgetItem } from "./api";
  import { bytes, ghz, netRate, pct, rate, temp } from "./format";
  import { live } from "./live.svelte";
  import { arrange, card, customGroup } from "./arrange.svelte";

  let config = $state<WidgetConfig | null>(null);
  api.widgetsGet().then((s) => (config = s.config));
  listen<WidgetConfig>("widgets-changed", (e) => (config = e.payload));
  live.start();

  // The tiles' order lives in the panel's settings, next to which tiles to show.
  customGroup(
    "widgets",
    () => config?.items ?? [],
    (ids) => config && api.widgetsSet({ ...config, items: ids as WidgetItem[] }),
  );

  const s = $derived(live.sample);

  /** Several histories added up slot by slot, newest aligned. */
  function sum(keys: string[]): number[] {
    const lists = keys.map((k) => live.history(k));
    const n = Math.max(0, ...lists.map((l) => l.length));
    return Array.from({ length: n }, (_, i) =>
      lists.reduce((a, l) => a + (l[i - (n - l.length)] ?? 0), 0),
    );
  }

  type Tile = {
    name: string;
    value: string;
    sub: string;
    series: { label: string; data: number[]; color: string }[];
    max?: number | "auto";
    format?: (v: number) => string;
  };

  function tile(item: WidgetItem): Tile | null {
    if (!s) return null;
    switch (item) {
      case "cpu": {
        const freqs = s.freqs.filter((f) => f > 0);
        const freq = freqs.length ? freqs.reduce((a, b) => a + b, 0) / freqs.length : null;
        return {
          name: "CPU",
          value: pct(s.cpu),
          sub: [ghz(freq), s.cpuTemp != null ? temp(s.cpuTemp) : ""].filter(Boolean).join(" · "),
          series: [{ label: "CPU", data: live.history("cpu"), color: "var(--cpu)" }],
          format: (v) => pct(v),
        };
      }
      case "memory":
        return {
          name: "Memory",
          value: pct((s.memUsed / s.memTotal) * 100),
          sub: `${bytes(s.memUsed)} of ${bytes(s.memTotal)}`,
          series: [{ label: "Memory", data: live.history("mem"), color: "var(--mem)" }],
          format: (v) => pct(v),
        };
      case "gpu": {
        const g = s.gpus[0];
        if (!g) return { name: "Graphics", value: "—", sub: "No graphics card readings", series: [] };
        return {
          name: "Graphics",
          value: pct(g.util),
          sub: [g.temp != null ? temp(g.temp) : "", g.memUsed != null ? bytes(g.memUsed) : ""].filter(Boolean).join(" · "),
          series: [{ label: "Graphics", data: live.history(`gpu:${g.id}:util`), color: "var(--gpu)" }],
          format: (v) => pct(v),
        };
      }
      case "network": {
        const rx = s.nets.reduce((a, n) => a + n.rx, 0);
        const tx = s.nets.reduce((a, n) => a + n.tx, 0);
        return {
          name: "Network",
          value: `↓ ${netRate(rx)}`,
          sub: `↑ ${netRate(tx)}`,
          series: [
            { label: "Download", data: sum(s.nets.map((n) => `net:${n.name}:rx`)), color: "var(--net)" },
            { label: "Upload", data: sum(s.nets.map((n) => `net:${n.name}:tx`)), color: "var(--gpu)" },
          ],
          max: "auto",
          format: netRate,
        };
      }
      case "disks": {
        const read = s.disks.reduce((a, d) => a + d.read, 0);
        const write = s.disks.reduce((a, d) => a + d.write, 0);
        return {
          name: "Disks",
          value: rate(read + write),
          sub: `read ${rate(read)} · write ${rate(write)}`,
          series: [
            { label: "Read", data: sum(s.disks.map((d) => `disk:${d.name}:read`)), color: "var(--disk)" },
            { label: "Write", data: sum(s.disks.map((d) => `disk:${d.name}:write`)), color: "var(--cpu)" },
          ],
          max: "auto",
          format: rate,
        };
      }
      case "temperature": {
        const gpuTemp = s.gpus.find((g) => g.temp != null)?.temp;
        return {
          name: "Temperature",
          value: temp(s.cpuTemp),
          sub: gpuTemp != null ? `Graphics ${temp(gpuTemp)}` : "CPU",
          series: [{ label: "CPU", data: live.history("cpuTemp"), color: "var(--temp)" }],
          format: (v) => temp(v),
        };
      }
    }
  }

  const hide = () => config && api.widgetsSet({ ...config, enabled: false });
</script>

{#if config}
  <div
    class="panel"
    class:horizontal={config.layout === "horizontal"}
    style:--tile="color-mix(in srgb, var(--panel) {Math.round(config.opacity * 100)}%, transparent)"
    data-tauri-drag-region={arrange.on ? undefined : true}
  >
    {#each config.items as item (item)}
      {@const t = tile(item)}
      <div class="tile" use:card={{ group: "widgets", id: item }} data-tauri-drag-region={arrange.on ? undefined : true}>
        {#if t}
          <div class="head">
            <span class="name">{t.name}</span>
            <b>{t.value}</b>
          </div>
          <div class="sub">{t.sub}</div>
          {#if config.graphs && t.series.length}
            <div class="graph">
              <Chart label={t.name} series={t.series} tick={live.tick} height={50} max={t.max ?? 100} format={t.format} scrub={false} />
            </div>
          {/if}
        {:else}
          <div class="sub">Starting…</div>
        {/if}
      </div>
    {/each}
    <div class="controls">
      <button title="Open Open Task Manager" onclick={() => api.showMain()}>↗</button>
      <button
        title={arrange.on ? "Done arranging" : "Rearrange the tiles"}
        aria-pressed={arrange.on}
        class:on={arrange.on}
        onclick={() => (arrange.on = !arrange.on)}>⇅</button
      >
      <button title="Hide the widgets (turn them back on in Settings)" onclick={hide}>×</button>
    </div>
  </div>
{/if}

<style>
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    height: 100%;
    box-sizing: border-box;
    cursor: grab;
  }

  .panel.horizontal {
    flex-direction: row;
  }

  .tile {
    flex: 1;
    min-width: 0;
    padding: 9px 12px;
    border-radius: 14px;
    background: var(--tile);
    border: 1px solid color-mix(in srgb, var(--border-strong) 70%, transparent);
  }

  /* Clicks go to the tile, which drags the window. */
  .tile * {
    pointer-events: none;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .name {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }

  .head b {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .sub {
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .graph {
    margin-top: 6px;
  }

  .controls {
    position: absolute;
    top: 12px;
    right: 12px;
    display: none;
    gap: 4px;
  }

  .panel:hover .controls,
  .controls:has(.on) {
    display: flex;
  }

  .controls button.on {
    background: var(--accent);
    color: white;
    border-color: transparent;
  }

  /* Arranging: the tiles move, not the window. */
  .panel:has(:global(.arranging)) {
    cursor: default;
  }

  .controls button {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 1px solid var(--border-strong);
    background: var(--panel-2);
    color: var(--text);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }
</style>
