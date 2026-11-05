<!--
  Startup: everything that starts with the computer or your session, in one
  list, with a switch each, how long it kept start-up busy and the memory it
  uses now. Switching off takes effect at the next start-up or login; what's
  running keeps running.
-->
<script lang="ts">
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, parseError, type StartupItem, type StartupList } from "$lib/api";
  import { bytes } from "$lib/format";

  let list = $state<StartupList | null>(null);
  let error = $state("");
  let query = $state("");
  let showOther = $state(false);
  let busy = $state("");
  let note = $state<{ id: string; text: string; bad?: boolean } | null>(null);

  async function load() {
    try {
      list = await api.startupList();
      error = "";
    } catch (e) {
      error = parseError(e).message;
    }
  }
  load();

  async function toggle(item: StartupItem) {
    const on = !item.enabled;
    if (!on && item.caution && !confirm(`Turn off ${item.name}?\n\n${item.caution}`)) return;
    busy = item.id;
    note = null;
    try {
      note = { id: item.id, text: await api.startupSet(item.id, on) };
      await load();
    } catch (e) {
      const message = parseError(e).message;
      if (message !== "Cancelled.") note = { id: item.id, text: message, bad: true };
    } finally {
      busy = "";
    }
  }

  const groups: { kind: StartupItem["kind"]; title: string; about: string }[] = [
    { kind: "app", title: "Apps that start when you log in", about: "Autostart entries: yours, and the ones your system and desktop add." },
    { kind: "user-service", title: "Your services", about: "Background services that start with your session." },
    { kind: "service", title: "System services", about: "Services that start with the computer, before anyone logs in. Switching them asks for your password." },
    { kind: "cron", title: "Run at start-up by your crontab", about: "Lines starting with @reboot in your crontab." },
  ];

  const matches = (i: StartupItem) => {
    const q = query.trim().toLowerCase();
    return !q || [i.name, i.description, i.command].some((s) => s.toLowerCase().includes(q));
  };
  /** Most costly first: memory in use, then start-up time; switched-off ones last. */
  const cost = (i: StartupItem) => (i.memory ?? 0) / 1e6 + (i.startup ?? 0) * 100;
  const shown = (kind: StartupItem["kind"]) =>
    (list?.items ?? [])
      .filter((i) => i.kind === kind && (showOther || !i.otherDesktop) && matches(i))
      .sort((a, b) => Number(b.enabled) - Number(a.enabled) || cost(b) - cost(a) || a.name.localeCompare(b.name));
  const others = $derived((list?.items ?? []).filter((i) => i.otherDesktop).length);
  const on = $derived((list?.items ?? []).filter((i) => i.enabled && !i.otherDesktop).length);
  const time = (s: number) => (s >= 1 ? `${s.toFixed(1)} s` : `${Math.max(1, Math.round(s * 1000))} ms`);
</script>

