# Open Task Manager

A free task manager for Linux. It brings together the open source tools your
system already has in one window, with live graphs.

- **Overview**: CPU, memory, every GPU, every drive and every network interface at a glance
- **Processes**: sortable, searchable; end, kill or change priority (asks for your password through polkit when a process belongs to someone else)
- **CPU, Memory, Disks, Network, GPU**: live graphs and details; per-thread CPU graphs; NVIDIA through `nvidia-smi`, AMD through the amdgpu driver
- **Disks**: a card per drive with model, space, temperature and SMART health, and a folder-by-folder look at what fills it
- **Network**: which programs use each interface, busiest first; flags programs that no installed package accounts for; internet speed test
- **Power & Thermals**: every sensor lm-sensors can see, plus GPU power and CPU power (RAPL, needs root)
- **Connections**: which programs hold which sockets, and which ones other computers can reach
- **Disk Space**: a fast, parallel scan in the spirit of ncdu and Filelight; browse folders, largest files and space by file type
- **Benchmarks**: CPU and memory built in; disk (fio), graphics (glmark2) and internet (curl + Cloudflare)
- **Installed Apps**: pacman, dpkg, rpm and Flatpak packages by size
- **System tray**: keeps running, and logging, with the window closed; can start in the tray at login
- **Settings**: graph style, units, start page, start at login, and logging chosen hardware to CSV files

No account, no telemetry and no paid tier. The app contacts the internet only
when you run the internet benchmark.

## Requirements

Linux, plus the tools for the features you want. The app tells you when one is
missing and how to install it.

| Feature | Needs |
|---|---|
| Sensors | `lm_sensors` |
| Connections, network activity | `iproute2` (installed almost everywhere) |
| Drive details | `util-linux` (`lsblk`, installed almost everywhere) |
| Drive health | `smartmontools` |
| NVIDIA GPU | the NVIDIA driver (`nvidia-smi`) |
| Disk benchmark | `fio` |
| Graphics benchmark | `glmark2` |
| Internet benchmark | `curl` |

On Arch-based systems: `sudo pacman -S lm_sensors smartmontools fio glmark2`

## Build and run

Needs Rust and Node.js, plus Tauri's
[Linux prerequisites](https://tauri.app/start/prerequisites/#linux).

```sh
cd open-task-manager
npm install
npm run tauri dev      # run in development
npm run tauri build    # build .deb, .rpm and AppImage packages
```

### Arch, CachyOS, Manjaro and other Arch-based systems

`packaging/arch/PKGBUILD` builds the latest commit as `open-task-manager-git`,
with a launcher entry and icons:

```sh
cd packaging/arch
makepkg -si
```

Flatpak and Snap aren't offered: their sandbox hides other programs'
processes, which a task manager needs to see.

In VS Code, open `open-task-manager.code-workspace`. It has tasks to run the
app, build packages and run all the checks (Ctrl+Shift+B).

To add a tool of your own, see [CONTRIBUTING.md](CONTRIBUTING.md).

### Making a release

1. Set the new version in `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`, then run `npm install` and `cargo build` in
   `src-tauri` so the lock files follow.
2. Commit, tag and push: `git tag v0.79.0 && git push origin main v0.79.0`.
3. GitHub Actions builds the `.deb`, `.rpm` and AppImage for x86_64 and ARM on
   Ubuntu 22.04 and uploads them to a draft release. Check it on the Releases
   page, then publish it.

## How much load it adds

The figures below were measured on a Ryzen 9 9950X3D with the window open,
updating once a second, over 20 seconds, with the release build. The window was
drawn in software in a headless test session, so on a real desktop with GPU
drawing the interface share should be lower.

| Page | App process | Interface (WebKit) | Total, as share of one core |
|---|---|---|---|
| Overview | 1.4% | 3.0% | ~4.6% |
| CPU | 1.2% | 2.2% | ~3.5% |
| Processes | 1.5% | 6.2% | ~8% |

On a 32-thread CPU, 4.6% of one core is about 0.15% of the whole processor.
Slower update speeds (2 s or 5 s, bottom left) cut this further. Pages you are
not looking at do no work, apart from the shared once-a-second sample that
keeps the graph history filled.

## License

Copyright © 2026 Open Task Manager contributors.

Open Task Manager is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version. See [LICENSE](LICENSE).

It is built on many other projects; see [CREDITS.md](CREDITS.md).
