<!--
  Offers to install a program a page needs, with the system's package
  manager. Polkit asks for the password; nothing installs without it.
  Programs the app can't install itself (or on systems whose package
  manager it doesn't know) get the command to run instead.

  As a card (the default) for a page that can't work without the program,
  or `compact`, a one-line notice for a feature within a page.
-->
<script module lang="ts">
  import { api, parseError, type ToolStatus } from "./api";

  // Looked up once for every prompt on the page; refreshed after installing.
  let statuses: Promise<ToolStatus[]> | null = null;
  const lookup = () => (statuses ??= api.toolStatus());
</script>

<script lang="ts">
  let {
    program,
    name = program,
    why = "",
    compact = false,
    oninstalled,
  }: {
    /** The program, as the Rust side knows it, e.g. "smartctl". */
    program: string;
    /** Its name for people, e.g. "smartmontools". */
    name?: string;
    /** What it is needed for. */
    why?: string;
    compact?: boolean;
    oninstalled?: () => void;
  } = $props();

  let status = $state<ToolStatus | null>(null);
  let installing = $state(false);
  let error = $state("");
  $effect(() => {
    const p = program;
    lookup().then((all) => (status = all.find((t) => t.program === p) ?? null));
  });

  const hint = $derived(status?.installHint ?? null);
  const canInstall = $derived(status?.installable ?? false);

  async function install() {
    installing = true;
    error = "";
    try {
      await api.installTool(program);
      statuses = null;
      oninstalled?.();
    } catch (e) {
      const message = parseError(e).message;
      if (message !== "Cancelled.") error = message;
    } finally {
      installing = false;
    }
  }
</script>

{#snippet button()}
  <button class="btn primary" onclick={install} disabled={installing}>
    {#if installing}<span class="spinner"></span> Installing…{:else}Install {name}{/if}
  </button>
{/snippet}

{#if compact}
  <div class="notice compact">
    <span>
      Needs <b>{name}</b>, which isn't installed.{why ? ` ${why}` : ""}
      {#if !canInstall && hint}Install it with <code>{hint}</code>{/if}
    </span>
    {#if canInstall}{@render button()}{/if}
  </div>
  {#if error}<div class="notice bad">{error}</div>{/if}
{:else}
  <section class="card install">
    <h3>{name} isn't installed</h3>
    <p>{why}{canInstall ? " Would you like to install it now?" : ""}</p>
    {#if canInstall && hint}
      <p class="faint">This runs <code>{hint.replace(/^sudo /, "")}</code> as administrator and asks for your password.</p>
    {:else if hint}
      <p>Install it with <code>{hint}</code>, then come back to this page.</p>
    {:else}
      <p class="faint">Open Task Manager doesn't know this system's package manager; install {name} with your software centre.</p>
    {/if}
    {#if error}<div class="notice bad">{error}</div>{/if}
    {#if canInstall}<div class="row">{@render button()}</div>{/if}
  </section>
{/if}

<style>
  .install {
    margin-bottom: 14px;
    border-left: 3px solid var(--accent);
  }

  .install p {
    max-width: 75ch;
    line-height: 1.55;
  }

  .compact {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .compact .btn {
    flex: none;
  }

  code {
    font-size: 12px;
  }
</style>
