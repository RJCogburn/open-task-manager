const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

/** 1536 → "1.5 KB", using 1024 steps like most file managers. */
export function bytes(n: number | null | undefined, digits = 1): string {
  if (n == null || !isFinite(n)) return "—";
  let i = 0;
  while (Math.abs(n) >= 1024 && i < UNITS.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(i === 0 ? 0 : n < 10 ? digits : n < 100 ? Math.min(digits, 1) : 0)} ${UNITS[i]}`;
}

export const rate = (n: number | null | undefined) => (n == null ? "—" : `${bytes(n)}/s`);

/** Network speeds read better in bits. */
export function bits(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond == null) return "—";
  let n = bytesPerSecond * 8;
  const units = ["bps", "Kbps", "Mbps", "Gbps"];
  let i = 0;
  while (n >= 1000 && i < units.length - 1) {
    n /= 1000;
    i++;
  }
  return `${n.toFixed(n < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

export const pct = (n: number | null | undefined, digits = 0) => (n == null ? "—" : `${n.toFixed(digits)}%`);

export const ghz = (mhz: number | null | undefined) => (mhz == null || mhz === 0 ? "—" : `${(mhz / 1000).toFixed(2)} GHz`);

export const celsius = (c: number | null | undefined) => (c == null ? "—" : `${c.toFixed(0)}°C`);

export const watts = (w: number | null | undefined) => (w == null ? "—" : `${w.toFixed(w < 10 ? 1 : 0)} W`);

export function duration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

export const count = (n: number) => n.toLocaleString();

/** Background tint for a value 0–1, like the heat colouring in Windows Task Manager. */
export function heat(fraction: number, color = "var(--cpu)"): string {
  const f = Math.max(0, Math.min(1, fraction));
  if (f < 0.005) return "transparent";
  return `color-mix(in srgb, ${color} ${Math.round(6 + f * 44)}%, transparent)`;
}
