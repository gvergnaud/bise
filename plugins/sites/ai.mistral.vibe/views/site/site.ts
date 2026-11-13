// Sites (plugins/sites, docs/plugin-apps.md migration step 5, A5, architect m_60421): the view
// that opens a web page or a site folder an agent made. The hub calls this opener on his click
// with the document's handle; the view runs on the document's own origin (`isolate: "document"`
// in view.json: one origin per site, so its localStorage, indexedDB and cookies are its own and
// no other site reads them). @bise/view gives the session url of the site's files on that origin
// (resources/url: `/h/<session>/<token>/`, a folder's rel giving its index.html), framed here.
//
// The frame is of this view's origin, so the view reads the site's pages itself (blocks.ts), and
// bise puts no script in the site:
//
// - his selection: the block he selected in, as an anchor key (keys.ts) with his words; the kit
//   shows its "type to ask about it" pill by it;
// - his notes: the kit asks where a key is (blocks.ts finds it on the page shown); a note on
//   another page of the site, or on a block gone, goes detached (the kit tells the host);
// - links: a link inside the site moves the frame; a link out (another origin, mailto) opens in
//   his browser through the host, on his click;
// - where he is (the page of the site, its scroll): the view's state, put back when the tab is
//   drawn again.
//
// A site's scripts can reach this view (one origin): they can do what it can — read this
// document, post a selection, set its state, open a link on his click. Sites has no write grant,
// no server, and a document view has no ui/message (A4), so that is all.

import { connect, type View } from "./~kit/view/view.js";
import { keyOf, placeOf } from "./keys.js";
import { elementOf, selected } from "./blocks.js";
import { inside, stateOf, type State } from "./where.js";
import { viewkit } from "./~kit/view/words.gen.js";

const main = document.getElementById("site")!;

/** The marks' look inside the site's pages: @bise/view's own stylesheet (the one designer signs),
 * linked into each page, since the view's styles are not in the site's documents. Same origin, so
 * the site CSP's style-src 'self' takes it. */
const MARKS_CSS = new URL("~kit/view/view.css", document.baseURI).href;

function say(state: "opening" | "failed", text: string) {
  let p = main.querySelector<HTMLElement>(":scope > .st-state");
  if (!p) {
    p = document.createElement("p");
    p.className = "st-state";
    main.prepend(p);
  }
  p.dataset.state = state;
  p.textContent = text;
}

/** The page the frame shows, when it is the site's (same origin); null while it loads. */
function pageOf(frame: HTMLIFrameElement): Document | null {
  try {
    return frame.contentDocument;
  } catch {
    return null;
  }
}

function wire(view: View, frame: HTMLIFrameElement, base: string) {
  const layer = view.notes.mount({
    place: (anchor) => {
      const doc = pageOf(frame);
      const p = placeOf(anchor);
      return doc && p ? elementOf(doc, p) : null;
    },
  });

  let saved: State | null = stateOf(view.state(), base);

  frame.addEventListener("load", () => {
    const doc = pageOf(frame);
    const win = frame.contentWindow;
    if (!doc || !win) return;
    const link = doc.createElement("link");
    link.rel = "stylesheet";
    link.href = MARKS_CSS;
    (doc.head ?? doc.documentElement).append(link);

    // where he was, once: the first page drawn after the tab came back
    if (saved && saved.path === win.location.pathname) win.scrollTo(0, saved.y);
    saved = null;

    const keep = () => view.setState({ path: win.location.pathname, y: Math.round(win.scrollY) } satisfies State);
    win.addEventListener("scroll", keep, { passive: true });
    keep();

    const tell = () => {
      const s = selected(doc);
      const key = s && keyOf(s.place);
      if (!s || !key) return view.onSelection(null);
      // the box is in the page's viewport: move it into the view's
      const f = frame.getBoundingClientRect();
      const r = s.rect;
      view.onSelection(key, s.text, { at: new DOMRect(f.left + r.left, f.top + r.top, r.width, r.height) });
    };
    doc.addEventListener("mouseup", () => setTimeout(tell, 0));
    doc.addEventListener("keyup", (e) => e.shiftKey && tell());
    doc.addEventListener("scroll", () => view.onSelection(null), { passive: true, capture: true });

    doc.addEventListener("click", (e) => {
      const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
      if (!a || e.defaultPrevented) return;
      if (inside(a.href, base)) {
        // the site's own page: in this frame, never a new window
        if (a.target && a.target !== "_self") {
          e.preventDefault();
          win.location.href = a.href;
        }
        return;
      }
      e.preventDefault();
      if (/^(https?:|mailto:)/i.test(a.href)) view.openLink(a.href);
    });

    layer.redraw();
  });
}

async function start() {
  say("opening", viewkit.opening);
  let view: View;
  try {
    view = await connect({ name: "site" });
  } catch {
    return say("failed", viewkit.didnt_open);
  }
  const doc = view.document();
  if (!doc) return say("failed", viewkit.didnt_open);
  let url: string;
  try {
    url = await view.url();
  } catch {
    return say("failed", viewkit.didnt_open);
  }
  // TODO(site-storage-seed): goes in v2026.12.15. The storage bise kept for this site while it
  // ran opaque, once, into its own origin's (this view's: one origin): a key the site wrote
  // since stays its own
  try {
    const kept = await view.seededStorage();
    for (const [k, v] of Object.entries(kept ?? {})) if (localStorage.getItem(k) === null) localStorage.setItem(k, v);
  } catch {
    // nothing kept, or no room: the site opens with what its origin has
  }
  // a folder's url ends in "/" (its index.html); a page's names the file
  const base = url.endsWith("/") ? url : url.slice(0, url.lastIndexOf("/") + 1);
  const frame = document.createElement("iframe");
  frame.className = "st-frame";
  frame.title = doc.name;
  const back = stateOf(view.state(), base);
  frame.src = back ? new URL(back.path, url).href : url;
  wire(view, frame, base);
  main.replaceChildren(frame);
}

void start();
