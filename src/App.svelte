<script lang="ts">
  import { setContext, type Component } from "svelte";
  import Icon from "./lib/Icon.svelte";
  import UpdateBanner from "./lib/UpdateBanner.svelte";
  import { updates } from "./lib/updates.svelte";
  import { live } from "./lib/live.svelte";
  import About from "./lib/pages/About.svelte";
  import Settings from "./lib/pages/Settings.svelte";
  import { settings } from "./lib/settings.svelte";
  import { sections as toolSections, tools } from "./tools/registry";

  type Page = { id: string; label: string; icon: string; component: Component; color?: string; value?: () => string };

  const about: Page = { id: "about", label: "About & Credits", icon: "about", component: About };
  // Tools draw their own icon path; the app's own pages name one of Icon's.
  const sections: { title: string; pages: (Page & { path?: string })[] }[] = [
    ...toolSections.map((sec) => ({
      title: sec.title,
      pages: tools
        .filter((t) => t.section === sec.id)
        .map((t) => ({ id: t.id, label: t.label, icon: "", path: t.icon, component: t.page, color: t.color, value: t.value })),
    })),
    { title: "", pages: [about] },
  ];
  const all = sections.flatMap((x) => x.pages);

  let current = $state(
    location.hash.slice(1) || (settings.startPage === "last" ? settings.lastPage : settings.startPage) || "overview",
  );
  // Settings is not in the page list: it opens from the gear and needs the list itself.
  const page = $derived(current === "settings" ? null : (all.find((p) => p.id === current) ?? all[0]));
  // The page header finds its tool's sources through this.
  setContext("otm.tool", () => tools.find((t) => t.id === page?.id));

  function go(id: string) {
    current = id;
    settings.lastPage = id;
    history.replaceState(null, "", `#${id}`);
    main?.scrollTo({ top: 0 });
  }

  let main = $state<HTMLElement>();
  live.start();
  updates.start();

  // Ctrl+1…9 jumps between pages; Ctrl+, opens Settings.
  function onkeydown(e: KeyboardEvent) {
    if (!e.ctrlKey || e.altKey) return;
    if (e.key === ",") {
      e.preventDefault();
      go("settings");
      return;
    }
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
        <path d="M4.3 17.6h5q1 0 1.6-1.4t1.5 0q.6 1.4 1.4 1.4l1.1 1.6 2.4-12.8 2.5 17.7 1.5-6.5h.8q1.2 0 2-2.1t1.5 0q.7 2.1 2.2 2.1" />
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
            <span class="icon" style:color={p.color}><Icon name={p.icon} path={p.path} /></span>
            <span class="label">{p.label}</span>
            {#if p.value}<span class="value">{p.value()}</span>{/if}
          </button>
        {/each}
      {/each}
    </div>
    <div class="foot">
      <button class="item" aria-current={current === "settings" ? "page" : undefined} onclick={() => go("settings")} title="Settings (Ctrl+,)">
        <span class="icon"><Icon name="settings" /></span>
        <span class="label">Settings</span>
        {#if updates.offer}<span class="dot" title="An update is available"></span>{/if}
      </button>
      <p class="faint">Free software · GPL-3.0-or-later</p>
    </div>
  </nav>

  <main bind:this={main}>
    <UpdateBanner />
    {#key current}
      {#if page}
        <page.component />
      {:else}
        <Settings pages={all} />
      {/if}
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

  .foot {
    padding: 8px 10px 14px;
    border-top: 1px solid var(--border);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }

  .foot p {
    margin: 6px 10px 0;
    font-size: 11.5px;
  }

  main {
    overflow-y: auto;
    padding: 26px 30px 40px;
    min-width: 0;
  }
</style>
