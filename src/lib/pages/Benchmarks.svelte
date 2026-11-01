<script lang="ts">
  import PageHeader from "../PageHeader.svelte";
  import { api, type ToolStatus } from "../api";
  import SpeedResult from "../SpeedResult.svelte";
  import { benchmarks, type Results, type Test } from "../benchmarks.svelte";
  import { settings } from "../settings.svelte";
  import { bytes } from "../format";
  import { live } from "../live.svelte";

  const b = benchmarks;
  let tools = $state<ToolStatus[]>([]);

  api.toolStatus().then((t) => (tools = t));
  $effect(() => {
    if (!b.diskFolder && live.info) b.diskFolder = live.info.homeDir;
  });

  const tool = (program: string) => tools.find((t) => t.program === program);

  async function run<T extends Test>(test: T, go: () => Promise<Omit<NonNullable<Results[T]>, "at">>) {
    await b.run(test, go);
    api.toolStatus().then((t) => (tools = t));
  }

  const when = (at?: number) => (at ? new Date(at).toLocaleString() : "");
</script>

<PageHeader
  title="Benchmarks"
  subtitle="Measure how fast this computer is. Close heavy programs first for steady numbers."
  sources={["fio", "glmark2", "ookla", "curl", "cloudflare"]}
/>

{#snippet status(test: Test, program?: string)}
  {#if b.errors[test]}
    <div class="notice" class:bad={!b.errors[test]!.missing}>
      <span>{b.errors[test]!.message}</span>
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
  <button class="btn primary" disabled={b.running !== null || disabled} onclick={go}>
    {#if b.running === test}<span class="spinner"></span> Running…{:else}{label}{/if}
  </button>
{/snippet}

<div class="grid" style="--min: 380px">
  <section class="card bench">
    <div class="spread">
      <div><h3>Processor</h3><p class="muted">Integer and floating-point work for 3 seconds on one thread, then on all {live.info?.logicalCores ?? ""}.</p></div>
      {@render button("cpu", "Run", () => run("cpu", api.benchCpu))}
    </div>
    {#if b.results.cpu}
      <div class="scores">
        <div><span>Single thread</span><b style:color="var(--cpu)">{b.results.cpu.single.toFixed(0)}</b><small>M ops/s</small></div>
        <div><span>All {b.results.cpu.threads} threads</span><b style:color="var(--cpu)">{b.results.cpu.multi.toFixed(0)}</b><small>M ops/s</small></div>
        <div><span>Scaling</span><b>{(b.results.cpu.multi / b.results.cpu.single).toFixed(1)}×</b></div>
      </div>
      <p class="faint when">Last run {when(b.results.cpu.at)}</p>
    {/if}
    {@render status("cpu")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div><h3>Memory</h3><p class="muted">Copies a 256 MB block over and over for 2 seconds.</p></div>
      {@render button("memory", "Run", () => run("memory", api.benchMemory))}
    </div>
    {#if b.results.memory}
      <div class="scores">
        <div><span>Copy speed</span><b style:color="var(--mem)">{b.results.memory.copyGbps.toFixed(1)}</b><small>GB/s</small></div>
      </div>
      <p class="faint when">Last run {when(b.results.memory.at)}</p>
    {/if}
    {@render status("memory")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div><h3>Disk</h3><p class="muted">fio writes a 1 GB test file, measures four access patterns for 5 seconds each, then deletes it.</p></div>
      {@render button("disk", "Run", () => run("disk", async () => ({ results: await api.benchDisk(b.diskFolder), folder: b.diskFolder })), tool("fio")?.installed === false)}
    </div>
    <label class="row field">
      <span class="muted">Test on the drive holding</span>
      <input class="input" style="flex: 1" bind:value={b.diskFolder} aria-label="Folder for the disk test file" />
    </label>
    {#if b.results.disk}
      <table class="mini">
        <tbody>
          {#each b.results.disk.results as r (r.name)}
            <tr>
              <td>{r.name}</td>
              <td class="num"><b style:color="var(--disk)">{bytes(r.bandwidth)}/s</b></td>
              <td class="num faint">{Math.round(r.iops).toLocaleString()} IOPS</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="faint when">Last run {when(b.results.disk.at)} in {b.results.disk.folder}</p>
    {/if}
    {@render status("disk", "fio")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div><h3>Graphics</h3><p class="muted">glmark2's standard OpenGL scenes, drawn off screen. Takes a few minutes.</p></div>
      {@render button("gpu", "Run", () => run("gpu", api.benchGpu), tool("glmark2")?.installed === false)}
    </div>
    {#if b.results.gpu}
      <div class="scores">
        <div><span>glmark2 score</span><b style:color="var(--gpu)">{b.results.gpu.score.toLocaleString()}</b></div>
      </div>
      <p class="faint when">{b.results.gpu.renderer} · {when(b.results.gpu.at)}</p>
    {/if}
    {@render status("gpu", "glmark2")}
  </section>

  <section class="card bench">
    <div class="spread">
      <div>
        <h3>Internet</h3>
        <p class="muted">
          With Ookla's speedtest command when it is installed, the same test as speedtest.net. Otherwise curl
          measures against Cloudflare's server over 6 connections, using up to about 250 MB down and 120 MB up.
        </p>
      </div>
      {@render button("network", "Run", () => run("network", () => api.benchNetwork(settings.speedTest)))}
    </div>
    <SpeedResult />
    {@render status("network")}
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
