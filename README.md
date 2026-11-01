# Open Task Manager

A free task manager for Linux. It brings together the open source tools your
system already has in one window, with live graphs.

- **Overview**: CPU, memory, every GPU, every drive and every network interface at a glance
- **Processes**: sortable, searchable; end, kill or change priority (asks for your password through polkit when a process belongs to someone else)
- **CPU, Memory, Disks, Network, GPU**: live graphs and details; per-thread CPU graphs; NVIDIA through `nvidia-smi`, AMD through the amdgpu driver
- **Power & Thermals**: every sensor lm-sensors can see, plus GPU power and CPU power (RAPL, needs root)
- **Connections**: which programs hold which sockets, and which ones other computers can reach
- **Disk Space**: a fast, parallel scan in the spirit of ncdu and Filelight; browse folders, largest files and space by file type
- **Benchmarks**: CPU and memory built in; disk (fio), graphics (glmark2) and internet (curl + Cloudflare)
- **Installed Apps**: pacman, Flatpak and dpkg packages by size

No account, no telemetry and no paid tier. The app contacts the internet only
when you run the internet benchmark.

## Requirements

Linux, plus the tools for the features you want. The app tells you when one is
missing and how to install it.

| Feature | Needs |
|---|---|
| Sensors | `lm_sensors` |
| Connections | `iproute2` (installed almost everywhere) |
| NVIDIA GPU | the NVIDIA driver (`nvidia-smi`) |
| Disk benchmark | `fio` |
| Graphics benchmark | `glmark2` |
| Internet benchmark | `curl` |

On Arch-based systems: `sudo pacman -S lm_sensors fio glmark2`

## Build and run

Needs Rust and Node.js, plus Tauri's
[Linux prerequisites](https://tauri.app/start/prerequisites/#linux).

```sh
cd open-task-manager
npm install
npm run tauri dev      # run in development
npm run tauri build    # build .deb, .rpm and AppImage packages
```

In VS Code, open `open-task-manager.code-workspace`. It has tasks to run the
app, build packages and run all the checks (Ctrl+Shift+B).

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
