// A saved composer entry (a draft, a history or a queue entry; drafts.ts keeps them as strings),
// editor-neutral: `{"v":2,"segs":[…]}`, the content as message.ts's segments. Pure, no editor.
//
// An entry it doesn't read (the old Lexical engine's, whose reader went in issue 31): null, and the
// caller keeps its raw JSON (never dropped).
//
// A draft also keeps its dictation takes (dictation-keep): a `take` segment holds the place where
// that take's words go, with its error once it failed, so a take survives a chat switch and a
// relaunch (pm/dictation.ts, ./parked.ts). Never in a message or the history: segmentsOf leaves
// them out.
import type { Chip, Segment } from './message';

/** A dictation take in a draft: its id (the store's), its short error once it failed ('try again'). */
export interface TakeSeg {
  readonly kind: 'take';
  readonly id: string;
  readonly failed?: string;
}

/** A draft's content: a message's segments and its takes. */
export type DraftSeg = Segment | TakeSeg;

const V = 2;

/** A draft's message: its takes left out. */
export const messageOf = (segs: readonly DraftSeg[]): Segment[] => segs.filter((x): x is Segment => x.kind !== 'take');

/** The entry for these segments. */
export function entryOf(segs: readonly DraftSeg[]): string {
  return JSON.stringify({ v: V, segs: tidy(segs) });
}

/** Adjacent text segments as one, empty ones out (one shape for equal content). */
export function tidy<S extends DraftSeg>(segs: readonly S[]): S[] {
  const out: S[] = [];
  for (const s of segs) {
    const last = out.at(-1);
    if (s.kind === 'text') {
      if (s.text === '') continue;
      if (last?.kind === 'text') out[out.length - 1] = { kind: 'text', text: last.text + s.text } as S;
      else out.push(s);
    } else out.push(s);
  }
  return out;
}

const isObj = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x);

const isChip = (x: unknown): x is Chip =>
  isObj(x) && typeof x.n === 'number' && (x.kind === 'image' || x.kind === 'paste' || x.kind === 'quote' || x.kind === 'artifact' || x.kind === 'anchor' || x.kind === 'plugin');

function isSegment(x: unknown): x is DraftSeg {
  if (!isObj(x)) return false;
  if (x.kind === 'text') return typeof x.text === 'string';
  if (x.kind === 'take') return typeof x.id === 'string' && x.id !== '' && (x.failed === undefined || typeof x.failed === 'string');
  return x.kind === 'chip' && isChip(x.chip);
}

/** An entry's segments (a draft's takes too); null when it doesn't read (the caller keeps its raw JSON). */
export function entrySegments(entry: string): DraftSeg[] | null {
  let v: unknown;
  try {
    v = JSON.parse(entry);
  } catch {
    return null;
  }
  if (!isObj(v) || v.v !== V) return null;
  return Array.isArray(v.segs) && v.segs.every(isSegment) ? tidy(v.segs) : null;
}
