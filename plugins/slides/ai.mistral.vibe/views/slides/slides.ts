// Slides (plugins/slides, docs/plugin-apps.md A4-A5, migration step 3): the slides plugin's view,
// opened by a click on a pptx. It connects to its host with @bise/view (served beside it at
// ~kit/view/), reads the deck through its handle and draws it (deck.js, on the office SDK at
// ~kit/office/v1/). Read only this round (no write grant): the slide he is on is the view's state
// (the host gives it back when it reopens the tab on a newer version, apps-host m_60432); a
// clicked shape is his selection (the "type to ask about it" pill); his notes mark their shapes
// and slides.

import { connect, isGone, viewkit, type View } from "./~kit/view/view.js";
import { anchorOf, draw } from "./deck.js";

/** A selection's place, its note's label (`slide 2`): the catalog's words, never a literal here. */
function placeOf(anchor: string | null) {
  const a = anchor ? anchorOf(anchor) : null;
  return a ? viewkit.slide({ n: a.slide }) : null;
}

const main = document.getElementById("slides")!;

/** The words for what went wrong, never a stack. */
function words(err: unknown, name: string) {
  if (isGone(err)) return `${name} is no longer there.`;
  const m = err instanceof Error ? err.message : String(err);
  return `${name} could not be opened: ${m}.`;
}

function say(state: "opening" | "failed", text: string) {
  const p = document.createElement("p");
  p.className = "s-state";
  p.dataset.state = state;
  p.textContent = text;
  main.replaceChildren(p);
}

/** The foot: what Slides does not draw of this deck, one line each. */
function foot(lines: string[]) {
  if (lines.length === 0) return;
  const f = document.createElement("footer");
  f.className = "s-foot";
  for (const line of lines) f.append(Object.assign(document.createElement("p"), { textContent: line }));
  main.append(f);
}

async function open(view: View) {
  const doc = view.document();
  if (!doc) return say("failed", "Slides opens a deck: none was given.");
  try {
    const read = await view.read();
    if (!read.bytes) throw new Error("it came as text, not as a PowerPoint file");
    const stage = document.createElement("div");
    stage.className = "s-view";
    const deck = await draw(stage, read.bytes, { select: (anchor, text, at) => view.onSelection(anchor, text, { at, label: placeOf(anchor) }) });
    main.replaceChildren(stage);
    foot(deck.notes);
    deck.restore(view.state() ?? null);
    deck.onChange(() => view.setState(deck.place()));
    view.notes.mount({ place: (anchor) => deck.find(anchor) });
  } catch (e) {
    say("failed", words(e, doc.name));
  }
}

say("opening", "opening…");
connect({ name: "slides" }).then(open, (e) => say("failed", words(e, "the deck")));
