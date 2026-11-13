// Sites · a note's place as the view's anchor key (docs/plugin-apps.md A5, architect m_60421,
// m_60380). A place is a block of the site's page: `block` is the element's `#id` when it is
// unique, else its tag path from the body (`body>main:nth-of-type(1)>p:nth-of-type(3)`), and
// `quote` the words he selected in it, the fallback when the path moved in a new version. The key
// is that place as JSON, its keys sorted, no whitespace: `{"block":"#intro","kind":"block",
// "quote":"net revenue"}`, the same shape the hub gives an older site note's typed anchor
// (bise_proto::anchor::as_view_key of `Anchor::Block`), so his notes from before Sites place too.
// The hub never reads a key; only this view does, and it parses it.
//
// Pure (tested by plugins/sites/test/site.test.mjs).

/** A place on a site: a block of its page, and his words in it. */
export type Place = { kind: "block"; block: string; quote?: string };

/** The longest key a view may give (bise_proto::anchor::VIEW_KEY_MAX). */
export const KEY_MAX = 512;
/** The most of his words a place keeps (the key stays under KEY_MAX with a long path). */
export const QUOTE_MAX = 200;
/** The longest block path a place keeps (a deeper block: its nearest ancestor that fits). */
export const BLOCK_MAX = 280;

/** `v` as JSON with every object's keys sorted, no whitespace. */
export function canonical(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (v !== null && typeof v === "object") {
    const o = v as Record<string, unknown>;
    const keys = Object.keys(o).filter((k) => o[k] !== undefined).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonical(o[k])}`).join(",")}}`;
  }
  return JSON.stringify(v);
}

/** His words as one line, spaces folded. */
export function norm(s: unknown): string {
  return String(s ?? "").replace(/\s+/g, " ").trim();
}

/** A place as its key: the quote folded and cut (to QUOTE_MAX, then as much as the key needs to
 * stay within KEY_MAX once escaped), left out when empty. Null: the block's path is empty or too
 * long to be a key. */
export function keyOf(p: Place): string | null {
  if (!p.block || p.block.length > BLOCK_MAX) return null;
  let quote = norm(p.quote).slice(0, QUOTE_MAX);
  for (;;) {
    const key = canonical({ kind: "block", block: p.block, quote: quote.trim() || undefined });
    if (key.length <= KEY_MAX) return key;
    if (!quote) return null;
    quote = quote.slice(0, Math.max(0, quote.length - (key.length - KEY_MAX)));
  }
}

/** The place a key names, or null (not JSON, another kind, bad fields, too long): the note shows
 * detached, never dropped. */
export function placeOf(key: unknown): Place | null {
  if (typeof key !== "string" || key.length === 0 || key.length > KEY_MAX) return null;
  let v: unknown;
  try {
    v = JSON.parse(key);
  } catch {
    return null;
  }
  if (v === null || typeof v !== "object" || Array.isArray(v)) return null;
  const o = v as Record<string, unknown>;
  if (o.kind !== "block" || typeof o.block !== "string" || o.block === "") return null;
  if (o.quote !== undefined && typeof o.quote !== "string") return null;
  const quote = norm(o.quote);
  return quote ? { kind: "block", block: o.block, quote } : { kind: "block", block: o.block };
}
