# Credits

Open Task Manager is built on the work of the projects below. The same list,
with a check of which tools are installed, is on the app's **About & Credits**
page. Keep this file in step with `src/lib/credits.ts`.

## Tools it runs

Open Task Manager starts these programs and shows what they report. They are
not copied into it, so their licenses apply to them, not to this app.

| Project | Used for | License |
|---|---|---|
| [lm-sensors](https://github.com/lm-sensors/lm-sensors) | Temperatures, fan speeds, voltages and power (`sensors`) | GPL-2.0-only OR LGPL-2.1-only |
| [iproute2](https://git.kernel.org/pub/scm/network/iproute2/iproute2.git) | Open network connections and their owners (`ss`) | GPL-2.0-or-later |
| [pciutils](https://mj.ucw.cz/sw/pciutils/) | Graphics card names (`lspci`) | GPL-2.0-only |
| [util-linux](https://github.com/util-linux/util-linux) | Drives, their models and filesystems (`lsblk`) | GPL-2.0-or-later |
| [smartmontools](https://www.smartmontools.org/) | Drive health from SMART data (`smartctl`) | GPL-2.0-or-later |
| [fio](https://github.com/axboe/fio) | Disk benchmark | GPL-2.0-only |
| [glmark2](https://github.com/glmark2/glmark2) | Graphics benchmark (OpenGL) | GPL-3.0 |
| [curl](https://curl.se/) | Internet speed test transfers | curl (MIT-style) |
| [pacman](https://pacman.archlinux.page/) | Installed packages on Arch-based systems | GPL-2.0-or-later |
| [Flatpak](https://flatpak.org/) | Installed Flatpak apps | LGPL-2.1-or-later |
| [dpkg](https://wiki.debian.org/Teams/Dpkg) | Installed packages on Debian-based systems | GPL-2.0-or-later |
| [RPM](https://rpm.org/) | Installed packages on Fedora, openSUSE and other RPM-based systems | GPL-2.0-or-later |
| [polkit](https://github.com/polkit-org/polkit) | Password prompt for acting on other users' processes (`pkexec`) | LGPL-2.0-or-later |
| [xdg-utils](https://gitlab.freedesktop.org/xdg/xdg-utils) | Opening folders in the file manager (`xdg-open`) | MIT |
| [systemd](https://systemd.io/) | Failed services and start-up times for the slowdown check, and switching services on and off on the Startup page (`systemctl`, `systemd-analyze`) | LGPL-2.1-or-later |
| [power-profiles-daemon](https://gitlab.freedesktop.org/upower/power-profiles-daemon) | The current power profile for the slowdown check (`powerprofilesctl`), only if installed | GPL-3.0-or-later |
| [nvme-cli](https://github.com/linux-nvme/nvme-cli) | Installing Western Digital firmware on NVMe drives (`nvme fw-download`, `fw-commit`), only if you do | GPL-2.0-or-later |
| [Lynis](https://github.com/CISOfy/lynis) | The security audit (`lynis audit system`), only if installed | GPL-3.0-only |
| [fwupd](https://fwupd.org/) | Drive firmware versions and updates from the LVFS (`fwupdmgr`) | LGPL-2.1-or-later |
| [NVIDIA System Management Interface](https://developer.nvidia.com/system-management-interface) | NVIDIA GPU readings (`nvidia-smi`), only if the NVIDIA driver is installed | Proprietary (not open source) |

## Data it reads

| Project | Used for | License |
|---|---|---|
| [Linux kernel](https://kernel.org/) | `/proc` counters, pressure stall information, amdgpu readings, RAPL CPU power | GPL-2.0-only |

## Online services

Contacted only when you start the speed test, check for app or firmware updates (app updates daily only if you agreed to it).

| Service | Used for | Terms |
|---|---|---|
| [LibreSpeed](https://librespeed.org/) | Internet speed test method and public test servers, run by volunteers and sponsors | LGPL-3.0-or-later (servers run by their sponsors) |
| [GitHub Releases](https://github.com/RJCogburn/open-task-manager/releases) | Where new versions are published and checked for | Online service (not open source) |
| [fwupd.org (LVFS)](https://fwupd.org/) | Firmware that drive makers publish for Linux, when you click Check fwupd.org for updates | Online service |
| [Western Digital firmware server](https://www.sandisk.com/) | Firmware for Western Digital and SanDisk NVMe drives, when you check a drive | Online service (not open source) |

## Ideas borrowed from

No code from these projects is included, but they shaped how parts of this app work.

| Project | What it inspired | License |
|---|---|---|
| [ncdu](https://dev.yorhel.nl/ncdu) | Disk space scanning: real disk usage, one filesystem, hard links counted once | MIT |
| [Filelight](https://apps.kde.org/filelight/) | Browsing disk usage folder by folder | GPL-2.0-or-later |
| [btop](https://github.com/aristocratos/btop) | Live graphs for every resource on one screen | Apache-2.0 |
| [KDE System Monitor](https://apps.kde.org/plasma-systemmonitor/) | Process table, sensors and per-page layout | GPL-2.0-or-later |
| [s-tui](https://github.com/amanusk/s-tui) | CPU frequency, temperature and power side by side | GPL-2.0-only |
| [wd_ssd_firmware_update](https://github.com/not-a-feature/wd_ssd_firmware_update) | How to find and install Western Digital firmware on Linux | GPL-3.0 |
| [Autostart Manager](https://github.com/filonux/Autostart_Manager) | One list of everything that starts with your session, each with a switch; system entries turned off with your own copy | GPL-3.0 |

## Built with

Libraries compiled into Open Task Manager. All are compatible with GPL-3.0-or-later.

| Project | Used for | License |
|---|---|---|
| [sysinfo](https://github.com/GuillaumeGomez/sysinfo) | Processes, CPU, memory, disks and network | MIT |
| [Tauri](https://tauri.app/) | The desktop app shell | Apache-2.0 OR MIT |
| [Svelte](https://svelte.dev/) | The interface | MIT |
| [Space Grotesk](https://github.com/floriankarsten/space-grotesk) | The typeface of the Neon theme | OFL-1.1 |
| [Chakra Petch](https://github.com/cadsondemak/Chakra-Petch) | The typeface of the HUD theme | OFL-1.1 |
| [Vite](https://vite.dev/) | Building the interface | MIT |
| [Rayon](https://github.com/rayon-rs/rayon) | Scanning folders on all CPU cores | Apache-2.0 OR MIT |
| [Serde](https://serde.rs/) | Passing data between Rust and the interface | Apache-2.0 OR MIT |
| [libc (Rust)](https://github.com/rust-lang/libc) | Ending processes and changing their priority | Apache-2.0 OR MIT |

Each project's name and logo belong to its authors. Open Task Manager is not
affiliated with or endorsed by any of them.
