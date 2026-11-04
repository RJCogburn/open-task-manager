<!--
  Offers to install a program a page needs, with the system's package
  manager. Polkit asks for the password; nothing installs without it.
-->
<script lang="ts">
  import { api, parseError } from "./api";

  let {
    program,
    name,
    why,
    hint,
    oninstalled,
  }: {
    /** The program, as the Rust side knows it, e.g. "lynis". */
    program: string;
    /** Its name for people, e.g. "Lynis". */
    name: string;
    /** What it is needed for. */
    why: string;
    /** The command it would run, to show. */
    hint: string | null;
    oninstalled: () => void;
  } = $props();

  let installing = $state(false);
  let error = $state("");

  async function install() {
    installing = true;
    error = "";
    try {
      await api.installTool(program);
      oninstalled();
    } catch (e) {
      const message = parseError(e).message;
      if (message !== "Cancelled.") error = message;
    } finally {
      installing = false;
    }
  }
</script>

<section class="card install">
  <h3>{name} isn't installed</h3>
  <p>{why} Would you like to install it now?</p>
  {#if hint}
    <p class="faint">This runs <code>{hint.replace(/^sudo /, "")}</code> as administrator and asks for your password.</p>
  {/if}
  {#if error}<div class="notice bad">{error}</div>{/if}
  <div class="row">
    <button class="btn primary" onclick={install} disabled={installing || !hint}>
      {#if installing}<span class="spinner"></span> Installing…{:else}Install {name}{/if}
    </button>
  </div>
  {#if !hint}
    <p class="faint">Open Task Manager doesn't know this system's package manager; install {name} with your software centre.</p>
  {/if}
</section>

<style>
  .install {
    margin-bottom: 14px;
    border-left: 3px solid var(--accent);
  }

  .install p {
    max-width: 75ch;
    line-height: 1.55;
  }

  code {
    font-size: 12px;
  }
</style>
