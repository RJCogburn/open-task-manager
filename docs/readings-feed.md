# The readings feed

Open Task Manager can share its live readings with other desktop widgets:
Plasma widgets, GNOME extensions, conky, or your own scripts. They read what
the app has already measured, so nothing is collected twice.

Turn it on in **Settings → Desktop widgets → Share readings with other desktop
widgets**. The app must be running; it can stay in the tray.

## Where it is

```
$XDG_RUNTIME_DIR/open-task-manager/readings.json
```

Usually `/run/user/1000/open-task-manager/readings.json`. That folder is kept in
memory and only your user can read it. Nothing is written to disk, and no
network port is opened.

The file is replaced every sample (once a second by default; see **Settings →
Update every**). It is written to a temporary file first and renamed, so a
reader never sees half a file. When the app is closed or sharing is turned
off, the file stops changing; turning sharing off deletes it. Check `time` to
tell fresh readings from stale ones.

Readings for pages switched off in **Settings → Pages** are left out: with the
GPU page off, `gpus` is empty, unless the widget panel or the log still uses
them. Keep a page on to keep its readings in the feed.

## Format

```json
{
  "format": 1,
  "app": "open-task-manager",
  "readings": { ... }
}
```

`format` changes only if fields are removed or change meaning; new fields can
appear at any time.

| Field in `readings` | Meaning |
|---|---|
| `time` | When the sample was taken, milliseconds since 1970 |
| `cpu` | Processor use, percent of all cores |
| `cores` | Use of each logical core, percent |
| `freqs` | Clock of each logical core, MHz |
| `cpuTemp` | Processor temperature, °C, or `null` |
| `memUsed`, `memTotal` | Memory in use and installed, bytes |
| `swapUsed`, `swapTotal` | Swap in use and available, bytes |
| `disks[]` | Each drive: `name` (`nvme0n1`), `label`, `read` and `write` in bytes per second, `busy` percent |
| `nets[]` | Each interface: `name`, `rx` and `tx` in bytes per second, `rxTotal` and `txTotal` in bytes since boot |
| `gpus[]` | Each graphics card: `name`, `util` percent, `memUsed` and `memTotal` bytes, `temp` °C, `power` W, `clockMhz`, `fan` percent. Any may be `null` |
| `processes`, `threads` | How many are running |
| `uptime` | Seconds since boot |
| `load` | Load average over 1, 5 and 15 minutes |

Temperatures are always in °C and speeds in bytes, whatever units the app shows.

## Examples

From a shell, with `jq`:

```sh
F=$XDG_RUNTIME_DIR/open-task-manager/readings.json
jq -r '.readings | "CPU \(.cpu | round)%  Memory \(.memUsed / .memTotal * 100 | round)%"' "$F"
```

In **conky**:

```
${execi 2 jq -r '.readings.cpu | round' $XDG_RUNTIME_DIR/open-task-manager/readings.json}%
```

On **KDE Plasma**, the *Command Output* widget from the KDE Store can run the
same `jq` command every few seconds. On **GNOME**, the *Executor* extension
does the same in the top bar.

In a **Plasma widget** you write yourself (QML), read the file with
`XMLHttpRequest` on a `file://` URL and `JSON.parse` the result on a timer.
