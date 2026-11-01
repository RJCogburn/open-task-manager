// Rearranging cards. Pages mark each card with `use:card={{ group, id }}`;
// in arrange mode the user drags cards (or moves a focused card with the
// arrow keys) into any order. The order is saved in settings, so it lasts
// across restarts. Cards are placed with the CSS `order` property, so pages
// keep rendering them in their own order and the grid does the rest.
import { settings } from "./settings.svelte";

class Arrange {
  /** Arrange mode, toggled from the page header. */
  on = $state(false);
  /** Cards on screen, so the header knows whether to offer arranging. */
  cards = $state(0);
}
export const arrange = new Arrange();

/** Card ids on screen in each group, in the order they first appeared. */
const present = $state<Record<string, string[]>>({});

/** Groups that save their order somewhere other than settings (the widget panel). */
const custom = new Map<string, { order: () => string[]; save: (ids: string[]) => void }>();

function saved(group: string): string[] {
  return custom.get(group)?.order() ?? settings.cardOrder[group] ?? [];
}

/** The order cards in a group are shown in: saved ones first, new ones after. */
export function orderOf(group: string): string[] {
  const here = present[group] ?? [];
  const known = saved(group).filter((id) => here.includes(id));
  return [...known, ...here.filter((id) => !known.includes(id))];
}

function save(group: string, ids: string[]) {
  const handler = custom.get(group);
  if (handler) return handler.save(ids);
  // Cards not on screen now (an unplugged drive) keep their place for later.
  const away = (settings.cardOrder[group] ?? []).filter((id) => !ids.includes(id));
  settings.cardOrder = { ...settings.cardOrder, [group]: [...ids, ...away] };
}

/** Puts `id` where `target` is, shifting the cards between them. */
function moveTo(group: string, id: string, target: string) {
  const list = orderOf(group);
  const from = list.indexOf(id);
  const to = list.indexOf(target);
  if (from < 0 || to < 0 || from === to) return;
  list.splice(from, 1);
  list.splice(to, 0, id);
  save(group, list);
}

function step(group: string, id: string, by: number) {
  const list = orderOf(group);
  const target = list[list.indexOf(id) + by];
  if (target) moveTo(group, id, target);
}

/** Forgets the saved order of every group with cards on screen. */
export function resetOrder() {
  const groups = Object.keys(present).filter((g) => present[g].length);
  const next = { ...settings.cardOrder };
  for (const g of groups) {
    const handler = custom.get(g);
    if (handler) handler.save([...present[g]].sort());
    else delete next[g];
  }
  settings.cardOrder = next;
}

/** Lets a group keep its order elsewhere, like the widget panel's settings file. */
export function customGroup(group: string, order: () => string[], save: (ids: string[]) => void) {
  custom.set(group, { order, save });
}

type CardOptions = { group: string; id: string };

export function card(node: HTMLElement, options: CardOptions) {
  let { group, id } = options;
  present[group] = [...(present[group] ?? []), id];
  arrange.cards++;

  const stop = $effect.root(() => {
    $effect(() => {
      node.style.order = String(orderOf(group).indexOf(id));
    });
    $effect(() => {
      const on = arrange.on;
      node.classList.toggle("arranging", on);
      if (on) {
        node.tabIndex = 0;
        node.setAttribute("aria-roledescription", "movable card");
      } else {
        node.removeAttribute("tabindex");
        node.removeAttribute("aria-roledescription");
      }
    });
  });

  // Pointer dragging: while the pointer is over another card of the same
  // group, this card takes its place, so the grid shows the result as you go.
  function onpointerdown(e: PointerEvent) {
    if (!arrange.on || e.button !== 0) return;
    e.preventDefault();
    node.setPointerCapture(e.pointerId);
    node.classList.add("dragging");
  }
  function onpointermove(e: PointerEvent) {
    if (!node.hasPointerCapture(e.pointerId)) return;
    const over = document
      .elementsFromPoint(e.clientX, e.clientY)
      .map((el) => el.closest<HTMLElement>("[data-card]"))
      .find((el) => el && el !== node && el.dataset.cardGroup === group);
    if (over?.dataset.card) moveTo(group, id, over.dataset.card);
  }
  function onpointerup(e: PointerEvent) {
    if (node.hasPointerCapture(e.pointerId)) node.releasePointerCapture(e.pointerId);
    node.classList.remove("dragging");
  }
  function onkeydown(e: KeyboardEvent) {
    if (!arrange.on) return;
    const by = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[e.key];
    if (!by) return;
    e.preventDefault();
    step(group, id, by);
    queueMicrotask(() => node.focus());
  }

  const mark = () => {
    node.dataset.card = id;
    node.dataset.cardGroup = group;
  };
  mark();
  node.addEventListener("pointerdown", onpointerdown);
  node.addEventListener("pointermove", onpointermove);
  node.addEventListener("pointerup", onpointerup);
  node.addEventListener("pointercancel", onpointerup);
  node.addEventListener("keydown", onkeydown);

  return {
    update(next: CardOptions) {
      if (next.group === group && next.id === id) return;
      present[group] = (present[group] ?? []).filter((x) => x !== id);
      ({ group, id } = next);
      present[group] = [...(present[group] ?? []), id];
      mark();
    },
    destroy() {
      stop();
      present[group] = (present[group] ?? []).filter((x) => x !== id);
      arrange.cards--;
      node.removeEventListener("pointerdown", onpointerdown);
      node.removeEventListener("pointermove", onpointermove);
      node.removeEventListener("pointerup", onpointerup);
      node.removeEventListener("pointercancel", onpointerup);
      node.removeEventListener("keydown", onkeydown);
    },
  };
}
