// Writer (plugins/docs, docs/plugin-apps.md A4-A5, migration steps 1 and 3): the docs plugin's
// view, opened by a click on a markdown or a Word file. It connects to its host with @bise/view
// (served beside it at ~kit/view/), reads the document through its handle, and draws it with the
// renderer of its type (markdown.js, docx.js; a renderer loads only when its type is opened).
//
// Read only this round (no write grant): the view keeps his place as its state (the kit sends
// it; the host gives it back when it reopens the tab on a newer version, apps-host m_60432), says
// his selection (the "type to ask about it" pill) and marks his notes on their parts.
//
// What a renderer is given (Host): open a link, say his selection, a note's quote. What it gives
// back (Drawn): what it could not draw (the foot), where he is (place, restore, onChange) and the
// part a note's anchor names (find).

import { connect, isGone, type View } from "./~kit/view/view.js";

export type Host = {
  openLink(url: string): void;
  /** his selection: its anchor and words, and where it is (the pill); nulls: nothing selected */
  select(anchor: string | null, text: string | null, at: DOMRect | null): void;
  /** the quote of his note on that anchor (to find it again when its part moved) */
  quoteOf(anchor: string): string | undefined;
};

export type Drawn = {
  /** what it could not draw of this file, one line each (the foot) */
  notes: string[];
  /** where he is, as JSON (the view's state) */
  place(): unknown;
  restore(place: unknown): void;
  /** fn runs when where he is changed (a scroll, a toggle) */
  onChange(fn: () => void): void;
  /** the part a note's anchor names; null: not in this version (the kit lists it as detached) */
  find(anchor: string): Element | null;
};

type Renderer = { draw(root: HTMLElement, file: string | Uint8Array, host: Host): Drawn | Promise<Drawn> };

/** The renderer of a file, by its name's ending; null: not Writer's. */
export function rendererOf(name: string): (() => Promise<Renderer>) | null {
  if (/\.(md|markdown)$/i.test(name)) return () => import("./markdown.js");
  if (/\.docx$/i.test(name)) return () => import("./docx.js");
  return null;
}

/** The words for what went wrong, never a stack. */
export function words(err: unknown, name: string) {
  if (isGone(err)) return `${name} is no longer there.`;
  const m = err instanceof Error ? err.message : String(err);
  return `${name} could not be opened: ${m}.`;
}

const main = document.getElementById("writer")!;

function say(state: "opening" | "failed", text: string) {
  let p = main.querySelector<HTMLElement>(":scope > .w-state");
  if (!p) {
    p = document.createElement("p");
    p.className = "w-state";
    main.prepend(p);
  }
  p.dataset.state = state;
  p.textContent = text;
}

/** The foot: what Writer does not draw of this file, one line each. */
function foot(lines: string[]) {
  if (lines.length === 0) return;
  const f = document.createElement("footer");
  f.className = "w-foot";
  for (const line of lines) {
    const p = document.createElement("p");
    p.textContent = line;
    f.append(p);
  }
  main.append(f);
}

async function open(view: View) {
  const doc = view.document();
  if (!doc) return say("failed", "Writer opens a file: none was given.");
  const load = rendererOf(doc.name);
  if (!load) return say("failed", `Writer can't draw ${doc.name}.`);
  try {
    const [renderer, read] = await Promise.all([load(), view.read()]);
    const stage = document.createElement("div");
    stage.className = "w-stage";
    const drawn = await renderer.draw(stage, read.text ?? read.bytes ?? "", {
      openLink: (url) => void view.openLink(url).catch(() => {}),
      select: (anchor, text, at) => view.onSelection(anchor, text, { at }),
      quoteOf: (anchor) => view.notes.all().find((n) => n.anchor === anchor)?.quote,
    });
    main.replaceChildren(stage);
    foot(drawn.notes);
    drawn.restore(view.state() ?? null);
    drawn.onChange(() => view.setState(drawn.place()));
    view.notes.mount({ place: (anchor) => drawn.find(anchor) });
  } catch (e) {
    say("failed", words(e, doc.name));
  }
}

say("opening", "opening…");
connect({ name: "writer" }).then(open, (e) => say("failed", words(e, "the file")));
