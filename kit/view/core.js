// generated from core.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · @bise/view's pure half (docs/plugin-apps.md A4, A5): what a view's document is, a
// read's and a write's shapes, a selection's anchor and quote, which notes a version marks and
// which are detached, the view's own state's size, the host's theme. No DOM, no window: the node
// checks (core.check.ts) run it as is; view.ts and notes.ts call it.
//
// A view's anchor is its own opaque string (1..=512 bytes, bise_proto::anchor's `view {key}`);
// the hub never reads it, so only the view can tell a note it can't place (`detached`, listed,
// never dropped).
/** The longest anchor a view may give, in UTF-8 bytes (bise_proto::anchor::VIEW_KEY_MAX). */
export const ANCHOR_MAX = 512;
/** The most characters a quote keeps (bise_proto::anchor::QUOTE_MAX). */
export const QUOTE_MAX = 200;
/** The most bytes of JSON a view's own state holds (the RFC's viewState). */
export const STATE_MAX = 64 * 1024;
/** The most bytes one read answers (A4); a bigger file is read by ranges. */
export const READ_MAX = 24 * 1024 * 1024;
/** How many times a save is sent with the same seq before its error is the view's. */
export const SAVE_TRIES = 3;
const utf8 = (s) => new TextEncoder().encode(s).length;
const isObj = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
const str = (v) => (typeof v === "string" ? v : "");
const num = (v) => (typeof v === "number" && Number.isFinite(v) ? v : null);
/**
 * The document from the opener's arguments (`{file: {name, resourceUri}}`, the RFC's shape) and
 * the host context's `document` ({name, type, version?, size, writable}); null when the view was
 * not opened on a file. The handle comes only from the arguments, the facts from the host.
 */
export function documentOf(args, ctx) {
    const file = isObj(args) && isObj(args["file"]) ? args["file"] : null;
    const uri = file ? str(file["resourceUri"]) : "";
    if (!uri)
        return null;
    const d = isObj(ctx) && isObj(ctx["document"]) ? ctx["document"] : {};
    return {
        uri,
        name: str(d["name"]) || str(file["name"]),
        type: str(d["type"]),
        version: num(d["version"]),
        size: num(d["size"]) ?? 0,
        writable: d["writable"] === true,
    };
}
/** resources/read's params: the handle, and a range when asked (whole bytes, length ≥ 1). */
export function readParams(uri, range) {
    if (!range)
        return { uri };
    const ok = Number.isInteger(range.offset) && range.offset >= 0 && Number.isInteger(range.length) && range.length >= 1 && range.length <= READ_MAX;
    if (!ok)
        throw new RangeError(`a range is {offset ≥ 0, length 1..${READ_MAX}}`);
    return { uri, range: { offset: range.offset, length: range.length } };
}
/** base64 → bytes (atob's binary string, byte by byte). */
export function bytesOf(b64) {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++)
        out[i] = bin.charCodeAt(i);
    return out;
}
/** bytes → base64, by chunks (no argument list of a whole file). */
export function base64Of(bytes) {
    let bin = "";
    for (let i = 0; i < bytes.length; i += 0x8000)
        bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    return btoa(bin);
}
/**
 * resources/read's result as a Read: MCP's `{contents: [{uri, text | blob, mimeType?}]}`, the
 * hub's facts in `_meta` (on the content, else on the result). Throws on a result with no
 * content or no etag: a view never saves on a read it can't match.
 */
export function readOf(result) {
    const contents = isObj(result) && Array.isArray(result["contents"]) ? result["contents"] : [];
    const c = contents.find(isObj);
    if (!c)
        throw new Error("the read has no content");
    const meta = isObj(c["_meta"]) ? c["_meta"] : isObj(result) && isObj(result["_meta"]) ? result["_meta"] : {};
    const etag = str(meta["etag"]);
    if (!etag)
        throw new Error("the read has no etag");
    const mimeType = str(c["mimeType"]) || undefined;
    const facts = { etag, writable: meta["writable"] === true, size: num(meta["size"]) ?? 0, ...(mimeType ? { mimeType } : {}) };
    if (typeof c["text"] === "string")
        return { text: c["text"], ...facts };
    if (typeof c["blob"] === "string")
        return { bytes: bytesOf(c["blob"]), ...facts };
    throw new Error("the read has neither text nor blob");
}
/** ai.mistral.vibe/resources/write's params: a string goes as text, bytes as a blob; `seq` the
 * save's number in this sitting (the same on a retry: the hub answers it once). */
