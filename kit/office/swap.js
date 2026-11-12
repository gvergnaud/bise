// generated from swap.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · live edits in the office viewer (office-artifacts plan §6, designer m_23868):
// the frame that follows the newest version (/a/<id>/view, data-follows) listens on
// /a/<id>/events (rust/switchboard/src/pages/artifact/events.rs) and, at each save:
//
// - fetches the new bytes (`?rev=<r>`: a save in the same turn keeps the copy's path) and
//   draws them through viewer.js's show(), which keeps his place (the sheet with its scroll
//   and cell);
// - outlines what this save changed (`rev_changed`) in --term-ok for 2 s;
// - puts a --term-ok dot on the sheet tabs the version changed, until he has looked at that
//   sheet;
// - draws the hub's words (`live`: the strip `∿ deck is updating · slide 2`, then the line
//   `v3 · 2 slides changed` and its `← v2` link to that version) in a 28 px strip, unless the
//   desktop's side panel frames it (window.name 'bise-app': the panel draws the row's).
//
// A view may hold a version (`view.hold(ev)`: sheet.js, its own saves and a version that came
// during his edits); `office.take(ev)` draws one past the hold.
//
// It decides no words: they come from the hub (bise_proto::office::live_words).
// Pure helpers exported for the tests: sheetsOf, cellIn, fileUrl.
const MARK_MS = 2000;
/** The sheets a list of anchors names, once each. */
export function sheetsOf(list) {
    const out = [];
    for (const a of list || [])
        if (a.kind === "cell" && !out.includes(a.sheet))
            out.push(a.sheet);
    return out;
}
const colNum = (s) => [...s].reduce((n, c) => n * 26 + (c.charCodeAt(0) - 64), 0);
/** Whether cell `ref` (`B4`) is in `range` (`B4`, `A1:C9`). */
export function cellIn(ref, range) {
    const p = (r) => {
        const m = /^([A-Z]+)(\d+)$/.exec(String(r).replace(/\$/g, "").toUpperCase());
        return m ? [colNum(m[1]), Number(m[2])] : null;
    };
    const [a, b = a] = String(range).split(":");
    const c = p(ref), x = p(a), y = p(b);
    if (!c || !x || !y)
        return false;
    return c[0] >= Math.min(x[0], y[0]) && c[0] <= Math.max(x[0], y[0]) && c[1] >= Math.min(x[1], y[1]) && c[1] <= Math.max(x[1], y[1]);
}
/** The copy of version `n` at save `rev`: the same path, a new query so no cache answers. */
export function fileUrl(id, n, name, rev) {
    return `/a/${encodeURIComponent(id)}/${n}/${encodeURIComponent(name)}?rev=${rev}`;
}
/** The parts of the drawn view an anchor points at (cells). */
function partsOf(main, a) {
    if (a.kind === "cell") {
        const tab = main.querySelector('.office-tabs button[aria-selected="true"]');
        if (!tab || tab.textContent !== a.sheet)
            return [];
        return [...main.querySelectorAll(".office-grid td[data-ref]")].filter((td) => cellIn(td.dataset.ref, a.range));
    }
    return [];
}
/** Outline what this save changed for 2 s. */
function mark(main, list) {
    const parts = (list || []).flatMap((a) => partsOf(main, a));
    for (const p of parts)
        p.classList.add("office-changed");
    setTimeout(() => {
        for (const p of parts)
            p.classList.remove("office-changed");
    }, MARK_MS);
}
/** Dots on the sheet tabs the version changed; gone once he looked there. */
const unseen = { sheets: new Set() };
function dots(main) {
    for (const b of main.querySelectorAll(".office-tabs button")) {
        if (b.getAttribute("aria-selected") === "true")
            unseen.sheets.delete(b.textContent);
        b.classList.toggle("office-unseen", unseen.sheets.has(b.textContent));
    }
}
/** The 28 px strip with the hub's words: the strip, or the line and its `← v2` link (it opens
 *  that version); none in the desktop's panel (it draws them from the row). */
function words(main, info, live) {
    if (window.name === "bise-app")
        return;
    let s = main.querySelector(":scope > .office-live");
    const text = live && (live.strip || live.line);
    if (!text) {
        s?.remove();
        return;
    }
    if (!s) {
        s = document.createElement("p");
        s.className = "office-live";
        s.setAttribute("role", "status");
        main.prepend(s);
    }
    s.dataset.state = live.strip ? "updating" : "done";
    s.replaceChildren(document.createTextNode(text));
    if (!live.strip && live.back) {
        const a = document.createElement("a");
        a.className = "back";
        a.href = `/a/${encodeURIComponent(info.id)}/${live.back.v}/view`;
        a.textContent = live.back.text;
        s.append(a);
    }
}
function start() {
    const office = window.biseOffice;
    const main = document.getElementById("bise-office");
    if (!office || !main || !office.info.follows)
        return;
    const info = office.info;
    info.rev ??= 1;
    let pending = null;
    let latest = null;
    async function take(ev, force = false) {
        if (ev.n === info.version && ev.rev === info.rev)
            return;
        // a view with edits of his own decides (sheet.js: its own saves, a version during his edits)
        // (office and main: checked by start() above; a function declaration keeps no narrowing)
        if (!force && office.view && office.view.hold && office.view.hold(ev))
            return;
        const url = fileUrl(info.id, ev.n, info.name, ev.rev);
        const r = await fetch(url, { cache: "no-store" });
        if (!r.ok)
            return;
        const bytes = new Uint8Array(await r.arrayBuffer());
        info.version = ev.n;
        info.latest = ev.n;
        info.rev = ev.rev;
        main.dataset.version = String(ev.n);
        main.dataset.rev = String(ev.rev);
        await office.show(bytes);
        for (const n of sheetsOf(ev.changed))
            unseen.sheets.add(n);
        dots(main);
        mark(main, ev.rev_changed);
    }
    // one swap at a time; saves that come during one: only the newest is drawn next
    async function pump() {
        if (pending)
            return;
        while (latest) {
            const ev = latest;
            latest = null;
            pending = take(ev).catch((e) => console.error(e));
            await pending;
            pending = null;
        }
    }
    // the view's `take v<n>` (sheet.js): that version drawn, its hold passed
    office.take = (ev) => take(ev, true).catch((e) => console.error(e));
    const es = new EventSource(`/a/${encodeURIComponent(info.id)}/events`);
    es.addEventListener("version", (e) => {
        const ev = JSON.parse(e.data);
        words(main, info, ev.live);
        latest = ev;
        pump();
    });
    es.addEventListener("live", (e) => words(main, info, JSON.parse(e.data)));
    // a click on a tab, a key: what he now sees is seen
    main.addEventListener("click", () => setTimeout(() => dots(main), 0));
    main.addEventListener("keydown", () => setTimeout(() => dots(main), 0));
}
if (typeof window !== "undefined" && typeof document !== "undefined") {
    if (window.biseOffice)
        start();
    else
        document.addEventListener("DOMContentLoaded", start);
}
