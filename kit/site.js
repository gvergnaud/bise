// bise kit · the site (pages-ui): the page server's `/` lists every page and site artifact in a
// sidebar (the server writes the rows, site.rs); this groups them by day or by agent, filters them
// as you type (/ focuses the search), and opens one inside the site, next to the list, in a frame
// (a page with its notes; a site artifact in its sandbox). An outside site opens in a tab. The
// address keeps what is open (#/p/<id>), so back and a reload come back to it.
(function () {
  "use strict";
  const doc = document;
  const side = doc.getElementById("bise-site");
  if (!side) return;
  const list = side.querySelector("ul");
  const rows = Array.from(list.querySelectorAll("li"));
  const search = side.querySelector("input[type=search]");
  let by = "day";

  const DAY = 86400000;
  function dayOf(at) {
    const d = new Date(at), now = new Date();
    const start = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const ago = Math.round((start(now) - start(d)) / DAY);
    if (ago <= 0) return "today";
    if (ago === 1) return "yesterday";
    if (ago < 7) return d.toLocaleDateString(undefined, { weekday: "long" }).toLowerCase();
    return d.toLocaleDateString(undefined, { day: "numeric", month: "short" }).toLowerCase();
  }

  function draw() {
    const q = (search.value || "").trim().toLowerCase();
    for (const h of Array.from(list.querySelectorAll("li.grp"))) h.remove();
    const shown = rows.filter((r) => {
      const hit = !q || r.textContent.toLowerCase().includes(q) || (r.dataset.agent || "").includes(q);
      r.hidden = !hit;
      return hit;
    });
    const key = (r) => (by === "day" ? dayOf(parseInt(r.dataset.at, 10) || 0) : r.dataset.agent || "you");
    const order = by === "day" ? shown : shown.slice().sort((a, b) => key(a).localeCompare(key(b)));
    let last = null;
    for (const r of order) {
      const k = key(r);
      if (k !== last) {
        const h = doc.createElement("li");
        h.className = "grp";
        h.textContent = k;
        list.append(h);
        last = k;
      }
      list.append(r);
    }
    if (!shown.length) {
      const h = doc.createElement("li");
      h.className = "grp";
      h.textContent = q ? "nothing matches" : "nothing yet";
      list.append(h);
    }
  }

  // the viewer: a frame next to the list
  let view = null;
  function open(href, push) {
    if (!view) {
      view = doc.createElement("iframe");
      view.id = "bise-view";
      view.title = "page";
      doc.body.append(view);
    }
    // a site artifact runs in its own sandbox (its CSP says so too)
    if (href.startsWith("/a/")) view.setAttribute("sandbox", "allow-scripts allow-popups");
    else view.removeAttribute("sandbox");
    view.src = href;
    doc.body.setAttribute("data-viewing", "");
    for (const r of rows) r.toggleAttribute("aria-current", r.querySelector("a").getAttribute("href") === href);
    if (push) history.pushState({ href }, "", "#" + href);
  }
  function close(push) {
    doc.body.removeAttribute("data-viewing");
    if (view) view.src = "about:blank";
    for (const r of rows) r.removeAttribute("aria-current");
    if (push) history.pushState({}, "", location.pathname);
  }
  // what the address opens: a row's own link (/p/<id> on the hub, <id>/index.html in the export)
  const linked = (h) => rows.some((r) => !r.hasAttribute("data-outside") && r.querySelector("a").getAttribute("href") === h);

  side.addEventListener("click", (e) => {
    const a = e.target.closest("a");
    if (a && !e.metaKey && !e.ctrlKey && !e.shiftKey) {
      if (a.classList.contains("home")) { e.preventDefault(); close(true); return; }
      const li = a.closest("li");
      if (li && !li.hasAttribute("data-outside")) { e.preventDefault(); open(a.getAttribute("href"), true); }
      return;
    }
    const b = e.target.closest("button[data-by]");
    if (b) {
      by = b.dataset.by;
      for (const x of side.querySelectorAll("button[data-by]")) x.setAttribute("aria-pressed", String(x === b));
      draw();
    }
  });
  // a page of "for you" opens inside the site too
  const home = doc.getElementById("bise-home");
  if (home) home.addEventListener("click", (e) => {
    const a = e.target.closest("[data-kit=pages] a");
    if (a && !e.metaKey && !e.ctrlKey) { e.preventDefault(); open(a.getAttribute("href"), true); }
  });
  search.addEventListener("input", draw);
  search.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { search.value = ""; draw(); search.blur(); }
    if (e.key === "Enter") { const r = rows.find((x) => !x.hidden); if (r) r.querySelector("a").click(); }
  });
  doc.addEventListener("keydown", (e) => {
    if (e.key === "/" && doc.activeElement !== search && !e.metaKey && !e.ctrlKey) { e.preventDefault(); search.focus(); }
  });
  addEventListener("popstate", () => {
    const h = decodeURIComponent(location.hash.slice(1));
    if (h.startsWith("/p/") || linked(h)) open(h, false); else close(false);
  });

  draw();
  const h = decodeURIComponent(location.hash.slice(1));
  if (h.startsWith("/p/") || linked(h)) open(h, false);
})();
