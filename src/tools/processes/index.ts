import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import Page from "./Processes.svelte";

export default defineTool({
  id: "processes",
  label: "Processes",
  section: "monitor",
  order: 20,
  icon: "M4 6h16M4 12h16M4 18h10",
  value: () => (live.sample ? `${live.sample.processes}` : ""),
  page: Page,
});
