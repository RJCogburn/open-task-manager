// The live feed: every `sample` event from the Rust sampler, plus a rolling
// history of each value for the graphs. History lives outside Svelte's
// reactivity (it changes every second and can be large); `tick` changes
// with each sample so graphs know to redraw.
import { untrack } from "svelte";
import { listen } from "@tauri-apps/api/event";
import { api, type Sample, type SystemInfo } from "./api";
import { build, settings, type Settings } from "./settings.svelte";

/** How many samples each graph keeps; set on the Settings page. */
export const historyLength = () => settings.chart.history;

class Live {
  sample = $state<Sample | null>(null);
  info = $state<SystemInfo | null>(null);
  tick = $state(0);
  readonly settings = settings;
  private series = new Map<string, number[]>();
  /** A copy of every series taken when the graphs were paused. */
  private frozen = $state.raw<Map<string, number[]> | null>(null);
  private latest: Sample | null = null;

  /** The moment every graph points at, as samples before the newest; null
   * when the pointer is off the graphs and nothing is paused. */
  cursor = $state<number | null>(null);
  get paused() {
    return this.frozen != null;
  }

  /** The recorded values for a key, oldest first. Reading it in a template
   * or effect re-runs that code on every new sample. While paused it is the
   * history as it was at the pause. */
  history(key: string): number[] {
    void this.tick;
    return (this.frozen ?? this.series).get(key) ?? [];
  }

  /** The clock time of a moment, `ago` samples before the newest. */
  timeAt(ago: number): number | null {
    const times = this.history("time");
    const t = times[times.length - 1 - ago];
    return t == null || !isFinite(t) ? null : t;
  }

  /** Freezes every graph, pointing at `ago` samples back (the newest if not given). */
  pause(ago = this.cursor ?? 0) {
    if (!this.frozen) this.frozen = new Map([...this.series].map(([k, v]) => [k, v.slice()]));
    this.cursor = Math.max(0, Math.min(this.span() - 1, ago));
  }

  resume() {
    this.frozen = null;
    this.cursor = null;
    if (this.latest) this.sample = this.latest;
    this.tick++;
  }

  /** Moves the paused cursor; positive steps go back in time. */
  step(by: number) {
    if (!this.frozen) this.pause();
    this.cursor = Math.max(0, Math.min(this.span() - 1, (this.cursor ?? 0) + by));
  }

  /** How many samples the paused history holds. */
  private span() {
    return (this.frozen ?? this.series).get("time")?.length ?? 1;
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
    this.push("time", x.time);
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
    build.missing = await api.missingTools();
    // The sampler skips readings only switched-off pages would use.
    $effect.root(() => {
      $effect(() => void api.setDisabledTools([...this.settings.disabledTools, ...build.missing]));
    });
    this.info = await api.systemInfo();
    // History keeps recording while the window is hidden in the tray, but
    // pages only redraw when it can be seen, and not while paused.
    const show = () => {
      if (document.hidden || !this.latest || this.frozen) return;
      this.sample = this.latest;
      this.tick++;
    };
    document.addEventListener("visibilitychange", show);
    await listen<Sample>("sample", (e) => {
      this.record(e.payload);
      this.latest = e.payload;
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
    const root = document.documentElement.style;
    if (this.settings.accent) root.setProperty("--accent", this.settings.accent);
    else root.removeProperty("--accent");
  }

  setAccent(color: string) {
    this.settings.accent = color;
    this.applyTheme();
  }
}

export const live = new Live();

/** Calls `fn` now and then every `ms` while the calling component is mounted.
 * While the window is hidden (in the tray) it pauses, and catches up as soon
 * as the window shows again. */
export function poll(fn: () => unknown, ms: () => number) {
  $effect(() => {
    const every = ms();
    const run = () => {
      if (!document.hidden) untrack(fn);
    };
    untrack(fn);
    const id = setInterval(run, every);
    document.addEventListener("visibilitychange", run);
    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", run);
    };
  });
}
