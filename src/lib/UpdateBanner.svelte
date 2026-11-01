<!-- The strip at the top of the window: asks once whether to check for
     updates, then says when a new version is out and installs it. -->
<script lang="ts">
  import { api } from "./api";
  import { bytes } from "./format";
  import { external } from "./link";
  import { settings } from "./settings.svelte";
  import { RELEASES, updates } from "./updates.svelte";

  const offer = $derived(updates.offer);
  const status = $derived(updates.status);
  const pct = $derived(
    updates.progress?.total ? Math.min(100, (updates.progress.done / updates.progress.total) * 100) : null,
  );

  function answer(yes: boolean) {
    settings.updates.autoCheck = yes;
    if (yes) void updates.check();
  }
</script>

{#if updates.installed}
  <div class="banner ok" role="status">
    <span><b>Update installed.</b> The new version takes over the next time Open Task Manager starts.</span>
    <button class="btn primary" onclick={() => api.appRestart()}>Restart now</button>
  </div>
{:else if updates.installing}
  <div class="banner" role="status">
    <span class="spinner" style:color="var(--accent)"></span>
    <span>
      Installing version {status?.available?.version}…
      {#if updates.progress}{bytes(updates.progress.done)}{#if updates.progress.total} of {bytes(updates.progress.total)}{/if}{/if}
    </span>
    {#if pct != null}<span class="bar" style="flex: 1; max-width: 240px"><i style:width="{pct}%"></i></span>{/if}
  </div>
{:else if offer && status}
  <div class="banner" role="status">
    <span>
      <b>Open Task Manager {offer.version} is available.</b> You have {status.current}.
      {#if status.method === "package-manager"}Update it the way you installed it: for the Arch package, <code>git pull</code> and
        <code>makepkg -si</code> again.{/if}
      {#if status.method === "development"}This is a development build; pull the latest code to update.{/if}
    </span>
    <a class="btn" href="{RELEASES}/tag/v{offer.version}" use:external>What's new</a>
    {#if status.canInstall}
      <button class="btn primary" onclick={() => updates.install()}>Update now</button>
    {/if}
    <button class="btn" onclick={() => (settings.updates.dismissed = offer.version)}>Not now</button>
  </div>
  {#if updates.error}<div class="banner bad" role="alert">{updates.error}</div>{/if}
{:else if settings.updates.autoCheck === null}
  <div class="banner" role="status">
    <span>
      <b>Check for updates automatically?</b> Open Task Manager would ask GitHub once a day whether a newer version
      is out. Nothing about you or this computer is sent.
    </span>
    <button class="btn primary" onclick={() => answer(true)}>Yes, check daily</button>
    <button class="btn" onclick={() => answer(false)}>No</button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 14px;
    padding: 10px 14px;
    margin-bottom: 18px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent) 10%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    font-size: 13px;
  }

  .banner > span:first-child,
  .banner > span:nth-child(2) {
    flex: 1;
    min-width: 260px;
  }

  .banner.ok {
    background: color-mix(in srgb, var(--ok) 10%, var(--panel));
    border-color: color-mix(in srgb, var(--ok) 35%, transparent);
  }

  .banner.bad {
    background: color-mix(in srgb, var(--bad) 9%, var(--panel));
    border-color: color-mix(in srgb, var(--bad) 30%, transparent);
  }

  a.btn {
    text-decoration: none;
    color: inherit;
  }
</style>
