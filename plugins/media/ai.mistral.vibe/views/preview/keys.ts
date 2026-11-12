// Preview · a note's place as the view's anchor key (docs/plugin-apps.md A5, architect m_60380).
// The key is the place as JSON with its keys sorted and no whitespace, its words apart (the
// note's quote): `{"kind":"region","rect":{"h":20,"w":30,"x":10,"y":5}}` (a box on an image),
// `{"kind":"region","page":3,"rect":{…}}` (a box or a passage on a pdf page),
// `{"kind":"time","t":12.5}` or `{"kind":"time","rect":{…},"t":12.5}` (a moment of a video, a box
// on its frame). The same shape the hub gives an older note's typed anchor (bise_proto::anchor::
// as_view_key), so his notes from before Preview place too. The hub never reads a key; only this
// view does, and it parses it (never compares strings: 10 and 10.0 are the same number).
//
// Pure (tested by plugins/media/test/keys.test.mjs).

/** A box in % of its frame (the image, the pdf page, the video frame): 0..100 each. */
export type Rect = { x: number; y: number; w: number; h: number };

/** A place on a Preview document, read off a key. */
export type Place =
  | { kind: "region"; rect: Rect; page?: number }
  | { kind: "time"; t: number; rect?: Rect };

/** The longest key a view may give (bise_proto::anchor::VIEW_KEY_MAX). */
export const KEY_MAX = 512;

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

/** A place as its key (its words never in it). */
export function keyOf(p: Place): string {
  return canonical(p);
}

const num = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
const pct = (v: unknown): v is number => num(v) && v >= 0 && v <= 100;

function rectOf(v: unknown): Rect | null {
  if (v === null || typeof v !== "object") return null;
  const r = v as Record<string, unknown>;
  return pct(r.x) && pct(r.y) && pct(r.w) && pct(r.h) ? { x: r.x, y: r.y, w: r.w, h: r.h } : null;
}

/** The place a key names, or null (not JSON, another kind, bad fields, too long): the note shows
 * detached, never dropped. */
export function placeOf(key: string): Place | null {
  if (typeof key !== "string" || key.length === 0 || key.length > KEY_MAX) return null;
  let v: unknown;
  try {
    v = JSON.parse(key);
  } catch {
    return null;
  }
  if (v === null || typeof v !== "object" || Array.isArray(v)) return null;
  const o = v as Record<string, unknown>;
  if (o.kind === "region") {
    const rect = rectOf(o.rect);
    if (!rect) return null;
    if (o.page === undefined) return { kind: "region", rect };
    return num(o.page) && Number.isInteger(o.page) && o.page >= 1 ? { kind: "region", rect, page: o.page } : null;
  }
  if (o.kind === "time") {
    if (!num(o.t) || o.t < 0) return null;
    if (o.rect === undefined) return { kind: "time", t: o.t };
    const rect = rectOf(o.rect);
    return rect ? { kind: "time", t: o.t, rect } : null;
  }
  return null;
}

/** Two points (px, in an element of `w` × `h`) as a box in % of it, 0.1 apart, clamped. */
export function boxPct(a: { x: number; y: number }, b: { x: number; y: number }, w: number, h: number): Rect {
  const clamp = (v: number) => Math.max(0, Math.min(100, Math.round(v * 10) / 10));
  const x0 = clamp((Math.min(a.x, b.x) / w) * 100);
  const y0 = clamp((Math.min(a.y, b.y) / h) * 100);
  const x1 = clamp((Math.max(a.x, b.x) / w) * 100);
  const y1 = clamp((Math.max(a.y, b.y) / h) * 100);
  return { x: x0, y: y0, w: Math.round((x1 - x0) * 10) / 10, h: Math.round((y1 - y0) * 10) / 10 };
}

/** A box too small to be one (a click): under 1% either way. */
export const isClick = (r: Rect) => r.w < 1 || r.h < 1;

/** A moment as a player says it: `0:12`, `12:05`, `1:02:05` (bise_proto::anchor::clock). */
export function clock(t: number): string {
  const s = Number.isFinite(t) && t > 0 ? Math.floor(t) : 0;
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}
