import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { rate } from "$lib/format";
import Page from "./Network.svelte";

export default defineTool({
  id: "network",
  label: "Network",
  section: "monitor",
  order: 60,
  icon: "M12 3v6M5 15v-3h14v3M3 15h4v6H3zM10 15h4v6h-4zM17 15h4v6h-4zM9 3h6v6H9z",
  color: "var(--net)",
  value: () => {
    const s = live.sample;
    return s ? rate(s.nets.reduce((a, n) => a + n.rx + n.tx, 0)) : "";
  },
  sources: ["sysinfo", "iproute2", "ookla", "librespeed"],
  page: Page,
});
