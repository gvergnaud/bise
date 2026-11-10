// generated from _handlers.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// What each telemetry route does, its world passed in (the store, today, the dashboard's password),
// so the tests run them whole on a throwaway Postgres. The route files (packets, delete, purge,
// dashboard) only call these with the world of their environment (`fromEnv`).
//
// What they read of a request: the body and content-type, content-encoding, content-length
// (_body.mts), and the dashboard's Authorization. Never the address, the user agent or another
// header; nothing is logged (tests/site_telemetry.test.ts reads the sources for both).
import { readJson } from "./_body.mjs";
import { renderDashboard, renderNote } from "./_render.mjs";
import { neon } from "./_sql.mjs";
import { deleteInstall, purge, readStats, storePackets } from "./_store.mjs";
import { checkDelete, checkPackets, dayOf } from "./_validate.mjs";
/** The world of the deployed functions: Vercel's environment and the clock. */
export function fromEnv() {
    const env = globalThis.process.env;
    const url = env.DATABASE_URL;
    return { sql: url ? neon(url) : null, today: dayOf(Date.now()), password: env.TELEMETRY_DASHBOARD_PASSWORD || null };
}
const NO_STORE = { "Cache-Control": "no-store", "X-Robots-Tag": "noindex, nofollow" };
function json(status, body) {
    return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json", ...NO_STORE } });
}
const storeDown = (e) => json(503, { error: `the store failed: ${e instanceof Error ? e.message : "unknown"}` });
/** POST /v1/packets: the day packets, all taken or none (a 4xx makes the app drop them; a 5xx keeps
 * them for later). */
export async function handlePackets(request, w) {
    if (request.method !== "POST")
        return json(405, { error: "POST only" });
    if (!w.sql)
        return json(503, { error: "no store yet" });
    const body = await readJson(request);
    if (!body.ok)
        return json(body.status, { error: body.error });
    const packets = checkPackets(body.value, w.today);
    if (!packets.ok)
        return json(400, { error: packets.error });
    try {
        const { stored } = await storePackets(w.sql, packets.value, w.today);
        return json(200, { stored });
    }
    catch (e) {
        return storeDown(e);
    }
}
/** POST /v1/delete: every packet of that install id; 200 also when there were none. */
export async function handleDelete(request, w) {
    if (request.method !== "POST")
        return json(405, { error: "POST only" });
    if (!w.sql)
        return json(503, { error: "no store yet" });
    const body = await readJson(request);
    if (!body.ok)
        return json(body.status, { error: body.error });
    const id = checkDelete(body.value);
    if (!id.ok)
        return json(400, { error: id.error });
    try {
        return json(200, { deleted: await deleteInstall(w.sql, id.value) });
    }
    catch (e) {
        return storeDown(e);
    }
}
/** GET /api/telemetry/purge, Vercel's daily cron: the packets past 13 months go. Open to anyone: it
 * only deletes what is already past retention (architect m_58375). */
export async function handlePurge(w) {
    if (!w.sql)
        return json(200, { purged: 0 });
    try {
        return json(200, { purged: await purge(w.sql, w.today) });
    }
    catch (e) {
        return storeDown(e);
    }
}
const enc = new TextEncoder();
/** Equal secrets, compared on their SHA-256 in a time that doesn't depend on where they differ. */
async function sameSecret(a, b) {
    const [x, y] = await Promise.all([a, b].map(async (s) => new Uint8Array(await crypto.subtle.digest("SHA-256", enc.encode(s)))));
    let diff = 0;
    for (let i = 0; i < x.length; i++)
        diff |= x[i] ^ y[i];
    return diff === 0;
}
/** The password of a Basic Authorization header (any user name), or null. */
export function basicPassword(header) {
    const m = header ? /^Basic\s+([A-Za-z0-9+/=]+)$/i.exec(header.trim()) : null;
    if (!m)
        return null;
    try {
        const text = new TextDecoder().decode(Uint8Array.from(atob(m[1]), (c) => c.charCodeAt(0)));
        const colon = text.indexOf(":");
        return colon < 0 ? null : text.slice(colon + 1);
    }
    catch {
        return null;
    }
}
const HTML_HEADERS = {
    "Content-Type": "text/html; charset=utf-8",
    "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    ...NO_STORE,
};
const html = (status, body, more = {}) => new Response(body, { status, headers: { ...HTML_HEADERS, ...more } });
/** GET bise.dev/telemetry: the dashboard, behind one password (Basic auth, the password in Vercel's
 * environment, never in the repo). No password set: closed to everyone. */
export async function handleDashboard(request, w) {
    if (!w.password)
        return html(503, renderNote("not set up", "set TELEMETRY_DASHBOARD_PASSWORD in the project's environment, then deploy."));
    const given = basicPassword(request.headers.get("authorization"));
    if (given === null || !(await sameSecret(given, w.password))) {
        return html(401, renderNote("password needed", "this page is private."), { "WWW-Authenticate": 'Basic realm="bise usage", charset="UTF-8"' });
    }
    if (!w.sql)
        return html(200, renderNote("no store yet", "connect a Neon database to the project (DATABASE_URL), then deploy."));
    try {
        return html(200, renderDashboard(await readStats(w.sql, w.today)));
    }
    catch (e) {
        return html(503, renderNote("the store failed", e instanceof Error ? e.message : "unknown"));
    }
}
