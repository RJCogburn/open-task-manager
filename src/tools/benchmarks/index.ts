import { defineTool } from "$lib/tool";
import Page from "./Benchmarks.svelte";

export default defineTool({
  id: "bench",
  label: "Benchmarks",
  section: "tools",
  order: 30,
  icon: "M4 20a8 8 0 1 1 16 0M12 20l4-6M4 20h16",
  page: Page,
});
