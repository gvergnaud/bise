// Sheets (plugins/sheets, docs/plugin-apps.md A4-A5, migration step 4): the sheets plugin's view,
// opened by a click on an xlsx or a csv. It connects to its host with @bise/view (served beside
// it at ~kit/view/), reads the document through its handle (a file past a read's 24 MiB through
// its session URL, resources/url), draws it in the grid (book.js) and saves his edits through the
// kit (the write grant). The sheet and range he is on are the view's state; a selected range is
// his selection (the "type to ask about it" pill); his notes mark their cells. Typing over a
// selection is the grid's own on a writable document (the kit's typeToAsk follows `writable`).

import { connect, isGone, type View } from "./~kit/view/view.js";
import { draw } from "./book.js";

const main = document.getElementById("grid")!;
/** The most one read answers (docs/plugin-apps.md A4; @bise/view's READ_MAX). */
const READ_MAX = 24 * 1024 * 1024;

/** The words for what went wrong, never a stack. */
function words(err: unknown, name: string) {
  if (isGone(err)) return `${name} is no longer there.`;
  const m = err instanceof Error ? err.message : String(err);
  return `${name} could not be opened: ${m}.`;
}

function say(state: "opening" | "failed", text: string) {
  const p = document.createElement("p");
  p.className = "g-state";
  p.dataset.state = state;
  p.textContent = text;
  main.replaceChildren(p);
}

/** The foot: what the grid does not draw of this file, one line each. */
function foot(lines: string[]) {
  if (lines.length === 0) return;
  const f = document.createElement("footer");
  f.className = "g-foot";
  for (const line of lines) f.append(Object.assign(document.createElement("p"), { textContent: line }));
  main.append(f);
}

/** The file whole: one read, or past a read's most, its session URL (connect-src 'self'). */
async function whole(view: View): Promise<{ file: string | Uint8Array; etag: string; writable: boolean }> {
  const doc = view.document()!;
  if (doc.size <= READ_MAX) {
    const r = await view.read();
    return { file: r.text ?? r.bytes ?? "", etag: r.etag, writable: r.writable };
  }
  const head = await view.read({ range: { offset: 0, length: 1 } });
  const res = await fetch(await view.url(), { cache: "no-store" });
  if (!res.ok) throw new Error(`the file could not be read (${res.status})`);
  return { file: new Uint8Array(await res.arrayBuffer()), etag: head.etag, writable: head.writable };
}

const dark = () => document.documentElement.dataset.theme === "dark";

async function open(view: View) {
  const doc = view.document();
  if (!doc) return say("failed", "Sheets opens a file: none was given.");
  try {
    const { file, etag, writable } = await whole(view);
    const stage = document.createElement("div");
    stage.className = "g-view";
    main.replaceChildren(stage);
    const darks = new Set<(on: boolean) => void>();
    view.onContext(() => darks.forEach((fn) => fn(dark())));
    const grid = await draw(stage, doc.name, file, { etag, writable: writable && doc.writable }, {
      write: (body, ifMatch) => view.save(body, { ifMatch }),
      select: (anchor, text, at) => view.onSelection(anchor, text, { at }),
      dark,
      onDark: (fn) => darks.add(fn),
    });
    foot(grid.notes);
    grid.restore(view.state() ?? null);
    grid.onChange(() => view.setState(grid.place()));
    view.notes.mount({ place: (anchor) => grid.find(anchor) });
    view.onTeardown(() => grid.flush());
  } catch (e) {
    say("failed", words(e, doc.name));
  }
}

say("opening", "opening…");
connect({ name: "sheets" }).then(open, (e) => say("failed", words(e, "the file")));
