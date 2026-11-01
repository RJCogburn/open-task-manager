<!-- The last internet speed test: the numbers, who measured them, and a way to
     check against speedtest.net in the browser. -->
<script lang="ts">
  import { api, type ToolStatus } from "./api";
  import { benchmarks } from "./benchmarks.svelte";
  import { external } from "./link";
  import { settings } from "./settings.svelte";

  let ookla = $state<ToolStatus | null>(null);
  api.toolStatus().then((t) => (ookla = t.find((x) => x.program === "speedtest") ?? null));

  const r = $derived(benchmarks.results.network);
  const mbps = (n: number) => `${n.toFixed(n < 10 ? 1 : 0)} Mbps`;
  const by = $derived(r?.provider === "ookla" ? "Speedtest by Ookla" : "Cloudflare");
</script>

{#if r}
  <div class="scores">
    <div><span>Download</span><b style:color="var(--net)">{mbps(r.downloadMbps)}</b></div>
    <div><span>Upload</span><b style:color="var(--gpu)">{mbps(r.uploadMbps)}</b></div>
    <div><span>Latency</span><b>{r.latencyMs.toFixed(0)} ms</b></div>
    {#if r.jitterMs != null}<div><span>Jitter</span><b>{r.jitterMs.toFixed(1)} ms</b></div>{/if}
  </div>
  <p class="faint meta">
    {by}{r.server && r.provider === "ookla" ? ` · ${r.server}` : ""}{r.isp ? ` · ${r.isp}` : ""} ·
    {new Date(r.at).toLocaleString()}
    {#if r.url}· <a href={r.url} use:external>result on speedtest.net</a>{/if}
  </p>
{/if}

{#if ookla && !ookla.installed && settings.speedTest !== "cloudflare"}
  <p class="faint meta">
    For results that match speedtest.net, install Ookla's speedtest command: {ookla.installHint}. It is free but not
    open source.
  </p>
{:else if ookla?.installed && settings.speedTest !== "cloudflare"}
  <p class="faint meta">
    Uses Ookla's speedtest command, which means accepting Ookla's
    <a href="https://www.speedtest.net/about/eula" use:external>license</a> and
    <a href="https://www.speedtest.net/about/privacy" use:external>privacy policy</a>.
  </p>
{/if}
<p class="faint meta">
  <a href="https://www.speedtest.net/" use:external>Open speedtest.net in your browser</a>
</p>

<style>
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
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.2;
  }

  .meta {
    font-size: 12px;
    margin: 0;
  }

  a {
    color: var(--accent);
  }
</style>
