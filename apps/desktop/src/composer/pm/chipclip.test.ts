// @vitest-environment happy-dom
// Chips on the clipboard (chipclip.ts, his m_30833): a copy in the field carries its chips, a paste
// brings them back with their attachment rows; copy then paste over the same selection, and cut then
// paste where it was, leave the field as it was. On real editor states and the field's own handlers.
import { describe, expect, it } from 'vitest';
import { getSchema } from '@tiptap/core';
import { AllSelection, EditorState, TextSelection, type Transaction } from '@tiptap/pm/state';
import { history } from '@tiptap/pm/history';
import type { Node as PmNode } from '@tiptap/pm/model';
import type { EditorView } from '@tiptap/pm/view';
import { copyFrom, pasteInto, type PasteCtx } from './paste';
import { FIELD_NODES } from './schema';
import { chipsOf, nodeType, plainText, segmentsOf } from './pmtext';
import { CLIP_MIME, placeChips, readClip } from './chipclip';
import { buildMessage, type Chip, type Segment } from '../message';

const schema = getSchema(FIELD_NODES);
/** A newline. */
const NL = String.fromCharCode(10);

const CHIPS: Record<Chip['kind'], Chip> = {
  image: { kind: 'image', n: 1, path: '/s/images/ab.png', name: 'shot.png', thumb: 'data:image/png;base64,AA', w: 1200, h: 630, bytes: 84000 },
  paste: { kind: 'paste', n: 2, text: `line${NL}`.repeat(14) },
  quote: { kind: 'quote', n: 3, text: 'the hub restarts', from: 'main', at: { file: 'src/a.rs', new: '12-14', old: '' } },
  file: { kind: 'file', n: 4, path: '/Users/g/deck.pptx', bytes: 9, icon: 'data:image/png;base64,BB' },
  artifact: { kind: 'artifact', n: 5, id: 'a1', title: 'plan', url: 'https://bise.dev/p/plan' },
  anchor: { kind: 'anchor', n: 6, artifact: 'q3.xlsx', version: 4, anchor: { kind: 'cell', sheet: 'Summary', range: 'C4:E4' }, quote: '12', file: 'q3.xlsx' },
  plugin: { kind: 'plugin', n: 7, name: 'computer' },
};
const IMAGE_PATH = '/s/images/ab.png';

/** A clipboard as Chromium's: types set by the copy, read by the paste. */
class Clip {
  readonly map = new Map<string, string>();
  readonly files: File[] = [];
  setData(t: string, v: string): void {
    this.map.set(t, v);
  }
  getData(t: string): string {
    return this.map.get(t) ?? '';
  }
  clearData(): void {
    this.map.clear();
  }
}
const event = (type: string, data: Clip): ClipboardEvent => ({ type, clipboardData: data, preventDefault: () => undefined }) as unknown as ClipboardEvent;

const inline = (p: string | Chip): PmNode[] => {
  if (typeof p !== 'string') return [nodeType(schema, 'chip').create({ chip: p })];
  return p.split(NL).flatMap((l, i) => [...(i > 0 ? [nodeType(schema, 'hardBreak').create()] : []), ...(l === '' ? [] : [schema.text(l)])]);
};

/** A field holding `parts` (a chip as an object), [from, to) of its plain text selected. */
function field(parts: readonly (string | Chip)[], from = 0, to = from) {
  const doc = nodeType(schema, 'doc').create(null, nodeType(schema, 'paragraph').create(null, parts.flatMap(inline)));
  let st = EditorState.create({ doc, plugins: [history()] });
  const dispatch = (tr: Transaction | null): boolean => {
    if (tr !== null) st = st.apply(tr);
    return true;
  };
  const select = (a: number, b: number): void => {
    dispatch(st.tr.setSelection(TextSelection.create(st.doc, a + 1, b + 1)));
  };
  select(from, to);
  const ctx: PasteCtx = { sources: () => ({}) as never, state: () => st, dispatch, view: () => null };
  const view = (): EditorView =>
    ({
      get state() {
        return st;
      },
      dispatch,
    }) as unknown as EditorView;
  return {
    get: () => st,
    segs: (): Segment[] => segmentsOf(st.doc),
    size: () => plainText(st.doc).length,
    select,
    selectAll: (): void => {
      dispatch(st.tr.setSelection(new AllSelection(st.doc)));
    },
    copy: (c: Clip, type: 'copy' | 'cut' = 'copy'): boolean => copyFrom(view(), event(type, c)),
    paste: (c: Clip): boolean => pasteInto(ctx, event('paste', c)),
  };
}

const KINDS = Object.keys(CHIPS) as Chip['kind'][];
const all = (): (string | Chip)[] => ['see ', CHIPS.image, ' and ', CHIPS.paste, `${NL}then `, CHIPS.quote, ' ', CHIPS.file, ' ', CHIPS.artifact, ' ', CHIPS.anchor, ' ok'];