<PageHeader title="Startup" subtitle="What starts with your computer and when you log in">
  {#snippet actions()}
    <button class="btn" onclick={load}>Refresh</button>
  {/snippet}
</PageHeader>

{#if error}<div class="notice bad">{error}</div>{/if}

{#if !list}
  <div class="empty"><span class="spinner"></span> Looking…</div>
{:else}
  <div class="bar">
    <input class="input" type="search" placeholder="Search" bind:value={query} aria-label="Search start-up items" />
    <span class="faint">{on} start automatically{list.desktop ? ` on ${list.desktop}` : ""}</span>
    {#if others}
      <label class="faint"><input type="checkbox" bind:checked={showOther} /> Show {others} for other desktops</label>
    {/if}
  </div>
  <p class="faint lead">
    Turning something off takes effect the next time you start the computer or log in; if it's running now, it keeps
    running. Turn it back on here at any time.
  </p>

  {#each groups as g (g.kind)}
    {@const items = shown(g.kind)}
    {#if items.length || (g.kind === "cron" && list.cron && !query)}
      <section class="card group">
        <h3>{g.title}</h3>
        <p class="faint about">{g.about}</p>
        {#if !items.length}
          <p class="muted">Nothing here.</p>
        {/if}
        <ul>
          {#each items as i (i.id)}
            <li class:off={!i.enabled} class:other={i.otherDesktop}>
              <label class="switch" title={i.locked ?? (i.enabled ? "Turn off" : "Turn on")}>
                <input
                  type="checkbox"
                  role="switch"
                  checked={i.enabled}
                  disabled={!!i.locked || busy !== ""}
                  onclick={(e) => {
                    e.preventDefault();
                    toggle(i);
                  }}
                  aria-label="{i.name}: {i.enabled ? 'on' : 'off'}"
                />
                <span></span>
              </label>
              <div class="what">
                <div class="name">
                  <b>{i.name}</b>
                  {#if i.origin === "system" && i.kind === "app"}<span class="tag">System</span>{/if}
                  {#if i.state === "failed"}<span class="tag bad">Failed</span>{/if}
                  {#if i.locked}<span class="tag" title={i.locked}>Needed</span>{:else if i.caution}<span class="tag" title={i.caution}>Careful</span>{/if}
                  {#if i.otherDesktop}<span class="tag">Other desktops only</span>{/if}
                </div>
                {#if i.description}<div class="muted desc">{i.description}</div>{/if}
                {#if i.command}<code title={i.command}>{i.command}</code>{/if}
                {#if i.locked}<div class="faint why">{i.locked}</div>{/if}
                {#if note?.id === i.id}<div class="notice {note.bad ? 'bad' : 'info'}">{note.text}</div>{/if}
              </div>
              <div class="cost">
                {#if i.memory != null}<span title="Memory it uses now">{bytes(i.memory)}</span>{/if}
                {#if i.startup != null}<span class="faint" title="How long it kept start-up busy last time">{time(i.startup)} at start-up</span>{/if}
                {#if i.state === "running" && i.memory == null}<span class="faint">running</span>{/if}
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/each}
{/if}

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
    margin-bottom: 6px;
  }

  .bar input[type="search"] {
    width: 240px;
  }

  .bar label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    cursor: pointer;
  }

  .lead {
    margin: 0 0 14px;
    font-size: 12.5px;
  }

  .group {
    margin-bottom: 14px;
  }

  .about {
    margin: 2px 0 8px;
    font-size: 12.5px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }

  li.off .what b,
  li.off .desc {
    opacity: 0.6;
  }

  li.other {
    opacity: 0.6;
  }

  .what {
    flex: 1;
    min-width: 0;
  }

  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .desc {
    font-size: 12.5px;
    margin-top: 2px;
  }

  code {
    display: block;
    margin-top: 3px;
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .why {
    font-size: 12px;
    margin-top: 3px;
  }

  .notice {
    margin-top: 8px;
  }

  .tag {
    padding: 0 6px;
    border-radius: 4px;
    font-size: 11px;
    border: 1px solid var(--border-strong);
    color: var(--faint);
  }

  .tag.bad {
    color: var(--bad);
    border-color: color-mix(in srgb, var(--bad) 50%, transparent);
  }

  .cost {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    min-width: 110px;
    font-size: 12.5px;
    text-align: right;
  }

  /* An on/off switch drawn over a checkbox. */
  .switch {
    position: relative;
    flex: none;
    width: 36px;
    height: 20px;
    margin-top: 2px;
    cursor: pointer;
  }

  .switch input {
    position: absolute;
    inset: 0;
    opacity: 0;
    margin: 0;
    cursor: inherit;
  }

  .switch span {
    position: absolute;
    inset: 0;
    border-radius: 10px;
    background: var(--grid);
    border: 1px solid var(--border-strong);
    transition: background 0.15s;
    pointer-events: none;
  }

  .switch span::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--muted);
    transition: transform 0.15s, background 0.15s;
  }

  .switch input:checked + span {
    background: color-mix(in srgb, var(--accent) 45%, transparent);
    border-color: var(--accent);
  }

  .switch input:checked + span::after {
    transform: translateX(16px);
    background: var(--accent);
  }

  .switch input:disabled + span {
    opacity: 0.45;
  }

  .switch input:disabled {
    cursor: not-allowed;
  }

  .switch input:focus-visible + span {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
