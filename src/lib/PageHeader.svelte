<!-- A page title with links to the projects that page's data comes from: a
     tool's own sources, or the ones passed in. -->
<script lang="ts">
  import { getContext, type Snippet } from "svelte";
  import type { Tool } from "./tool";
  import { credits, type CreditId } from "./credits";
  import { external } from "./link";

  let {
    title,
    subtitle = "",
    sources = [],
    actions,
  }: { title: string; subtitle?: string; sources?: CreditId[]; actions?: Snippet } = $props();

  const tool = getContext<(() => Tool | undefined) | undefined>("otm.tool");
  const shown = $derived(sources.length ? sources : (tool?.()?.sources ?? []));
</script>

<header>
  <div class="text">
    <h1>{title}</h1>
    {#if subtitle}<p class="muted">{subtitle}</p>{/if}
  </div>
  <div class="right">
    {#if shown.length}
      <div class="sources" aria-label="Data sources">
        <span class="faint">Powered by</span>
        {#each shown as id (id)}
          {@const c = credits[id]}
          <a
            class="chip"
            href={c.url}
            use:external
            title="{c.name} · {c.license}. {c.role}">{c.name}<svg viewBox="0 0 12 12" aria-hidden="true"
              ><path d="M4.5 2.5h5v5M9.5 2.5 3 9" /></svg
            ></a
          >
        {/each}
      </div>
    {/if}
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 18px;
  }

  h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  p {
    margin: 3px 0 0;
    font-size: 13px;
  }

  .right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
  }

  .sources {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 9px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--panel);
    color: var(--text);
    text-decoration: none;
    font-weight: 550;
    transition: border-color 0.12s, background 0.12s;
  }

  .chip:hover {
    border-color: var(--accent);
    background: var(--panel-hover);
  }

  .chip svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    opacity: 0.6;
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
