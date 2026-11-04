<!-- A chart a plugin asked for: the app's Chart, fed from history keys. -->
<script lang="ts">
  import Chart from "./Chart.svelte";
  import { live } from "./live.svelte";

  let {
    series,
    max = 100,
    height = 140,
    format,
    label,
  }: {
    series: { label: string; key: string; color?: string }[];
    max?: number | "auto";
    height?: number;
    format?: (v: number) => string;
    label: string;
  } = $props();

  const palette = ["var(--accent)", "var(--mem)", "var(--disk)", "var(--net)", "var(--gpu)", "var(--temp)"];
</script>

<Chart
  {label}
  {max}
  {height}
  {format}
  tick={live.tick}
  series={series.map((s, i) => ({ label: s.label, data: live.history(s.key), color: s.color ?? palette[i % palette.length] }))}
/>
