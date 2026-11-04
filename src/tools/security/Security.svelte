<!--
  Security audit: runs Lynis and shows its hardening index and findings.
  Nothing here changes the system. Security settings changed blindly can
  lock people out, so each finding explains itself, with the check's own
  description and a link to its code on GitHub, instead of offering a button.
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import Gauge from "$lib/Gauge.svelte";
  import InstallPrompt from "$lib/InstallPrompt.svelte";
  import PageHeader from "$lib/PageHeader.svelte";
  import { api, parseError, type Audit, type AuditAction, type AuditChange, type AuditFinding, type SavedAudits } from "$lib/api";
  import { external } from "$lib/link";

  let saved = $state<SavedAudits | null>(null);
  let audit = $state<Audit | null>(null);
  let running = $state<"user" | "admin" | null>(null);
  let started = $state(0);
  let now = $state(Date.now());
  let error = $state("");
  /** Whether Lynis is installed, and the command that would install it. */
  let lynis = $state<{ installed: boolean; hint: string | null } | null>(null);
  async function checkInstalled() {
    const s = (await api.toolStatus()).find((t) => t.program === "lynis");
    lynis = { installed: s?.installed ?? false, hint: s?.installHint ?? null };
  }
  checkInstalled();
  let open = $state<Record<string, boolean>>({});
  let showServer = $state(false);

  // Fixes: what has been changed from here, the action running now, and
  // what each finding's last action said, by finding.
  let changes = $state<AuditChange[]>([]);
  let fixing = $state("");
  let results = $state<Record<string, { text: string; bad?: boolean; output?: string | null }>>({});
  let undoNote = $state<{ text: string; bad?: boolean } | null>(null);
  const loadChanges = async () => (changes = await api.securityChanges());
  loadChanges();
  // Kernel settings count one by one, whether set alone or with others.
  const setNow = $derived(new Set(changes.filter((c) => c.fix === "sysctl").flatMap((c) => c.target?.split(",") ?? [])));
  const isDone = (a: AuditAction) =>
    !a.readOnly &&
    (a.fix === "sysctl"
      ? (a.target?.split(",") ?? []).every((t) => setNow.has(t))
      : changes.some((c) => c.fix === a.fix && c.target === a.target));

  async function act(k: string, a: AuditAction) {
    if (!a.readOnly) {
      const password = a.admin ? "\nIt will ask for your password." : "";
      if (!confirm(`${a.explain}\n\nIt runs: ${a.command}${password}\n\nYou can undo it from this page.`)) return;
    }
    fixing = a.fix + a.target;
    try {
      const r = await api.securityFix(a.fix, a.target);
      results[k] = { text: r.message, output: r.output };
      await loadChanges();
      if (a.fix === "install") await refreshAudit();
    } catch (e) {
      const message = parseError(e).message;
      if (message !== "Cancelled.") results[k] = { text: message, bad: true };
    } finally {
      fixing = "";
    }
  }

  async function undo(i: number) {
    fixing = `undo${i}`;
    undoNote = null;
    try {
      undoNote = { text: await api.securityUndo(i) };
      await loadChanges();
    } catch (e) {
      const message = parseError(e).message;
      if (message !== "Cancelled.") undoNote = { text: message, bad: true };
    } finally {
      fixing = "";
    }
  }

  /** After installing a tool Lynis looks for, the finding changes with the next audit. */
  async function refreshAudit() {
    saved = await api.securityLast();
  }

  api.securityLast().then((s) => {
    saved = s;
    audit ??= s.last;
  });

  const timer = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(timer));

  async function run(admin: boolean) {
    running = admin ? "admin" : "user";
    started = Date.now();
    error = "";
    try {
      audit = await api.securityAudit(admin);
      open = {};
      saved = await api.securityLast();
    } catch (e) {
      const err = parseError(e);
      if (err.kind === "missing-tool") await checkInstalled();
      else if (err.message !== "Cancelled.") error = err.message;
    } finally {
      running = null;
    }
  }

  const elapsed = $derived(Math.max(0, Math.round((now - started) / 1000)));

  /** The score before this one, from a run of the same kind. */
  const previous = $derived.by(() => {
    if (!audit || !saved) return null;
    const earlier = saved.scores.filter(([t, admin]) => t < audit!.time && admin === audit!.admin);
    return earlier.length ? earlier[earlier.length - 1] : null;
  });

  const band = (score: number) =>
    score >= 80
      ? { word: "Well hardened", color: "var(--ok)" }
      : score >= 65
        ? { word: "Reasonably hardened", color: "var(--accent)" }
        : { word: "Room to tighten", color: "var(--warn)" };

  const warnings = $derived(audit?.warnings ?? []);
  const suggestions = $derived((audit?.suggestions ?? []).filter((f) => showServer || !f.server));
  const hidden = $derived((audit?.suggestions ?? []).filter((f) => f.server).length);

  const key = (kind: string, f: AuditFinding) => `${kind}:${f.test}:${f.text}:${f.details ?? ""}`;
  const when = (t: number) => new Date(t).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
