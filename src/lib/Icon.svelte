<!-- Simple line icons for the sidebar. -->
<script lang="ts">
  let { name }: { name: string } = $props();

  const paths: Record<string, string> = {
    overview: "M3 13h7V3H3zM14 21h7v-10h-7zM3 21h7v-5H3zM14 7h7V3h-7z",
    processes: "M4 6h16M4 12h16M4 18h10",
    cpu: "M7 7h10v10H7zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4",
    memory: "M3 8h18v9H3zM7 8v9M11 8v9M15 8v9M5 17v3M19 17v3",
    disks: "M4 5h16v6H4zM4 13h16v6H4zM8 8h.01M8 16h.01",
    network: "M12 3v6M5 15v-3h14v3M3 15h4v6H3zM10 15h4v6h-4zM17 15h4v6h-4zM9 3h6v6H9z",
    gpu: "M2 7h20v10H2zM6 17v3M10 17v3M8 12a3 3 0 1 0 6 0 3 3 0 1 0-6 0M18 10h1M18 14h1",
    sensors: "M14 14.8V4a2 2 0 0 0-4 0v10.8a4 4 0 1 0 4 0zM12 9v8",
    connections: "M5 12a7 7 0 0 1 14 0M8.5 12a3.5 3.5 0 0 1 7 0M12 12v8M2 12h2M20 12h2",
    space: "M12 3a9 9 0 1 0 9 9h-9zM15 3.5V9h5.5A9 9 0 0 0 15 3.5z",
    bench: "M4 20a8 8 0 1 1 16 0M12 20l4-6M4 20h16",
    apps: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
    settings:
      "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z",
    about: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 16v-5M12 8h.01",
  };
</script>

<svg viewBox="0 0 24 24" aria-hidden="true"><path d={paths[name] ?? ""} /></svg>

<style>
  svg {
    width: 18px;
    height: 18px;
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
