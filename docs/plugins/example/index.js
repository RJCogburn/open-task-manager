// An Open Task Manager plugin: a page that shows the kernel and uptime (from
// two commands the manifest declares) and two live charts. Copy this folder
// to ~/.local/share/open-task-manager/plugins/ to try it. See docs/plugins.md.

export default {
  async mount(el, host) {
    // The app's own classes (card, stats, stat, muted…) follow the theme.
    el.innerHTML = `
      <div class="grid" style="--min: 340px">
        <section class="card">
          <h3>This computer</h3>
          <div class="stats">
            <div class="stat"><span>Kernel</span><b data-kernel>…</b></div>
            <div class="stat"><span>Up</span><b data-uptime>…</b></div>
            <div class="stat"><span>CPU now</span><b data-cpu>…</b></div>
          </div>
        </section>
        <section class="card">
          <h3>Processor</h3>
          <div data-cpu-chart style="margin-top: 12px"></div>
        </section>
        <section class="card">
          <h3>Load average</h3>
          <div data-load-chart style="margin-top: 12px"></div>
        </section>
      </div>`;
    const $ = (name) => el.querySelector(`[data-${name}]`);

    // Commands run by their name in the manifest, never by a command line.
    const text = async (command) => {
      try {
        const out = await host.run(command);
        return out.status === 0 ? out.stdout.trim() : out.stderr.trim() || "—";
      } catch (e) {
        return String(e);
      }
    };
    $("kernel").textContent = await text("kernel");
    $("uptime").textContent = (await text("uptime")).replace(/^up /, "");

    // A chart of a history key the app already keeps.
    const stopCpuChart = host.chart($("cpu-chart"), {
      series: [{ label: "CPU", key: "cpu", color: "var(--cpu)" }],
      format: (v) => host.format.pct(v),
    });

    // A series of the plugin's own: push a value with each sample.
    const stopLoadChart = host.chart($("load-chart"), {
      series: [{ label: "1 minute", key: "own:load" }],
      max: "auto",
      format: (v) => v.toFixed(2),
    });
    const stopSamples = host.onSample((s) => {
      if (!s) return;
      $("cpu").textContent = host.format.pct(s.cpu);
      host.push("load", s.load[0]);
    });

    // Called when the page closes.
    return () => {
      stopSamples();
      stopCpuChart();
      stopLoadChart();
    };
  },
};
