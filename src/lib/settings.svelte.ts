// Preferences that live in the window, saved to localStorage whenever one
// changes. Settings the Rust side needs (logging, start at login) are kept
// there instead; see the Settings page.

export type ChartStyle = "smooth" | "straight" | "step" | "bars";
/** What a single chart shows: a graph style, or one of the latest-value views. */
export type ChartType = ChartStyle | "number" | "ring" | "meter";

export type Settings = {
  intervalMs: number;
  theme: "system" | "dark" | "light" | "neon" | "hud";
  /** A colour for buttons, highlights and the selected page, or "" for the theme's own. */
  accent: string;
  /** A page id, or "last" for wherever the window was when it closed. */
  startPage: string;
  lastPage: string;
  chart: {
    style: ChartStyle;
    fill: boolean;
    grid: boolean;
    thickness: number;
    /** How many samples each graph keeps. */
    history: number;
  };
  temperature: "c" | "f";
  networkUnit: "bits" | "bytes";
  confirmEnd: boolean;
  showIdleInterfaces: boolean;
  rememberBenchmarks: boolean;
  /** Charts given their own type, by the chart's label. Others follow `chart.style`. */
  chartTypes: Record<string, ChartType>;
  /** The order of cards in each group, as the user arranged them. */
  cardOrder: Record<string, string[]>;
  /** Each board's columns of card ids, as the user arranged them. */
  cardLayout: Record<string, string[][]>;
  /** Tools switched off in Settings. Kept as the ones off, so a new tool starts on. */
  disabledTools: string[];
  updates: {
    /** null until the user has been asked; checking contacts GitHub. */
    autoCheck: boolean | null;
    autoInstall: boolean;
    /** When the last check ran, in ms since 1970. */
    lastCheck: number;
    /** A version the user chose to skip the reminder for. */
    dismissed: string;
  };
};

const KEY = "otm.settings";

export const defaults: Settings = {
  intervalMs: 1000,
  theme: "system",
  accent: "",
  startPage: "overview",
  lastPage: "overview",
  chart: { style: "smooth", fill: true, grid: true, thickness: 1.75, history: 120 },
  temperature: "c",
  networkUnit: "bits",
  confirmEnd: true,
  showIdleInterfaces: false,
  rememberBenchmarks: true,
  chartTypes: {},
  cardOrder: {},
  cardLayout: {},
  disabledTools: [],
  updates: { autoCheck: null, autoInstall: false, lastCheck: 0, dismissed: "" },
};

function load(): Settings {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      ...defaults,
      ...saved,
      chart: { ...defaults.chart, ...saved.chart },
      updates: { ...defaults.updates, ...saved.updates },
    };
  } catch {
    return structuredClone(defaults);
  }
}

export const settings = $state<Settings>(load());

$effect.root(() => {
  $effect(() => localStorage.setItem(KEY, JSON.stringify(settings)));
});

/** Tools this build of the app was made without (see Cargo.toml's features). */
export const build = $state({ missing: [] as string[] });

/** Whether a tool is in this build. */
export const toolBuilt = (id: string) => !build.missing.includes(id);

/** Whether a tool is switched on. Overview can't be switched off: it's where the app falls back to. */
export const toolOn = (id: string) => id === "overview" || (toolBuilt(id) && !settings.disabledTools.includes(id));

export function setToolOn(id: string, on: boolean) {
  const rest = settings.disabledTools.filter((t) => t !== id);
  settings.disabledTools = on ? rest : [...rest, id];
}

export function resetSettings() {
  // Keeps the update choice and check history: they're answers, not preferences to reset.
  Object.assign(settings, structuredClone(defaults), { lastPage: settings.lastPage, updates: settings.updates });
}
