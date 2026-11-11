// A part of an artifact as a composer attachment (plan §9b, designer's composer-attachments A rows:
// '❝ Q3!B4:D9  q3-forecast.xlsx · v4'): a selection in the side panel's frame, sent as
// Part::Anchor (art-notes' wire). Its words are bise_proto::anchor::label's, pinned to it by the
// shared fixture rust/proto/fixtures/anchor_label.json (anchor.test.ts). Pure. Owner: art-desktop.
import type { Anchor } from '@/proto/anchor';

/** A sheet's name as a reference says it: `Q3`, `'My sheet'` (Excel's rule: quoted unless letters,
 *  digits, `_` and `.`, not starting with a digit; a `'` inside doubled). Pure. */
function sheetRef(s: string): string {
  const plain = s !== '' && !/^[0-9]/u.test(s) && /^[\p{L}\p{N}_.]+$/u.test(s);
  return plain ? s : `'${s.replace(/'/gu, "''")}'`;
}

/** One line, at most `max` characters, `…` when cut. Pure. */
function short(s: string, max: number): string {
  const one = s.split(/\s+/u).filter((w) => w !== '').join(' ');
  const cs = Array.from(one);
  return cs.length <= max ? one : `${cs.slice(0, max - 1).join('').trimEnd()}…`;
}

/** A video's moment as a player says it: `0:12`, `12:05`, `1:02:05` (bise_proto::anchor::clock). Pure. */
export function clock(t: number): string {
  const s = Number.isFinite(t) && t > 0 ? Math.floor(t) : 0;
  const two = (n: number): string => String(n).padStart(2, '0');
  const h = Math.floor(s / 3600);
  return h > 0 ? `${String(h)}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}` : `${String(Math.floor(s / 60))}:${two(s % 60)}`;
}

const quoted = (q: string): string => `"${short(q, 40)}"`;
const has = (q: string | undefined): q is string => q !== undefined && q.trim() !== '';

/**
 * Where in the artifact: `Q3!B4:D9`, `slide 2`, `slide 2 · a box`, `slide 2 · a shape`,
 * `"net revenue…"`, `page 1 · a box`, `page 1 · "net revenue…"`, `line 40`, `0:12`, `0:12 · a box`; a kind this window
 * doesn't know: `a part`. Pure.
 */
export function anchorLabel(a: Anchor): string {
  switch (a.kind) {
    case 'block':
      return has(a.quote) ? quoted(a.quote) : `block ${a.block}`;
    case 'cell':
      return `${sheetRef(a.sheet)}!${a.range}`;
    case 'slide':
      return a.rect === undefined ? `slide ${String(a.slide)}` : `slide ${String(a.slide)} · a box`;
    case 'shape':
      // a shape is said by its words, never its id (designer m_24098)
      return has(a.quote) ? `slide ${String(a.slide)} · ${quoted(a.quote)}` : `slide ${String(a.slide)} · a shape`;
    case 'para':
      return has(a.quote) ? quoted(a.quote) : 'a paragraph';
    case 'region':
      // a pdf passage is said by its words (pdf-notes): `page 2 · "net revenue…"`
      if (has(a.quote)) return a.page === undefined ? quoted(a.quote) : `page ${String(a.page)} · ${quoted(a.quote)}`;
      return a.page === undefined ? 'a box' : `page ${String(a.page)} · a box`;
    case 'text':
      return `line ${String(a.line)}`;
    case 'time':
      // a video's moment, or a box on its frame there (video-view)
      return a.rect === undefined ? clock(a.t) : `${clock(a.t)} · a box`;
    case 'view':
      // a plugin's view: its key is opaque; its label names the place, his words what is there
      // (plugin-apps A5, architect m_60509)
      if (has(a.label)) return has(a.quote) ? `${a.label} · ${quoted(a.quote)}` : a.label;
      return has(a.quote) ? quoted(a.quote) : 'a part';
    default:
      return 'a part';
  }
}

/** The row's facts: 'q3-forecast.xlsx · v4'. Pure. */
export const anchorWhere = (file: string, version: number): string => `${file} · v${String(version)}`;
