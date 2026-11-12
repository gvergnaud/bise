// Preview (plugins/media, docs/plugin-apps.md migration steps 1-2): the view that opens an
// image, a pdf or a video. The hub calls this opener on his click with the file's handle
// (`view-file://…`); @bise/view (~kit/view/, from bise's install) reads it, keeps the view's state
// and his notes. This picks the renderer by the file's name (kind.ts), gives it the bytes (one
// read) or the session url (resources/url, read by its ranges: a video always, a big pdf), and
// wires the rest:
//
// - his selection: the renderer's place as an anchor key (keys.ts), with its words; the kit
//   shows its "type to ask about it" pill by it;
// - his notes: the kit asks where a key is (keys.ts parses it, the renderer finds it on this
//   version); a key it can't parse or place goes detached (the kit says so to the host);
// - where he is (the zoom, the scroll, the moment): the view's state, put back when the tab is
//   drawn again;
// - what the renderer can't draw: one line each in the foot.
//
// Read only: Preview never writes the file (no write grant in view.json).

import { connect, viewkit, type View } from "./~kit/view/view.js";
import { clock, keyOf, placeOf, type Place } from "./keys.js";
import { kindOf, wayOf, words, type Kind } from "./kind.js";
import type { Renderer, Shown, Source } from "./shown.js";

const RENDERERS: Record<Kind, () => Promise<Renderer>> = {
  image: () => import("./image.js"),
  pdf: () => import("./pdf.js"),
  video: () => import("./video.js"),
};

const main = document.getElementById("preview")!;

/** What the composer's chip and the note's line call a place (Anchor::View's label), in the words
 * designer signed (bise_proto::words::viewkit, m_61894): `an area`, `page 2 · an area`, `0:12`,
 * `0:12 · an area`; a passage (`words`: his words are its quote) is its page, `page 2`. */
function labelOf(p: Place, words = false): string {
  if (p.kind === "time") return p.rect ? viewkit.moment_area({ t: clock(p.t) }) : clock(p.t);
  if (words) return viewkit.page({ n: p.page ?? 1 });
  return p.page ? viewkit.page_area({ n: p.page }) : viewkit.area;
}

/** The view's state before its file is drawn: opening, or it didn't open and why (its own line). */
function say(state: "opening" | "failed", why?: string) {
  let p = main.querySelector<HTMLElement>(":scope > .pv-state");
  if (!p) {
    p = document.createElement("div");
    p.className = "pv-state";
    main.prepend(p);
  }
  p.dataset.state = state;
  const head = document.createElement("p");
  head.textContent = state === "opening" ? viewkit.opening : viewkit.didnt_open;
  const lines = [head];
  if (why) {
    const reason = document.createElement("p");
    reason.className = "why";
    reason.textContent = why;
    lines.push(reason);
  }
  p.replaceChildren(...lines);
}

/** The foot: what Preview doesn't draw of this file, one line each. */
function foot(lines: string[]) {
  main.querySelector(":scope > .pv-foot")?.remove();
  if (!lines.length) return;
  const f = document.createElement("footer");
  f.className = "pv-foot";
  for (const n of lines) {
    const p = document.createElement("p");
    p.textContent = n;
    f.append(p);
  }
  main.append(f);
}

async function source(view: View, kind: Kind, name: string, size: number): Promise<Source> {
  if (wayOf(kind, size) === "url") return { name, url: await view.url() };
  const r = await view.read();
  return { name, bytes: r.bytes ?? new TextEncoder().encode(r.text ?? "") };
}

async function open() {
  const view = await connect({ name: "preview", version: "0.1.0" });
  const doc = view.document();
  if (!doc) return say("failed");
  const kind = kindOf(doc.name);
  say("opening");
  if (!kind) return say("failed", doc.name);

  let shown: Shown | null = null;
  let layer: { redraw(): void } | null = null;
  const stage = document.createElement("div");
  stage.className = "pv-stage";
  try {
    const renderer = await RENDERERS[kind]();
    const src = await source(view, kind, doc.name, doc.size);
    shown = await renderer.draw(stage, src, {
      select: (place: Place | null, text?: string | null, at?: Element | DOMRect | null) =>
        view.onSelection(place ? keyOf(place) : null, text ?? null, { at: at ?? null, label: place ? labelOf(place, !!text) : null }),
      moved: () => shown && view.setState(shown.state()),
      redrawn: () => layer?.redraw(),
      label: (place: Place) => labelOf(place),
    });
  } catch (e) {
    console.error(e);
    return say("failed", words(e as { code?: unknown; message?: unknown }, doc.name));
  }
  main.querySelector(":scope > .pv-state")?.remove();
  main.prepend(stage);
  shown.restore(view.state());
  foot(shown.foot);

  // his notes: a key this view parses and finds on this version is marked; the rest detached
  const quoteOf = (key: string) => view.notes.all().find((n) => n.anchor === key)?.quote ?? "";
  layer = view.notes.mount({
    place: (key) => {
      const p = placeOf(key);
      return p && shown ? shown.place(p, quoteOf(key)) : null;
    },
  });
}

open().catch((e) => {
  console.error(e);
  say("failed", words(e as { code?: unknown; message?: unknown }, ""));
});
