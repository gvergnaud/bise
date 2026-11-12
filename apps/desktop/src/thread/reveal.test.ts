// msg-rail-fix (his m_63243 (4): "clicker dessus devrait scroll au bon endroit, ça me scroll dans un
// endroit random"): a tick's reveal in a long thread with images, on a laid-out fake of the list
// (rows of their real heights, Virtuoso's scrollToIndex on its estimate, the rows near the view
// drawn). While the list goes up, the older page comes in front (the target sits near the view's
// first rows: asksOlder) and images above it decode. The message itself ends at 16 px under the
// list's top and stays there.
// Mutations: revealStep finding its row by the index it had at the reveal's first frame (the old
// `firstIndex + i`) brings the row a page later to the top (red); done on the first frame at its
// place (no STILL_FRAMES) leaves it 260 px off once the image above decodes (red).
import { describe, expect, it } from 'vitest';
import { revealStep, STILL_FRAMES, type RevealFrame, type RevealMove } from './reveal';

const VIEW = { top: 100, bottom: 700 };
/** Virtuoso's size for a row it never measured. */
const EST = 80;
const START = 1_000_000;

/** A thread's list as laid out: keys, firstItemIndex, each row's height now, scrollTop. */
class FakeList {
  keys: string[];
  first: number;
  h = new Map<string, number>();
  scrollTop = 0;
  constructor(keys: string[], height: (k: string) => number) {
    this.keys = keys;
    this.first = START - keys.length;
    for (const k of keys) this.h.set(k, height(k));
    this.scrollTop = this.total() - (VIEW.bottom - VIEW.top);
  }
  total(): number {
    return this.keys.reduce((s, k) => s + (this.h.get(k) ?? 0), 0);
  }
  offset(j: number): number {
    return this.keys.slice(0, j).reduce((s, k) => s + (this.h.get(k) ?? 0), 0);
  }
  drawn(): number[] {
    const out: number[] = [];
    this.keys.forEach((k, j) => {
      const y = this.offset(j) - this.scrollTop;
      if (y + (this.h.get(k) ?? 0) > -600 && y < VIEW.bottom - VIEW.top + 600) out.push(this.first + j);
    });
    return out;
  }
  box(item: number): { top: number; bottom: number } | null {
    const j = item - this.first;
    const k = this.keys[j];
    if (k === undefined || !this.drawn().includes(item)) return null;
    const top = VIEW.top + this.offset(j) - this.scrollTop;
    return { top, bottom: top + (this.h.get(k) ?? 0) };
  }
  /** Virtuoso's scrollToIndex: by its estimate, never the real heights */
  scrollToIndex(at: number, offset: number): void {
    this.scrollTop = Math.max(0, at * EST + offset);
  }
  /** an older page in front: what was drawn stays where it was (Virtuoso's prepend) */
  prepend(keys: string[], height: (k: string) => number): void {
    for (const k of keys) this.h.set(k, height(k));
    const add = keys.reduce((s, k) => s + (this.h.get(k) ?? 0), 0);
    this.keys = [...keys, ...this.keys];
    this.first -= keys.length;
    this.scrollTop += add;
  }
}

const key = (pos: number): string => `e:${String(pos)}`;
/** his message every 10 entries; an answer with an image (200 px once decoded) every 7 */
const height = (k: string): number => (Number(k.slice(2)) % 10 === 0 ? 72 : Number(k.slice(2)) % 7 === 0 ? 40 : 96 + (Number(k.slice(2)) % 5) * 24);

describe('a reveal follows its row by key (msg-rail-fix)', () => {
  it("a tick's message ends 16 px under the list's top, through an older page in front and images decoding above it", () => {
    // the view holds entries 1000..1299; he clicks his message at 1030 (near the first rows)
    const list = new FakeList(Array.from({ length: 300 }, (_, i) => key(1000 + i)), height);
    const target = key(1030);
    const top = 16;
    let lastTop: number | undefined;
    let still = 0;
    let move: RevealMove = { kind: 'index', at: list.keys.indexOf(target), seek: 0 };
    list.scrollToIndex(list.keys.indexOf(target), -top);
    let frame = 0;
    for (; frame < 90; frame++) {
      // frame 3: the older page comes in front (the list neared its first rows)
      if (frame === 3) list.prepend(Array.from({ length: 60 }, (_, i) => key(940 + i)), height);
      // frames 4 to 12: the images of the answers above it decode, one a frame (40 -> 300 px)
      for (const k of list.keys) if (frame >= 4 && frame <= 12 && Number(k.slice(2)) % 7 === 0 && Number(k.slice(2)) >= 1000 + (12 - frame) && Number(k.slice(2)) < 1030 && (list.h.get(k) ?? 0) === 40) list.h.set(k, 300);
      const f: RevealFrame = { key: target, keys: list.keys, first: list.first, box: (i) => list.box(i), view: VIEW, drawn: list.drawn(), scrollTop: list.scrollTop, lastTop, still, top };
      move = revealStep(f);
      if (move.kind === 'done' || move.kind === 'gone') break;
      if (move.kind === 'index') list.scrollToIndex(move.at, -top);
      else {
        still = move.still;
        list.scrollTop += move.px;
      }
      lastTop = move.seek;
    }
    expect(move.kind).toBe('done');
    const at = list.box(list.first + list.keys.indexOf(target));
    expect(at).not.toBeNull();
    expect(Math.abs((at?.top ?? 0) - (VIEW.top + top))).toBeLessThan(2);
    // the row at the top is his message, not the one a page later or before
    const atTop = list.drawn().find((i) => (list.box(i)?.top ?? 0) >= VIEW.top + top - 2);
    expect(list.keys[(atTop ?? 0) - list.first]).toBe(target);
  });

  it('done only after the row sat at its place STILL_FRAMES frames', () => {
    const f = (still: number): RevealFrame => ({ key: 'e:1', keys: ['e:0', 'e:1'], first: 10, box: (i) => (i === 11 ? { top: VIEW.top + 16, bottom: VIEW.top + 80 } : null), view: VIEW, drawn: [10, 11], scrollTop: 0, lastTop: undefined, still, top: 16 });
    expect(revealStep(f(0))).toEqual({ kind: 'by', px: 0, still: 1 });
    expect(revealStep(f(STILL_FRAMES - 1))).toEqual({ kind: 'done' });
  });

  it('a row that left the list: nothing more', () => {
    expect(revealStep({ key: 'e:9', keys: ['e:1'], first: 0, box: () => null, view: VIEW, drawn: [], scrollTop: 0, lastTop: undefined, still: 0 }).kind).toBe('gone');
  });
});
