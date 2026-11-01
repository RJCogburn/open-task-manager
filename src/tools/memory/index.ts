import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { pct } from "$lib/format";
import Page from "./Memory.svelte";

export default defineTool({
  id: "memory",
  label: "Memory",
  section: "monitor",
  order: 40,
  icon: "M3 8h18v9H3zM7 8v9M11 8v9M15 8v9M5 17v3M19 17v3",
  color: "var(--mem)",
  value: () => {
    const s = live.sample;
    return s ? pct((s.memUsed / s.memTotal) * 100) : "";
  },
  sources: ["sysinfo", "linux"],
  page: Page,
});
