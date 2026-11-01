import { defineTool } from "$lib/tool";
import Page from "./Apps.svelte";

export default defineTool({
  id: "apps",
  label: "Installed Apps",
  section: "tools",
  order: 40,
  icon: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
  sources: ["pacman", "flatpak", "dpkg", "rpm"],
  page: Page,
});