export function writeParams(uri, data, ifMatch, seq) {
    if (!Number.isInteger(seq) || seq < 1)
        throw new RangeError("seq counts from 1");
    const body = typeof data === "string" ? { text: data } : { blob: base64Of(data) };
    return { uri, ...(ifMatch ? { ifMatch } : {}), ...body, seq };
}
/** The write's result as a Saved; anything else is an error (the view shows it, never guesses). */
export function savedOf(result) {
    const r = isObj(result) ? result : {};
    const etag = str(r["etag"]);
    if (r["outcome"] === "saved" && etag)
        return { outcome: "saved", etag };
    if (r["outcome"] === "conflict" && etag)
        return { outcome: "conflict", etag };
    if (r["outcome"] === "too-large" && num(r["maxBytes"]) != null)
        return { outcome: "too-large", maxBytes: num(r["maxBytes"]) };
    throw new Error("the save's answer is not one bise knows");
}
/** resources/url's result: its `url` (the hub's session URL, relative or absolute). */
export function urlOf(result) {
    const url = isObj(result) ? str(result["url"]) : "";
    if (!url)
        throw new Error("the answer has no url");
    return url;
}
/** A link he may open: http, https or mailto only (anything else is the view's bug). */
export function linkOf(url) {
    if (!/^(https?:\/\/|mailto:)/i.test(String(url)))
        throw new RangeError("a link is http(s) or mailto");
    return url;
}
/** JSON-RPC codes the kit tells apart (bise_proto::view_rpc's; the bridge's own timeout). */
export const CODE = { serverGone: -32000, timeout: -32099, tooLarge: -32038, badHandle: -32039, noWrite: -32040, gone: -32041 };
/** A save error worth sending again with the same seq: the way to the hub broke (the hub
 * answers a seq once, so a resend never makes a second version). Refusals are final. */
export function retryable(err) {
    const code = isObj(err) ? err["code"] : undefined;
    return code === CODE.serverGone || code === CODE.timeout;
}
// ---- selection
/** An anchor a view may give: a string of 1..=512 UTF-8 bytes. */
export function anchorOk(a) {
    return typeof a === "string" && a.length > 0 && utf8(a) <= ANCHOR_MAX;
}
/** What he saw selected, one line, at most 200 characters ("" none). */
export function quoteOf(text) {
    const one = String(text || "").replace(/[\s\u0000-\u001f\u007f]+/g, " ").trim();
    return [...one].length > QUOTE_MAX ? `${[...one].slice(0, QUOTE_MAX - 1).join("")}…` : one;
}
/** The character a keydown types, when it is exactly one printable grapheme and no shortcut, IME
 * composition or dead key; null otherwise (what makes "type to ask about it"). */
export function typed(e) {
    if (e.ctrlKey || e.metaKey || e.altKey || e.isComposing || e.key === "Dead" || typeof e.key !== "string")
        return null;
    const g = typeof Intl !== "undefined" && Intl.Segmenter ? [...new Intl.Segmenter().segment(e.key)].length : [...e.key].length;
    return g === 1 && !/[\u0000-\u001f\u007f]/.test(e.key) ? e.key : null;
}
/** The most characters a place's label keeps (bise_proto::anchor::LABEL_MAX). */
export const LABEL_MAX = 60;
/** A view's own short name for a place (`page 2 · a box`, `0:12`): one line, 60 characters. */
export function labelOf(text) {
    const one = String(text || "").replace(/[\s\u0000-\u001f\u007f]+/g, " ").trim();
    return [...one].length > LABEL_MAX ? `${[...one].slice(0, LABEL_MAX - 1).join("")}…` : one;
}
/** dev.bise/selection's params: an anchor, its quote and its label (display only: the place is
 * the anchor alone), or null (nothing selected), and how he asked about it, if he did. A bad
 * anchor throws: the view's bug, said at once. */
