# Writing a plugin

A plugin adds a page to Open Task Manager without changing the app. It's a
folder with two files, installed in:

```
~/.local/share/open-task-manager/plugins/<id>/
├── manifest.json
└── index.js
```

The quickest start is the [example plugin](plugins/example): copy its folder
there, open **Settings → Plugins**, and approve it.

## Trust

A plugin's code runs inside Open Task Manager, with the same access the app
has. So:

- **No plugin loads until you approve it** in Settings → Plugins. The
  approval screen lists the programs the plugin may run.
- **A changed plugin asks again.** Approval covers the manifest and code as
  they were; change either and the plugin waits for approval again.
- **A plugin can only run the programs its manifest declares**, and never one
  that asks for administrator rights (`sudo`, `pkexec`, `su`, `doas`, `run0`).
  This keeps an honest plugin honest; it is not a sandbox. Only approve
  plugins from people you trust.

## manifest.json

```json
{
  "id": "battery",
  "name": "Battery",
  "version": "1.0.0",
  "apiVersion": 1,
  "description": "Charge, health and wear for each battery",
  "author": "Your name",
  "icon": "M3 7h16v10H3zM21 10v4",
  "section": "monitor",
  "order": 75,
  "commands": {
    "status": ["upower", "-d"],
    "details": ["./details.sh", "--json"]
  }
}
```

| Field | Meaning |
|---|---|
| `id` | Lowercase letters, digits and dashes. Must match the folder's name, and not be one of the app's own pages |
| `name` | The name in the sidebar |
| `version` | Your plugin's version |
| `apiVersion` | The plugin API it was made for. This version of the app offers **1** |
| `description`, `author` | Shown on the page and in Settings |
| `icon` | Optional: an SVG path on a 24×24 grid, drawn with a 1.7 stroke |
| `section` | `"monitor"` for live readings, `"tools"` (the default) for things you run or look up |
| `order` | Position in its section; the app's own pages use 10 to 100 |
| `commands` | The programs it may run, each under a name of your own. A program starting with `./` is a file in your plugin's folder |

## index.js

A JavaScript module, already built: one file, no imports. It default-exports
an object with a `mount` function. The app calls it with an element to draw
in and the host API; it may return a function to clean up when the page closes.

```js
export default {
  async mount(el, host) {
    el.innerHTML = `<section class="card"><h3>Hello</h3><p data-cpu></p></section>`;
    const stop = host.onSample((s) => {
      el.querySelector("[data-cpu]").textContent = host.format.pct(s?.cpu);
    });
    return stop;
  },
};
```

Write it in plain JavaScript, or in Svelte, React or anything else that
bundles to a single ES module. The app's CSS classes (`card`, `grid`,
`stats`, `stat`, `muted`, `faint`, `btn`, `notice`) and variables (`--text`,
`--muted`, `--panel`, `--border`, `--accent`, `--cpu`…) follow the user's theme,
so a page built from them fits in.

## The host API (version 1)

| | |
|---|---|
| `host.apiVersion` | `1` |
| `host.plugin` | `{ id, name, version }` from your manifest |
| `host.sample()` | The latest sample: CPU, cores, memory, disks, networks, GPUs, load and more. The fields are listed in the [readings feed](readings-feed.md#format) |
| `host.onSample(fn)` | Calls `fn(sample)` now and with each new sample (once a second by default). Returns a function that stops it |
| `host.history(key)` | Recent values of a history key, oldest first |
| `host.push(name, value)` | Adds a value to a series of your own, `own:<name>` |
| `host.chart(el, options)` | Draws a live chart in the app's style into `el`. Returns a function that removes it |
| `host.run(name)` | Runs the command your manifest declares under `name`. Resolves to `{ status, stdout, stderr }` |
| `host.format` | The app's formatting: `bytes`, `rate`, `netRate`, `pct`, `ghz`, `temp`, `watts`, `duration`… They follow the user's unit settings |

### History keys

`cpu`, `core:<n>`, `freq`, `cpuTemp`, `mem`, `memBytes`, `swap`,
`disk:<name>:read`, `disk:<name>:write`, `disk:<name>:busy`,
`net:<name>:rx`, `net:<name>:tx`, `gpu:<id>:util`, `gpu:<id>:mem`,
`gpu:<id>:temp`, `gpu:<id>:power`, and `own:<name>` for your own.

### Chart options

```js
host.chart(el, {
  series: [
    { label: "Download", key: "net:eth0:rx", color: "var(--net)" },
    { label: "Upload", key: "net:eth0:tx" },
  ],
  max: "auto",        // or a fixed top value; default 100
  height: 140,
  format: host.format.netRate,
});
```

Users can switch the chart to a number, ring or meter from its corner menu,
like any other chart.

## Testing

Put your folder in the plugins folder (**Settings → Plugins → Open plugins
folder**), click **Look again**, and approve it. After changing your files,
click **Look again** and approve it again.

If something is wrong with your manifest, Settings → Plugins says what.
Errors from your code appear on your page, and in the developer console when
running the app with `npm run tauri dev`.
