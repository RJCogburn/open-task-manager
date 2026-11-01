<!--
  A live graph drawn on a canvas: one or more series with smooth lines and a
  soft fill, a light grid, and a readout that follows the pointer. It redraws
  only when `tick` changes or the size changes, so idle pages cost nothing.
-->
<script lang="ts">
  import { HISTORY } from "./live.svelte";

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

  let canvas: HTMLCanvasElement;
  let width = $state(0);
  let hover = $state<number | null>(null);

  function resolve(color: string): string {
    if (!color.startsWith("var(")) return color;
    const name = color.slice(4, -1).trim();
    return getComputedStyle(canvas).getPropertyValue(name).trim() || "#888";
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
    const step = width / (HISTORY - 1);
    const y = (v: number) => pad + h - (Math.min(v, peak) / peak) * h;

    if (grid) {
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
      const offset = HISTORY - s.data.length;
      const pts: [number, number][] = [];
      s.data.forEach((v, i) => {
        if (isFinite(v)) pts.push([(offset + i) * step, y(v)]);
      });
      if (pts.length < 2) continue;
      const path = new Path2D();
      path.moveTo(pts[0][0], pts[0][1]);
      // Curves through midpoints: smooth, and never overshoot the data.
      for (let i = 1; i < pts.length - 1; i++) {
        const mx = (pts[i][0] + pts[i + 1][0]) / 2;
        const my = (pts[i][1] + pts[i + 1][1]) / 2;
        path.quadraticCurveTo(pts[i][0], pts[i][1], mx, my);
      }
      path.lineTo(pts[pts.length - 1][0], pts[pts.length - 1][1]);

      if (s.fill !== false) {
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
      ctx.lineWidth = 1.75;
      ctx.lineJoin = "round";
      ctx.stroke(path);
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
    const v = s.data[slot - (HISTORY - s.data.length)];
    return v != null && isFinite(v) ? v : null;
  }

  function onmove(e: PointerEvent) {
    const r = canvas.getBoundingClientRect();
    hover = Math.max(0, Math.min(HISTORY - 1, Math.round(((e.clientX - r.left) / r.width) * (HISTORY - 1))));
  }

  $effect(() => {
    void tick;
    void width;
    void hover;
    void series;
    draw();
  });

  // Theme changes swap the CSS colours; redraw to pick them up.
  $effect(() => {
    const observer = new MutationObserver(draw);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  });

  const latest = $derived.by(() => {
    void tick;
    return series.map((s) => `${s.label} ${format(s.data[s.data.length - 1] ?? 0)}`).join(", ");
  });
  const ago = $derived(hover == null ? 0 : HISTORY - 1 - hover);
</script>

<div class="chart" bind:clientWidth={width} style:height="{height}px" role="img" aria-label="{label}: {latest}">
  <canvas
    bind:this={canvas}
    style:width="{width}px"
    style:height="{height}px"
    aria-hidden="true"
    onpointermove={onmove}
    onpointerleave={() => (hover = null)}
  ></canvas>
  {#if hover != null}
    <div class="tip" class:left={hover > HISTORY / 2}>
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