export function selectionParams(anchor, text, ask, label) {
    if (anchor === null)
        return { anchor: null };
    if (!anchorOk(anchor))
        throw new RangeError(`an anchor is a string of 1 to ${ANCHOR_MAX} bytes`);
    const q = quoteOf(text);
    const l = labelOf(label);
    const how = !ask ? {} : "key" in ask ? { key: ask.key } : { take: true };
    return { anchor, ...(q ? { text: q } : {}), ...(l ? { label: l } : {}), ...how };
}
/** Where the pill sits by a selection's box, in a window of `w` × `h` (select.js's rule: under
 * it, kept 8 px from the left and inside the window). */
export function pillAt(r, w, h) {
    return { left: Math.max(8, Math.min(w - 160, r.left)), top: Math.max(8, Math.min(h - 36, r.bottom + 6)) };
}
/** dev.bise/notes's params as notes: each one checked, a malformed one left out (the host's
 * list is bise's, but a view never draws a shape it did not check). */
export function notesOf(params) {
    const list = isObj(params) && Array.isArray(params["notes"]) ? params["notes"] : [];
    const out = [];
    for (const n of list) {
        if (!isObj(n) || !str(n["id"]) || !anchorOk(n["anchor"]))
            continue;
        const status = n["status"];
        if (status !== "draft" && status !== "sent" && status !== "done")
            continue;
        const version = num(n["version"]);
        if (version == null)
            continue;
        const doneIn = num(n["done_in"]);
        const quote = str(n["quote"]);
        out.push({ id: str(n["id"]), anchor: n["anchor"], text: str(n["text"]), status, version, ...(quote ? { quote } : {}), ...(doneIn != null ? { done_in: doneIn } : {}) });
    }
    return out;
}
/** A note's mark in version `v`: `draft`, `sent`, `done` (✓, on the version that answered it),
 * or null (answered in another version: no mark). */
export function markOf(note, v) {
    if (note.status === "draft")
        return "draft";
    if (note.status === "sent")
        return "sent";
    if (note.status === "done" && note.done_in != null && v != null)
        return note.done_in === v ? "done" : null;
    return null;
}
export function layout(notes, v, placed) {
    const marked = [];
    const detached = [];
    for (const note of notes) {
        const mark = markOf(note, v);
        if (!mark)
            continue;
        if (placed(note.anchor))
            marked.push({ note, mark });
        else
            detached.push(note);
    }
    return { marked, detached };
}
// ---- the view's own state (ai.mistral.vibe/viewState)
/** The bytes of a state as JSON, null when it is not JSON (a function, a cycle, a bigint). */
export function stateBytes(state) {
    try {
        const s = JSON.stringify(state);
        return s === undefined ? null : utf8(s);
    }
    catch {
        return null;
    }
}
/** The state the host gave back at ui/initialize (undefined: none). */
export function stateOf(ctx) {
    return isObj(ctx) ? ctx["ai.mistral.vibe/viewState"] : undefined;
}
// ---- the theme
/** The host context's theme and its CSS variables (the spec's names, bise's values): only
 * `--` names and values with no `;`, `{`, `}` or `<` (a value is a token, never a rule). */
export function themeOf(ctx) {
    const c = isObj(ctx) ? ctx : {};
    const theme = c["theme"] === "light" || c["theme"] === "dark" ? c["theme"] : null;
    const styles = isObj(c["styles"]) && isObj(c["styles"]["variables"]) ? c["styles"]["variables"] : {};
    const vars = Object.entries(styles).filter((kv) => /^--[\w-]+$/.test(kv[0]) && typeof kv[1] === "string" && !/[;{}<]/.test(kv[1]));
    return { theme, vars };
}
