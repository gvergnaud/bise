// bise kit · live edits in the office viewer (office-artifacts plan §6, designer m_23868):
// the frame that follows the newest version (/a/<id>/view, data-follows) listens on
// /a/<id>/events (rust/switchboard/src/pages/artifact/events.rs) and, at each save:
//
// - fetches the new bytes (`?rev=<r>`: a save in the same turn keeps the copy's path) and
//   draws them through viewer.js's show(), which keeps his place;
// - draws the hub's words (`live`: the strip `∿ deck is updating · slide 2`, then the line
//   `v3 · 2 slides changed` and its `← v2` link to that version) in a 28 px strip, unless the
//   desktop's side panel frames it (window.name 'bise-app': the panel draws the row's).
//
// A view may hold a version (`view.hold(ev)`); `office.take(ev)` draws one past the hold.
//
// It decides no words: they come from the hub (bise_proto::office::live_words).
// Pure helper exported for the tests: fileUrl.

/** The copy of version `n` at save `rev`: the same path, a new query so no cache answers. */
export function fileUrl(id: string, n: number, name: string, rev: number | string) {
  return `/a/${encodeURIComponent(id)}/${n}/${encodeURIComponent(name)}?rev=${rev}`;
}

/** The 28 px strip with the hub's words: the strip, or the line and its `← v2` link (it opens
 *  that version); none in the desktop's panel (it draws them from the row). */
function words(main: HTMLElement, info: OfficeInfo, live: OfficeLive | null | undefined) {
  if (window.name === "bise-app") return;
  let s = main.querySelector<HTMLElement>(":scope > .office-live");
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
  if (!office || !main || !office.info.follows) return;
  const info = office.info;
  info.rev ??= 1;
  let pending: Promise<void> | null = null;
  let latest: OfficeVersion | null = null;

  async function take(ev: OfficeVersion, force = false) {
    if (ev.n === info.version && ev.rev === info.rev) return;
    // a view with edits of his own decides (sheet.js: its own saves, a version during his edits)
    // (office and main: checked by start() above; a function declaration keeps no narrowing)
    if (!force && office!.view && office!.view.hold && office!.view.hold(ev)) return;
    const url = fileUrl(info.id, ev.n, info.name, ev.rev);
    const r = await fetch(url, { cache: "no-store" });
    if (!r.ok) return;
    const bytes = new Uint8Array(await r.arrayBuffer());
    info.version = ev.n;
    info.latest = ev.n;
    info.rev = ev.rev;
    main!.dataset.version = String(ev.n);
    main!.dataset.rev = String(ev.rev);
    await office!.show(bytes);
  }

  // one swap at a time; saves that come during one: only the newest is drawn next
  async function pump() {
    if (pending) return;
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
}

if (typeof window !== "undefined" && typeof document !== "undefined") {
  if (window.biseOffice) start();
  else document.addEventListener("DOMContentLoaded", start);
}