describe('a copy with chips, pasted (his m_30833)', () => {
  it.each(KINDS)("a %s chip comes back whole in another thread's empty field, with its row", (kind) => {
    const a = field(['look ', CHIPS[kind], ' here']);
    a.select(0, a.size());
    const clip = new Clip();
    expect(a.copy(clip)).toBe(true);
    const b = field([]);
    expect(b.paste(clip)).toBe(true);
    expect(b.segs()).toEqual(a.segs());
    expect(chipsOf(b.get().doc)).toEqual([CHIPS[kind]]);
  });

  it("another app reads plain text: the words with the chips' labels", () => {
    const a = field(all());
    a.select(0, a.size());
    const clip = new Clip();
    a.copy(clip);
    expect(clip.getData('text/plain')).toBe(`see [Image #1] and ▤ 2${NL}then ❝ 3 [File #4] ↗ plan ❝ 6 ok`);
    expect(clip.getData(CLIP_MIME)).not.toBe('');
  });

  it('words alone: plain text only, no private type', () => {
    const a = field(['just words']);
    a.select(0, 4);
    const clip = new Clip();
    a.copy(clip);
    expect([...clip.map.keys()]).toEqual(['text/plain']);
  });

  it('the no-op law: copy then paste over the same selection, cut then paste where it was, for every selection', () => {
    const base = all();
    const n = field(base).size();
    for (let from = 0; from <= n; from += 1)
      for (let to = from + 1; to <= n; to += 1) {
        const before = field(base, from, to);
        const want = before.segs();
        const clip = new Clip();
        before.copy(clip);
        before.paste(clip);
        expect(before.segs(), `copy+paste ${String(from)}..${String(to)}`).toEqual(want);
        const cut = field(base, from, to);
        cut.copy(clip, 'cut');
        expect(cut.size()).toBe(n - (to - from));
        cut.paste(clip);
        expect(cut.segs(), `cut+paste ${String(from)}..${String(to)}`).toEqual(want);
      }
  });

  it('⌘A (the whole document selected), copy, paste: the field as before', () => {
    const f = field(all());
    const want = f.segs();
    f.selectAll();
    const clip = new Clip();
    f.copy(clip);
    f.paste(clip);
    expect(f.segs()).toEqual(want);
    f.selectAll();
    f.copy(clip, 'cut');
    expect(f.segs()).toEqual([]);
    f.paste(clip);
    expect(f.segs()).toEqual(want);
  });

  it('select all, cut, paste: the field and its attachment list as before', () => {
    const f = field(all());
    const want = { segs: f.segs(), chips: chipsOf(f.get().doc) };
    f.select(0, f.size());
    const clip = new Clip();
    f.copy(clip, 'cut');
    expect(f.segs()).toEqual([]);
    f.paste(clip);
    expect({ segs: f.segs(), chips: chipsOf(f.get().doc) }).toEqual(want);
  });

  it('pasted again in the same draft: the same chip, one number, one part sent', () => {
    const f = field(['see ', CHIPS.image, ' ']);
    f.select(4, 5);
    const clip = new Clip();
    f.copy(clip);
    f.select(f.size(), f.size());
    f.paste(clip);
    expect(chipsOf(f.get().doc).map((c) => c.n)).toEqual([1, 1]);
    expect(buildMessage(f.segs()).parts).toEqual([
      { kind: 'text', text: 'see ' },
      { kind: 'image', name: '[Image #1]', source: IMAGE_PATH },
      { kind: 'text', text: ' [Image #1]' },
    ]);
  });

  it('into a draft whose numbers are taken by other chips: the next free ones, never two chips on one number', () => {
    const a = field([CHIPS.image, ' ', CHIPS.paste]);
    a.select(0, a.size());
    const clip = new Clip();
    a.copy(clip);
    const other: Chip = { kind: 'image', n: 1, path: '/s/images/cd.png', name: 'other.png' };
    const b = field([other, ' ']);
    b.select(2, 2);
    b.paste(clip);
    expect(chipsOf(b.get().doc).map((c) => [c.kind, c.n])).toEqual([
      ['image', 1],
      ['image', 2],
      ['paste', 3],
    ]);
  });

  it('a stale private type (the plain text changed since) is never trusted: the text goes in', () => {
    const a = field(['x ', CHIPS.quote]);
    a.select(0, a.size());
    const clip = new Clip();
    a.copy(clip);
    clip.setData('text/plain', 'something else');
    const b = field([]);
    b.paste(clip);
    expect(b.segs()).toEqual([{ kind: 'text', text: 'something else' }]);
    expect(readClip('{not json', 'x')).toBeNull();
  });
});

describe('placeChips (the numbers)', () => {
  const img = CHIPS.image;
  const seg = (c: Chip): Segment => ({ kind: 'chip', chip: c });
  const ns = (s: Segment[]): number[] => s.flatMap((x) => (x.kind === 'chip' ? [x.chip.n] : []));
  it('keeps a free number, and the number of the same chip already there', () => {
    expect(ns(placeChips([seg(img)], []))).toEqual([1]);
    expect(ns(placeChips([seg(img)], [img]))).toEqual([1]);
  });
  it('the same chip under another number takes that number; a different one the next free', () => {
    expect(ns(placeChips([seg(img)], [{ ...CHIPS.quote, n: 1 }, { ...img, n: 7 }]))).toEqual([7]);
    expect(ns(placeChips([seg(img), seg(img)], [{ ...CHIPS.quote, n: 1 }, { ...CHIPS.file, n: 4 }]))).toEqual([5, 5]);
  });
});
