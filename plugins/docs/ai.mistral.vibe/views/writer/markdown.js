// generated from markdown.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// Writer · markdown (docs/plugin-apps.md, migration step 1; md-artifact, architect m_38679): a .md
// file drawn as a document (headings, tables, quotes, lists, code, links) in bise's look. The
// parser is micromark with GFM (office-md.js beside this file, built from plugins/docs/src/), what
// the desktop thread's react-markdown + remark-gfm run on: a file reads as an answer does. Raw
// HTML in the file shows as text.
//
// A 'source' toggle in the corner shows the file as written (mono, wrapped) and back. Each block
// (a heading, a paragraph, a table, a quote, a list item, a code block) gets `data-block="b<n>"`
// in document order; a note's anchor is the canonical JSON of `{kind: "block", block}` (sorted
// keys, no quote: architect m_60380, so the notes made on the old viewer place too), found again
// by its note's quote when the blocks moved.
//
// Links: http(s) and mailto leave through the host (View.openLink when the kit has it); a path
// beside the file can't be followed (a view never gets a path), so it stays words; a picture
// beside the file shows its alt text.
//
// Pure helpers (plugins/docs/test/markdown.test.mjs): textOf, linkOf, blockKey, blockOfKey.
import { markdownHtml } from "./office-md.js";
const W = { source: "source", sourceTip: "show the markdown as written", docTip: "show it as a document" };
const BLOCKS = "h1, h2, h3, h4, h5, h6, p, blockquote, pre, table, li, hr";
const norm = (s) => String(s || "").replace(/\s+/g, " ").trim();
/** The file's text: utf-8, a BOM dropped, CRLF as LF. */
export function textOf(src) {
    const s = typeof src === "string" ? src : new TextDecoder("utf-8").decode(src);
    return s.replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n");
}
/** What a link of the file is: `out` (the web or a mail: the host opens it), `here` (a place in
 * the file), or `none` (a path beside the file, or anything else: words only). */
export function linkOf(href) {
    if (!href)
        return "none";
    if (href.startsWith("#"))
        return "here";
    if (/^(https?:|mailto:)/i.test(href))
        return "out";
    return "none";
}
/** A block's anchor: `{"block":"b3","kind":"block"}`, the old viewer's anchor minus its quote. */
export function blockKey(block) {
    return JSON.stringify({ block, kind: "block" });
}
/** The block an anchor names; null when it is not a block anchor. */
export function blockOfKey(key) {
    try {
        const a = JSON.parse(key);
        return a && a.kind === "block" && typeof a.block === "string" ? a.block : null;
    }
    catch {
        return null;
    }
}
export function draw(root, file, host) {
    root.classList.add("w-md");
    const text = textOf(file);
    const doc = document.createElement("article");
    doc.className = "md-doc";
    // micromark escapes raw HTML and drops a url of an unsafe protocol (office-md.js)
    doc.innerHTML = markdownHtml(text);
    for (const a of doc.querySelectorAll("a[href]")) {
        const href = a.getAttribute("href");
        const kind = linkOf(href);
        if (kind === "here")
            continue;
        a.removeAttribute("href");
        if (kind === "out") {
            a.setAttribute("role", "link");
            a.tabIndex = 0;
            a.title = href;
            const go = () => host.openLink(href);
            a.addEventListener("click", go);
            a.addEventListener("keydown", (e) => e.key === "Enter" && go());
        }
        else if (href)
            a.title = href;
    }
    // a picture beside the file can't be read by a view: its words in its place
    for (const img of doc.querySelectorAll("img")) {
        const words = document.createElement("span");
        words.className = "md-img";
        words.textContent = img.alt ? `[${img.alt}]` : "[picture]";
        img.replaceWith(words);
    }
    // a block inside a list item or a quote is its container's: only the outermost of each kind
    // but list items (a nested item is its own block)
    let n = 0;
    for (const b of doc.querySelectorAll(BLOCKS)) {
        if (b.tagName !== "LI" && b.parentElement !== doc && b.parentElement.closest("li, blockquote, table"))
            continue;
        b.dataset.block = `b${++n}`;
    }
    const src = document.createElement("pre");
    src.className = "md-source";
    src.textContent = text;
    src.hidden = true;
    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "md-toggle";
    toggle.textContent = W.source;
    toggle.title = W.sourceTip;
    toggle.setAttribute("aria-pressed", "false");
    const page = document.createElement("div");
    page.className = "md-page";
    page.append(doc, src);
    root.append(toggle, page);
    const changed = new Set();
    const tell = () => changed.forEach((fn) => fn());
    const showSource = (on) => {
        src.hidden = !on;
        doc.hidden = on;
        toggle.setAttribute("aria-pressed", String(on));
        // designer m_38797: pressed, the tip says where it goes back to
        toggle.title = on ? W.docTip : W.sourceTip;
    };
    toggle.addEventListener("click", () => {
        showSource(src.hidden);
        tell();
    });
    page.addEventListener("scroll", tell, { passive: true });
    // his selection: the block it starts in and its words, the pill by it
    document.addEventListener("selectionchange", () => {
        const s = window.getSelection();
        const node = s && s.anchorNode && (s.anchorNode.nodeType === 1 ? s.anchorNode : s.anchorNode.parentElement);
        const b = node && doc.contains(node) ? node.closest("[data-block]") : null;
        const quote = norm(s && s.toString());
        if (!b || !quote || !s || s.rangeCount === 0)
            return host.select(null, null, null);
        host.select(blockKey(b.dataset.block), quote, s.getRangeAt(0).getBoundingClientRect());
    });
    return {
        notes: [],
        place: () => ({ top: page.scrollTop, source: !src.hidden }),
        restore(p) {
            const at = p;
            if (!at)
                return;
            showSource(!!at.source);
            page.scrollTop = Number(at.top) || 0;
        },
        onChange: (fn) => changed.add(fn),
        // a note's block: by its id while its words are still there, else the first block that has them
        find(anchor) {
            const block = blockOfKey(anchor);
            if (!block)
                return null;
            const q = norm(host.quoteOf(anchor));
            const has = (e) => !q || norm(e.textContent).includes(q);
            const byId = doc.querySelector(`[data-block="${CSS.escape(block)}"]`);
            if (byId && has(byId))
                return byId;
            return q ? [...doc.querySelectorAll("[data-block]")].find(has) ?? null : null;
        },
    };
}
