import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { pct } from "$lib/format";
import Page from "./Cpu.svelte";

export default defineTool({
  id: "cpu",
  label: "CPU",
  section: "monitor",
  order: 30,
  icon: "M7 7h10v10H7zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4",
  color: "var(--cpu)",
  value: () => pct(live.sample?.cpu),
  sources: ["sysinfo", "linux"],
  page: Page,
});
