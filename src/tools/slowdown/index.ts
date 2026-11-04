import { defineTool } from "$lib/tool";
import Page from "./Slowdown.svelte";

export default defineTool({
  id: "slowdown",
  label: "Slowdown check",
  section: "monitor",
  order: 15,
  icon: "M3.5 17a9 9 0 1 1 17 0M12 14l4.5-4.5M12 14h.01",
  color: "var(--warn)",
  sources: ["linux", "systemd", "powerProfiles", "smartmontools", "nvidiaSmi"],
  page: Page,
});
