// Types for what the Rust side sends, and typed wrappers around its commands.
import { invoke } from "@tauri-apps/api/core";

export type DiskIo = { name: string; label: string; read: number; write: number; busy: number };
export type NetIo = { name: string; rx: number; tx: number; rxTotal: number; txTotal: number };
export type GpuSample = {
  id: string;
  name: string;
  vendor: string;
  source: string;
  util: number | null;
  memUsed: number | null;
  memTotal: number | null;
  temp: number | null;
  power: number | null;
  powerLimit: number | null;
  clockMhz: number | null;
  fan: number | null;
};

export type Sample = {
  time: number;
  cpu: number;
  cores: number[];
  freqs: number[];
  cpuTemp: number | null;
  memUsed: number;
  memTotal: number;
  swapUsed: number;
  swapTotal: number;
  disks: DiskIo[];
  nets: NetIo[];
  gpus: GpuSample[];
  processes: number;
  threads: number;
  uptime: number;
  load: [number, number, number];
};

export type SystemInfo = {
  hostName: string;
  userName: string;
  homeDir: string;
  os: string;
  kernel: string;
  cpuBrand: string;
  physicalCores: number;
  logicalCores: number;
  memTotal: number;
  swapTotal: number;
};

export type Mount = {
  device: string;
  mountPoint: string;
  fileSystem: string;
  total: number;
  available: number;
  removable: boolean;
};

export type Proc = {
  pid: number;
  ppid: number | null;
  name: string;
  command: string;
  user: string;
  cpu: number;
  memory: number;
  diskRead: number;
  diskWrite: number;
  status: string;
  startTime: number;
  nice: number | null;
  kernel: boolean;
};

export type Reading = {
  label: string;
  kind: "temperature" | "fan" | "voltage" | "power" | "current" | "frequency" | "pwm" | "humidity";
  value: number;
  unit: string;
  max: number | null;
  crit: number | null;
};
export type Chip = { id: string; name: string; adapter: string; readings: Reading[] };
export type SensorReport = { chips: Chip[]; cpuPower: number | null; cpuPowerNote: string | null };

export type Connection = {
  protocol: string;
  state: string;
  localAddress: string;
  localPort: string;
  peerAddress: string;
  peerPort: string;
  process: string | null;
  pid: number | null;
  exposure: "network" | "local" | "outbound";
};

export type ScanProgress = {
  files: number;
  bytes: number;
  errors: number;
  seconds: number;
  done: boolean;
  cancelled: boolean;
};
export type ScanEntry = { id: number; name: string; size: number; items: number; isDir: boolean };
export type ScanFolder = {
  id: number;
  path: string;
  size: number;
  items: number;
  trail: ScanEntry[];
  children: ScanEntry[];
  more: number;
};
export type ScanSummary = {
  largest: { path: string; size: number }[];
  types: { extension: string; size: number; count: number }[];
};

export type CpuScore = { single: number; multi: number; threads: number };
export type MemoryScore = { copyGbps: number };
export type DiskResult = { name: string; bandwidth: number; iops: number };
export type GpuScore = { score: number; renderer: string };
export type NetScore = { latencyMs: number; downloadMbps: number; uploadMbps: number };

export type App = {
  name: string;
  version: string;
  description: string;
  size: number;
  source: "pacman" | "flatpak" | "dpkg";
  explicit: boolean;
  url: string;
  installed: string;
};
export type AppList = { apps: App[]; sources: string[] };

export type ToolStatus = { program: string; installed: boolean; installHint: string | null };

export const api = {
  systemInfo: () => invoke<SystemInfo>("system_info"),
  mounts: () => invoke<Mount[]>("mounts"),
  setInterval: (ms: number) => invoke<void>("set_interval", { ms }),
  processes: () => invoke<Proc[]>("processes"),
  endProcess: (pid: number, force: boolean) => invoke<void>("end_process", { pid, force }),
  setPriority: (pid: number, nice: number) => invoke<void>("set_priority", { pid, nice }),
  adminAction: (pid: number, action: "end" | "kill" | "priority", nice?: number) =>
    invoke<void>("admin_action", { pid, action, nice }),
  sensors: () => invoke<SensorReport>("sensors"),
  connections: () => invoke<Connection[]>("connections"),
  scanStart: (path: string) => invoke<void>("scan_start", { path }),
  scanCancel: () => invoke<void>("scan_cancel"),
  scanFolder: (id: number) => invoke<ScanFolder>("scan_folder", { id }),
  scanSummary: () => invoke<ScanSummary>("scan_summary"),
  benchCpu: () => invoke<CpuScore>("bench_cpu"),
  benchMemory: () => invoke<MemoryScore>("bench_memory"),
  benchDisk: (folder: string) => invoke<DiskResult[]>("bench_disk", { folder }),
  benchGpu: () => invoke<GpuScore>("bench_gpu"),
  benchNetwork: () => invoke<NetScore>("bench_network"),
  installedApps: () => invoke<AppList>("installed_apps"),
  toolStatus: () => invoke<ToolStatus[]>("tool_status"),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
};

/** Errors from the Rust side carry a kind before the first colon. */
export function parseError(e: unknown): { kind: "needs-admin" | "missing-tool" | "other"; message: string } {
  const text = String(e);
  for (const kind of ["needs-admin", "missing-tool"] as const) {
    if (text.startsWith(kind + ":")) return { kind, message: text.slice(kind.length + 1) };
  }
  return { kind: "other", message: text };
}
