// Benchmark results, shared by the Benchmarks page and the Network page's
// speed test. Kept between sessions unless turned off in Settings.
import { api, missingProgram, parseError, type CpuScore, type DiskResult, type GpuScore, type MemoryScore, type NetScore } from "./api";
import { settings } from "./settings.svelte";

export type Results = {
  cpu?: CpuScore & { at: number };
  memory?: MemoryScore & { at: number };
  disk?: { results: DiskResult[]; folder: string; at: number };
  gpu?: GpuScore & { at: number };
  network?: NetScore & { at: number };
};
export type Test = keyof Results;

const KEY = "otm.benchmarks";

function load(): Results {
  if (!settings.rememberBenchmarks) return {};
  try {
    const saved: Results = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    // Versions before 0.85 could test with Ookla's speedtest, no longer used.
    if ((saved.network?.provider as string) === "ookla") delete saved.network;
    return saved;
  } catch {
    return {};
  }
}

class Benchmarks {
  results = $state<Results>(load());
  running = $state<Test | null>(null);
  /** `missing` names the program a test needs and couldn't find. */
  errors = $state<Partial<Record<Test, { message: string; missing: string | null }>>>({});
  diskFolder = $state(localStorage.getItem(KEY + ".folder") ?? "");

  async run<T extends Test>(test: T, go: () => Promise<Omit<NonNullable<Results[T]>, "at">>) {
    this.running = test;
    this.errors[test] = undefined;
    try {
      const value = await go();
      this.results[test] = { ...value, at: Date.now() } as Results[T];
      this.save();
    } catch (e) {
      const err = parseError(e);
      this.errors[test] = { message: err.message, missing: missingProgram(e) };
    } finally {
      this.running = null;
    }
  }

  runNetwork = () => this.run("network", api.benchNetwork);

  save() {
    if (!settings.rememberBenchmarks) return;
    localStorage.setItem(KEY, JSON.stringify(this.results));
    localStorage.setItem(KEY + ".folder", this.diskFolder);
  }

  /** Deletes the saved copy; what is on screen stays until the app closes. */
  forgetSaved() {
    localStorage.removeItem(KEY);
    localStorage.removeItem(KEY + ".folder");
  }

  /** Forgets saved results, and the ones on screen too. */
  clear() {
    this.results = {};
    this.forgetSaved();
  }
}

export const benchmarks = new Benchmarks();
