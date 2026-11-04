// Types for what the Rust side sends, and typed wrappers around its commands.
import { invoke } from "@tauri-apps/api/core";
import { credits, type Credit } from "./credits";

/** Every program a credit names, for the installed check. */
const programs = (Object.values(credits) as Credit[]).flatMap((c) => (c.program ? [c.program] : []));

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

export type DriveHealth = {
  status: "good" | "warning" | "failing" | "unknown";
  note: string;
  needsAdmin: boolean;
  temp: number | null;
  powerOnHours: number | null;
  wear: number | null;
  spare: number | null;
  mediaErrors: number | null;
  badSectors: number | null;
};
export type DriveFilesystem = {
  device: string;
  label: string;
  fileSystem: string;
  mountPoints: string[];
  total: number;
  used: number | null;
  available: number | null;
  encrypted: boolean;
};
export type Drive = {
  name: string;
  model: string;
  vendor: string;
  serial: string;
  revision: string;
  size: number;
  transport: string;
  rotational: boolean;
  removable: boolean;
  filesystems: DriveFilesystem[];
  health: DriveHealth;
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
export type SpeedProvider = "auto" | "ookla" | "librespeed";
export type NetScore = {
  latencyMs: number;
  downloadMbps: number;
  uploadMbps: number;
  provider: "ookla" | "librespeed" | "cloudflare";
  jitterMs: number | null;
  server: string | null;
  isp: string | null;
  url: string | null;
};

export type KnownIssue = { text: string; source: string };
export type DriveMaker = { name: string; how: string; url: string };
export type DriveFirmware = {
  version: string;
  /** fwupd's id for the drive, if it can update it. */
  fwupdId: string | null;
  /** A newer version fwupd has for it. */
  update: string | null;
  issue: KnownIssue | null;
  maker: DriveMaker | null;
};
export type FirmwareReport = {
  fwupd: boolean;
  /** Days since fwupd last fetched its list of firmware. */
  metadataAgeDays: number | null;
  /** By block device name. */
  drives: Record<string, DriveFirmware>;
};

export type WdCheck = {
  current: string;
  /** Western Digital's newest version for this model. */
  latest: string | null;
  /** A version that can be installed now. */
  install: string | null;
  note: string;
};

export type SlowdownCheck = {
  id: string;
  title: string;
  status: "bad" | "warn" | "ok" | "skipped";
  detail: string;
  /** The page with more, by tool id. */
  page: string | null;
  /** Paragraphs on what was found, why it matters and what to do. */
  more: string[];
  actions: SlowdownAction[];
};
/** A fix offered with a finding; `fix` and `target` go back to slowdownFix unchanged. */
export type SlowdownAction = {
  fix: string;
  target: string | null;
  label: string;
  explain: string;
  command: string;
  admin: boolean;
};
/** Share of the last minute some programs waited, in percent. */
export type Pressure = { cpu: number; memory: number; io: number };
export type SlowdownReport = { checks: SlowdownCheck[]; pressure: Pressure | null };
/** One second of watching for slowdowns. */
export type WatchPoint = {
  /** Share of the second some program waited, in percent. */
  cpuWait: number | null;
  memoryWait: number | null;
  ioWait: number | null;
  /** How busy the processor was, in percent. */
  cpu: number;
  /** Pages swapped in and out. */
  swapped: number;
};
export type WatchReport = { started: number; points: WatchPoint[]; checks: SlowdownCheck[] };

export type AuditFinding = {
  /** Lynis's test id, such as "SSH-7408". */
  test: string;
  text: string;
  details: string | null;
  solution: string | null;
  /** Advice mostly meant for servers rather than a desktop. */
  server: boolean;
  /** A short tag and a sentence on what the finding means on this computer. */
  note?: string | null;
  noteText?: string | null;
  /** Lynis's own description of the check, from its source. */
  about?: { description: string; notes: string | null; url: string | null } | null;
  /** What Lynis found behind the finding. */
  found?: string[];
  /** Why it's best left as it is, where it is. */
  advice?: string | null;
  actions?: AuditAction[];
};
export type AuditAction = {
  fix: string;
  target: string | null;
  label: string;
  explain: string;
  command: string;
  admin: boolean;
  /** Only looks; changes nothing. */
  readOnly: boolean;
};
/** A change made from the Security audit page, which can be undone. */
export type AuditChange = { fix: string; target: string | null; label: string; time: number };
export type Audit = {
  time: number;
  admin: boolean;
  version: string | null;
  /** Lynis's hardening index, 0 to 100. */
  score: number | null;
  tests: number | null;
  warnings: AuditFinding[];
  suggestions: AuditFinding[];
};
/** The last audit, and earlier scores as [time, admin, score], oldest first. */
export type SavedAudits = { last: Audit | null; scores: [number, boolean, number][] };
export type PluginInfo = {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  /** An SVG path on a 24×24 grid. */
  icon: string;
  section: "monitor" | "tools";
  order: number;
  /** The programs it may run, by its own name for each. */
  commands: Record<string, string[]>;
  /** Why it can't be loaded, if it can't. */
  problem: string | null;
  approved: boolean;
};
export type RunOutput = { status: number; stdout: string; stderr: string };

export type App = {
  name: string;
  version: string;
  description: string;
  size: number;
  source: "pacman" | "flatpak" | "dpkg" | "rpm";
  explicit: boolean;
  url: string;
  installed: string;
};
export type AppList = { apps: App[]; sources: string[] };

export type Origin = {
  kind:
    | "package"
    | "flatpak"
    | "snap"
    | "appimage"
    | "home"
    | "manual"
    | "elsewhere"
    | "unpackaged"
    | "temporary"
    | "deleted"
    | "hidden"
    | "unchecked";
  detail: string;
  suspicious: boolean;
};
export type NetUsage = { rx: number; tx: number; connections: number };
export type NetProcess = {
  pid: number | null;
  name: string;
  exe: string | null;
  byInterface: Record<string, NetUsage>;
  unmeasured: number;
  listening: number;
  remotes: string[];
  origin: Origin;
};
export type ProcessSource = {
  pid: number;
  name: string;
  exe: string | null;
  folder: string | null;
  command: string;
  cwd: string | null;
  user: string;
  parent: string | null;
  started: number;
  origin: Origin;
};

export type LogConfig = {
  enabled: boolean;
  cpu: boolean;
  memory: boolean;
  disks: boolean;
  network: boolean;
  gpu: boolean;
  temperatures: boolean;
  every: number;
  keepDays: number;
};
export type LogDay = { day: string; kinds: string[]; size: number };
export type LogSeries = {
  label: string;
  /** The average in each slice of the window; null where nothing was recorded. */
  values: (number | null)[];
  avg: number | null;
  max: number | null;
  /** When the peak was, in seconds after midnight. */
  maxAt: number | null;
};
export type LogGroup = {
  id: string;
  title: string;
  unit: "percent" | "mhz" | "load" | "bytes" | "celsius" | "watts";
  series: LogSeries[];
};
export type LogView = {
  day: string;
  /** The window, in seconds after midnight. */
  from: number;
  to: number;
  /** Seconds each value covers. */
  slice: number;
  /** Seconds between recorded rows. */
  every: number | null;
  rows: number;
  groups: LogGroup[];
};
export type LogStatus = { config: LogConfig; folder: string; size: number; files: number };
export type Autostart = { enabled: boolean; minimized: boolean; program: string };
export type TrayConfig = { enabled: boolean; closeToTray: boolean; showUsage: boolean };

export type UpdateStatus = {
  current: string;
  available: { version: string; notes: string | null } | null;
  method: "appimage" | "deb" | "rpm" | "package-manager" | "development";
  canInstall: boolean;
};

export type WidgetItem = "cpu" | "memory" | "gpu" | "network" | "disks" | "temperature";
export type WidgetConfig = {
  enabled: boolean;
  items: WidgetItem[];
  layout: "vertical" | "horizontal";
  graphs: boolean;
  opacity: number;
  position: [number, number] | null;
  feed: boolean;
};
export type WidgetStatus = { config: WidgetConfig; feedPath: string | null };

export type ToolStatus = {
  program: string;
  installed: boolean;
  installHint: string | null;
  /** Whether the app can install it itself (installTool). */
  installable: boolean;
};

export const api = {
  systemInfo: () => invoke<SystemInfo>("system_info"),
  mounts: () => invoke<Mount[]>("mounts"),
  drives: () => invoke<Drive[]>("drives"),
  driveHealthAdmin: () => invoke<void>("drive_health_admin"),
  setInterval: (ms: number) => invoke<void>("set_interval", { ms }),
  setDisabledTools: (tools: string[]) => invoke<void>("set_disabled_tools", { tools }),
  missingTools: () => invoke<string[]>("missing_tools"),
  slowdownCheck: () => invoke<SlowdownReport>("slowdown_check"),
  slowdownWatch: (seconds: number) => invoke<WatchReport>("slowdown_watch", { seconds }),
  slowdownWatchStop: () => invoke<void>("slowdown_watch_stop"),
  firmware: () => invoke<FirmwareReport>("firmware"),
  firmwareRefresh: () => invoke<void>("firmware_refresh"),
  firmwareUpdate: (deviceId: string) => invoke<string>("firmware_update", { deviceId }),
  wdFirmwareCheck: (device: string) => invoke<WdCheck>("wd_firmware_check", { device }),
  wdFirmwareInstall: (device: string, version: string) => invoke<string>("wd_firmware_install", { device, version }),
  securityAudit: (admin: boolean) => invoke<Audit>("security_audit", { admin }),
  securityCancel: () => invoke<void>("security_cancel"),
  securityLast: () => invoke<SavedAudits>("security_last"),
  securityFix: (fix: string, target: string | null) =>
    invoke<{ message: string; output: string | null }>("security_fix", { fix, target }),
  securityUndo: (index: number) => invoke<string>("security_undo", { index }),
  securityChanges: () => invoke<AuditChange[]>("security_changes"),
  slowdownFix: (fix: string, target: string | null) => invoke<string>("slowdown_fix", { fix, target }),
  pluginsList: () => invoke<PluginInfo[]>("plugins_list"),
  pluginSource: (id: string) => invoke<string>("plugin_source", { id }),
  pluginApprove: (id: string, approve: boolean) => invoke<void>("plugin_approve", { id, approve }),
  pluginRun: (id: string, command: string) => invoke<RunOutput>("plugin_run", { id, command }),
  pluginsOpenFolder: () => invoke<void>("plugins_open_folder"),
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
  benchNetwork: (provider: SpeedProvider) => invoke<NetScore>("bench_network", { provider }),
  installedApps: () => invoke<AppList>("installed_apps"),
  /** Whether each program a credit names is installed. */
  /** Installs a tool's package; asks for the password. */
  installTool: (program: string) => invoke<void>("install_tool", { program }),
  toolStatus: () => invoke<ToolStatus[]>("tool_status", { programs }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  updateLocal: () => invoke<UpdateStatus>("update_local"),
  updateCheck: () => invoke<UpdateStatus>("update_check"),
  updateInstall: () => invoke<void>("update_install"),
  appRestart: () => invoke<void>("app_restart"),
  widgetsGet: () => invoke<WidgetStatus>("widgets_get"),
  widgetsSet: (config: WidgetConfig) => invoke<void>("widgets_set", { config }),
  showMain: () => invoke<void>("show_main"),
  netActivity: () => invoke<NetProcess[]>("net_activity"),
  processSource: (pid: number) => invoke<ProcessSource>("process_source", { pid }),
  loggingGet: () => invoke<LogStatus>("logging_get"),
  loggingSet: (config: LogConfig) => invoke<void>("logging_set", { config }),
  logDays: () => invoke<LogDay[]>("log_days"),
  logRead: (day: string, from: number, to: number, slices: number) =>
    invoke<LogView>("log_read", { day, from, to, slices }),
  trayGet: () => invoke<TrayConfig>("tray_get"),
  traySet: (config: TrayConfig) => invoke<void>("tray_set", { config }),
  autostartGet: () => invoke<Autostart>("autostart_get"),
  autostartSet: (enabled: boolean, minimized: boolean) => invoke<Autostart>("autostart_set", { enabled, minimized }),
};

/** The program a "missing-tool" error names ("smartctl is not installed…"), or null for other errors. */
export function missingProgram(e: unknown): string | null {
  const err = parseError(e);
  return err.kind === "missing-tool" ? (err.message.split(" ")[0] ?? null) : null;
}

/** Errors from the Rust side carry a kind before the first colon. */
export function parseError(e: unknown): { kind: "needs-admin" | "missing-tool" | "other"; message: string } {
  const text = String(e);
  for (const kind of ["needs-admin", "missing-tool"] as const) {
    if (text.startsWith(kind + ":")) return { kind, message: text.slice(kind.length + 1) };
  }
  return { kind: "other", message: text };
}
