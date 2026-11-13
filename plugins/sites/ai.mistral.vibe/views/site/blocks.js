// generated from blocks.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// Sites · where a place is in a page of the site (docs/plugin-apps.md A5, architect m_60421): the
// rules kit/site-select.js ran inside the site, now run by the view on the site's document (the
// same origin, so the view reads it; the site gets no script of bise's). Each takes the page's
// `Document`, so it works on whichever page of the site the frame shows.
//
// - a block: the closest of BLOCKS around a node; its path: its `#id` when unique in the page,
//   else its tag path from the body (or from the nearest unique id above it);
// - a place back to its element: by its path, kept only when it still holds his words, else the
//   smallest block that holds them (a block that moved keeps its note); none: detached.
//
// No state; DOM in, DOM out (the desktop check drives it on the fixture site).
import { BLOCK_MAX, norm } from "./keys.js";
/** The elements a note can sit on. */
export const BLOCKS = "p,li,h1,h2,h3,h4,h5,h6,td,th,pre,blockquote,figcaption,dt,dd,summary,caption,figure,section,article,aside,header,footer,main,nav,div,body";
/** The path of `e` in its page: `#id` when unique, else `body>…:nth-of-type(k)` (or from an id
 * above it); a path past BLOCK_MAX: its parent's, the nearest that fits. */
export function pathOf(e) {
    const doc = e.ownerDocument;
    for (let at = e; at; at = at.parentElement) {
        const p = rawPath(doc, at);
        if (p.length <= BLOCK_MAX)
            return p;
    }
    return "body";
}
function rawPath(doc, e) {
    const parts = [];
    for (let el = e; el && el !== doc.body; el = el.parentElement) {
        if (el.id && doc.querySelectorAll(`#${CSS.escape(el.id)}`).length === 1) {
            parts.unshift(`#${CSS.escape(el.id)}`);
            return parts.join(">");
        }
        const tag = el.tagName.toLowerCase();
        let k = 1;
        for (let s = el.previousElementSibling; s; s = s.previousElementSibling)
            if (s.tagName === el.tagName)
                k++;
        parts.unshift(`${tag}:nth-of-type(${k})`);
    }
    return parts.length ? `body>${parts.join(">")}` : "body";
}
/** The block around `node` (a text node: its parent's), or null. */
export function blockOf(node) {
    const e = node && (node.nodeType === 1 ? node : node.parentElement);
    return e ? e.closest(BLOCKS) : null;
}
/** The element a path names in `doc`, or null (a bad selector too). */
function find(doc, path) {
    try {
        return doc.querySelector(path);
    }
    catch {
        return null;
    }
}
/** The smallest block whose words hold `quote`, or null. */
function byQuote(doc, quote) {
    if (!quote)
        return null;
    let best = null;
    for (const e of doc.querySelectorAll(BLOCKS))
        if (norm(e.textContent).includes(quote) && (!best || best.contains(e)))
            best = e;
    return best;
}
/** Where a place is in `doc`: its path's element when it still holds his words, else the block
 * that holds them; null: not on this page (the note shows detached). */
export function elementOf(doc, p) {
    const quote = norm(p.quote);
    const at = find(doc, p.block);
    if (at && (!quote || norm(at.textContent).includes(quote)))
        return at;
    return byQuote(doc, quote);
}
/** His selection in `doc` as a place and the box it covers (in the page's viewport), or null:
 * nothing selected, or no words. */
export function selected(doc) {
    const sel = doc.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount)
        return null;
    const range = sel.getRangeAt(0);
    const block = blockOf(range.commonAncestorContainer);
    const text = norm(sel.toString());
    if (!block || !text)
        return null;
    return { place: { kind: "block", block: pathOf(block), quote: text }, text, rect: range.getBoundingClientRect() };
}
