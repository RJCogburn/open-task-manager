<!-- A ring that fills to a percentage, with the value in the middle. -->
<script lang="ts">
  let {
    value,
    color,
    size = 92,
    label,
    text,
  }: { value: number | null; color: string; size?: number; label: string; text?: string } = $props();

  const r = 40;
  const circumference = 2 * Math.PI * r;
  const fraction = $derived(Math.max(0, Math.min(1, (value ?? 0) / 100)));
</script>

<div class="gauge" style:width="{size}px" style:height="{size}px" role="meter" aria-label={label}
  aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(value ?? 0)}>
  <svg viewBox="0 0 100 100">
    <circle cx="50" cy="50" {r} class="track" />
    <circle
      cx="50"
      cy="50"
      {r}
      class="fill"
      style:stroke={color}
      stroke-dasharray={circumference}
      stroke-dashoffset={circumference * (1 - fraction)}
    />
  </svg>
  <span>{text ?? (value == null ? "—" : `${Math.round(value)}%`)}</span>
</div>

<style>
  .gauge {
    position: relative;
    flex: none;
  }

  svg {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  circle {
    fill: none;
    stroke-width: 9;
  }

  .track {
    stroke: var(--grid);
  }

  .fill {
    stroke-linecap: round;
  }

  span {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-weight: 650;
    font-size: 18px;
    letter-spacing: -0.02em;
  }
</style>
