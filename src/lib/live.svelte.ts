// The live feed: every `sample` event from the Rust sampler, plus a rolling
// history of each value for the graphs. History lives outside Svelte's
// reactivity (it changes every second and can be large); `tick` changes
// with each sample so graphs know to redraw.
import { untrack } from "svelte";
import { listen } from "@tauri-apps/api/event";
import { api, type Sample, type SystemInfo } from "./api";
import { settings, type Settings } from "./settings.svelte";

/** How many samples each graph keeps; set on the Settings page. */
export const historyLength = () => settings.chart.history;

class Live {
  sample = $state<Sample | null>(null);
  info = $state<SystemInfo | null>(null);
  tick = $state(0);
  readonly settings = settings;
  private series = new Map<string, number[]>();

  /** The recorded values for a key, oldest first. Reading it in a template
   * or effect re-runs that code on every new sample. */
  history(key: string): number[] {
    void this.tick;
    return this.series.get(key) ?? [];
  }

  private push(key: string, value: number | null | undefined) {
    let s = this.series.get(key);
    if (!s) {
      s = [];
      this.series.set(key, s);
    }
    s.push(value ?? NaN);
    const max = historyLength();
    if (s.length > max) s.splice(0, s.length - max);
  }

  private record(x: Sample) {
    this.push("cpu", x.cpu);
    x.cores.forEach((c, i) => this.push(`core:${i}`, c));
    const freqs = x.freqs.filter((f) => f > 0);
    this.push("freq", freqs.length ? freqs.reduce((a, b) => a + b, 0) / freqs.length : NaN);
    this.push("cpuTemp", x.cpuTemp);
    this.push("mem", (x.memUsed / x.memTotal) * 100);
    this.push("memBytes", x.memUsed);
    this.push("swap", x.swapTotal ? (x.swapUsed / x.swapTotal) * 100 : 0);
    for (const d of x.disks) {
      this.push(`disk:${d.name}:read`, d.read);
      this.push(`disk:${d.name}:write`, d.write);
      this.push(`disk:${d.name}:busy`, d.busy);
    }
    for (const n of x.nets) {
      this.push(`net:${n.name}:rx`, n.rx);
      this.push(`net:${n.name}:tx`, n.tx);
    }
    for (const g of x.gpus) {
      this.push(`gpu:${g.id}:util`, g.util);
      this.push(`gpu:${g.id}:mem`, g.memUsed != null && g.memTotal ? (g.memUsed / g.memTotal) * 100 : null);
      this.push(`gpu:${g.id}:temp`, g.temp);
      this.push(`gpu:${g.id}:power`, g.power);
    }
  }

  async start() {
    this.applyTheme();
    await api.setInterval(this.settings.intervalMs);
    this.info = await api.systemInfo();
    // History keeps recording while the window is hidden in the tray, but
    // pages only redraw when it can be seen.
    let latest: Sample | null = null;
    const show = () => {
      if (document.hidden || !latest) return;
      this.sample = latest;
      this.tick++;
    };
    document.addEventListener("visibilitychange", show);
    await listen<Sample>("sample", (e) => {
      this.record(e.payload);
      latest = e.payload;
      show();
    });
  }

  async setIntervalMs(ms: number) {
    this.settings.intervalMs = ms;
    await api.setInterval(ms);
  }

  setTheme(theme: Settings["theme"]) {
    this.settings.theme = theme;
    this.applyTheme();
  }

  applyTheme() {
    const prefersLight = matchMedia("(prefers-color-scheme: light)").matches;
    const theme = this.settings.theme === "system" ? (prefersLight ? "light" : "dark") : this.settings.theme;
    document.documentElement.dataset.theme = theme;
  }
}

export const live = new Live();

/** Calls `fn` now and then every `ms` while the calling component is mounted. */
export function poll(fn: () => unknown, ms: () => number) {
  $effect(() => {
    const every = ms();
    untrack(fn);
    const id = setInterval(fn, every);
    return () => clearInterval(id);
  });
}
