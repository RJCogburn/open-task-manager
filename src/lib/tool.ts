// What a tool tells the app about itself. Each folder in src/tools has an
// index.ts that default-exports one of these; the sidebar finds them all.
import type { Component } from "svelte";
import type { CreditId } from "./credits";

export type Tool = {
  /** Stable id, used in the address (#cpu) and in saved settings. */
  id: string;
  /** Name in the sidebar. */
  label: string;
  /** "monitor" for live readings, "tools" for things you run or look up. */
  section: "monitor" | "tools";
  /** Position within its section; lower comes first. */
  order: number;
  /** The tool's icon: an SVG path drawn on a 24×24 grid with a 1.7 stroke. */
  icon: string;
  /** Colour of the icon, usually the tool's graph colour, e.g. "var(--cpu)". */
  color?: string;
  /** A live reading shown beside the name, e.g. "12%". Read live.sample here. */
  value?: () => string;
  /** The projects its data comes from, by their id in credits.ts. They are
   * linked in the page header and the About page, and any program among them
   * is checked for and offered for install. */
  sources?: CreditId[];
  page: Component;
};

export const defineTool = (tool: Tool) => tool;
