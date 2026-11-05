<script lang="ts">
  import { setContext, type Component } from "svelte";
  import Icon from "./lib/Icon.svelte";
  import UpdateBanner from "./lib/UpdateBanner.svelte";
  import { updates } from "./lib/updates.svelte";
  import { arrange } from "./lib/arrange.svelte";
  import { live } from "./lib/live.svelte";
  import About from "./lib/pages/About.svelte";
  import Settings from "./lib/pages/Settings.svelte";
  import { settings, toolOn } from "./lib/settings.svelte";
  import { allTools, loadPlugins } from "./lib/plugins.svelte";
  import { sections as toolSections } from "./tools/registry";

  type Page = {
    id: string;
    label: string;
    icon: string;
    component: Component<any>;
    color?: string;
    value?: () => string;
    props?: Record<string, unknown>;
  };

  const about: Page = { id: "about", label: "About & Credits", icon: "about", component: About };
  // Tools draw their own icon path; the app's own pages name one of Icon's.
  // Tools switched off in Settings leave the sidebar; a section left empty goes too.
  const sections: { title: string; pages: (Page & { path?: string })[] }[] = $derived([
    ...toolSections
      .map((sec) => ({
        title: sec.title,
        pages: allTools()
          .filter((t) => t.section === sec.id && toolOn(t.id))
          .map((t) => ({ id: t.id, label: t.label, icon: "", path: t.icon, component: t.page, color: t.color, value: t.value, props: t.props })),
      }))
      .filter((sec) => sec.pages.length),
    { title: "", pages: [about] },
  ]);
  const all = $derived(sections.flatMap((x) => x.pages));

  let current = $state(
    location.hash.slice(1) || (settings.startPage === "last" ? settings.lastPage : settings.startPage) || "overview",
  );
  // Settings is not in the page list: it opens from the gear and needs the list itself.
  const page = $derived(current === "settings" ? null : (all.find((p) => p.id === current) ?? all[0]));
  // Pages link to other pages through this.
  setContext("otm.go", (id: string) => go(id));
  // A page's name by its id, for pages that link to others.
  setContext("otm.label", (id: string) => allTools().find((t) => t.id === id)?.label);
  // The page header finds its tool's sources through this.
  setContext("otm.tool", () => allTools().find((t) => t.id === page?.id));

  function go(id: string) {
    current = id;
    arrange.on = false;
    settings.lastPage = id;
    history.replaceState(null, "", `#${id}`);
    main?.scrollTo({ top: 0 });
  }

  let main = $state<HTMLElement>();
  live.start();
  loadPlugins();
  updates.start();

  // Paused graphs: arrows step through time, Space and Esc resume. Space
  // also pauses. Keys typed into fields and menus are left alone.
  function scrubKeys(e: KeyboardEvent): boolean {
    if (e.ctrlKey || e.altKey || e.metaKey) return false;
    const field = (e.target as HTMLElement).closest("input, textarea, select, button, [contenteditable]");
    if (e.key === "Escape" && live.paused) live.resume();
    else if (field) return false;
    else if (e.key === " ") live.paused ? live.resume() : live.pause();
    else if (live.paused && (e.key === "ArrowLeft" || e.key === "ArrowRight"))
      live.step((e.key === "ArrowLeft" ? 1 : -1) * (e.shiftKey ? 10 : 1));
    else return false;
    e.preventDefault();
    return true;
  }

  const pausedAt = $derived(live.paused && live.cursor != null ? live.timeAt(live.cursor) : null);

  // Ctrl+1…9 jumps between pages; Ctrl+, opens Settings.
  function onkeydown(e: KeyboardEvent) {
    if (scrubKeys(e)) return;
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
    {#if live.paused}
      <div class="paused" role="status">
        <span class="mark"></span>
        <b>Paused</b>
        <span>Showing {pausedAt == null ? "an earlier moment" : new Date(pausedAt).toLocaleTimeString()}{live.cursor ? ` · ${live.cursor} ${live.cursor === 1 ? "sample" : "samples"} before the pause` : ""}</span>
        <span class="faint keys">← → step · Shift+← → 10 at a time · click or drag a graph · Esc resumes</span>
        <button class="btn" onclick={() => live.resume()}>Resume</button>
      </div>
    {/if}
    {#key current}
      {#if page}
        <page.component {...page.props} />
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

  .paused {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 12px;
    margin: -26px -30px 18px;
    padding: 9px 30px;
    background: color-mix(in srgb, var(--accent) 14%, var(--panel));
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    font-size: 13px;
  }

  .paused .mark {
    width: 10px;
    height: 10px;
    border-left: 3px solid var(--accent);
    border-right: 3px solid var(--accent);
  }

  .paused .keys {
    font-size: 12px;
  }

  .paused button {
    margin-left: auto;
  }

  main {
    overflow-y: auto;
    padding: 26px 30px 40px;
    min-width: 0;
  }
</style>
