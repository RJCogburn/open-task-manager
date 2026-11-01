import { openUrl } from "@tauri-apps/plugin-opener";

/** Opens web links in the default browser instead of inside the app window. */
export function external(node: HTMLAnchorElement) {
  const onclick = (e: MouseEvent) => {
    e.preventDefault();
    void openUrl(node.href);
  };
  node.addEventListener("click", onclick);
  return { destroy: () => node.removeEventListener("click", onclick) };
}
