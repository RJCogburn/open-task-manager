import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { pct } from "$lib/format";
import Page from "./Gpu.svelte";

export default defineTool({
  id: "gpu",
  label: "GPU",
  section: "monitor",
  order: 70,
  icon: "M2 7h20v10H2zM6 17v3M10 17v3M8 12a3 3 0 1 0 6 0 3 3 0 1 0-6 0M18 10h1M18 14h1",
  color: "var(--gpu)",
  value: () => (live.sample?.gpus[0] ? pct(live.sample.gpus[0].util) : ""),
  sources: ["nvidiaSmi", "linux", "pciutils"],
  page: Page,
});
