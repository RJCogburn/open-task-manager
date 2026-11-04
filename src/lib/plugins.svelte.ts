// Plugins: pages from folders in ~/.local/share/open-task-manager/plugins,
// loaded while the app runs. The Rust side lists them, keeps track of which
// the user approved, and hands over an approved plugin's code; this loads it
// and gives it the host API below. See docs/plugins.md.
import { mount, unmount, untrack } from "svelte";
import { api, type PluginInfo, type RunOutput, type Sample } from "./api";
import * as format from "./format";
import { live } from "./live.svelte";
import PluginChart from "./PluginChart.svelte";
import PluginPage from "./PluginPage.svelte";
import type { Tool } from "./tool";
import { sections, tools } from "../tools/registry";

/** The host API version; it must match the manifest's apiVersion. */
export const API_VERSION = 1;

export const plugins = $state({ list: [] as PluginInfo[], loaded: false });

export async function loadPlugins() {
  try {
    plugins.list = await api.pluginsList();
  } catch {
    plugins.list = [];
  }
  plugins.loaded = true;
  // Code from before a reload may have changed.
  modules.clear();
}

export async function approvePlugin(p: PluginInfo, approve: boolean) {
  await api.pluginApprove(p.id, approve);
  await loadPlugins();
}

const PUZZLE =
  "M10 3h4v3a2 2 0 1 0 0 0h4v4h-3a2 2 0 1 0 0 4h3v4h-4v-3a2 2 0 1 0-4 0v3H6v-4h3a2 2 0 1 0 0-4H6V6h4z";

/** Built-in tools and every plugin that can load, as one sorted list. */
export function allTools(): Tool[] {
  const own: Tool[] = plugins.list
    .filter((p) => !p.problem)
    .map((p) => ({
      id: p.id,
      label: p.name,
      section: p.section,
      order: p.order,
      icon: p.icon || PUZZLE,
      color: "var(--accent)",
      page: PluginPage,
      props: { plugin: p },
    }));
  return [...tools, ...own].sort(
    (a, b) =>
      sections.findIndex((s) => s.id === a.section) - sections.findIndex((s) => s.id === b.section) ||
      a.order - b.order,
  );
}

/** What a plugin's index.js default-exports. */
export type PluginModule = {
  /** Draws the page into `el`. May return a function that cleans up. */
  mount(el: HTMLElement, host: HostApi): void | (() => void) | Promise<void | (() => void)>;
};

export type ChartOptions = {
  /** Each line: a history key ("cpu", "mem", "net:eth0:rx"…), or "own:<name>" for one the plugin keeps with push(). */
  series: { label: string; key: string; color?: string }[];
  /** A fixed top value, or "auto" to fit the data. Default 100. */
  max?: number | "auto";
  height?: number;
  format?: (v: number) => string;
  label?: string;
};

/** What the app offers a plugin. Version 1. */
export type HostApi = {
  apiVersion: number;
  plugin: { id: string; name: string; version: string };
  /** The latest sample, as the readings feed describes it. */
  sample(): Sample | null;
  /** Calls `fn` with each new sample, and once now. Returns a function that stops it. */
  onSample(fn: (s: Sample | null) => void): () => void;
  /** Recent values of a history key, oldest first. */
  history(key: string): number[];
  /** Adds a value to one of the plugin's own series, for charts and history("own:<name>"). */
  push(name: string, value: number | null): void;
  /** A live chart in the app's style. Returns a function that removes it. */
  chart(el: HTMLElement, options: ChartOptions): () => void;
  /** Runs a command the manifest declares, by its name there. */
  run(command: string): Promise<RunOutput>;
  /** The app's formatting, following the user's unit settings. */
  format: typeof format;
};

function host(p: PluginInfo): HostApi {
  const own = (key: string) => (key.startsWith("own:") ? `plugin:${p.id}:${key.slice(4)}` : key);
  return {
    apiVersion: API_VERSION,
    plugin: { id: p.id, name: p.name, version: p.version },
    sample: () => live.sample,
    onSample(fn) {
      return $effect.root(() => {
        $effect(() => {
          void live.tick;
          untrack(() => fn(live.sample));
        });
      });
    },
    history: (key) => untrack(() => live.history(own(key))),
    push: (name, value) => live.push(`plugin:${p.id}:${name}`, value),
    chart(el, options) {
      const series = options.series.map((s) => ({ ...s, key: own(s.key) }));
      const chart = mount(PluginChart, {
        target: el,
        props: { ...options, series, label: options.label ?? `${p.name}: ${series.map((s) => s.label).join(", ")}` },
      });
      return () => unmount(chart);
    },
    run: (command) => api.pluginRun(p.id, command),
    format,
  };
}

// Each plugin's code is evaluated once, until the plugins are read again.
const modules = new Map<string, Promise<PluginModule>>();

async function load(p: PluginInfo): Promise<PluginModule> {
  let m = modules.get(p.id);
  if (!m) {
    m = (async () => {
      const code = await api.pluginSource(p.id);
      const url = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
      try {
        const mod = await import(/* @vite-ignore */ url);
        if (typeof mod.default?.mount !== "function") throw new Error("Its index.js doesn't export a mount function.");
        return mod.default as PluginModule;
      } finally {
        URL.revokeObjectURL(url);
      }
    })();
    modules.set(p.id, m);
    m.catch(() => modules.delete(p.id));
  }
  return m;
}

/** Loads a plugin and draws it into `el`. Returns a function that cleans up. */
export async function mountPlugin(p: PluginInfo, el: HTMLElement): Promise<() => void> {
  const mod = await load(p);
  const cleanup = await mod.mount(el, host(p));
  return () => {
    if (typeof cleanup === "function") cleanup();
    el.replaceChildren();
  };
}
