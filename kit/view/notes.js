// generated from notes.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · @bise/view's marks (docs/plugin-apps.md A5): his notes drawn on the view's own parts.
// The view says where an anchor is (`place(anchor)`: its elements, none when this version has no
// such part); the kit marks them (`data-bise-mark` = sent | draft | done, view.css draws 3 px of
// accent, ✓ on the version that answered) and tells the host which notes it could not place
// (dev.bise/detached: the host lists them, never lost). What a version marks is core.layout's.
// Drawn again on each new notes list and on `redraw()` (the view drew its parts again).
import * as core from "./core.js";
/** The attribute the kit owns on the view's parts. */
export const MARK = "data-bise-mark";
const RANK = { draft: 1, done: 2, sent: 3 };
export function mountNotes(o) {
    let marked = [];
    let lastDetached = "";
    const els = (anchor) => {
        const at = o.place(anchor);
        return !at ? [] : Array.isArray(at) ? [...at] : [at];
    };
    function draw() {
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
        },
    };
}
