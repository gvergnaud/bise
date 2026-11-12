// generated from notes.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · @bise/view's marks (docs/plugin-apps.md A5): his notes drawn on the view's own parts.
// The view says where an anchor is (`place(anchor)`: its elements, none when this version has no
// such part); the kit marks them (`data-bise-mark` = sent | draft | done, view.css draws 3 px of
// accent, ✓ on the version that answered) and tells the host which notes it could not place
// (dev.bise/detached: the host lists them, never lost). What a version marks is core.layout's.
// Drawn again on each new notes list and on `redraw()` (the view drew its parts again).
// What changed since he last looked (the host context's dev.bise/changed, architect m_60734): the
// same places, `data-bise-changed` = flash (a 2 s outline the first time a part is placed), then
// dot while it hasn't been in sight.
import * as core from "./core.js";
/** The attributes the kit owns on the view's parts: his notes' marks, and what changed. */
export const MARK = "data-bise-mark";
export const CHANGED = "data-bise-changed";
const RANK = { draft: 1, done: 2, sent: 3 };
export function mountNotes(o) {
    let marked = [];
    let lastDetached = "";
    const els = (anchor) => {
        const at = o.place(anchor);
        return !at ? [] : Array.isArray(at) ? [...at] : [at];
    };
    // ---- what changed: each changed part outlined for 2 s the first time the view places it, then
    // a dot while it hasn't been in sight (an IntersectionObserver says when it is), then nothing
    const shown = new Set();
    const timers = new Set();
    const seen = new WeakSet();
    const IO = globalThis.IntersectionObserver;
    const sight = IO
        ? new IO((entries) => {
            for (const en of entries) {
                if (!en.isIntersecting)
                    continue;
                seen.add(en.target);
                if (en.target.getAttribute(CHANGED) === "dot") {
                    en.target.removeAttribute(CHANGED);
                    sight.unobserve(en.target);
                }
            }
        })
        : null;
    function changedParts() {
        const c = o.changed?.();
        if (!c)
            return;
        for (const a of c.anchors) {
            if (shown.has(a))
                continue;
            const at = els(a);
            if (!at.length)
                continue;
            shown.add(a);
            for (const e of at) {
                e.setAttribute(CHANGED, "flash");
                sight?.observe(e);
            }
            const t = setTimeout(() => {
                timers.delete(t);
                for (const e of at) {
                    // no way to tell what he saw (no observer): the outline was all
                    if (!sight || seen.has(e)) {
                        e.removeAttribute(CHANGED);
                        sight?.unobserve(e);
                    }
                    else
                        e.setAttribute(CHANGED, "dot");
                }
            }, core.CHANGED_MS);
            timers.add(t);
        }
    }
    function draw() {
        changedParts();
        for (const e of marked)
            e.removeAttribute(MARK);
        marked = [];
        const found = new Map();
        const where = (a) => {
            if (!found.has(a))
                found.set(a, els(a));
            return found.get(a);
        };
        const { marked: on, detached } = core.layout(o.notes.all(), o.version(), (a) => where(a).length > 0);
        for (const { note, mark } of on) {
            for (const e of where(note.anchor)) {
                // the strongest mark wins on a part with many notes: sent over done over draft
                const had = e.getAttribute(MARK);
                if (!had || RANK[mark] > RANK[had])
                    e.setAttribute(MARK, mark);
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
            for (const e of marked)
                e.removeAttribute(MARK);
            marked = [];
            for (const t of timers)
                clearTimeout(t);
            timers.clear();
            sight?.disconnect();
        },
    };
}
