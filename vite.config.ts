import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed dev port and handles its own Rust rebuilds.
export default defineConfig({
  plugins: [svelte()],
  // Tools live in their own folders at different depths; $lib reaches the shared code from any of them.
  resolve: { alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) } },
  clearScreen: false,
  server: {
    port: 1430,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  build: {
    target: ["es2022", "safari16"],
  },
});
