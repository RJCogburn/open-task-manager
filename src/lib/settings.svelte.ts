// Preferences that live in the window, saved to localStorage whenever one
// changes. Settings the Rust side needs (logging, start at login) are kept
// there instead; see the Settings page.

export type ChartStyle = "smooth" | "straight" | "step" | "bars";

export type Settings = {
  intervalMs: number;
  theme: "system" | "dark" | "light";
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
  /** Which service the internet speed test uses; "auto" prefers Ookla when installed. */
  speedTest: "auto" | "ookla" | "cloudflare";
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
  startPage: "overview",
  lastPage: "overview",
  chart: { style: "smooth", fill: true, grid: true, thickness: 1.75, history: 120 },
  temperature: "c",
  networkUnit: "bits",
  confirmEnd: true,
  showIdleInterfaces: false,
  rememberBenchmarks: true,
  speedTest: "auto",
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

export function resetSettings() {
  // Keeps the update choice and check history: they're answers, not preferences to reset.
  Object.assign(settings, structuredClone(defaults), { lastPage: settings.lastPage, updates: settings.updates });
}
