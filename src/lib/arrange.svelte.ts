// Arranging cards. Pages mark each card with `use:card={{ group, id }}` and
// the element holding them with `use:board={{ group, min }}`. In arrange mode
// the user drags cards anywhere: into any column, at any height. Each card's
// column and place in it are saved in settings, so the layout lasts across
// restarts.
//
// A board places its cards itself, in as many columns as fit (each at least
// `min` pixels wide), measuring every card, so cards of different heights
// stack without gaps and moving one only shifts the cards below it. A card
// marked `data-wide="true"` spans every column, above the rest.
//
// The widget panel's tiles have no board: they keep a simple order, set with
// the CSS `order` property, and save it through `customGroup`.
import { settings } from "./settings.svelte";

/** Pointer capture keeps a drag going outside the element; a drag still works without it. */
function capture(el: HTMLElement, pointer: number) {
  try {
    el.setPointerCapture(pointer);
  } catch {
    // The pointer is already gone.
  }
}

class Arrange {
  /** Arrange mode, toggled from the page header. */
  on = $state(false);
  /** Cards on screen, so the header knows whether to offer arranging. */
  cards = $state(0);
}
export const arrange = new Arrange();

// ---------------------------------------------------------------------------
// Boards: cards in columns.

const GAP = 14;

/** Groups with a board on screen, so Reset knows what to forget. */
const boards = new Map<string, () => void>();

function cardsIn(node: HTMLElement): HTMLElement[] {
  return [...node.children].filter((c): c is HTMLElement => c instanceof HTMLElement && !!c.dataset.card);
}

