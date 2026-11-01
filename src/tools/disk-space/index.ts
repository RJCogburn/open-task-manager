import { defineTool } from "$lib/tool";
import Page from "./DiskSpace.svelte";

export default defineTool({
  id: "space",
  label: "Disk Space",
  section: "tools",
  order: 20,
  icon: "M12 3a9 9 0 1 0 9 9h-9zM15 3.5V9h5.5A9 9 0 0 0 15 3.5z",
  page: Page,
});
