<script lang="ts">
  import type { Component } from "svelte";
  import Icon from "./lib/Icon.svelte";
  import { live } from "./lib/live.svelte";
  import { pct, rate, celsius } from "./lib/format";
  import Overview from "./lib/pages/Overview.svelte";
  import Processes from "./lib/pages/Processes.svelte";
  import Cpu from "./lib/pages/Cpu.svelte";
  import Memory from "./lib/pages/Memory.svelte";
  import Disks from "./lib/pages/Disks.svelte";
  import Network from "./lib/pages/Network.svelte";
  import Gpu from "./lib/pages/Gpu.svelte";
  import Sensors from "./lib/pages/Sensors.svelte";
  import Connections from "./lib/pages/Connections.svelte";
  import DiskSpace from "./lib/pages/DiskSpace.svelte";
  import Benchmarks from "./lib/pages/Benchmarks.svelte";
  import Apps from "./lib/pages/Apps.svelte";
  import About from "./lib/pages/About.svelte";

  type Page = { id: string; label: string; icon: string; component: Component; color?: string; value?: () => string };

  const s = $derived(live.sample);
  const sections: { title: string; pages: Page[] }[] = [
    {
      title: "Monitor",
      pages: [
        { id: "overview", label: "Overview", icon: "overview", component: Overview },
        { id: "processes", label: "Processes", icon: "processes", component: Processes, value: () => (s ? `${s.processes}` : "") },
        { id: "cpu", label: "CPU", icon: "cpu", component: Cpu, color: "var(--cpu)", value: () => pct(s?.cpu) },
        { id: "memory", label: "Memory", icon: "memory", component: Memory, color: "var(--mem)", value: () => (s ? pct((s.memUsed / s.memTotal) * 100) : "") },
        { id: "disks", label: "Disks", icon: "disks", component: Disks, color: "var(--disk)", value: () => (s ? rate(s.disks.reduce((a, d) => a + d.read + d.write, 0)) : "") },
        { id: "network", label: "Network", icon: "network", component: Network, color: "var(--net)", value: () => (s ? rate(s.nets.reduce((a, n) => a + n.rx + n.tx, 0)) : "") },
        { id: "gpu", label: "GPU", icon: "gpu", component: Gpu, color: "var(--gpu)", value: () => (s?.gpus[0] ? pct(s.gpus[0].util) : "") },
        { id: "sensors", label: "Power & Thermals", icon: "sensors", component: Sensors, color: "var(--temp)", value: () => celsius(s?.cpuTemp) },
      ],
    },
    {
      title: "Tools",
      pages: [
        { id: "connections", label: "Connections", icon: "connections", component: Connections },
        { id: "space", label: "Disk Space", icon: "space", component: DiskSpace },
        { id: "bench", label: "Benchmarks", icon: "bench", component: Benchmarks },
        { id: "apps", label: "Installed Apps", icon: "apps", component: Apps },
      ],
    },
    { title: "", pages: [{ id: "about", label: "About & Credits", icon: "about", component: About }] },
  ];
  const all = sections.flatMap((x) => x.pages);

  let current = $state(location.hash.slice(1) || "overview");
  const page = $derived(all.find((p) => p.id === current) ?? all[0]);

  function go(id: string) {
    current = id;
    history.replaceState(null, "", `#${id}`);
    main?.scrollTo({ top: 0 });
  }

  let main = $state<HTMLElement>();
  live.start();

  // Ctrl+1…9 jumps between pages.
  function onkeydown(e: KeyboardEvent) {
    if (!e.ctrlKey || e.altKey) return;
    const n = Number(e.key);
    if (n >= 1 && n <= 9 && all[n - 1]) {
      e.preventDefault();
      go(all[n - 1].id);
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="shell">
  <nav aria-label="Pages">
    <div class="brand">
      <svg viewBox="0 0 32 32" aria-hidden="true">
        <rect x="1" y="1" width="30" height="30" rx="8" />
        <path d="M6 18h5l2.5-7 4 13 3-8H26" />
      </svg>
      <div>
        <b>Open Task Manager</b>
        <span class="faint">{live.info?.hostName ?? ""}</span>
      </div>
    </div>
    <div class="scroll">
      {#each sections as section (section.title)}
        {#if section.title}<h2>{section.title}</h2>{:else}<hr />{/if}
        {#each section.pages as p (p.id)}
          <button class="item" aria-current={current === p.id ? "page" : undefined} onclick={() => go(p.id)}>
            <span class="icon" style:color={p.color}><Icon name={p.icon} /></span>
            <span class="label">{p.label}</span>
            {#if p.value}<span class="value">{p.value()}</span>{/if}
          </button>
        {/each}
      {/each}
    </div>
    <div class="settings">
      <label>
        <span>Update every</span>
        <select class="input" value={live.settings.intervalMs} onchange={(e) => live.setIntervalMs(Number(e.currentTarget.value))}>
          <option value={500}>0.5 s</option>
          <option value={1000}>1 s</option>
          <option value={2000}>2 s</option>
          <option value={5000}>5 s</option>
        </select>
      </label>
      <label>
        <span>Theme</span>
        <select class="input" value={live.settings.theme} onchange={(e) => live.setTheme(e.currentTarget.value as "system")}>
          <option value="system">System</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </label>
      <p class="faint">Free software · GPL-3.0-or-later</p>
    </div>
  </nav>

  <main bind:this={main}>
    {#key page.id}
      <page.component />
    {/key}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 236px 1fr;
    height: 100%;
  }

  nav {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 18px 14px;
  }

  .brand svg {
    width: 32px;
    height: 32px;
    flex: none;
  }

  .brand rect {
    fill: color-mix(in srgb, var(--cpu) 18%, transparent);
    stroke: color-mix(in srgb, var(--cpu) 50%, transparent);
  }

  .brand path {
    fill: none;
    stroke: var(--cpu);
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .brand div {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .brand b {
    font-size: 14px;
    letter-spacing: -0.01em;
  }

  .brand span {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 4px 10px;
  }

  h2 {
    margin: 14px 10px 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--faint);
    font-weight: 650;
  }

  hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 12px 8px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    cursor: pointer;
    text-align: left;
    color: var(--muted);
    font-weight: 550;
    position: relative;
    transition: background 0.12s, color 0.12s;
  }

  .item:hover {
    background: var(--panel-hover);
    color: var(--text);
  }

  .item[aria-current="page"] {
    background: var(--panel-hover);
    color: var(--text);
  }

  .item[aria-current="page"]::before {
    content: "";
    position: absolute;
    left: -10px;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
  }

  .icon {
    display: grid;
    color: var(--muted);
  }

  .label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .value {
    font-size: 12px;
    color: var(--faint);
    font-weight: 500;
  }

  .settings {
    padding: 12px 18px 16px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .settings label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12.5px;
    color: var(--muted);
  }

  .settings select {
    height: 28px;
    font-size: 12.5px;
  }

  .settings p {
    margin: 4px 0 0;
    font-size: 11.5px;
  }

  main {
    overflow-y: auto;
    padding: 26px 30px 40px;
    min-width: 0;
  }
</style>
