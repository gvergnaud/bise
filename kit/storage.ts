// bise kit · Web Storage for a site artifact in its sandbox (art-viewer; architect m_25527). The
// page server puts this one script first in each html page it serves under /a/<id>/<v>/ (plain
// and framed, site.rs `tagged`); the kept copy never changes.
//
// The site runs in an opaque origin (ARTIFACT_CSP: `sandbox allow-scripts`, no same-origin), where
// reading window.localStorage or sessionStorage throws a SecurityError. A page that reads one at
// its top level (`localStorage.getItem(...)` to restore a pick) then stops before it draws: the
// panel showed half a page. Here, only when that access throws, each one becomes a Storage in
// memory; a working Storage is never replaced.
//
// localStorage is kept by the hub (web-storage, architect m_46151): the server puts the kept map
// just before this script, an inert `<script type="application/json" id="bise-storage">`
// (site.rs `seed_block`), read here once at load; each write (coalesced, one per task) posts the
// whole map to the parent, `{bise: "site-storage", local}`. The copy's old framer that sent it
// to the hub is gone: a web page opens in Sites (plugins/sites) now, on its own origin, where the
// kept map seeds its real localStorage once (docs/plugin-apps.md A5). A copy opened alone in a tab
// has no parent: its writes stay in memory for that load. sessionStorage is in memory, per load.
//
// Not covered, still throwing in the sandbox: indexedDB, document.cookie, caches, and anything
// else tied to an origin. A page that needs those still stops where it reads them, as it would
// in any sandboxed frame.
(function () {
  "use strict";
  // the kind the viewer takes (kit/office/frame.ts FRAME.storage; a classic script can't import it)
  const KIND = "site-storage";
  /** The map the hub kept for this site: {} when none, or not ours to read. */
  const seed = (): Map<string, string> => {
    const m = new Map<string, string>();
    try {
      const el = document.getElementById("bise-storage");
      const v: unknown = el && el.getAttribute("type") === "application/json" ? JSON.parse(el.textContent || "{}") : {};
      if (v && typeof v === "object") for (const [k, x] of Object.entries(v)) if (typeof x === "string") m.set(k, x);
    } catch (_) {
      // a broken seed: an empty storage, as before
    }
    return m;
  };
  for (const name of ["localStorage", "sessionStorage"] as const) {
    try {
      if (window[name]) continue;
    } catch (_) {
      // the sandbox forbids it: the in-memory one below
    }
    const kept = name === "localStorage";
    const m = kept ? seed() : new Map<string, string>();
    let queued = false;
    // the whole map to the viewer, once per task however many writes it made
    const changed = () => {
      if (!kept || queued || window.parent === window) return;
      queued = true;
      queueMicrotask(() => {
        queued = false;
        try {
          window.parent.postMessage({ bise: KIND, local: Object.fromEntries(m) }, "*");
        } catch (_) {
          // no parent to take it: this load only
        }
      });
    };
    const s = {
      getItem: (k: unknown) => (m.has(String(k)) ? m.get(String(k)) : null),
      setItem: (k: unknown, v: unknown) => {
        m.set(String(k), String(v));
        changed();
      },
      removeItem: (k: unknown) => {
        if (m.delete(String(k))) changed();
      },
      clear: () => {
        if (m.size) changed();
        m.clear();
      },
      key: (i: number) => [...m.keys()][i] ?? null,
      get length() {
        return m.size;
      },
    };
    try {
      Object.defineProperty(window, name, { value: s, configurable: true, enumerable: true });
    } catch (_) {
      // the browser keeps it unforgeable: the page meets the sandbox's error, as before
    }
  }
})();
