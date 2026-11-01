// New versions: checking for them, installing them, and the daily check.
// Checking contacts GitHub, so automatic checks only run once the user has
// agreed (settings.updates.autoCheck); "Check now" always works.
import { listen } from "@tauri-apps/api/event";
import { api, parseError, type UpdateStatus } from "./api";
import { settings } from "./settings.svelte";

export const RELEASES = "https://github.com/RJCogburn/open-task-manager/releases";
const DAY = 24 * 60 * 60 * 1000;

class Updates {
  status = $state<UpdateStatus | null>(null);
  checking = $state(false);
  installing = $state(false);
  /** Installed, waiting for a restart. */
  installed = $state(false);
  progress = $state<{ done: number; total: number | null } | null>(null);
  error = $state("");

  constructor() {
    api.updateLocal().then((s) => (this.status ??= s));
    listen<[number, number | null]>("update-progress", (e) => {
      this.progress = { done: e.payload[0], total: e.payload[1] };
    });
  }

  /** The newer version, unless the user put off hearing about it. */
  get offer() {
    const available = this.status?.available;
    return available && !this.installed && available.version !== settings.updates.dismissed ? available : null;
  }

  async check() {
    if (this.checking || this.installing) return;
    this.checking = true;
    this.error = "";
    try {
      this.status = await api.updateCheck();
      settings.updates.lastCheck = Date.now();
      if (this.status.available && this.status.canInstall && settings.updates.autoInstall) await this.install();
    } catch (e) {
      this.error = parseError(e).message;
    } finally {
      this.checking = false;
    }
  }

  async install() {
    if (this.installing) return;
    this.installing = true;
    this.error = "";
    this.progress = null;
    try {
      await api.updateInstall();
      this.installed = true;
    } catch (e) {
      this.error = parseError(e).message;
    } finally {
      this.installing = false;
    }
  }

  /** Soon after start, then hourly, check if a day has passed since the last check. */
  start() {
    const due = () => {
      if (settings.updates.autoCheck && Date.now() - settings.updates.lastCheck > DAY) void this.check();
    };
    setTimeout(due, 15_000);
    setInterval(due, 60 * 60 * 1000);
  }
}

export const updates = new Updates();
