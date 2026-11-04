<!--
  A live graph drawn on a canvas: one or more series with smooth lines and a
  soft fill, a light grid, and a readout that follows the pointer. It redraws
  only when `tick` changes or the size changes, so idle pages cost nothing.

  Each chart can be shown another way, chosen from the menu in its corner and
  remembered by its label: as a big number over a sparkline, as rings, or as
  segmented meters. Those show the latest value against the graph's scale.
-->
<script lang="ts">
  import Gauge from "./Gauge.svelte";
  import { historyLength } from "./live.svelte";
  import { settings, type ChartType } from "./settings.svelte";

  type Series = { label: string; data: number[]; color: string; fill?: boolean };

  let {
    series,
    tick,
    max = 100,
    height = 160,
    format = (v: number) => v.toFixed(0),
    grid = true,
    label = "Graph",
  }: {
    series: Series[];
    tick: number;
    /** A fixed top value, or "auto" to fit the data. */
    max?: number | "auto";
    height?: number;
    format?: (v: number) => string;
    grid?: boolean;
    label?: string;
  } = $props();

  const types: { id: ChartType; label: string }[] = [
    { id: "smooth", label: "Smooth" },
    { id: "straight", label: "Straight" },
    { id: "step", label: "Steps" },
    { id: "bars", label: "Bars" },
    { id: "number", label: "Number" },
    { id: "ring", label: "Ring" },
    { id: "meter", label: "Meter" },
  ];
  const type = $derived<ChartType>(settings.chartTypes[label] ?? settings.chart.style);
  const drawn = $derived(type !== "ring" && type !== "meter");

  function choose(t: string) {
    // Choosing what Settings already gives every chart clears the override.
    if (t === "" || t === settings.chart.style) delete settings.chartTypes[label];
    else settings.chartTypes[label] = t as ChartType;
  }

  const SEGMENTS = 20;
  const last = (s: Series) => {
    for (let i = s.data.length - 1; i >= 0; i--) if (isFinite(s.data[i])) return s.data[i];
    return null;
  };
  /** The latest value as a share of the graph's top, 0 to 1. */
  const share = (s: Series) => Math.max(0, Math.min(1, (last(s) ?? 0) / top()));

  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let hover = $state<number | null>(null);

  // Theme colours, looked up once: asking for computed styles on every redraw
  // is the slowest part of drawing. Cleared when the theme changes.
  const colors = new Map<string, string>();

  function resolve(color: string): string {
    if (!color.startsWith("var(")) return color;
    let value = colors.get(color);
    if (value === undefined) {
      value = getComputedStyle(canvas!).getPropertyValue(color.slice(4, -1).trim()).trim() || "#888";
      colors.set(color, value);
    }
    return value;
  }

  function top(): number {
    if (max !== "auto") return max;
    let m = 0;
    for (const s of series) for (const v of s.data) if (isFinite(v) && v > m) m = v;
    // Round up to a tidy value so the scale doesn't jitter every second.
    if (m <= 0) return 1;
    const p = Math.pow(10, Math.floor(Math.log10(m)));
    for (const step of [1, 2, 2.5, 5, 10]) if (m <= step * p) return step * p;
    return m;
  }

  function draw() {
    if (!canvas || width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(width * dpr);
    const h0 = Math.round(height * dpr);
    // Setting the size reallocates the canvas, so only do it when it changes.
    if (canvas.width !== w || canvas.height !== h0) {
      canvas.width = w;
      canvas.height = h0;
    }
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const peak = top();
    const pad = 2;
    const h = height - pad * 2;
    const look = settings.chart;
    // The number view draws a bare sparkline under its readout.
    const style = type === "number" ? "smooth" : type;
    const sparkline = type === "number";
    // Themes can make lines glow (Neon does); most leave --chart-glow unset.
    const glow = Number(resolve("var(--chart-glow)")) || 0;
    const slots = historyLength();
    const step = width / (slots - 1);
    const y = (v: number) => pad + h - (Math.min(v, peak) / peak) * h;

    if (grid && look.grid && !sparkline) {
      ctx.strokeStyle = resolve("var(--grid)");
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let i = 1; i < 4; i++) {
        const gy = Math.round((height / 4) * i) + 0.5;
        ctx.moveTo(0, gy);
        ctx.lineTo(width, gy);
      }
      for (let i = 1; i < 6; i++) {
        const gx = Math.round((width / 6) * i) + 0.5;
        ctx.moveTo(gx, 0);
        ctx.lineTo(gx, height);
      }
      ctx.stroke();
    }

    for (const s of series) {
      const color = resolve(s.color);
      const offset = slots - s.data.length;
      const pts: [number, number][] = [];
      s.data.forEach((v, i) => {
        if (isFinite(v)) pts.push([(offset + i) * step, y(v)]);
      });
      if (style === "bars") {
        // Several series share a slot side by side.
        const n = series.length;
        const barW = Math.max(1, (step * 0.8) / n);
        const k = series.indexOf(s);
        ctx.fillStyle = s.fill === false ? color : withAlpha(color, 0.75);
        for (const [x, py] of pts) ctx.fillRect(x - (step * 0.4) + k * barW, py, barW, height - py);
        continue;
      }
      if (pts.length < 2) continue;
      const path = new Path2D();
      path.moveTo(pts[0][0], pts[0][1]);
      if (style === "smooth") {
        // Curves through midpoints: smooth, and never overshoot the data.
        for (let i = 1; i < pts.length - 1; i++) {
          const mx = (pts[i][0] + pts[i + 1][0]) / 2;
          const my = (pts[i][1] + pts[i + 1][1]) / 2;
          path.quadraticCurveTo(pts[i][0], pts[i][1], mx, my);
        }
        path.lineTo(pts[pts.length - 1][0], pts[pts.length - 1][1]);
      } else {
        for (let i = 1; i < pts.length; i++) {
          if (style === "step") path.lineTo(pts[i][0], pts[i - 1][1]);
          path.lineTo(pts[i][0], pts[i][1]);
        }
      }

      if (s.fill !== false && look.fill) {
        const area = new Path2D(path);
        area.lineTo(pts[pts.length - 1][0], height);
        area.lineTo(pts[0][0], height);
        area.closePath();
        const gradient = ctx.createLinearGradient(0, 0, 0, height);
        gradient.addColorStop(0, withAlpha(color, 0.32));
        gradient.addColorStop(1, withAlpha(color, 0.02));
        ctx.fillStyle = gradient;
        ctx.fill(area);
      }
      ctx.strokeStyle = color;
      ctx.lineWidth = look.thickness;
      ctx.lineJoin = "round";
      ctx.shadowColor = color;
      ctx.shadowBlur = glow;
      ctx.stroke(path);
      ctx.shadowBlur = 0;
    }

    if (hover != null) {
      const x = Math.round(hover * step) + 0.5;
      ctx.strokeStyle = resolve("var(--border-strong)");
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
      for (const s of series) {
        const v = valueAt(s, hover);
        if (v == null) continue;
        ctx.fillStyle = resolve(s.color);
        ctx.beginPath();
        ctx.arc(x, y(v), 3.5, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  function withAlpha(color: string, alpha: number): string {
    if (color.startsWith("#") && color.length === 7) {
      return color + Math.round(alpha * 255).toString(16).padStart(2, "0");
    }
    return color;
  }

  function valueAt(s: Series, slot: number): number | null {
    const v = s.data[slot - (historyLength() - s.data.length)];
    return v != null && isFinite(v) ? v : null;
  }

  function onmove(e: PointerEvent) {
    if (!canvas) return;
    const r = canvas.getBoundingClientRect();
    const last = historyLength() - 1;
    hover = Math.max(0, Math.min(last, Math.round(((e.clientX - r.left) / r.width) * last)));
  }

  $effect(() => {
    void tick;
    void width;
    void hover;
    void series;
    void type;
    void canvas;
    void JSON.stringify(settings.chart);
    draw();
  });

  // Theme changes swap the CSS colours; redraw to pick them up.
  $effect(() => {
    const observer = new MutationObserver(() => {
      colors.clear();
      draw();
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  });

  const latest = $derived.by(() => {
    void tick;
    return series.map((s) => `${s.label} ${format(s.data[s.data.length - 1] ?? 0)}`).join(", ");
  });
  const ago = $derived(hover == null ? 0 : historyLength() - 1 - hover);
</script>

<div class="chart" bind:clientWidth={width} style:height="{height}px" role="img" aria-label="{label}: {latest}">
  {#if drawn}
    <canvas
      bind:this={canvas}
      style:width="{width}px"
      style:height="{height}px"
      aria-hidden="true"
      onpointermove={onmove}
      onpointerleave={() => (hover = null)}
    ></canvas>
  {/if}
  {#if type === "number"}
    <div class="readout" style:font-size="{Math.max(16, Math.min(34, height * 0.32))}px" aria-hidden="true">
      {#each series as s (s.label)}
        {@const v = last(s)}
        <span style:color={s.color}>{#if series.length > 1}<small>{s.label}</small>{/if}{v == null ? "—" : format(v)}</span>
      {/each}
    </div>
  {:else if type === "ring"}
    <div class="rings" aria-hidden="true">
      {#each series as s (s.label)}
        {@const v = last(s)}
        <div class="ring">
          <Gauge value={share(s) * 100} color={s.color} size={Math.max(48, Math.min(120, height - (series.length > 1 ? 30 : 14)))}
            label={s.label} text={v == null ? "—" : format(v)} />
          {#if series.length > 1}<small>{s.label}</small>{/if}
        </div>
      {/each}
    </div>
  {:else if type === "meter"}
    <div class="meters" aria-hidden="true">
      {#each series as s (s.label)}
        {@const v = last(s)}
        {@const lit = Math.round(share(s) * SEGMENTS)}
        <div class="meter">
          <div class="meter-head"><small>{s.label}</small><b style:color={s.color}>{v == null ? "—" : format(v)}</b></div>
          <div class="segments">
            {#each { length: SEGMENTS }, i (i)}
              <i style:background={i < lit ? s.color : undefined}></i>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
  <select class="type" value={settings.chartTypes[label] ?? ""} onchange={(e) => choose(e.currentTarget.value)}
    title="Show this chart as…" aria-label="Show {label} as">
    <option value="">Default ({types.find((t) => t.id === settings.chart.style)?.label})</option>
    {#each types as t (t.id)}
      <option value={t.id}>{t.label}</option>
    {/each}
  </select>
  {#if hover != null && drawn}
    <div class="tip" class:left={hover > historyLength() / 2}>
      <div class="faint">{ago === 0 ? "now" : `${ago} samples ago`}</div>
      {#each series as s (s.label)}
        {@const v = valueAt(s, hover)}
        <div class="line"><i style:background={s.color}></i>{s.label}<b>{v == null ? "—" : format(v)}</b></div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .chart {
    position: relative;
    width: 100%;
    border-radius: 10px;
    overflow: hidden;
    background: linear-gradient(to bottom, transparent, color-mix(in srgb, var(--panel-2) 60%, transparent));
    border: 1px solid var(--border);
  }

  canvas {
    display: block;
    cursor: crosshair;
  }

  /* The type menu shows while the pointer is over the chart. */
  .type {
    position: absolute;
    bottom: 6px;
    right: 6px;
    opacity: 0;
    font: inherit;
    font-size: 11px;
    padding: 2px 4px;
    color: var(--muted);
    background: color-mix(in srgb, var(--panel) 92%, transparent);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: opacity 0.12s;
  }

  .chart:hover .type,
  .type:focus-visible {
    opacity: 1;
  }

  .readout {
    position: absolute;
    top: 6px;
    left: 10px;
    display: flex;
    gap: 18px;
    pointer-events: none;
    font-weight: 650;
    letter-spacing: -0.01em;
    line-height: 1.1;
  }

  .readout small,
  .ring small,
  .meter small {
    display: block;
    font-size: 11px;
    font-weight: 550;
    color: var(--muted);
    letter-spacing: 0.02em;
  }

  .rings {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 22px;
    height: 100%;
  }

  .ring {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .meters {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 10px;
    height: 100%;
    padding: 8px 12px;
  }

  .meter-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 4px;
  }

  .segments {
    display: grid;
    grid-template-columns: repeat(20, 1fr);
    gap: 3px;
  }

  .segments i {
    height: 10px;
    border-radius: 1px;
    background: var(--grid);
  }

  .tip {
    position: absolute;
    top: 8px;
    right: 8px;
    pointer-events: none;
    padding: 7px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--panel) 92%, transparent);
    border: 1px solid var(--border-strong);
    backdrop-filter: blur(8px);
    font-size: 12px;
    min-width: 130px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  }

  .tip.left {
    right: auto;
    left: 8px;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .line i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .line b {
    margin-left: auto;
    padding-left: 10px;
    font-weight: 600;
  }
</style>