export function board(node: HTMLElement, options: { group: string; min: number }) {
  let { group, min } = options;
  let frame = 0;
  // While dragging: the card, where it is grabbed, and where it would land.
  let drag: { el: HTMLElement; id: string; dx: number; dy: number; col: number; index: number } | null = null;
  let columns = 1;
  let colWidth = 0;
  let laidOut: string[][] = [];

  node.style.position = "relative";
  node.style.display = "block";

  /** Saved columns, fitted to how many columns there are now. */
  function columnsFor(ids: string[], count: number, heights: Map<string, number>): string[][] {
    const cols: string[][] = Array.from({ length: count }, () => []);
    const placed = new Set<string>();
    const saved = settings.cardLayout[group];
    if (saved) {
      saved.forEach((col, c) => {
        // Columns that no longer fit pour into the last one.
        const into = cols[Math.min(c, count - 1)];
        for (const id of col) if (ids.includes(id) && !placed.has(id)) (into.push(id), placed.add(id));
      });
    }
    // New cards (and everything, the first time) go where the column is shortest.
    const legacy = settings.cardOrder[group] ?? [];
    const rest = ids
      .filter((id) => !placed.has(id))
      .sort((a, b) => (legacy.indexOf(a) + 1 || 1e9) - (legacy.indexOf(b) + 1 || 1e9) || ids.indexOf(a) - ids.indexOf(b));
    const height = (col: string[]) => col.reduce((h, id) => h + (heights.get(id) ?? 0) + GAP, 0);
    for (const id of rest) {
      const shortest = cols.reduce((best, col, i) => (height(col) < height(cols[best]) ? i : best), 0);
      cols[shortest].push(id);
    }
    return cols;
  }

  function layout() {
    frame = 0;
    const cards = cardsIn(node);
    const width = node.clientWidth;
    if (!width) return;
    columns = Math.max(1, Math.floor((width + GAP) / (min + GAP)));
    colWidth = (width - GAP * (columns - 1)) / columns;

    const wide = cards.filter((c) => c.dataset.wide === "true");
    const narrow = cards.filter((c) => c.dataset.wide !== "true");
    for (const c of wide) c.style.width = `${width}px`;
    for (const c of narrow) c.style.width = `${colWidth}px`;
    const heights = new Map(cards.map((c) => [c.dataset.card!, c.offsetHeight]));
    const byId = new Map(cards.map((c) => [c.dataset.card!, c]));

    let top = 0;
    for (const c of wide) {
      place(c, 0, top);
      top += heights.get(c.dataset.card!)! + GAP;
    }
    const ids = narrow.map((c) => c.dataset.card!).filter((id) => id !== drag?.id);
    laidOut = columnsFor(ids, columns, heights);
    let bottom = top;
    laidOut.forEach((col, c) => {
      let y = top;
      col.forEach((id, i) => {
        if (drag && drag.col === c && drag.index === i) y += heights.get(drag.id)! + GAP;
        place(byId.get(id)!, c * (colWidth + GAP), y);
        y += heights.get(id)! + GAP;
      });
      if (drag && drag.col === c && drag.index >= col.length) y += heights.get(drag.id)! + GAP;
      bottom = Math.max(bottom, y);
    });
    node.style.height = `${Math.max(0, bottom - GAP)}px`;
  }

  function place(card: HTMLElement, x: number, y: number) {
    card.style.position = "absolute";
    card.style.left = `${x}px`;
    card.style.top = `${y}px`;
  }

  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(layout);
  };

  /** Saves the columns as laid out, with `id` put at (col, index). */
  function commit(id: string, col: number, index: number) {
    const cols = laidOut.map((c) => c.filter((x) => x !== id));
    cols[col].splice(Math.min(index, cols[col].length), 0, id);
    // Cards not on screen now (an unplugged drive) keep their place for later.
    const here = new Set(cols.flat());
    const old = settings.cardLayout[group] ?? [];
    old.forEach((c, i) => {
      for (const x of c) if (!here.has(x) && cols[Math.min(i, cols.length - 1)]) cols[Math.min(i, cols.length - 1)].push(x);
    });
    settings.cardLayout = { ...settings.cardLayout, [group]: cols };
  }

  function where(id: string): [number, number] {
    for (let c = 0; c < laidOut.length; c++) {
      const i = laidOut[c].indexOf(id);
      if (i >= 0) return [c, i];
    }
    return [0, 0];
  }

  function onpointerdown(e: PointerEvent) {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-card]");
    if (!arrange.on || e.button !== 0 || !el || el.parentElement !== node || el.dataset.wide === "true") return;
    e.preventDefault();
    // Pin the layout as it is, so lifting a card doesn't re-place the others.
    if (!settings.cardLayout[group]) settings.cardLayout = { ...settings.cardLayout, [group]: laidOut.map((c) => [...c]) };
    const box = el.getBoundingClientRect();
    const [col, index] = where(el.dataset.card!);
    drag = { el, id: el.dataset.card!, dx: e.clientX - box.left, dy: e.clientY - box.top, col, index };
    el.classList.add("dragging");
    capture(node, e.pointerId);
    layout();
  }

  function onpointermove(e: PointerEvent) {
    if (!drag) return;
    const area = node.getBoundingClientRect();
    const x = e.clientX - area.left - drag.dx;
    const y = e.clientY - area.top - drag.dy;
    drag.el.style.left = `${x}px`;
    drag.el.style.top = `${y}px`;
    // The column under the card's middle, and the first card there whose middle is below the pointer.
    const col = Math.max(0, Math.min(columns - 1, Math.floor((x + colWidth / 2 + GAP / 2) / (colWidth + GAP))));
    const others = laidOut[col] ?? [];
    let index = others.length;
    for (let i = 0; i < others.length; i++) {
      const box = cardsIn(node).find((c) => c.dataset.card === others[i])!.getBoundingClientRect();
      if (e.clientY < box.top + box.height / 2) {
        index = i;
        break;
      }
    }
    if (col !== drag.col || index !== drag.index) {
      drag.col = col;
      drag.index = index;
      const { el } = drag;
      const keep = [el.style.left, el.style.top];
      layout();
      [el.style.left, el.style.top] = keep;
    }
  }

  function onpointerup(e: PointerEvent) {
    if (!drag) return;
    if (node.hasPointerCapture(e.pointerId)) node.releasePointerCapture(e.pointerId);
    const { el, id, col, index } = drag;
    drag = null;
    el.classList.remove("dragging");
    commit(id, col, index);
    layout();
  }

  function onkeydown(e: KeyboardEvent) {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-card]");
    if (!arrange.on || !el || el.parentElement !== node || el.dataset.wide === "true") return;
    const move = { ArrowUp: [0, -1], ArrowDown: [0, 1], ArrowLeft: [-1, 0], ArrowRight: [1, 0] }[e.key];
    if (!move) return;
    e.preventDefault();
    const id = el.dataset.card!;
    const [col, index] = where(id);
    const to = Math.max(0, Math.min(columns - 1, col + move[0]));
    const length = laidOut[to].filter((x) => x !== id).length;
    commit(id, to, Math.max(0, Math.min(length, to === col ? index + move[1] : index)));
    layout();
    el.focus();
  }

  const sizes = new ResizeObserver(schedule);
  const watch = () => {
    sizes.disconnect();
    sizes.observe(node);
    for (const c of cardsIn(node)) sizes.observe(c);
  };
  const changes = new MutationObserver(() => {
    watch();
    schedule();
  });
  changes.observe(node, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-wide"] });
  watch();
  // Layout follows saved columns, so Reset and other windows' changes show at once.
  const stop = $effect.root(() => {
    $effect(() => {
      void settings.cardLayout[group];
      void settings.cardOrder[group];
      schedule();
    });
  });
  boards.set(group, schedule);
  node.addEventListener("pointerdown", onpointerdown);
  node.addEventListener("pointermove", onpointermove);
  node.addEventListener("pointerup", onpointerup);
  node.addEventListener("pointercancel", onpointerup);
  node.addEventListener("keydown", onkeydown);
  node.classList.add("board");
  schedule();

  return {
    update(next: { group: string; min: number }) {
      ({ group, min } = next);
      schedule();
    },
    destroy() {
      cancelAnimationFrame(frame);
      sizes.disconnect();
      changes.disconnect();
      stop();
      boards.delete(group);
      node.removeEventListener("pointerdown", onpointerdown);
      node.removeEventListener("pointermove", onpointermove);
      node.removeEventListener("pointerup", onpointerup);
      node.removeEventListener("pointercancel", onpointerup);
      node.removeEventListener("keydown", onkeydown);
    },
  };
}

