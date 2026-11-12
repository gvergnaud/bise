// generated from dom.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// Preview · small DOM helpers the renderers share.
/** Hold the pointer on `el` while he drags: a pointer already gone (lifted before the press was
 * handled, or one a test made) has nothing to hold, and the drag still works without it. */
export function capture(el, pointerId) {
    try {
        el.setPointerCapture(pointerId);
    }
    catch {
        // no such pointer any more: nothing to hold
    }
}
