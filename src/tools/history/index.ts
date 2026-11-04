import { defineTool } from "$lib/tool";
import Page from "./History.svelte";

export default defineTool({
  id: "history",
  label: "History",
  section: "monitor",
  order: 17,
  icon: "M3.5 12a8.5 8.5 0 1 0 2.5-6M3.5 4v4h4M12 7.5V12l3 2",
  color: "var(--accent)",
  sources: ["linux"],
  page: Page,
});
