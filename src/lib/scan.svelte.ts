// Disk space scan state. It lives here rather than in the page so a scan keeps
// reporting, and its result stays, while you look at other pages.
import { listen } from "@tauri-apps/api/event";
import { api, type ScanFolder, type ScanProgress, type ScanSummary } from "./api";

class Scan {
  path = $state("");
  progress = $state<ScanProgress | null>(null);
  running = $state(false);
  folder = $state<ScanFolder | null>(null);
  summary = $state<ScanSummary | null>(null);
  error = $state("");

  constructor() {
    listen<ScanProgress>("scan-progress", async (e) => {
      this.progress = e.payload;
      if (!e.payload.done) return;
      this.running = false;
      if (e.payload.cancelled) return;
      await this.open(0);
      this.summary = await api.scanSummary();
    });
  }

  async start() {
    this.error = "";
    this.folder = null;
    this.summary = null;
    this.progress = null;
    try {
      this.running = true;
      await api.scanStart(this.path.trim());
    } catch (e) {
      this.running = false;
      this.error = String(e);
    }
  }

  async open(id: number) {
    try {
      this.folder = await api.scanFolder(id);
    } catch (e) {
      this.error = String(e);
    }
  }
}

export const scan = new Scan();
