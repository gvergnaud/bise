// A selection in the side panel's frame (plan §9b, art-notes m_23894): the viewer, framed with
// name="bise-app", posts {bise: 'anchor', artifact, version, anchor, quote} to the window instead of
// opening its own note. The window takes it only from that frame (event.source is the frame's
// contentWindow) on the pages server's origin, keeps only those four fields (and an optional key,
// his first typed character, architect m_24155; rev when a viewer sends it), the anchor read through
// the generated Anchor type (an unknown kind or a wrong field: dropped), and drops anything else.
// Pure, laws in frameNote.test.ts. Owner: art-desktop.
import { ANCHOR_KINDS, type Anchor, type Rect } from '@/proto/anchor';
import { FRAME_KINDS } from '@/shared/frame';

export { FRAME_KINDS };

/** The frame's name the viewers read to know they are in the app (notes.js / select.js). */
export const APP_FRAME_NAME = 'bise-app';

/** What a selection in the frame gives the composer. */
export interface FrameNote {
  readonly artifact: string;
  readonly version: number;
  /** the save of that version he looked at (art-live's rev), when the viewer knows it */
  readonly rev?: number;
  readonly anchor: Anchor;
  /** the selected words, when the selection has some */
  readonly quote?: string;
  /** his first typed character, when typing took the selection (architect m_24155): it goes after the chip */
  readonly key?: string;
}

const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

/** One printable character he typed: exactly one grapheme, no control character. Pure. */
export function isTypedKey(v: unknown): v is string {
  if (typeof v !== 'string' || v === '' || /\p{Cc}/u.test(v)) return false;
  return Array.from(graphemes.segment(v)).length === 1;
}

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);
const str = (v: unknown): v is string => typeof v === 'string';
const nat = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;
const pct = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const optStr = (o: Obj, k: string): { quote?: string } | null => {
  const v = o[k];
  if (v === undefined) return {};
  return str(v) ? { quote: v } : null;
};

/** A rect in % of its frame: four finite numbers, nothing else. Pure. */
export function readRect(v: unknown): Rect | null {
  if (!isObj(v) || !pct(v.x) || !pct(v.y) || !pct(v.w) || !pct(v.h)) return null;
  return { x: v.x, y: v.y, w: v.w, h: v.h };
}

/** An anchor as bise_proto::anchor says it, rebuilt field by field; null when anything is off. Pure. */
export function readAnchor(v: unknown): Anchor | null {
  if (!isObj(v) || !str(v.kind) || !(ANCHOR_KINDS as readonly string[]).includes(v.kind)) return null;
  const kind = v.kind as Anchor['kind'];
  switch (kind) {
    case 'block': {
      const q = optStr(v, 'quote');
      return str(v.block) && v.block !== '' && q !== null ? { kind, block: v.block, ...q } : null;
    }
    case 'cell':
      return str(v.sheet) && str(v.range) && v.range !== '' ? { kind, sheet: v.sheet, range: v.range } : null;
    case 'slide': {
      if (!nat(v.slide)) return null;
      if (v.rect === undefined) return { kind, slide: v.slide };
      const rect = readRect(v.rect);
      return rect === null ? null : { kind, slide: v.slide, rect };
    }
    case 'shape': {
      const q = optStr(v, 'quote');
      return nat(v.slide) && str(v.shape) && v.shape !== '' && q !== null ? { kind, slide: v.slide, shape: v.shape, ...q } : null;
    }
    case 'para': {
      const q = optStr(v, 'quote');
      return str(v.para) && v.para !== '' && q !== null ? { kind, para: v.para, ...q } : null;
    }
    case 'region': {
      const rect = readRect(v.rect);
      if (rect === null) return null;
      if (v.page === undefined) return { kind, rect };
      return nat(v.page) ? { kind, page: v.page, rect } : null;
    }
    case 'text': {
      const q = optStr(v, 'quote');
      return nat(v.line) && q !== null ? { kind, line: v.line, ...q } : null;
    }
    case 'time': {
      // a video's moment: seconds, finite and never negative (bise_proto::anchor's `moment`)
      if (!pct(v.t) || v.t < 0) return null;
      if (v.rect === undefined) return { kind, t: v.t };
      const rect = readRect(v.rect);
      return rect === null ? null : { kind, t: v.t, rect };
    }
    case 'view': {
      // a plugin's view's own place: its key 1..=512 bytes (bise_proto::anchor's `view_key`)
      const q = optStr(v, 'quote');
      const label = v.label === undefined ? {} : str(v.label) ? { label: v.label } : null;
      return str(v.key) && v.key !== '' && new TextEncoder().encode(v.key).length <= 512 && q !== null && label !== null ? { kind, key: v.key, ...q, ...label } : null;
    }
  }
}