// ---------------------------------------------------------------------------
// Cards. On a board, the board places and moves them; elsewhere (the widget
// panel) they keep a simple order.

/** Card ids on screen in each ordered group, in the order they first appeared. */
const present = $state<Record<string, string[]>>({});

/** Groups that save their order somewhere other than settings (the widget panel). */
const custom = new Map<string, { order: () => string[]; save: (ids: string[]) => void }>();

/** Lets a group keep its order elsewhere, like the widget panel's settings file. */
export function customGroup(group: string, order: () => string[], save: (ids: string[]) => void) {
  custom.set(group, { order, save });
}

/** The order cards in a group are shown in: saved ones first, new ones after. */
function orderOf(group: string): string[] {
  const here = present[group] ?? [];
  const saved = custom.get(group)?.order() ?? settings.cardOrder[group] ?? [];
  const known = saved.filter((id) => here.includes(id));
  return [...known, ...here.filter((id) => !known.includes(id))];
}

function moveTo(group: string, id: string, target: string) {
  const list = orderOf(group);
  const from = list.indexOf(id);
  const to = list.indexOf(target);
  if (from < 0 || to < 0 || from === to) return;
  list.splice(from, 1);
  list.splice(to, 0, id);
  const handler = custom.get(group);
  if (handler) handler.save(list);
  else settings.cardOrder = { ...settings.cardOrder, [group]: list };
}

/** Forgets the arrangement of every group on screen. */
export function resetOrder() {
  const layout = { ...settings.cardLayout };
  const order = { ...settings.cardOrder };
  for (const g of [...boards.keys(), ...Object.keys(present).filter((g) => present[g].length)]) {
    const handler = custom.get(g);
    if (handler) handler.save([...(present[g] ?? [])].sort());
    delete layout[g];
    delete order[g];
  }
  settings.cardLayout = layout;
  settings.cardOrder = order;
}

type CardOptions = { group: string; id: string };

export function card(node: HTMLElement, options: CardOptions) {
  let { group, id } = options;
  const onBoard = () => node.parentElement?.classList.contains("board") ?? false;
  present[group] = [...(present[group] ?? []), id];
  arrange.cards++;

  const mark = () => {
    node.dataset.card = id;
    node.dataset.cardGroup = group;
  };
  mark();

  const stop = $effect.root(() => {
    $effect(() => {
      // On a board, position comes from the board.
      node.style.order = onBoard() ? "" : String(orderOf(group).indexOf(id));
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

  // Off a board: while the pointer is over another card of the group, this
  // card takes its place.
  function onpointerdown(e: PointerEvent) {
    if (!arrange.on || e.button !== 0 || onBoard()) return;
    e.preventDefault();
    capture(node, e.pointerId);
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
    if (!arrange.on || onBoard()) return;
    const by = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[e.key];
    if (!by) return;
    e.preventDefault();
    const list = orderOf(group);
    const target = list[list.indexOf(id) + by];
    if (target) moveTo(group, id, target);
    queueMicrotask(() => node.focus());
  }

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
