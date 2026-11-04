// The chips on the clipboard (his words via main m_30833: « si je copy du contenu avec des chips
// dedans, le fait de paste devrait remettre les chips et les attachments; copy et paste au même
// endroit devrait être une no-op sur le state »). A copy in the field writes its words with the
// chips' labels as text/plain (what another app reads) and, when it holds a chip, its segments in a
// private type (CLIP_MIME); a paste that finds them rebuilds the chips, so their attachment rows
// come back with them. Pure: no view, no DOM event (paste.ts wires it).
//
// The numbers (placeChips): a pasted chip keeps its own number when it is free in the field, or
// when the field holds that very chip under it (one row, one attachment: a second mention of it);
// the same chip under another number takes that number; any other gets the next free one. A copy
// then a paste over the same selection, or a cut then a paste where it was, gives the field back
// as it was.
import type { Node as PmNode, Slice } from '@tiptap/pm/model';
import type { Chip, Segment } from '../message';
import { tidy } from '../entry';

/** The private clipboard type of the field's copy: `{"v":1,"text":…,"segs":[…]}`. */
export const CLIP_MIME = 'application/x-bise-composer';

const V = 1;

/** The segments of a copied slice: its words (a newline '\n'), its chips, no dictation. */
export function segmentsOfSlice(slice: Slice): Segment[] {
  const out: Segment[] = [];
  slice.content.descendants((n: PmNode) => {
    if (n.isText) out.push({ kind: 'text', text: n.text ?? '' });
    else if (n.type.name === 'hardBreak') out.push({ kind: 'text', text: '\n' });
    else if (n.type.name === 'chip') out.push({ kind: 'chip', chip: n.attrs.chip as Chip });
    return true;
  });
  return tidy(out);
}

/** The private type's payload for a copy, null when it holds no chip (text/plain says it all). */
export function writeClip(segs: readonly Segment[], text: string): string | null {
  if (!segs.some((s) => s.kind === 'chip')) return null;
  return JSON.stringify({ v: V, text, segs });
}

const isObj = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x);
const KINDS: readonly string[] = ['image', 'paste', 'quote', 'file', 'artifact', 'anchor', 'plugin'];
const isChip = (x: unknown): x is Chip => isObj(x) && Number.isInteger(x.n) && (x.n as number) > 0 && typeof x.kind === 'string' && KINDS.includes(x.kind);
const isSegment = (x: unknown): x is Segment =>
  isObj(x) && (x.kind === 'text' ? typeof x.text === 'string' : x.kind === 'chip' && isChip(x.chip));
const lines = (t: string): string => t.replace(/\r\n?/gu, '\n');

/** A paste's segments from the private type, when it was written with the same plain text (a
 *  later copy elsewhere replaced both; a stale or foreign payload is never trusted); else null. */
export function readClip(raw: string, plain: string): Segment[] | null {
  if (raw === '') return null;
  let v: unknown;
  try {
    v = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isObj(v) || v.v !== V || typeof v.text !== 'string' || lines(v.text) !== lines(plain)) return null;
  return Array.isArray(v.segs) && v.segs.every(isSegment) ? tidy(v.segs) : null;
}

/** A chip's data in one shape (its keys sorted), with or without its number. */
function keyOf(c: Chip, withN: boolean): string {
  const o = c as unknown as Record<string, unknown>;
  const keys = Object.keys(o)
    .filter((k) => withN || k !== 'n')
    .sort();
  return JSON.stringify(keys.map((k) => [k, o[k]]));
}

/** The pasted segments with their chips numbered for a field that holds `inField` (the field
 *  without the selection the paste replaces). See the header for the rules. Pure. */
export function placeChips(pasted: readonly Segment[], inField: readonly Chip[]): Segment[] {
  const held = new Map<number, Chip>();
  for (const c of inField) if (!held.has(c.n)) held.set(c.n, c);
  // a chip pasted twice in one paste stays one chip
  const placed = new Map<string, number>();
  const numberOf = (c: Chip): number => {
    const whole = keyOf(c, true);
    const known = placed.get(whole);
    if (known !== undefined) return known;
    const there = held.get(c.n);
    let n: number;
    if (there === undefined || keyOf(there, true) === whole) n = c.n;
    else {
      const data = keyOf(c, false);
      n = [...held.values()].find((h) => keyOf(h, false) === data)?.n ?? 1 + Math.max(0, ...held.keys());
    }
    placed.set(whole, n);
    if (!held.has(n)) held.set(n, { ...c, n });
    return n;
  };
  return pasted.map((s) => (s.kind === 'chip' ? { kind: 'chip', chip: { ...s.chip, n: numberOf(s.chip) } as Chip } : s));
}