/** Where a message came from, and the one frame and origin it may come from. */
export interface FrameFrom {
  readonly origin: string;
  readonly source: unknown;
}
export interface FrameOf {
  /** the pages server's origin (`http://127.0.0.1:<port>`), from the frame's own src */
  readonly origin: string | null;
  /** the side panel's frame's contentWindow */
  readonly window: unknown;
}

/** The origin of the frame's src (`http://127.0.0.1:47438`), null for none or a bad url. Pure. */
export function originOf(src: string | null): string | null {
  if (src === null) return null;
  const m = /^(https?:\/\/[^/?#]+)/iu.exec(src);
  return m?.[1]?.toLowerCase() ?? null;
}

/** From that frame (source) on the pages server (origin): the one check every message passes. Pure. */
function fromFrame(from: FrameFrom, frame: FrameOf): boolean {
  if (frame.origin === null || frame.window == null) return false;
  return from.source === frame.window && from.origin.toLowerCase() === frame.origin;
}

/** What the frame said: a selection, or a web page's pinch or pan (web-pinch). */
export type FrameMsg =
  | { readonly kind: 'anchor'; readonly note: FrameNote }
  | { readonly kind: 'pinch'; readonly dy: number; readonly x: number; readonly y: number; readonly w: number }
  | { readonly kind: 'pan'; readonly dx: number; readonly dy: number; readonly w: number };

/** The most one wheel may say, in CSS px (a pinch's delta, a pan's, a point, a width). */
const MAX_WHEEL = 100_000;
const num = (v: unknown): v is number => pct(v) && Math.abs(v) <= MAX_WHEEL;
const width = (v: unknown): v is number => num(v) && v > 0;

/** A web page's pinch or pan (main/pinch.ts PINCH_FORWARD): its numbers, all finite and in bounds; null else. Pure. */
function readWheel(data: Obj): FrameMsg | null {
  if (data.bise === FRAME_KINDS.pinch) {
    return num(data.dy) && num(data.x) && num(data.y) && width(data.w) ? { kind: 'pinch', dy: data.dy, x: data.x, y: data.y, w: data.w } : null;
  }
  return num(data.dx) && num(data.dy) && width(data.w) ? { kind: 'pan', dx: data.dx, dy: data.dy, w: data.w } : null;
}

/**
 * A message from the frame, read once for every kind it may send the window (kit/office/frame.js):
 * a selection (readFrameNote), a web page's {bise: pinch} or {bise: pan}; null for anything else, or not from
 * that frame on the pages server. Pure.
 */
export function readFrame(data: unknown, from: FrameFrom, frame: FrameOf): FrameMsg | null {
  if (!fromFrame(from, frame) || !isObj(data)) return null;
  if (data.bise === FRAME_KINDS.pinch || data.bise === FRAME_KINDS.pan) return readWheel(data);
  const note = readFrameNote(data, from, frame);
  return note === null ? null : { kind: 'anchor', note };
}

/**
 * The selection a message from the frame carries, or null: not from that frame (source), not from
 * the pages server (origin), not a {bise: 'anchor'} message, or a field missing or of the wrong
 * shape. Only artifact, version, anchor and quote are kept. Pure.
 */
export function readFrameNote(data: unknown, from: FrameFrom, frame: FrameOf): FrameNote | null {
  if (!fromFrame(from, frame)) return null;
  if (!isObj(data) || data.bise !== FRAME_KINDS.anchor) return null;
  if (!str(data.artifact) || data.artifact === '' || !nat(data.version) || data.version < 1) return null;
  const anchor = readAnchor(data.anchor);
  if (anchor === null) return null;
  const q = optStr(data, 'quote');
  if (q === null) return null;
  if (data.rev !== undefined && !nat(data.rev)) return null;
  if (data.key !== undefined && !isTypedKey(data.key)) return null;
  const rev = data.rev === undefined ? {} : { rev: data.rev };
  const key = data.key === undefined ? {} : { key: data.key };
  return { artifact: data.artifact, version: data.version, ...rev, anchor, ...(q.quote === undefined || q.quote === '' ? {} : q), ...key };
}
