// Every tool, found by its folder: src/tools/<name>/index.ts. Adding a tool
// means adding a folder; nothing here needs to change.
import type { Tool } from "$lib/tool";

const found = import.meta.glob<{ default: Tool }>("./*/index.ts", { eager: true });

export const sections = [
  { id: "monitor", title: "Monitor" },
  { id: "tools", title: "Tools" },
] as const;

export const tools: Tool[] = Object.values(found)
  .map((m) => m.default)
  .sort(
    (a, b) =>
      sections.findIndex((s) => s.id === a.section) - sections.findIndex((s) => s.id === b.section) ||
      a.order - b.order,
  );

const ids = new Set<string>();
for (const t of tools) {
  if (ids.has(t.id)) console.error(`Two tools use the id "${t.id}"; ids must be unique.`);
  ids.add(t.id);
}
