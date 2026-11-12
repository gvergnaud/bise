// bise kit · notes on an artifact in its viewer (office-artifacts plan §5, §9b; architect m_23874):
// the selection layer and the marks, over every renderer (sheet.js,
// image.js, site.js: a site draws its marks in its own frame through view.marks; pdf.js places
// its notes itself through view.where, re-anchored on the version's text; video.js too, ticks on
// its track, and it says `bise:office-selection` when its selection moves with no mouseup). One store (pages/artifact/notes.rs), two doors:
//
// - in the desktop's side panel (the frame's name is `bise-app`, art-desktop m_23900) a
//   selection's `note` button posts {bise: FRAME.anchor, artifact, version, anchor, quote} to the
//   parent, on his click only (architect m_23971): it becomes a chip in the maker's composer, and the hub records the note when
//   he sends his message;
// - in a browser the same button opens notes.js's note: the model, the words and the send of a
//   page's notes (BiseNotes.core), drafts saved to /api/a/<id>/notes, sent with /api/a/<id>/send.
//
// A note keeps its anchor in `at` (notes.js's own `anchor` is its ok/moved).
//
// Marks: every note sent and not yet answered keeps a 3 px accent mark on its part; the version
// that answers it (`done_in`, decided by the hub) shows ✓ there; later versions show nothing.
// They are drawn again on each version (swap.js's `bise:office-drawn`), from /a/<id>/notes and
// the artifact's SSE (`notes`); a note whose part is gone is `moved` in the list.
//
// Pure helpers (exported for office/sdk/test/viewer.test.ts): inRange, markOf, quoteOf, message.

import { FRAME } from "/kit/office/frame.js";

const A = globalThis.BiseAnchors;
const N = globalThis.BiseNotes && globalThis.BiseNotes.core;

// ---- the words (art-notes'; designer signs them)
const W = {
  note: "note",
  // the thread's quote pill (apps/desktop QuotePop's words): typing or a click adds it
  toMessage: "type to ask about it · ⌘C copy",
  added: "added to your message",
  moved: N ? N.NW.moved : "moved",
  doneIn: (v: number | string) => `✓ done in v${v}`,
  empty: {
    xlsx: "no notes yet. select cells, then note.",
    image: "no notes yet. draw a box, then note.",
    site: "no notes yet. select some text, then note.",
    pdf: "no notes yet. select some text or draw a box, then note.",
    video: "no notes yet. pause it or draw a box on it, then note.",
  },
};

/** `B4` inside `B4:D9` (refs as the grid writes them). */
export function inRange(ref: string | null | undefined, range: string | null | undefined) {
  const p = (r: string | null | undefined) => {
    const m = /^\$?([A-Z]{1,3})\$?(\d+)$/i.exec(r || "");
    if (!m) return null;
    let col = 0;
    for (const ch of m[1].toUpperCase()) col = col * 26 + (ch.charCodeAt(0) - 64);
    return { col, row: Number(m[2]) };
  };
  const [a, b = a] = String(range || "").split(":");
  const c = p(ref), x = p(a), y = p(b);
  if (!c || !x || !y) return false;
  return c.col >= Math.min(x.col, y.col) && c.col <= Math.max(x.col, y.col) && c.row >= Math.min(x.row, y.row) && c.row <= Math.max(x.row, y.row);
}

/** What a note's mark is in version `v`: `draft`, `sent`, `done` (✓, the answering version),
 * or null (no mark: answered in an older version, or not his yet). */
export function markOf(note: Pick<OfficeNote, "status" | "done_in">, v: number): "draft" | "sent" | "done" | null {
  if (note.status === "draft") return "draft";
  if (note.status === "sent") return "sent";
  if (note.status === "done" && note.done_in != null) return Number(note.done_in) === Number(v) ? "done" : null;
  return null;
}

/** What he saw selected, one line, at most 200 characters ("" none). */
export function quoteOf(text: string | null | undefined) {
  const one = String(text || "").replace(/\s+/g, " ").trim();
  return one.length > 200 ? `${one.slice(0, 199)}…` : one;
}

