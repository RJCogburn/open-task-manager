import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { temp } from "$lib/format";
import Page from "./Sensors.svelte";

export default defineTool({
  id: "sensors",
  label: "Power & Thermals",
  section: "monitor",
  order: 80,
  icon: "M14 14.8V4a2 2 0 0 0-4 0v10.8a4 4 0 1 0 4 0zM12 9v8",
  color: "var(--temp)",
  value: () => temp(live.sample?.cpuTemp),
  page: Page,
});
