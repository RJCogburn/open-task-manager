<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import PageHeader from "../PageHeader.svelte";
  import { api, type ToolStatus } from "../api";
  import { credits, groupTitles, type Credit, type CreditGroup } from "../credits";
  import { external } from "../link";

  let tools = $state<ToolStatus[]>([]);
  let version = $state("");
  getVersion().then((v) => (version = v));
  api.toolStatus().then((t) => (tools = t));

  const groups = Object.keys(groupTitles) as CreditGroup[];
  const all: Credit[] = Object.values(credits);
  const installed = (c: Credit) => tools.find((t) => t.program === c.program);
</script>

<PageHeader title="About & Credits" subtitle="Open Task Manager is free software, built on the work of these projects." />

<section class="card intro">
  <div>
    <h2>Open Task Manager {#if version}<span class="faint version">version {version}</span>{/if}</h2>
    <p>
      A free task manager for Linux that brings together tools your system already has: live graphs for CPU, memory,
      disks, network and graphics cards; a process manager; sensors and power; network connections; a disk space
      analyser; benchmarks; and a list of installed software. No account, no telemetry, nothing held back for a paid
      version.
    </p>
    <p>
      Licensed under the
      <a href="https://www.gnu.org/licenses/gpl-3.0.html" use:external>GNU General Public License, version 3 or later</a>. You may use, study,
      share and change it. If you share a changed version, share its source under the same license.
    </p>
  </div>
  <span class="badge ok license">GPL-3.0-or-later</span>
</section>

{#each groups as group (group)}
  {@const items = all.filter((c) => c.group === group)}
  <h2 class="group">{groupTitles[group].title}</h2>
  <p class="muted blurb">{groupTitles[group].blurb}</p>
  <div class="table-wrap">
    <table class="data">
      <tbody>
        {#each items as c (c.name)}
          {@const t = installed(c)}
          <tr>
            <td class="name"><a href={c.url} use:external>{c.name}</a></td>
            <td class="role">{c.role}</td>
            <td><span class="badge" class:warn={c.proprietary}>{c.license}</span></td>
            <td class="num">
              {#if t}
                <span class="badge" class:ok={t.installed} title={!t.installed && t.installHint ? t.installHint : ""}>
                  {t.installed ? "Installed" : "Not installed"}
                </span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/each}

<p class="faint foot">
  Each project's name and logo belong to its authors. Open Task Manager is not affiliated with or endorsed by any of
  them.
</p>

<style>
  .version {
    font-size: 13px;
    font-weight: 500;
    margin-left: 6px;
  }

  .intro {
    display: flex;
    gap: 20px;
    align-items: flex-start;
    justify-content: space-between;
  }

  .intro h2 {
    margin: 0 0 6px;
    font-size: 18px;
  }

  .intro p {
    margin: 0 0 10px;
    max-width: 760px;
    color: var(--muted);
  }

  .license {
    font-size: 13px;
    padding: 5px 12px;
  }

  .group {
    font-size: 15px;
    margin: 26px 0 2px;
  }

  .blurb {
    font-size: 13px;
    margin: 0 0 10px;
  }

  .name {
    width: 210px;
    font-weight: 600;
  }

  .name a {
    text-decoration: none;
  }

  .name a:hover {
    text-decoration: underline;
  }

  .role {
    white-space: normal !important;
    color: var(--muted);
    max-width: none !important;
  }

  .foot {
    font-size: 12px;
    margin-top: 22px;
  }
</style>
