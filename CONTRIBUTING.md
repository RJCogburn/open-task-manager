# Contributing

Thanks for helping. Bug reports, fixes and new tools are all welcome.

Before sending a change, run the checks (in VS Code: Ctrl+Shift+B, or):

```sh
npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets && cargo test
```

## Adding a tool

Each page in the sidebar is a tool: a folder in `src/tools/`. The sidebar finds
tools by their folder, so a new one shows up without touching the rest of the
app.

### 1. The page

Create `src/tools/<your-tool>/` with two files.

`index.ts` describes the tool:

```ts
import { defineTool } from "$lib/tool";
import { live } from "$lib/live.svelte";
import { pct } from "$lib/format";
import Page from "./Battery.svelte";

export default defineTool({
  id: "battery",            // stable: used in the address (#battery) and saved settings
  label: "Battery",
  section: "monitor",       // "monitor" for live readings, "tools" for things you run
  order: 75,                // position in its section; see the other tools' numbers
  icon: "M3 7h16v10H3zM21 10v4M6 10v4",  // SVG path on a 24×24 grid
  color: "var(--temp)",     // optional
  value: () => pct(live.sample?.cpu),     // optional live reading beside the name
  sources: ["linux"],       // projects the data comes from, by id in src/lib/credits.ts
  page: Page,
});
```

`Battery.svelte` is the page. Import shared code through `$lib`:

- `$lib/PageHeader.svelte`: the page title. It links your tool's `sources` by itself.
- `$lib/Chart.svelte`: live graphs that follow the user's graph settings
- `$lib/live.svelte`: the latest sample (`live.sample`), history for graphs (`live.history(key)`) and `poll()` for refreshing your own data
- `$lib/format`: `bytes`, `rate`, `netRate`, `pct`, `temp` and more. Use `temp` and `netRate` so the user's unit settings apply.
- `$lib/settings.svelte`: user settings, if your tool needs one (add it to the Settings page too)
- `$lib/arrange.svelte`: put `use:card={{ group: "battery", id: "..." }}` on each card in a grid, and the page gets an Arrange button that lets people reorder them

Copy an existing tool that is close to yours; `src/tools/sensors` and
`src/tools/connections` are small.

### 2. Data from the system (Rust)

If the page needs data the live sample doesn't have:

1. Add `src-tauri/src/tools/<your_tool>.rs`, named like your folder in
   `src/tools/` (with `_` for `-`), with `#[tauri::command]` functions. Use
   `tauri::async_runtime::spawn_blocking` for anything slow, so the window
   stays responsive.
2. Add `pub mod <your_tool>;` to `src-tauri/src/tools.rs`, and your commands
   to the tools part of the `generate_handler!` list in `src-tauri/src/lib.rs`.
   Tauri needs every command in that one list.
3. If the tool keeps state between calls, add a field to `AppState` in
   `lib.rs`.
4. Add a typed wrapper for each command to `api` in `src/lib/api.ts`.

### 3. Programs it runs

If the tool runs another program (like `sensors`, `ss` or `smartctl`):

- Run it with `programs::run`, which reports a missing program with install
  instructions.
- Credit the project in `src/lib/credits.ts`, with the program's name in
  `program`, and in `CREDITS.md`. Add its id to your tool's `sources`. The page
  header, the About page and the "is it installed?" check all follow from that.
- Add the program's package names for each distribution to `install_hint` in
  `src-tauri/src/programs.rs`.

Prefer reading `/proc` and `/sys` directly when that's simple; it is faster and
needs nothing installed.

### Ground rules

- Nothing leaves the computer unless the user starts it (like the speed test) or
  has agreed to it (like the daily update check).
- Anything that needs administrator rights asks through polkit (`pkexec`),
  and only when the user clicks.
- Say what's happening in plain words. "Needs administrator rights to read
  drive health" beats an error code.
