// bise kit · @bise/view's marks (docs/plugin-apps.md A5): his notes drawn on the view's own parts.
// The view says where an anchor is (`place(anchor)`: its elements, none when this version has no
// such part); the kit marks them (`data-bise-mark` = sent | draft | done, view.css draws 3 px of
// accent, ✓ on the version that answered) and tells the host which notes it could not place
// (dev.bise/detached: the host lists them, never lost). What a version marks is core.layout's.
// Drawn again on each new notes list and on `redraw()` (the view drew its parts again).
import * as core from "./core.js";

/** Where an anchor is in the view: its elements (one or more), or none. */
export type Place = (anchor: string) => Element | readonly Element[] | null | undefined;

export type NotesLayer = {
  /** draw the marks again (the view redrew its parts) */
  redraw(): void;
  /** take the marks off and stop following the notes */
  unmount(): void;
};

/** The attribute the kit owns on the view's parts. */
export const MARK = "data-bise-mark";

const RANK = { draft: 1, done: 2, sent: 3 } as const;

export function mountNotes(o: {
  place: Place;
  version: () => number | null;
  notes: { all(): readonly core.Note[]; on(fn: (n: readonly core.Note[]) => void): () => void };
  detached: (ids: string[]) => void;
}): NotesLayer {
  let marked: Element[] = [];
  let lastDetached = "";
  const els = (anchor: string): Element[] => {
    const at = o.place(anchor);
    return !at ? [] : Array.isArray(at) ? [...at] : [at as Element];
  };

  function draw() {
    for (const e of marked) e.removeAttribute(MARK);
    marked = [];
    const found = new Map<string, Element[]>();
    const where = (a: string) => {
      if (!found.has(a)) found.set(a, els(a));
      return found.get(a)!;
    };
    const { marked: on, detached } = core.layout(o.notes.all(), o.version(), (a) => where(a).length > 0);
    for (const { note, mark } of on) {
      for (const e of where(note.anchor)) {
        // the strongest mark wins on a part with many notes: sent over done over draft
        const had = e.getAttribute(MARK) as keyof typeof RANK | null;
        if (!had || RANK[mark] > RANK[had]) e.setAttribute(MARK, mark);
        marked.push(e);
      }
    }
    const ids = detached.map((n) => n.id);
    const key = JSON.stringify(ids);
    if (key !== lastDetached) {
      lastDetached = key;
      o.detached(ids);
    }
  }

  const off = o.notes.on(draw);
  draw();
  return {
    redraw: draw,
    unmount() {
      off();
      for (const e of marked) e.removeAttribute(MARK);
      marked = [];
    },
  };
}
