// Everything Open Task Manager is built on, with its license and home page.
// The About page lists all of it; each page links to the entries it uses.
// Keep CREDITS.md in step with this file.

export type CreditGroup = "runs" | "reads" | "service" | "inspired" | "built";

export type Credit = {
  name: string;
  /** What Open Task Manager uses it for. */
  role: string;
  license: string;
  url: string;
  group: CreditGroup;
  /** The program this app runs, for showing whether it is installed. */
  program?: string;
  /** Not open source. */
  proprietary?: boolean;
};

export const credits = {
  lmSensors: {
    name: "lm-sensors",
    role: "Temperatures, fan speeds, voltages and power (the sensors command)",
    license: "GPL-2.0-only OR LGPL-2.1-only",
    url: "https://github.com/lm-sensors/lm-sensors",
    group: "runs",
    program: "sensors",
  },
  iproute2: {
    name: "iproute2",
    role: "Open network connections and the processes that own them (the ss command)",
    license: "GPL-2.0-or-later",
    url: "https://git.kernel.org/pub/scm/network/iproute2/iproute2.git",
    group: "runs",
    program: "ss",
  },
  pciutils: {
    name: "pciutils",
    role: "Graphics card names (the lspci command)",
    license: "GPL-2.0-only",
    url: "https://mj.ucw.cz/sw/pciutils/",
    group: "runs",
    program: "lspci",
  },
  utilLinux: {
    name: "util-linux",
    role: "Drives, their models and the filesystems on them (the lsblk command)",
    license: "GPL-2.0-or-later",
    url: "https://github.com/util-linux/util-linux",
    group: "runs",
    program: "lsblk",
  },
  smartmontools: {
    name: "smartmontools",
    role: "Drive health from SMART data (the smartctl command)",
    license: "GPL-2.0-or-later",
    url: "https://www.smartmontools.org/",
    group: "runs",
    program: "smartctl",
  },
  fio: {
    name: "fio",
    role: "Disk benchmark: sequential and random read and write speed",
    license: "GPL-2.0-only",
    url: "https://github.com/axboe/fio",
    group: "runs",
    program: "fio",
  },
  glmark2: {
    name: "glmark2",
    role: "Graphics benchmark (OpenGL)",
    license: "GPL-3.0",
    url: "https://github.com/glmark2/glmark2",
    group: "runs",
    program: "glmark2",
  },
  curl: {
    name: "curl",
    role: "Internet speed test transfers",
    license: "curl (MIT-style)",
    url: "https://curl.se/",
    group: "runs",
    program: "curl",
  },
  pacman: {
    name: "pacman",
    role: "Installed packages on Arch-based systems",
    license: "GPL-2.0-or-later",
    url: "https://pacman.archlinux.page/",
    group: "runs",
    program: "pacman",
  },
  flatpak: {
    name: "Flatpak",
    role: "Installed Flatpak apps",
    license: "LGPL-2.1-or-later",
    url: "https://flatpak.org/",
    group: "runs",
    program: "flatpak",
  },
  dpkg: {
    name: "dpkg",
    role: "Installed packages on Debian-based systems",
    license: "GPL-2.0-or-later",
    url: "https://wiki.debian.org/Teams/Dpkg",
    group: "runs",
    program: "dpkg-query",
  },
  rpm: {
    name: "RPM",
    role: "Installed packages on Fedora, openSUSE and other RPM-based systems",
    license: "GPL-2.0-or-later",
    url: "https://rpm.org/",
    group: "runs",
    program: "rpm",
  },
  polkit: {
    name: "polkit",
    role: "Asking for your password when a process belongs to another user (pkexec)",
    license: "LGPL-2.0-or-later",
    url: "https://github.com/polkit-org/polkit",
    group: "runs",
    program: "pkexec",
  },
  xdgUtils: {
    name: "xdg-utils",
    role: "Opening folders in your file manager (xdg-open)",
    license: "MIT",
    url: "https://gitlab.freedesktop.org/xdg/xdg-utils",
    group: "runs",
    program: "xdg-open",
  },
  nvidiaSmi: {
    name: "NVIDIA System Management Interface",
    role: "NVIDIA graphics card load, memory, temperature and power (nvidia-smi). Used only if the NVIDIA driver is installed.",
    license: "Proprietary (NVIDIA driver license)",
    url: "https://developer.nvidia.com/system-management-interface",
    group: "runs",
    program: "nvidia-smi",
    proprietary: true,
  },
  linux: {
    name: "Linux kernel",
    role: "Processes, CPU, memory, disk and network counters (/proc), AMD GPU readings (amdgpu), CPU power (RAPL)",
    license: "GPL-2.0-only",
    url: "https://kernel.org/",
    group: "reads",
  },
  ookla: {
    name: "Speedtest CLI by Ookla",
    role: "Internet speed test against speedtest.net servers (the speedtest command). Used only if you install it.",
    license: "Proprietary (free to use; Ookla's license and privacy policy apply)",
    url: "https://www.speedtest.net/apps/cli",
    group: "runs",
    program: "speedtest",
    proprietary: true,
  },
  librespeed: {
    name: "LibreSpeed",
    role: "Internet speed test: the method and the public test servers, which volunteers and sponsors run (librespeed.org)",
    license: "LGPL-3.0-or-later (servers run by their sponsors)",
    url: "https://librespeed.org/",
    group: "service",
  },
  githubReleases: {
    name: "GitHub Releases",
    role: "Where new versions are published and checked for",
    license: "Online service (not open source)",
    url: "https://github.com/RJCogburn/open-task-manager/releases",
    group: "service",
    proprietary: true,
  },
  ncdu: {
    name: "ncdu",
    role: "The disk space scanner works the way ncdu does: real disk usage, one filesystem, hard links counted once",
    license: "MIT",
    url: "https://dev.yorhel.nl/ncdu",
    group: "inspired",
  },
  filelight: {
    name: "Filelight",
    role: "Browsing disk usage folder by folder",
    license: "GPL-2.0-or-later",
    url: "https://apps.kde.org/filelight/",
    group: "inspired",
  },
  btop: {
    name: "btop",
    role: "Live graphs for every resource on one screen",
    license: "Apache-2.0",
    url: "https://github.com/aristocratos/btop",
    group: "inspired",
  },
  systemMonitor: {
    name: "KDE System Monitor",
    role: "Process table, sensors and per-page layout",
    license: "GPL-2.0-or-later",
    url: "https://apps.kde.org/plasma-systemmonitor/",
    group: "inspired",
  },
  sTui: {
    name: "s-tui",
    role: "Showing CPU frequency, temperature and power side by side",
    license: "GPL-2.0-only",
    url: "https://github.com/amanusk/s-tui",
    group: "inspired",
  },
  sysinfo: {
    name: "sysinfo",
    role: "Rust library that reads processes, CPU, memory, disks and network",
    license: "MIT",
    url: "https://github.com/GuillaumeGomez/sysinfo",
    group: "built",
  },
  tauri: {
    name: "Tauri",
    role: "The desktop app shell",
    license: "Apache-2.0 OR MIT",
    url: "https://tauri.app/",
    group: "built",
  },
  svelte: {
    name: "Svelte",
    role: "The interface",
    license: "MIT",
    url: "https://svelte.dev/",
    group: "built",
  },
  vite: {
    name: "Vite",
    role: "Builds the interface",
    license: "MIT",
    url: "https://vite.dev/",
    group: "built",
  },
  rayon: {
    name: "Rayon",
    role: "Scans folders on all CPU cores at once",
    license: "Apache-2.0 OR MIT",
    url: "https://github.com/rayon-rs/rayon",
    group: "built",
  },
  serde: {
    name: "Serde",
    role: "Passing data between the Rust side and the interface",
    license: "Apache-2.0 OR MIT",
    url: "https://serde.rs/",
    group: "built",
  },
  libc: {
    name: "libc (Rust)",
    role: "Ending processes and changing their priority",
    license: "Apache-2.0 OR MIT",
    url: "https://github.com/rust-lang/libc",
    group: "built",
  },
} satisfies Record<string, Credit>;

export type CreditId = keyof typeof credits;

export const groupTitles: Record<CreditGroup, { title: string; blurb: string }> = {
  runs: {
    title: "Tools it runs",
    blurb: "Open Task Manager starts these programs and shows what they report. They are not copied into it.",
  },
  reads: { title: "Data it reads", blurb: "Read straight from the files the kernel provides." },
  service: {
    title: "Online services",
    blurb: "Contacted only when you start the speed test, or check for updates (daily only if you agreed to it).",
  },
  inspired: {
    title: "Ideas borrowed from",
    blurb: "No code from these projects is included, but they shaped how parts of this app work.",
  },
  built: { title: "Built with", blurb: "Libraries compiled into Open Task Manager." },
};
