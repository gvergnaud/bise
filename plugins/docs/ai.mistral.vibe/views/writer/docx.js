// generated from docx.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// Writer · docx (docs/plugin-apps.md, migration step 3; office-artifacts plan §4): docx-preview
// (office-view.js beside this file, built from plugins/docs/src/docx.ts) draws the pages; then
// each paragraph it drew in the body is tagged with its id from the docs module's outline (the
// office SDK at ~kit/office/v1/office.js: docs.open(bytes).inspect(), office-docs m_23801: the
// body's paragraphs in document order, a table's in cell order), so a note anchors on its
// paragraph: the canonical JSON of `{kind: "para", para}` (sorted keys, no quote: architect
// m_60380, so the notes made on the old viewer place too), found again by its note's quote.
// Headers, footers, footnotes and text boxes are not in the outline: their paragraphs stay
// untagged. Without the outline the pages still draw. Not drawn as Word does, said in the foot:
// fields show their saved text.
//
// Pure helpers (plugins/docs/test/docx.test.mjs): parasOf, matchParas, paraKey, paraOfKey.
import { renderDocx } from "./office-view.js";
import { docs } from "./~kit/office/v1/office.js";
const norm = (s) => String(s || "").replace(/\s+/g, " ").trim();
/** The outline's paragraphs in document order (tables' in cell order; comments and changes out). */
export function parasOf(nodes, out = []) {
    for (const n of nodes || []) {
        if (n.type === "para")
            out.push(n);
        else if (n.type === "table")
            parasOf(n.children, out);
    }
    return out;
}
/**
 * Pair drawn paragraphs (their texts, in order) with the outline's (in order): the n-th drawn
 * that has the outline's text, looking a few ahead (a drawn paragraph the outline skips, a
 * text box's, is passed over). Returns, for each drawn index, the outline index or -1.
 */
export function matchParas(drawn, outline, ahead = 6) {
    const out = drawn.map(() => -1);
    let d = 0;
    for (let o = 0; o < outline.length && d < drawn.length; o++) {
        const want = norm(outline[o]);
        for (let k = d; k < Math.min(drawn.length, d + ahead); k++) {
            if (norm(drawn[k]) === want) {
                out[k] = o;
                d = k + 1;
                break;
            }
        }
    }
    return out;
}
/** A paragraph's anchor: `{"kind":"para","para":"<w14:paraId>"}`, the old viewer's minus its quote. */
export function paraKey(para) {
    return JSON.stringify({ kind: "para", para });
}
/** The paragraph an anchor names; null when it is not a paragraph anchor. */
export function paraOfKey(key) {
    try {
        const a = JSON.parse(key);
        return a && a.kind === "para" && typeof a.para === "string" ? a.para : null;
    }
    catch {
        return null;
    }
}
export async function draw(root, file, host) {
    // a docx is a blob: a text read is not one
    if (typeof file === "string")
        throw new Error("it came as text, not as a Word file");
    const bytes = file;
    root.classList.add("w-docx");
    const pages = document.createElement("div");
    pages.className = "w-pages";
    const styles = document.createElement("div");
    root.append(styles, pages);
    await renderDocx(bytes, pages, styles, {
        className: "docx",
        inWrapper: true,
        ignoreLastRenderedPageBreak: true,
        renderHeaders: true,
        renderFooters: true,
        renderFootnotes: true,
        renderEndnotes: true,
        renderComments: false,
        useBase64URL: true,
        experimental: false,
    });
    const notes = [];
    try {
        const outline = parasOf((await docs.open(bytes)).inspect().nodes);
        // the body only: docx-preview's <article> of each page (headers and footers are beside it)
        const drawn = [...pages.querySelectorAll("section.docx > article p")].filter((p) => !p.parentElement.closest("p"));
        matchParas(drawn.map((p) => p.textContent), outline.map((n) => n.text)).forEach((o, i) => {
            if (o >= 0)
                drawn[i].dataset.para = outline[o].id;
        });
    }
    catch (err) {
        // a file the docs module refuses: the pages stay, untagged
        notes.push(`paragraphs could not be named for notes: ${err?.message ?? err}`);
    }
    if (pages.querySelector(".docx [class*='field'], .docx-field"))
        notes.push("fields show their saved text.");
    root.dataset.pages = String(pages.querySelectorAll("section.docx").length);
    document.addEventListener("selectionchange", () => {
        const s = window.getSelection();
        const node = s && s.anchorNode && (s.anchorNode.nodeType === 1 ? s.anchorNode : s.anchorNode.parentElement);
        const p = node && pages.contains(node) ? node.closest("[data-para]") : null;
        const quote = norm(s && s.toString());
        if (!p || !quote || !s || s.rangeCount === 0)
            return host.select(null, null, null);
        host.select(paraKey(p.dataset.para), quote, s.getRangeAt(0).getBoundingClientRect());
    });
    const changed = new Set();
    const scroller = () => root.parentElement;
    root.addEventListener("scroll", () => changed.forEach((fn) => fn()), { passive: true, capture: true });
    return {
        notes,
        place: () => ({ top: scroller()?.scrollTop ?? 0 }),
        restore(p) {
            const at = p;
            const el = scroller();
            if (at && el)
                el.scrollTop = Number(at.top) || 0;
        },
        onChange: (fn) => changed.add(fn),
        // a note's paragraph: by its id while its words are still there, else the first that has them
        find(anchor) {
            const para = paraOfKey(anchor);
            if (!para)
                return null;
            const q = norm(host.quoteOf(anchor));
            const has = (e) => !q || norm(e.textContent).includes(q);
            const byId = pages.querySelector(`[data-para="${CSS.escape(para)}"]`);
            if (byId && has(byId))
                return byId;
            return q ? [...pages.querySelectorAll("[data-para]")].find(has) ?? null : null;
        },
    };
}
