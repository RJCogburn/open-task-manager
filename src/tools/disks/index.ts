import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { rate } from "$lib/format";
import Page from "./Disks.svelte";

export default defineTool({
  id: "disks",
  label: "Disks",
  section: "monitor",
  order: 50,
  icon: "M4 5h16v6H4zM4 13h16v6H4zM8 8h.01M8 16h.01",
  color: "var(--disk)",
  value: () => {
    const s = live.sample;
    return s ? rate(s.disks.reduce((a, d) => a + d.read + d.write, 0)) : "";
  },
  page: Page,
});
