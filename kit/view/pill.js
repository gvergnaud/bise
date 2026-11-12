// generated from pill.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · @bise/view's pill by a selection (docs/plugin-apps.md A5; kit/office/select.js's in
// the app): `type to ask about it` in ink under what he selected, when the view says where it is.
// A click on it, or his first typed character, asks about the selection (view.ts sends it with
// `take` or `key`); then it says `added to your message` and goes. Escape, or a new selection
// with no place, takes it away. Placed through CSSOM (the view's CSP has no inline style).
import { pillAt } from "./core.js";
// the words: bise_proto::words::viewkit (designer signs them there)
import { viewkit } from "./words.gen.js";
export function pill(doc, onTake) {
    const btn = doc.createElement("button");
    btn.type = "button";
    btn.className = "bv-pill";
    btn.textContent = viewkit.ask;
    // a mousedown on it keeps the view's own selection
    btn.addEventListener("mousedown", (e) => e.preventDefault());
    btn.addEventListener("click", () => onTake());
    doc.addEventListener("keyup", (e) => e.key === "Escape" && btn.remove());
    let gone = null;
    const win = doc.defaultView;
    return {
        place(at) {
            if (gone)
                clearTimeout(gone);
            gone = null;
            if (!at)
                return btn.remove();
            const r = "getBoundingClientRect" in at ? at.getBoundingClientRect() : at;
            const p = pillAt(r, win.innerWidth, win.innerHeight);
            btn.textContent = viewkit.ask;
            btn.style.left = `${p.left}px`;
            btn.style.top = `${p.top}px`;
            if (!btn.isConnected)
                doc.body.append(btn);
        },
        asked() {
            if (!btn.isConnected)
                return;
            btn.textContent = viewkit.added;
            gone = setTimeout(() => btn.remove(), 1200);
        },
    };
}
