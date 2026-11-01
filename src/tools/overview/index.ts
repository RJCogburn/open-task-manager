import { defineTool } from "$lib/tool";
import Page from "./Overview.svelte";

export default defineTool({
  id: "overview",
  label: "Overview",
  section: "monitor",
  order: 10,
  icon: "M3 13h7V3H3zM14 21h7v-10h-7zM3 21h7v-5H3zM14 7h7V3h-7z",
  page: Page,
});
