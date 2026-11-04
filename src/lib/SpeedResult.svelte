<!-- The last internet speed test: the numbers, and who measured them. -->
<script lang="ts">
  import { benchmarks } from "./benchmarks.svelte";

  const r = $derived(benchmarks.results.network);
  const mbps = (n: number) => `${n.toFixed(n < 10 ? 1 : 0)} Mbps`;
  const names = { librespeed: "LibreSpeed", cloudflare: "Cloudflare" };
  // A result saved by an older version may be Cloudflare's.
  const by = $derived(r ? names[r.provider] : "");
</script>

{#if r}
  <div class="scores">
    <div><span>Download</span><b style:color="var(--net)">{mbps(r.downloadMbps)}</b></div>
    <div><span>Upload</span><b style:color="var(--gpu)">{mbps(r.uploadMbps)}</b></div>
    <div><span>Latency</span><b>{r.latencyMs.toFixed(0)} ms</b></div>
    {#if r.jitterMs != null}<div><span>Jitter</span><b>{r.jitterMs.toFixed(1)} ms</b></div>{/if}
  </div>
  <p class="faint meta">
    {by}{r.server ? ` · ${r.server}` : ""}{r.isp ? ` · ${r.isp}` : ""} ·
    {new Date(r.at).toLocaleString()}
  </p>
{/if}

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
</style>