</script>

<PageHeader title="Security audit" subtitle="How well this computer is locked down, checked by Lynis">
  {#snippet actions()}
    {#if running === "user"}
      <button class="btn" onclick={() => api.securityCancel()}>Stop</button>
    {/if}
    {#if lynis?.installed}
      <button class="btn" onclick={() => run(false)} disabled={running != null}>Quick audit</button>
      <button class="btn primary" onclick={() => run(true)} disabled={running != null}>Full audit</button>
    {/if}
  {/snippet}
</PageHeader>

{#if lynis && !lynis.installed}
  <InstallPrompt
    program="lynis"
    name="Lynis"
    why="The security audit is done by Lynis, a free, open source auditing tool from CISOfy."
    hint={lynis.hint}
    oninstalled={checkInstalled}
  />
{/if}
{#if error}<div class="notice bad">{error}</div>{/if}

{#if running}
  <section class="card running">
    <span class="spinner"></span>
    <div>
      <b>Auditing… {elapsed} s</b>
      <p class="muted">
        Lynis runs a few hundred checks; this usually takes one to three minutes.
        {#if running === "admin"}Once you have entered your password it runs as administrator, and can't be stopped from here.{/if}
      </p>
    </div>
  </section>
{/if}

{#if !audit && !running}
  <section class="card intro">
    <h3>No audit yet</h3>
    <p>
      Lynis checks how this computer is set up for security: the boot loader, kernel settings, user accounts, the
      firewall, SSH, file permissions, packages with known problems, and more. It gives a hardening score out of 100
      and a list of things that could be tightened.
    </p>
    <p>
      <b>Full audit</b> asks for your password and runs every check. <b>Quick audit</b> needs no password but skips the
      checks that need administrator rights. Neither changes anything; nothing leaves your computer.
    </p>
  </section>
{:else if audit}
  {@const b = audit.score == null ? null : band(audit.score)}
  <section class="card verdict" style:border-left-color={b?.color}>
    {#if audit.score != null && b}
      <Gauge value={audit.score} color={b.color} label="Hardening index" text={String(audit.score)} />
    {/if}
    <div class="summary">
      <div class="big">{b ? b.word : "Audit finished"}</div>
      <p class="muted">
        Hardening index {audit.score ?? "—"} out of 100{#if previous}, {audit.score! > previous[2]
            ? `up from ${previous[2]}`
            : audit.score! < previous[2]
              ? `down from ${previous[2]}`
              : "the same as"} on {when(previous[0])}{/if}.
        {warnings.length} {warnings.length === 1 ? "warning" : "warnings"}, {audit.suggestions.length}
        {audit.suggestions.length === 1 ? "suggestion" : "suggestions"}.
      </p>
      <p class="faint">
        {audit.admin ? "Full audit" : "Quick audit, without administrator rights: some checks were skipped"} ·
        {when(audit.time)}{audit.tests ? ` · ${audit.tests} checks` : ""}{audit.version ? ` · Lynis ${audit.version}` : ""}
      </p>
      <p class="faint">
        Desktops commonly score in the 60s; the score is a guide, not a grade. Most suggestions are worth reading, few are
        urgent.
      </p>
    </div>
  </section>

  {#snippet list(kind: "warning" | "suggestion", items: AuditFinding[])}
    <div class="list">
      {#each items as f (key(kind, f))}
        {@const k = key(kind, f)}
        {@const isOpen = open[k] ?? false}
        <section class="card item {kind}">
          <button class="head" aria-expanded={isOpen} onclick={() => (open[k] = !isOpen)}>
            <span class="dot" aria-hidden="true"></span>
            <span class="body">
              <h3>{f.text}</h3>
              <span class="detail">
                {f.test}{f.details ? ` · ${f.details}` : ""}{#if f.actions?.some((a) => !a.readOnly)}<span
                    class="tag fixable">{f.actions.every((a) => a.readOnly || isDone(a)) ? "Fixed" : "Can fix"}</span
                  >{:else if f.advice}<span class="tag">Advice</span>{/if}{#if f.note}<span class="tag">{f.note}</span>{/if}{#if f.server}<span class="tag">Mostly for servers</span>{/if}
              </span>
            </span>
            <span class="chev" aria-hidden="true">{isOpen ? "▾" : "▸"}</span>
          </button>
          {#if isOpen}
            <div class="more">
              {#if f.about}
                <p><b>What {f.test} checks:</b> {f.about.description}{f.about.notes ? ` (${f.about.notes})` : ""}</p>
              {/if}
              {#if f.details}<p><b>Found:</b> {f.details}</p>{/if}
              {#if f.found?.length}
                <p><b>What Lynis found:</b></p>
                <ul class="found">
                  {#each f.found as line, i (i)}<li class:aside={line.startsWith("  ")}>{line.trim()}</li>{/each}
                </ul>
              {/if}
              {#if f.noteText}<p><b>On this computer:</b> {f.noteText}</p>{/if}
              {#if f.advice}<p><b>What to do:</b> {f.advice}</p>{/if}
              {#if f.solution}<p><b>Lynis suggests:</b> {f.solution}</p>{/if}
              {#if f.server && !f.advice}
                <p class="muted">
                  This is mostly advice for servers. On a desktop it is usually safe to leave, unless it describes something
                  you rely on.
                </p>
              {/if}
              {#each f.actions ?? [] as a (a.fix + a.target)}
                {@const isFixed = isDone(a)}
                <div class="action">
                  <button class="btn" class:primary={!a.readOnly} disabled={fixing !== "" || isFixed} onclick={() => act(k, a)}>
                    {isFixed ? "Done" : fixing === a.fix + a.target ? "Working…" : a.label}
                  </button>
                  <div>
                    <div class="muted">{a.explain}</div>
                    <code>{a.admin ? "pkexec " : ""}{a.command}</code>
                    {#if a.admin}<span class="faint">· asks for your password</span>{/if}
                    {#if isFixed}<span class="faint">· undo it under Changes made here</span>{/if}
                  </div>
                </div>
              {/each}
              {#if results[k]}
                <div class="notice {results[k].bad ? 'bad' : 'info'}">{results[k].text}</div>
                {#if results[k].output}<pre>{results[k].output}</pre>{/if}
              {/if}
              {#if !f.actions?.length && !f.advice && !f.noteText}
                <p class="muted">
                  There's no safe one-click fix for this one.{#if f.about?.url} The check's code shows exactly what Lynis looks
                    at.{/if}
                </p>
              {/if}
              {#if f.about?.url}
                <a class="btn" href={f.about.url} use:external>View {f.test} on GitHub</a>
              {/if}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  {/snippet}

  {#if warnings.length}
    <h2>Warnings</h2>
    {@render list("warning", warnings)}
  {/if}

  {#if audit.suggestions.length}
    <div class="spread heading">
      <h2>Suggestions</h2>
      {#if hidden}
        <label class="faint"><input type="checkbox" bind:checked={showServer} /> Show {hidden} mostly for servers</label>
      {/if}
    </div>
    {@render list("suggestion", suggestions)}
  {/if}
{/if}

{#if changes.length || undoNote}
  <h2>Changes made here</h2>
  <section class="card">
    {#if undoNote}<div class="notice {undoNote.bad ? 'bad' : 'info'}">{undoNote.text}</div>{/if}
    {#if changes.length}
      <p class="muted">Run the audit again to see them in the score. Undo puts each back as it was, and asks for your password.</p>
      <ul class="changes">
        {#each changes as c, i (c.fix + c.target)}
          <li>
            <span>{c.label}<span class="faint"> · {when(c.time)}</span></span>
            <button class="btn" disabled={fixing !== ""} onclick={() => undo(i)}>{fixing === `undo${i}` ? "Undoing…" : "Undo"}</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">Nothing changed from here is left to undo.</p>
    {/if}
  </section>
{/if}

<style>
  .running {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    margin-bottom: 14px;
  }

  .running p {
    margin: 4px 0 0;
  }

  .intro p {
    max-width: 75ch;
    line-height: 1.55;
  }

  .verdict {
    display: flex;
    align-items: center;
    gap: 22px;
    margin-bottom: 18px;
    border-left: 3px solid var(--border);
  }

  .verdict .big {
    font-size: 22px;
  }

  .summary p {
    margin: 6px 0 0;
  }

  h2 {
    margin: 18px 0 10px;
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .heading {
    align-items: center;
  }

  .heading label {
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }

  .list {
    display: grid;
    gap: 10px;
    margin-bottom: 14px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .head .body {
    flex: 1;
    min-width: 0;
  }

  /* Findings are sentences; themes that set headings in capitals would make them hard to read. */
  .item h3 {
    color: var(--text);
    font-size: 14px;
    text-transform: none;
    letter-spacing: normal;
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    margin-top: 4px;
    border-radius: 50%;
    background: var(--accent);
  }

  .warning .dot {
    background: var(--bad);
  }

  .detail {
    display: block;
    margin-top: 4px;
    color: var(--muted);
    word-break: break-word;
  }

  .tag {
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
    border: 1px solid var(--border-strong);
    color: var(--faint);
  }

  .tag.fixable {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }

  .found {
    margin: -4px 0 12px;
    padding-left: 20px;
    font-size: 13px;
  }

  .found li.aside {
    list-style: none;
    margin-left: -2px;
    color: var(--faint);
  }

  .action {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin: 12px 0;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }

  .action .btn {
    flex: none;
  }

  .action code {
    display: inline-block;
    margin-top: 4px;
    font-size: 12px;
    word-break: break-all;
  }

  pre {
    max-height: 320px;
    overflow: auto;
    margin: 0 0 12px;
    padding: 10px 12px;
    font-size: 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }

  .notice {
    margin-bottom: 10px;
  }

  .changes {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .changes li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }

  .chev {
    color: var(--faint);
  }

  .more {
    margin: 12px 0 0 22px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .more p {
    margin: 0 0 10px;
    line-height: 1.55;
    word-break: break-word;
  }
</style>
