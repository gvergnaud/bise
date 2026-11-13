// generated from where.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// Sites · where he is in a site, and what is the site's (docs/plugin-apps.md A5): the rules for
// a link (inside the site's files, or out) and for the view's state (the page and its scroll,
// put back when the tab is drawn again).
//
// Pure (tested by plugins/sites/test/site.test.mjs).
/** Whether `href` is one of the site's own files: on `base`'s origin, under its path. A bad url
 * is not. */
export function inside(href, base) {
    try {
        const u = new URL(href);
        const b = new URL(base);
        return u.origin === b.origin && u.pathname.startsWith(b.pathname);
    }
    catch {
        return false;
    }
}
/** The view's kept state read back, kept only when it names a page under `base` (a state of an
 * older session names another session's path: dropped, the site opens on its first page). */
export function stateOf(v, base) {
    if (v === null || typeof v !== "object")
        return null;
    const o = v;
    if (typeof o.path !== "string" || typeof o.y !== "number" || !Number.isFinite(o.y))
        return null;
    let b;
    try {
        b = new URL(base);
    }
    catch {
        return null;
    }
    if (!o.path.startsWith(b.pathname))
        return null;
    return { path: o.path, y: Math.max(0, Math.round(o.y)) };
}
