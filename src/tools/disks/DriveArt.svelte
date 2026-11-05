<script lang="ts">
  // A faint drawing of the kind of drive behind each drive's card: an M.2
  // stick, an opened hard drive, an external drive, a 2.5" SSD or an SD card.
  // Line art in the theme's disk colour, so it follows the theme.
  export type ArtKind = "nvme" | "hdd" | "external" | "ssd" | "sd";
  let { kind }: { kind: ArtKind } = $props();
</script>

<svg class="art" viewBox="0 0 200 140" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
  {#if kind === "nvme"}
    <g transform="rotate(-14 100 70)">
      <!-- The board, with its key notch and gold fingers on the left, and the screw notch on the right. -->
      <path d="M14 48 h170 a4 4 0 0 1 4 4 v12 a6 6 0 0 0 0 12 v12 a4 4 0 0 1 -4 4 h-170 v-14 h-4 v-6 h4 z" />
      {#each Array(12) as _, i (i)}
        <line x1="14" x2="22" y1={52 + i * 3.2} y2={52 + i * 3.2} stroke-width="1.4" opacity="0.8" />
      {/each}
      <!-- Controller, memory chips and the label. -->
      <rect x="34" y="56" width="26" height="28" rx="2" fill="currentColor" fill-opacity="0.12" />
      <rect x="40" y="62" width="14" height="16" rx="1" opacity="0.6" />
      <rect x="68" y="58" width="16" height="24" rx="1.5" fill="currentColor" fill-opacity="0.08" />
      <rect x="92" y="54" width="36" height="32" rx="2" fill="currentColor" fill-opacity="0.12" />
      <rect x="134" y="54" width="36" height="32" rx="2" fill="currentColor" fill-opacity="0.12" />
      <line x1="98" x2="122" y1="64" y2="64" opacity="0.6" />
      <line x1="98" x2="114" y1="70" y2="70" opacity="0.6" />
      <line x1="140" x2="164" y1="64" y2="64" opacity="0.6" />
      <line x1="140" x2="156" y1="70" y2="70" opacity="0.6" />
      <circle cx="188" cy="70" r="2.4" />
    </g>
  {:else if kind === "hdd"}
    <!-- The case, its corner screws and the connector. -->
    <rect x="22" y="6" width="156" height="128" rx="9" />
    {#each [[32, 16], [168, 16], [32, 124], [168, 124]] as [x, y] (`${x}${y}`)}
      <circle cx={x} cy={y} r="3" />
    {/each}
    <!-- The platter, its rings and the spindle. -->
    <circle cx="88" cy="66" r="52" fill="currentColor" fill-opacity="0.08" />
    <circle cx="88" cy="66" r="42" opacity="0.45" />
    <circle cx="88" cy="66" r="32" opacity="0.35" />
    <circle cx="88" cy="66" r="15" fill="currentColor" fill-opacity="0.12" />
    {#each [0, 72, 144, 216, 288] as a (a)}
      <circle cx={88 + 9 * Math.cos((a * Math.PI) / 180)} cy={66 + 9 * Math.sin((a * Math.PI) / 180)} r="1.6" />
    {/each}
    <path d="M60 30 a44 44 0 0 1 36 -8" opacity="0.5" />
    <!-- The arm, from its pivot to the head over the platter. -->
    <circle cx="156" cy="104" r="11" fill="currentColor" fill-opacity="0.12" />
    <circle cx="156" cy="104" r="4" />
    <path d="M150 96 L104 44 l-4 3 L146 100" fill="currentColor" fill-opacity="0.1" />
    <rect x="96" y="38" width="9" height="6" rx="1" transform="rotate(-48 100 41)" />
    <path d="M164 112 l8 8 h-12 z" opacity="0.6" />
  {:else if kind === "external"}
    <!-- The enclosure, a groove, the light, and a cable to a USB plug. -->
    <rect x="40" y="10" width="96" height="120" rx="14" fill="currentColor" fill-opacity="0.08" />
    <rect x="48" y="18" width="80" height="104" rx="9" opacity="0.4" />
    <line x1="58" x2="118" y1="104" y2="104" opacity="0.5" />
    <circle cx="88" cy="114" r="2.6" fill="currentColor" />
    <path d="M136 66 h10 c26 0 22 -40 40 -40" />
    <rect x="180" y="16" width="14" height="20" rx="2" fill="currentColor" fill-opacity="0.12" />
    <rect x="184" y="8" width="6" height="8" />
  {:else if kind === "ssd"}
    <!-- A 2.5-inch case with a label and the SATA connector along one edge. -->
    <rect x="26" y="18" width="148" height="104" rx="6" fill="currentColor" fill-opacity="0.08" />
    <rect x="40" y="32" width="96" height="62" rx="3" opacity="0.5" />
    <line x1="50" x2="110" y1="48" y2="48" opacity="0.5" />
    <line x1="50" x2="96" y1="58" y2="58" opacity="0.5" />
    <rect x="148" y="34" width="20" height="30" rx="2" fill="currentColor" fill-opacity="0.15" />
    <rect x="148" y="70" width="20" height="40" rx="2" fill="currentColor" fill-opacity="0.15" />
    {#each [[34, 26], [166, 26], [34, 114], [166, 114]] as [x, y] (`${x}${y}`)}
      <circle cx={x} cy={y} r="2.4" />
    {/each}
  {:else}
    <!-- An SD card: the cut corner, the lock switch and the contacts. -->
    <path d="M66 14 h70 a6 6 0 0 1 6 6 v100 a6 6 0 0 1 -6 6 h-82 a6 6 0 0 1 -6 -6 v-94 z" fill="currentColor" fill-opacity="0.08" />
    <rect x="42" y="60" width="6" height="16" rx="1" />
    {#each Array(8) as _, i (i)}
      <rect x={66 + i * 9} y="20" width="5" height="16" rx="1" fill="currentColor" fill-opacity="0.2" />
    {/each}
    <rect x="62" y="54" width="68" height="52" rx="3" opacity="0.5" />
  {/if}
</svg>

<style>
  .art {
    position: absolute;
    right: 14px;
    /* Beside the details, below the name and the space bar. */
    top: 132px;
    width: 42%;
    max-width: 260px;
    height: auto;
    color: var(--disk);
    opacity: 0.16;
    pointer-events: none;
    z-index: -1;
  }
</style>