/** The message the frame posts to the desktop for a selection: its tag and exactly the four
 * facts (architect m_23971: no token, no list, no url), whatever window it is in; `key`: his
 * first typed character when typing took it, so "why" never arrives as "hy" (architect m_24155). */
export function message(info: Pick<OfficeInfo, "id" | "version">, anchor: Anchor, quote: string, key?: string | null) {
  return { bise: FRAME.anchor, artifact: info.id, version: info.version, anchor, quote, ...(key ? { key } : {}) };
}

/** The character a keydown types, when it is exactly one printable grapheme and no shortcut, IME
 * composition or dead key (those take the selection with no key); null otherwise. */
export function typed(e: Pick<KeyboardEvent, "ctrlKey" | "metaKey" | "altKey" | "isComposing" | "key">) {
  if (e.ctrlKey || e.metaKey || e.altKey || e.isComposing || e.key === "Dead" || typeof e.key !== "string") return null;
  const g = typeof Intl !== "undefined" && Intl.Segmenter ? [...new Intl.Segmenter().segment(e.key)].length : [...e.key].length;
  return g === 1 && !/[\u0000-\u001f\u007f]/.test(e.key) ? e.key : null;
}

// ======================================================================================
// the DOM
// ======================================================================================

function mount() {
  const main = document.getElementById("bise-office");
  const office = window.biseOffice;
  if (!main || !office || !A) return;
  const info = office.info;
  const inApp = window.name === "bise-app";
  const token = (document.querySelector<HTMLMetaElement>('meta[name="bise-token"]') || ({} as { content?: string })).content || "";
  const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string | null, text?: string | null) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  };
  const stage = () => main.querySelector(":scope > .office-stage");
  const version = () => Number(office.info.version) || Number(info.version) || 1;
  let notes: OfficeNote[] = [];

  // ---- where an anchor is in the drawing: the elements to mark, and whether it is still there
  // (office, A: checked above; a function declaration keeps no narrowing)
  function where(a: Anchor | null | undefined): { els: HTMLElement[]; found: boolean } {
    const s = stage();
    if (!s || !a) return { els: [], found: false };
    // a renderer that places its own notes (pdf.js: re-anchored on the version's text)
    const own = office!.view && office!.view.where ? office!.view.where(a) : null;
    if (own) return own as { els: HTMLElement[]; found: boolean };
    const q = (sel: string) => [...s.querySelectorAll<HTMLElement>(sel)];
    const esc = (v: unknown) => CSS.escape(String(v));
    switch (a.kind) {
      case "cell": {
        const tabs = q(".office-tabs button");
        const tab = tabs.find((b) => b.textContent === a.sheet);
        const shown = office!.view && office!.view.place && office!.view.place().sheet === a.sheet;
        const els = shown ? q("td[data-ref]").filter((td) => inRange(td.dataset.ref, a.range)) : tab ? [tab] : [];
        return { els, found: !!tab || !!shown };
      }
      case "region": {
        const frame = s.querySelector(".office-image .frame, .frame");
        if (!frame) return { els: [], found: false };
        const box = el("div", "office-mark-box");
        box.dataset.ours = "1";
        Object.assign(box.style, { left: `${a.rect!.x}%`, top: `${a.rect!.y}%`, width: `${a.rect!.w}%`, height: `${a.rect!.h}%` });
        frame.append(box);
        return { els: [box], found: true };
      }
      default:
        return { els: [], found: false };
    }
  }

  // ---- the marks: cleared and drawn again (a new version, a sheet or slide shown, new notes)
  let drawing = false;
  function marks() {
    const s = stage();
    if (!s) return;
    const view = office!.view;
    // a site draws its marks inside its own frame (kit/office/site.js): what to draw, no words
    if (view && view.marks) {
      const v = version();
      view.onFound = (found) => {
        notes.forEach((n, i) => (n.here = found.has(i)));
        if (list) list.render();
      };
      view.marks(notes.map((n) => ({ at: n.at, mark: markOf(n, v) })));
      if (list) list.render();
      return;
    }
    drawing = true;
    for (const e of s.querySelectorAll<HTMLElement>("[data-mark]")) delete e.dataset.mark;
    for (const e of s.querySelectorAll(".office-mark-box")) e.remove();
    const v = version();
    for (const n of notes) {
      const m = markOf(n, v);
      const w = n.at ? where(n.at) : { els: [], found: false };
      n.here = w.found;
      if (!m) {
        for (const e of w.els) if (e.classList.contains("office-mark-box")) e.remove();
        continue;
      }
      for (const e of w.els) {
        // the strongest mark wins on a part with many notes: sent over draft, ✓ over nothing
        const rank: Record<string, number> = { draft: 1, done: 2, sent: 3 };
        if (!e.dataset.mark || rank[m] > rank[e.dataset.mark]) e.dataset.mark = m;
      }
    }
    drawing = false;
    if (list) list.render();
  }
  let queued = 0;
  const later = () => {
    if (drawing || queued) return;
    queued = requestAnimationFrame(() => {
      queued = 0;
      marks();
    });
  };
  // the renderers redraw parts (a sheet's tab, a slide, rows as you scroll): mark them again
  new MutationObserver((rs) => {
    if (rs.every((r) => [...r.addedNodes, ...r.removedNodes].every((x) => (x as HTMLElement).dataset && (x as HTMLElement).dataset.ours))) return;
    later();
  }).observe(main, { childList: true, subtree: true });

  // ---- the server: the notes and their outcomes, then each change
  // the page server's answers: the notes, and the send's agent
  type Answer = { notes?: OfficeNote[]; agent?: string };
  const api = (path: string, body?: unknown): Promise<Answer> =>
    fetch(path, {
      method: body ? "POST" : "GET",
      headers: body ? { "Content-Type": "application/json", "X-Bise-Token": token } : {},
      body: body ? JSON.stringify(body) : undefined,
      credentials: "same-origin",
      cache: "no-store",
    }).then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))));
  function took(server: OfficeNote[] | null | undefined) {
    notes = (server || []).map((n) => ({ ...n }));
    if (list) list.merge(notes);
    marks();
  }
  const load = () => api(`/a/${encodeURIComponent(info.id)}/notes`).then((r) => took(r.notes), () => {});
  load();
  try {
    const es = new EventSource(`/a/${encodeURIComponent(info.id)}/events`);
    es.addEventListener("notes", (e) => {
      try {
        took(JSON.parse(e.data).notes);
      } catch {}
    });
  } catch {}
  main.addEventListener("bise:office-drawn", () => load());

  // ---- the selection: a `note` button by it
  const btn = el("button", "office-note-btn", inApp ? W.toMessage : W.note);
  btn.type = "button";
  btn.dataset.ours = "1";
  let picked: { anchor: Anchor; quote: string } | null = null;
  function selectedBox() {
    const s = stage();
    if (!s) return null;
    if (office!.view && office!.view.box) return office!.view.box();
    const parts = [...s.querySelectorAll("td.sel, .office-box")];
    const r = parts.length ? parts.map((e) => e.getBoundingClientRect()) : [];
    const sel = window.getSelection();
    if (!r.length && sel && sel.rangeCount && !sel.isCollapsed) r.push(sel.getRangeAt(0).getBoundingClientRect());
    if (!r.length) return null;
    return { left: Math.min(...r.map((x) => x.left)), top: Math.min(...r.map((x) => x.top)), right: Math.max(...r.map((x) => x.right)), bottom: Math.max(...r.map((x) => x.bottom)) };
  }
  function quoteNow(a: Anchor | null | undefined) {
    const s = stage();
    if (!s || !a) return "";
    if (office!.view && office!.view.quote) return quoteOf(office!.view.quote());
    if (a.kind === "cell") return quoteOf([...s.querySelectorAll("td.sel")].map((td) => td.textContent).filter(Boolean).join(" "));
    if (a.kind === "region") return quoteOf(a.quote);
    return "";
  }
  // a mouseup, or a renderer's bise:office-selection (its target: the main)
  function offer(e: Event) {
    const t = e.target as Element;
    if (btn.contains(t) || (list && list.el.contains(t))) return;
    // after the renderer's own handler set its selection
    setTimeout(() => {
      const view = office!.view;
      const a = view && view.selection ? view.selection() : null;
      const box = a && selectedBox();
      const r = box || (a && t.getBoundingClientRect && t.getBoundingClientRect());
      if (!a || !r) {
        btn.remove();
        picked = null;
        return;
      }
      const quote = quoteNow(a);
      picked = { anchor: a, quote };
      btn.textContent = inApp ? W.toMessage : W.note;
      Object.assign(btn.style, { left: `${Math.max(8, Math.min(window.innerWidth - 140, r.left))}px`, top: `${Math.min(window.innerHeight - 36, r.bottom + 6)}px` });
      if (!btn.isConnected) document.body.append(btn);
    }, 0);
  }
  main.addEventListener("mouseup", offer);
  // a renderer whose selection changes with no mouseup (video.js: paused, a frame stepped)
  main.addEventListener("bise:office-selection", offer);
  main.addEventListener("keyup", (e) => {
    if (e.key === "Escape") {
      btn.remove();
      picked = null;
    }
  });
  btn.addEventListener("mousedown", (e) => e.preventDefault()); // keep the text selection
  // his gesture only, never on load or a timer (architect m_23971)
  function take(key?: string) {
    if (!picked) return;
    if (inApp) {
      // the desktop checks the source and the origin (art-desktop m_23900)
      window.parent.postMessage(message(office!.info, picked.anchor, picked.quote, key), "*");
      btn.textContent = W.added;
      setTimeout(() => btn.remove(), 1200);
    } else {
      // (the browser's list: there whenever not in the app, notes.js loaded)
      list!.open(picked.anchor, picked.quote);
      btn.remove();
    }
    picked = null;
  }
  btn.addEventListener("click", () => take());
  // in the app, as on a thread's quote: typing adds it (his words go to the composer, which
  // takes the focus), ⌘C copies what is selected
  document.addEventListener("keydown", (e) => {
    if (!inApp || !picked || !btn.isConnected || e.metaKey || e.ctrlKey || e.altKey) return;
    const key = typed(e);
    // his first character goes with it (not typed here); a composition or a dead key: no key
    if (key) {
      e.preventDefault();
      take(key);
    } else if (e.isComposing || e.key === "Dead") take();
  });
  document.addEventListener("copy", (e) => {
    if (!picked || !picked.quote) return;
    const sel = window.getSelection();
    if (sel && !sel.isCollapsed) return;
    e.clipboardData!.setData("text/plain", picked.quote);
    e.preventDefault();
  });

  // ---- the browser's door: notes.js's model and words, a list beside the drawing
  const list = !inApp && N ? browserList() : null;
  function browserList() {
    // (N: checked by the list's caller; A: by mount's start)
    const M = N!.model([]);
    const NW = N!.NW;
    const box = el("aside", "office-notes");
    box.dataset.ours = "1";
    box.setAttribute("aria-label", NW.say.list);
    const head = el("div", "head");
    const items = el("ol", "items");
    const composer = el("form", "composer");
    const whereLine = el("div", "where");
    const input = el("textarea");
    input.rows = 2;
    input.placeholder = NW.placeholder;
    composer.append(whereLine, input);
    composer.hidden = true;
    const bar = el("div", "bar");
    const barText = el("span", "text");
    const send = el("button", "send", NW.send);
    send.type = "button";
    bar.append(barText, send);
    box.append(head, items, composer, bar);
    document.body.append(box);
    document.body.classList.add("with-notes");
    let draft: { anchor: Anchor; quote: string } | null = null;
    const agent = N!.spaced(info.agent || "");

    let saveT = 0;
    const save = () => {
      clearTimeout(saveT);
      saveT = setTimeout(() => api(`/api/a/${encodeURIComponent(info.id)}/notes`, M.payload()).then((r) => took(r.notes), () => (barText.textContent = NW.failed)), 300);
    };
    function open(anchor: Anchor, quote: string) {
      draft = { anchor, quote };
      whereLine.textContent = A!.label(anchor);
      composer.hidden = false;
      input.value = "";
      input.focus();
    }
    function keep() {
      const text = input.value.trim();
      if (draft && text) {
        M.add({ kind: "note", at: draft.anchor, quote: draft.quote || undefined, text, version: version(), ...(office!.info.rev ? { rev: office!.info.rev } : {}) }, Date.now());
        save();
      }
      draft = null;
      composer.hidden = true;
      render();
      marks();
    }
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey && !e.metaKey) {
        e.preventDefault();
        keep();
      } else if (e.key === "Enter" && e.metaKey) {
        e.preventDefault();
        keep();
        doSend();
      } else if (e.key === "Escape") {
        draft = null;
        composer.hidden = true;
      }
    });
    composer.addEventListener("submit", (e) => e.preventDefault());
    async function doSend() {
      if (!M.drafts().length) return;
      clearTimeout(saveT);
      try {
        await api(`/api/a/${encodeURIComponent(info.id)}/notes`, M.payload());
        const r = await api(`/api/a/${encodeURIComponent(info.id)}/send`, {});
        M.sent(Date.now());
        // the agent by its real name, office-lead (designer m_24905)
        barText.textContent = NW.sent(r.agent || info.agent);
        load();
      } catch {
        barText.textContent = NW.failed;
      }
    }
    send.addEventListener("click", doSend);
    items.addEventListener("click", (e) => {
      const rm = (e.target as Element).closest<HTMLElement>("[data-remove]");
      if (rm) {
        M.remove(rm.dataset.remove as string);
        save();
        render();
        marks();
      }
    });
    function tagOf(n: OfficeNote) {
      const v = version();
      if (n.status === "done" && n.done_in != null) return Number(n.done_in) === v ? { tag: "done", text: NW.done } : { tag: "done", text: W.doneIn(n.done_in) };
      if (n.here === false && (n.status === "draft" || n.status === "sent")) return { tag: "moved", text: W.moved };
      if (n.status === "sent") return { tag: "sent", text: NW.sentTag };
      if (n.status === "answered") return { tag: "answer", text: n.answer || "" };
      return { tag: "draft", text: "" };
    }
    function render() {
      const all = M.all();
      const here = Object.fromEntries(notes.map((n) => [n.id, n.here]));
      head.textContent = NW.listTitle(info.name || info.title, all.length);
      items.replaceChildren();
      if (!all.length) items.append(el("li", "empty", (W.empty as Record<string, string>)[info.kind] || W.empty.image));
      for (const n of all) {
        const li = el("li", "item");
        li.dataset.status = n.status;
        li.append(el("div", "where", n.at ? A.label(n.at) : ""));
        li.append(el("div", "words", n.text || ""));
        const t = tagOf({ ...n, here: here[n.id] });
        if (t.text) {
          const tag = el("span", "tag", t.text);
          tag.dataset.tag = t.tag;
          li.append(tag);
        }
        if (n.status === "draft") {
          const rm = el("button", "remove", NW.remove);
          rm.type = "button";
          rm.dataset.remove = n.id;
          li.append(rm);
        }
        items.append(li);
      }
      const d = M.drafts().length;
      bar.hidden = d === 0 && !barText.textContent;
      send.hidden = d === 0;
      if (d) barText.textContent = NW.sendBar(d, agent);
    }
    // the server's notes: its statuses (and the hub's outcome) win, his drafts stay
    const merge = (server: OfficeNote[]) => {
      M.merge(server);
      render();
    };
    render();
    return { el: box, open, render, merge };
  }
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", mount);
  else mount();
}
