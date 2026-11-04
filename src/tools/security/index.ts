import { defineTool } from "$lib/tool";
import Page from "./Security.svelte";

export default defineTool({
  id: "security",
  label: "Security audit",
  section: "tools",
  order: 35,
  icon: "M12 3 5 6v5c0 4.5 3 8.3 7 10 4-1.7 7-5.5 7-10V6zM9 12l2 2 4-4",
  color: "var(--ok)",
  sources: ["lynis", "polkit"],
  page: Page,
});
