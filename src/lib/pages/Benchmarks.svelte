<script lang="ts">
  import PageHeader from "../PageHeader.svelte";
  import { api, parseError, type CpuScore, type DiskResult, type GpuScore, type MemoryScore, type NetScore, type ToolStatus } from "../api";
  import { bytes } from "../format";
  import { live } from "../live.svelte";

  type Results = {
    cpu?: CpuScore & { at: number };
    memory?: MemoryScore & { at: number };
    disk?: { results: DiskResult[]; folder: string; at: number };
    gpu?: GpuScore & { at: number };
    network?: NetScore & { at: number };
  };
  type Test = keyof Results;

  const KEY = "otm.benchmarks";
  let results = $state<Results>(JSON.parse(localStorage.getItem(KEY) ?? "{}"));
  let running = $state<Test | null>(null);
  let errors = $state<Partial<Record<Test, { message: string; missing: boolean }>>>({});
  let tools = $state<ToolStatus[]>([]);
  let diskFolder = $state("");

  api.toolStatus().then((t) => (tools = t));
  $effect(() => {
    if (!diskFolder && live.info) diskFolder = live.info.homeDir;
  });

  const tool = (program: string) => tools.find((t) => t.program === program);

  async function run<T extends Test>(test: T, go: () => Promise<Omit<NonNullable<Results[T]>, "at">>) {
    running = test;
    errors[test] = undefined;
    try {
      const value = await go();
      results[test] = { ...value, at: Date.now() } as Results[T];
      localStorage.setItem(KEY, JSON.stringify(results));
    } catch (e) {
      const err = parseError(e);
      errors[test] = { message: err.message, missing: err.kind === "missing-tool" };
    } finally {
      running = null;
      api.toolStatus().then((t) => (tools = t));
    }
  }

  const when = (at?: number) => (at ? new Date(at).toLocaleString() : "");
  const mbps = (n: number) => `${n.toFixed(n < 10 ? 1 : 0)} Mbps`;
</script>

<PageHeader
  title="Benchmarks"
  subtitle="Measure how fast this computer is. Close heavy programs first for steady numbers."
  sources={["fio", "glmark2", "curl", "cloudflare"]}
/>

{#snippet status(test: Test, program?: string)}
  {#if errors[test]}
    <div class="notice" class:bad={!errors[test]!.missing}>
      <span>{errors[test]!.message}</span>
    </div>
  {:else if program && tool(program) && !tool(program)!.installed}
    <div class="notice">
      <span>
        Needs <b>{program}</b>, which isn't installed.
        {#if tool(program)!.installHint}Install it with <code>{tool(program)!.installHint}</code>{/if}
      </span>
    </div>
  {/if}
{/snippet}

{#snippet button(test: Test, label: string, go: () => void, disabled = false)}
  <button class="btn primary" disabled={running !== null || disabled} onclick={go}>
    {#if running === test}<span class="spinner"></span> Running…{:else}{label}{/if}
  </button>
{/snippet}

<div class="grid" style="--min: 380px">
  <section class="card bench">
    <div class="spread">
      <div><h3>Processor</h3><p class="muted">Integer and floating-point work for 3 seconds on one thread, then on all {live.info?.logicalCores ?? ""}.</p></div>
      {@render button("cpu", "Run", () => run("cpu", api.benchCpu))}
    </div>
    {#if results.cpu}
      <div class="scores">
        <div><span>Single thread</span><b style:color="var(--cpu)">{results.cpu.single.toFixed(0)}</b><small>M ops/s</small></div>
        <div><span>All {results.cpu.threads} threads</span><b style:color="var(--cpu)">{results.cpu.multi.toFixed(0)}</b><small>M ops/s</small></div>
        <div><span>Scaling</span><b>{(results.cpu.multi / results.cpu.single).toFixed(1)}×</b></div>
      </div>
      <p class="faint when">Last run {when(results.cpu.at)}</p>
    {/if}
    {@render status("cpu")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div><h3>Memory</h3><p class="muted">Copies a 256 MB block over and over for 2 seconds.</p></div>
      {@render button("memory", "Run", () => run("memory", api.benchMemory))}
    </div>
    {#if results.memory}
      <div class="scores">
        <div><span>Copy speed</span><b style:color="var(--mem)">{results.memory.copyGbps.toFixed(1)}</b><small>GB/s</small></div>
      </div>
      <p class="faint when">Last run {when(results.memory.at)}</p>
    {/if}
    {@render status("memory")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div><h3>Disk</h3><p class="muted">fio writes a 1 GB test file, measures four access patterns for 5 seconds each, then deletes it.</p></div>
      {@render button("disk", "Run", () => run("disk", async () => ({ results: await api.benchDisk(diskFolder), folder: diskFolder })), tool("fio")?.installed === false)}
    </div>
    <label class="row field">
      <span class="muted">Test on the drive holding</span>
      <input class="input" style="flex: 1" bind:value={diskFolder} aria-label="Folder for the disk test file" />
    </label>
    {#if results.disk}
      <table class="mini">
        <tbody>
          {#each results.disk.results as r (r.name)}
            <tr>
              <td>{r.name}</td>
              <td class="num"><b style:color="var(--disk)">{bytes(r.bandwidth)}/s</b></td>
              <td class="num faint">{Math.round(r.iops).toLocaleString()} IOPS</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="faint when">Last run {when(results.disk.at)} in {results.disk.folder}</p>
    {/if}
    {@render status("disk", "fio")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div><h3>Graphics</h3><p class="muted">glmark2's standard OpenGL scenes, drawn off screen. Takes a few minutes.</p></div>
      {@render button("gpu", "Run", () => run("gpu", api.benchGpu), tool("glmark2")?.installed === false)}
    </div>
    {#if results.gpu}
      <div class="scores">
        <div><span>glmark2 score</span><b style:color="var(--gpu)">{results.gpu.score.toLocaleString()}</b></div>
      </div>
      <p class="faint when">{results.gpu.renderer} · {when(results.gpu.at)}</p>
    {/if}
    {@render status("gpu", "glmark2")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div>
        <h3>Internet</h3>
        <p class="muted">Downloads 100 MB from and uploads 25 MB to Cloudflare's speed test server, with curl.</p>
      </div>
      {@render button("network", "Run", () => run("network", api.benchNetwork))}
    </div>
    {#if results.network}
      <div class="scores">
        <div><span>Download</span><b style:color="var(--net)">{mbps(results.network.downloadMbps)}</b></div>
        <div><span>Upload</span><b style:color="var(--gpu)">{mbps(results.network.uploadMbps)}</b></div>
        <div><span>Latency</span><b>{results.network.latencyMs.toFixed(0)} ms</b></div>
      </div>
      <p class="faint when">Last run {when(results.network.at)}</p>
    {/if}
    {@render status("network", "curl")}
  </section>
</div>

<style>
  .bench {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .bench p {
    margin: 4px 0 0;
    font-size: 13px;
  }

  .spread {
    align-items: flex-start;
  }

  .scores {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 30px;
  }

  .scores div {
    display: flex;
    flex-direction: column;
  }

  .scores span {
    font-size: 12px;
    color: var(--muted);
  }

  .scores b {
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.2;
  }

  .scores small {
    color: var(--faint);
    font-size: 11.5px;
  }

  .bench .when {
    font-size: 12px;
    margin: 0;
  }

  .field {
    font-size: 13px;
  }

  .mini {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .mini td {
    padding: 5px 0;
    border-bottom: 1px solid var(--border);
  }

  .mini td.num {
    text-align: right;
    padding-left: 14px;
  }
</style>
