// generated from viewer.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · the office viewer (office-artifacts plan §4): the shell the page server sends at
// /a/<id>/view and /a/<id>/<v>/view (rust/switchboard/src/pages/artifact/view.rs) carries the
// file's facts on main#bise-office; this fetches the copy's bytes and draws them with the
// renderer of its kind (site.js for a site, art-notes; a picture, a pdf or a video opens
// in Preview, plugins/media; a .md file or a docx in Writer, plugins/docs; a pptx in Slides,
// plugins/slides; an xlsx or a csv in Sheets, plugins/sheets), on the office SDK's bundle
// (/kit/office/office.js, office/dist in the app root). No inline script: the viewer's CSP
// refuses one (architect m_23781).
//
// What a renderer exports: `draw(root, bytes, info)` → a view { place(), restore(place),
// selection(), notes: string[], dispose?() }: where the user is (a sheet and its scroll, a slide, the
// doc's scroll), put back after a new version is drawn; what is selected as an Anchor
// (bise_proto::anchor); and what it could not draw, said in the frame's foot.
//
// window.biseOffice: { info, view, show(bytes) } for the swap in place (art-live's swap.js)
// and the notes layer (art-notes): show() draws new bytes and keeps the user's place.
const RENDERERS = { site: "./site.js" };
// (the shell always has it and writes each of these: pages/artifact/view.rs)
const main = document.getElementById("bise-office");
const info = {
    id: main.dataset.artifact,
    kind: main.dataset.kind,
    version: Number(main.dataset.version),
    latest: Number(main.dataset.latest),
    follows: main.hasAttribute("data-follows"),
    file: main.dataset.file,
    name: main.dataset.name,
    agent: main.dataset.agent,
    title: main.dataset.title,
};
/** The frame's one line of state: opening, failed, or what it could not draw. */
function say(state, text) {
    let p = main.querySelector(":scope > .office-state");
    if (!p) {
        p = document.createElement("p");
        p.className = "office-state";
        main.prepend(p);
    }
    p.dataset.state = state;
    p.textContent = text;
    return p;
}
/** The words for an error from the SDK or the fetch: what happened, never a stack. */
export function words(err, kind, name) {
    const code = err && err.code;
    if (code === "not_built")
        return `this bise can't draw ${kind} files yet: open ${name} in its app, or update bise.`;
    if (code === "not_a_zip" || code === "not_office")
        return `${name} is not a readable ${kind} file (${err.message}).`;
    if (code === "fetch")
        return `the copy of ${name} could not be read: ${err.message}.`;
    return `${name} could not be drawn: ${(err && err.message) || err}.`;
}
/** The foot: what this viewer does not draw of this file (plan §4), one line each. */
export function foot(root, notes) {
    root.querySelector(":scope > .office-foot")?.remove();
    if (!notes || notes.length === 0)
        return;
    const f = document.createElement("footer");
    f.className = "office-foot";
    for (const n of notes) {
        const p = document.createElement("p");
        p.textContent = n;
        f.append(p);
    }
    root.append(f);
}
async function fetchBytes(url) {
    const r = await fetch(url, { cache: "no-store" });
    if (!r.ok)
        throw Object.assign(new Error(`${r.status} ${(await r.text()).trim()}`), { code: "fetch" });
    return new Uint8Array(await r.arrayBuffer());
}
let renderer = null;
let view = null;
/** Draw `bytes` (a new version's, or the first), keeping the user's place. */
async function show(bytes) {
    const place = view ? view.place() : null;
    try {
        renderer ??= (await import(RENDERERS[info.kind]));
        const stage = document.createElement("div");
        stage.className = "office-stage";
        const next = await renderer.draw(stage, bytes, info);
        main.querySelector(":scope > .office-stage")?.remove();
        main.querySelector(":scope > .office-state")?.remove();
        main.prepend(stage);
        view?.dispose?.();
        view = next;
        if (place)
            view.restore(place);
        foot(main, view.notes);
        window.biseOffice.view = view;
        main.dispatchEvent(new CustomEvent("bise:office-drawn", { bubbles: true, detail: { version: info.version } }));
    }
    catch (e) {
        say("failed", words(e, info.kind, info.name));
        console.error(e);
    }
}
window.biseOffice = { info, view: null, show };
if (main && RENDERERS[info.kind]) {
    fetchBytes(info.file).then(show, (e) => say("failed", words(e, info.kind, info.name)));
}
else if (main) {
    say("failed", `bise has no viewer for ${info.kind || "this"} files.`);
}
