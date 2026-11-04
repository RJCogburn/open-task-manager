<script lang="ts">
  import { board, card } from "$lib/arrange.svelte";
  import Chart from "$lib/Chart.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import ScanResults from "$lib/ScanResults.svelte";
  import { api, parseError, type Drive, type FirmwareReport, type Mount, type WdCheck } from "$lib/api";
  import { external } from "$lib/link";
  import { bytes, temp, count, pct, rate } from "$lib/format";
  import { live, poll } from "$lib/live.svelte";
  import { scan } from "$lib/scan.svelte";

  const s = $derived(live.sample);
  let drives = $state<Drive[]>([]);
  let mounts = $state<Mount[]>([]);
  let error = $state("");
  let healthError = $state("");
  let readingHealth = $state(false);

  const load = async () => {
    try {
      drives = await api.drives();
      error = "";
    } catch (e) {
      error = parseError(e).message;
    }
  };
  poll(load, () => 15_000);
  poll(async () => (mounts = await api.mounts()), () => 15_000);

  // Firmware is read once: asking fwupd takes a moment.
  let firmware = $state<FirmwareReport | null>(null);
  let firmwareBusy = $state("");
  let firmwareNote = $state<{ text: string; bad?: boolean } | null>(null);
  const loadFirmware = async () => {
    try {
      firmware = await api.firmware();
    } catch (e) {
      firmwareNote = { text: parseError(e).message, bad: true };
    }
  };
  loadFirmware();

  async function refreshFirmware() {
    firmwareBusy = "refresh";
    firmwareNote = null;
    try {
      await api.firmwareRefresh();
      await loadFirmware();
      firmwareNote = { text: "The firmware list is up to date." };
    } catch (e) {
      firmwareNote = { text: parseError(e).message, bad: true };
    } finally {
      firmwareBusy = "";
    }
  }

  // Western Digital's own firmware, checked only when asked: it contacts WD's server.
  let wd = $state<Record<string, WdCheck>>({});
  const isWd = (d: Drive) => d.transport === "nvme" && firmware?.drives[d.name]?.maker?.name.startsWith("Western Digital");

  async function checkWd(d: Drive) {
    firmwareBusy = `wd:${d.name}`;
    firmwareNote = null;
    try {
      wd[d.name] = await api.wdFirmwareCheck(d.name);
    } catch (e) {
      firmwareNote = { text: parseError(e).message, bad: true };
    } finally {
      firmwareBusy = "";
    }
  }

  async function installWd(d: Drive, version: string) {
    const ok = confirm(
      `Install Western Digital firmware ${version} on the ${d.model}?\n\n` +
        "This uses the same firmware files as WD's own Dashboard, but WD doesn't support installing them this way.\n\n" +
        "• Back up anything important on this drive first.\n" +
        "• Don't turn the computer off while it installs; it takes under a minute.\n" +
        "• The new firmware takes over when you next restart.\n\n" +
        "It will ask for your password.",
    );
    if (!ok) return;
    firmwareBusy = `wd:${d.name}`;
    firmwareNote = null;
    try {
      firmwareNote = { text: await api.wdFirmwareInstall(d.name, version) };
      delete wd[d.name];
    } catch (e) {
      firmwareNote = { text: parseError(e).message, bad: true };
    } finally {
      firmwareBusy = "";
    }
  }

  async function updateFirmware(d: Drive, id: string, version: string) {
    const ok = confirm(
      `Update ${d.model || d.name} to firmware ${version}?\n\n` +
        "Back up anything important on it first. Don't turn the computer off or unplug the drive until it finishes; " +
        "some drives finish installing at the next restart.\n\nfwupd will ask for your password.",
    );
    if (!ok) return;
    firmwareBusy = d.name;
    firmwareNote = null;
    try {
      firmwareNote = { text: await api.firmwareUpdate(id) };
      await loadFirmware();
    } catch (e) {
      firmwareNote = { text: parseError(e).message, bad: true };
    } finally {
      firmwareBusy = "";
    }
  }

  const io = $derived(new Map(s?.disks.map((d) => [d.name, d]) ?? []));
  const onDrives = $derived(new Set(drives.flatMap((d) => d.filesystems.flatMap((f) => f.mountPoints))));
  const otherMounts = $derived(mounts.filter((m) => !onDrives.has(m.mountPoint)));
  const needsAdmin = $derived(drives.some((d) => d.health.needsAdmin));

  /** Mount points on a drive, shortest first so / comes before /home. */
  const targets = (d: Drive) =>
    [...new Set(d.filesystems.flatMap((f) => f.mountPoints))].sort((a, b) => a.length - b.length || a.localeCompare(b));

  function space(d: Drive) {
    const mounted = d.filesystems.filter((f) => f.used != null && f.available != null);
    const used = mounted.reduce((a, f) => a + (f.used ?? 0), 0);
    const available = mounted.reduce((a, f) => a + (f.available ?? 0), 0);
    return { mounted: mounted.length > 0, used, available, share: used + available ? (used / (used + available)) * 100 : 0 };
  }

  const kind = (d: Drive) => (d.transport === "nvme" ? "NVMe SSD" : d.rotational ? "Hard drive" : "SSD");
  const connection = (d: Drive) =>
    ({ nvme: "NVMe", sata: "SATA", ata: "SATA", usb: "USB", sas: "SAS", mmc: "SD card", virtio: "Virtual" })[d.transport] ??
    (d.transport.toUpperCase() || "—");
  const fillColor = (share: number) => (share > 90 ? "var(--bad)" : share > 75 ? "var(--warn)" : "var(--disk)");
  const healthBadge = { good: "ok", warning: "warn", failing: "bad", unknown: "" } as const;
  const healthLabel = { good: "Healthy", warning: "Warning", failing: "Failing", unknown: "Health unknown" } as const;

  // One drive open at a time: the scan behind it is shared with the Disk Space page.
  let expanded = $state<string | null>(null);
  let chosen = $state<Record<string, string>>({});
  const targetOf = (d: Drive) => chosen[d.name] ?? targets(d)[0];

  function toggle(d: Drive) {
    expanded = expanded === d.name ? null : d.name;
    const target = targetOf(d);
    if (expanded && target && !scan.running && scan.scanned !== target) scan.start(target);
  }

  function choose(d: Drive, target: string) {
    chosen[d.name] = target;
    if (!scan.running) scan.start(target);
  }

  async function readHealth() {
    readingHealth = true;
    healthError = "";
    try {
      await api.driveHealthAdmin();
      await load();
    } catch (e) {
      healthError = parseError(e).message;
    } finally {
      readingHealth = false;
    }
  }
