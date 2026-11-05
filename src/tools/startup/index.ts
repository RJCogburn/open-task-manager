import { defineTool } from "$lib/tool";
import Page from "./Startup.svelte";

export default defineTool({
  id: "startup",
  label: "Startup",
  section: "tools",
  order: 15,
  icon: "M12 3v8M7.5 6.2a7 7 0 1 0 9 0",
  color: "var(--ok)",
  sources: ["systemd", "polkit", "autostartManager"],
  page: Page,
});
