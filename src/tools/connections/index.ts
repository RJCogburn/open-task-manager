import { defineTool } from "$lib/tool";
import Page from "./Connections.svelte";

export default defineTool({
  id: "connections",
  label: "Connections",
  section: "tools",
  order: 10,
  icon: "M5 12a7 7 0 0 1 14 0M8.5 12a3.5 3.5 0 0 1 7 0M12 12v8M2 12h2M20 12h2",
  sources: ["iproute2"],
  page: Page,
});