</script>

<PageHeader
  title="Disks"
  subtitle="Each drive, how full it is, how healthy it is, and how busy"
/>

{#if error}<div class="notice bad" style="margin-bottom: 14px">{error}</div>{/if}

{#if needsAdmin}
  <div class="notice info spread" style="margin-bottom: 14px">
    <span>Drive health comes from each drive's SMART data, which only an administrator can read.</span>
    <button class="btn" disabled={readingHealth} onclick={readHealth}>
      {#if readingHealth}<span class="spinner"></span>{/if}Read drive health
    </button>
  </div>
{/if}
{#if healthError}<div class="notice bad" style="margin-bottom: 14px">{healthError}</div>{/if}

<div use:board={{ group: "disks", min: 440 }}>
  {#each drives as d (d.name)}
    {@const sp = space(d)}
    {@const h = d.health}
    {@const rw = io.get(d.name)}
    {@const open = expanded === d.name}
    {@const target = targetOf(d)}
    <section class="card drive" use:card={{ group: "disks", id: d.name }} class:open data-wide={open ? "true" : undefined}>
      <div class="spread top">
        <div class="name">
          <h3>/dev/{d.name} · {kind(d)}{d.removable ? " · removable" : ""}</h3>
          <div class="title" title={d.model}>{[d.vendor, d.model].filter(Boolean).join(" ") || d.name}</div>
        </div>
        <span class="badge {healthBadge[h.status]}" title={h.note}>{healthLabel[h.status]}</span>
      </div>

      <div class="space">
        {#if sp.mounted}
          <div class="spread">
            <span><b>{bytes(sp.used)}</b> <span class="muted">used</span></span>
            <span><b>{bytes(sp.available)}</b> <span class="muted">free of {bytes(d.size)}</span></span>
          </div>
          <div class="bar" style="--c: {fillColor(sp.share)}"><i style:width="{sp.share}%"></i></div>
        {:else}
          <span class="muted">{bytes(d.size)}, nothing on it is mounted</span>
        {/if}
      </div>

      <dl class="facts">
        <div><dt>Model</dt><dd>{d.model || "—"}</dd></div>
        <div><dt>Serial</dt><dd class="mono">{d.serial || "—"}</dd></div>
        <div><dt>Firmware</dt><dd>{d.revision || "—"}</dd></div>
        <div><dt>Connection</dt><dd>{connection(d)}</dd></div>
        <div><dt>Temperature</dt><dd>{temp(h.temp)}</dd></div>
        <div><dt>Powered on</dt><dd>{h.powerOnHours != null ? `${count(h.powerOnHours)} hours` : "—"}</dd></div>
        {#if h.wear != null}<div><dt>Endurance used</dt><dd>{pct(h.wear)}</dd></div>{/if}
        {#if h.spare != null}<div><dt>Spare left</dt><dd>{pct(h.spare)}</dd></div>{/if}
        {#if h.badSectors != null}<div><dt>Bad sectors</dt><dd>{count(h.badSectors)}</dd></div>{/if}
        {#if h.mediaErrors != null}<div><dt>Media errors</dt><dd>{count(h.mediaErrors)}</dd></div>{/if}
      </dl>
      {#if h.note}<p class="note" class:faint={h.status === "unknown" || h.status === "good"}>{h.note}</p>{/if}

      {#if firmware?.drives[d.name]}
        {@const fw = firmware.drives[d.name]}
        <div class="firmware">
          {#if fw.issue}
            <div class="notice bad">
              <b>Firmware {fw.version} has a known bug.</b>
              {fw.issue.text}
              <a href={fw.issue.source} use:external>Source</a>
            </div>
          {/if}
          {#if fw.update && fw.fwupdId}
            <div class="spread">
              <span>Firmware <b>{fw.update}</b> is available from fwupd (now {fw.version}).</span>
              <button class="btn primary" disabled={!!firmwareBusy} onclick={() => updateFirmware(d, fw.fwupdId!, fw.update!)}>
                {#if firmwareBusy === d.name}<span class="spinner"></span>Updating…{:else}Update firmware{/if}
              </button>
            </div>
          {:else if fw.maker}
            <span class="faint">
              {fw.maker.name} publishes firmware through {fw.maker.how.replace(/\.$/, "")}.
              <a href={fw.maker.url} use:external>Firmware updates</a>
            </span>
          {/if}
          {#if isWd(d)}
            {@const check = wd[d.name]}
            <div class="spread wd">
              {#if check}
                <span>{check.note}</span>
                {#if check.install}
                  <button class="btn primary" disabled={!!firmwareBusy} onclick={() => installWd(d, check.install!)}>
                    {#if firmwareBusy === `wd:${d.name}`}<span class="spinner"></span>Installing…{:else}Install {check.install}{/if}
                  </button>
                {/if}
              {:else}
                <span class="faint">This app can also install Western Digital's firmware directly.</span>
                <button class="btn" disabled={!!firmwareBusy} onclick={() => checkWd(d)} title="Contacts Western Digital's server">
                  {#if firmwareBusy === `wd:${d.name}`}<span class="spinner"></span>{/if}Check Western Digital
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/if}

      {#if d.filesystems.length}
        <ul class="fs">
          {#each d.filesystems as f (f.device)}
            {@const share = f.used != null && f.total ? (f.used / f.total) * 100 : null}
            <li>
              <span class="where" title={f.mountPoints.join(", ")}>
                <b>{f.mountPoints[0] ?? "Not mounted"}</b>
                {#if f.mountPoints.length > 1}<span class="faint">+{f.mountPoints.length - 1}</span>{/if}
              </span>
              <span class="faint what">{f.label || f.device} · {f.fileSystem}{f.encrypted ? " · encrypted" : ""}</span>
              {#if share != null}
                <span class="bar" style="--c: {fillColor(share)}"><i style:width="{share}%"></i></span>
                <span class="num">{bytes(f.available)} free</span>
              {:else}
                <span></span>
                <span class="num faint">{bytes(f.total)}</span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      {#if rw}
        <div class="io">
          <Chart
            label="{d.name} read and write speed"
            series={[
              { label: "Read", data: live.history(`disk:${d.name}:read`), color: "var(--disk)" },
              { label: "Write", data: live.history(`disk:${d.name}:write`), color: "var(--cpu)" },
            ]}
            tick={live.tick}
            height={90}
            max="auto"
            format={rate}
          />
          <div class="stats">
            <div class="stat"><span><i style:background="var(--disk)"></i>Read</span><b>{rate(rw.read)}</b></div>
            <div class="stat"><span><i style:background="var(--cpu)"></i>Write</span><b>{rate(rw.write)}</b></div>
            <div class="stat"><span>Active time</span><b>{pct(rw.busy)}</b></div>
          </div>
        </div>
      {/if}

      <button class="expand" aria-expanded={open} disabled={!target} onclick={() => toggle(d)}>
        <span class="chev">▸</span>
        {target ? (open ? "Hide what's using the space" : "Show what's using the space") : "Mount it to see what's using the space"}
      </button>

      {#if open && target}
        <div class="browse">
          <div class="spread">
            {#if targets(d).length > 1}
              <div class="seg" role="group" aria-label="Filesystem to look at">
                {#each targets(d) as t (t)}
                  <button aria-pressed={t === target} disabled={scan.running} onclick={() => choose(d, t)}>{t}</button>
                {/each}
              </div>
            {:else}
              <b>{target}</b>
            {/if}
            {#if scan.running}
              <button class="btn danger" onclick={() => api.scanCancel()}>Stop</button>
            {:else}
              <button class="btn" onclick={() => scan.start(target)}>{scan.scanned === target ? "Scan again" : "Scan"}</button>
            {/if}
          </div>
          {#if scan.running}
            <div class="progress">
              <span class="spinner" style:color="var(--accent)"></span>
              {#if scan.scanned === target}
                <span><b>{count(scan.progress?.files ?? 0)}</b> files</span>
                <span><b>{bytes(scan.progress?.bytes ?? 0)}</b> found</span>
              {:else}
                <span class="muted">Finishing the scan of {scan.scanned} first…</span>
              {/if}
            </div>
          {/if}
          {#if scan.error}<div class="notice bad" style="margin-top: 12px">{scan.error}</div>{/if}
          {#if scan.scanned === target && !scan.running}
            {#if scan.progress?.cancelled}
              <p class="faint">Stopped before it finished.</p>
            {:else}
              <ScanResults />
            {/if}
          {/if}
        </div>
      {/if}
    </section>
  {/each}
</div>

{#if firmware}
  <h2 class="section">Firmware updates</h2>
  <section class="card">
    {#if !firmware.fwupd}
      <p class="muted">
        Firmware updates come from fwupd, which isn't installed. Install the <code>fwupd</code> package to get updates
        that drive makers publish for Linux.
      </p>
    {:else}
      <div class="spread">
        <p class="muted" style="margin: 0">
          fwupd installs firmware that makers publish and sign on fwupd.org. Most consumer SSD makers publish only
          through their own tools instead; each drive above says where.
          {firmware.metadataAgeDays == null
            ? "fwupd hasn't fetched its list of firmware yet."
            : firmware.metadataAgeDays === 0
              ? "Its list of firmware was fetched today."
              : `Its list of firmware is ${firmware.metadataAgeDays} day${firmware.metadataAgeDays === 1 ? "" : "s"} old.`}
        </p>
        <button class="btn" disabled={!!firmwareBusy} onclick={refreshFirmware} title="Contacts fwupd.org">
          {#if firmwareBusy === "refresh"}<span class="spinner"></span>{/if}Check fwupd.org for updates
        </button>
      </div>
    {/if}
    {#if firmwareNote}<div class="notice {firmwareNote.bad ? 'bad' : 'info'}" style="margin-top: 12px">{firmwareNote.text}</div>{/if}
  </section>
{/if}

{#if otherMounts.length}
  <h2 class="section">Other filesystems</h2>
  <div class="table-wrap">
    <table class="data">
      <thead>
        <tr>
          <th><span>Mounted at</span></th>
          <th><span>Device</span></th>
          <th><span>Type</span></th>
          <th style="width: 34%"><span>Used</span></th>
          <th class="num"><span>Free</span></th>
          <th class="num"><span>Size</span></th>
        </tr>
      </thead>
      <tbody>
        {#each otherMounts as m (m.mountPoint)}
          {@const used = m.total ? ((m.total - m.available) / m.total) * 100 : 0}
          <tr>
            <td><b>{m.mountPoint}</b> {#if m.removable}<span class="badge">removable</span>{/if}</td>
            <td class="muted">{m.device}</td>
            <td class="muted">{m.fileSystem}</td>
            <td>
              <div class="row">
                <div class="bar" style="flex: 1; --c: {fillColor(used)}"><i style:width="{used}%"></i></div>
                <span style="width: 44px; text-align: right">{pct(used)}</span>
              </div>
            </td>
            <td class="num">{bytes(m.available)}</td>
            <td class="num">{bytes(m.total)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .firmware {
    margin-top: 10px;
    font-size: 13px;
  }

  .firmware .notice {
    margin-bottom: 8px;
  }

  .firmware .wd {
    margin-top: 8px;
  }

  .top {
    align-items: flex-start;
  }

  .name {
    min-width: 0;
  }

  .title {
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .space {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 14px;
    font-size: 13px;
  }

  .space .bar {
    height: 8px;
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 10px 18px;
    margin: 14px 0 0;
  }

  .facts dt {
    font-size: 11.5px;
    color: var(--muted);
  }

  .facts dd {
    margin: 1px 0 0;
    font-size: 13px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .note {
    font-size: 12px;
    margin: 10px 0 0;
  }

  .fs {
    list-style: none;
    margin: 14px 0 0;
    padding: 10px 0 0;
    border-top: 1px solid var(--border);
  }

  .fs li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) 70px 90px;
    gap: 10px;
    align-items: center;
    font-size: 12.5px;
    padding: 3px 0;
  }

  .where,
  .what {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .fs .num {
    text-align: right;
  }

  .io {
    margin-top: 14px;
  }

  .stat i {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 5px;
  }

  .expand {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin-top: 14px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    font-size: 13px;
    font-weight: 550;
    cursor: pointer;
    text-align: left;
  }

  .expand:hover:not(:disabled) {
    background: var(--panel-hover);
  }

  .expand:disabled {
    cursor: default;
    color: var(--faint);
  }

  .chev {
    display: inline-block;
    color: var(--disk);
    transition: transform 0.15s;
  }

  .expand[aria-expanded="true"] .chev {
    transform: rotate(90deg);
  }

  .browse {
    margin-top: 14px;
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-top: 12px;
    font-size: 13px;
  }

  .section {
    font-size: 15px;
    margin: 26px 0 10px;
  }
</style>
