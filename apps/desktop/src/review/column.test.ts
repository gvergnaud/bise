import { describe, expect, it } from 'vitest';
import type { DiffFile, Guide } from '@/proto';
import type { Diff } from '@/store';
import { aboutTokens, fileKey, guidable, guideColumn, kindColumn, numberOf, shownCards, viewedCount, type Words } from './column';

const W: Words = { kind: { code: 'Code', tests: 'Tests', docs: 'Words and docs', generated: 'Generated' }, files: (n) => `${String(n)} files`, root: 'top level', rest: 'The rest' };

const hunk = (id: string, text: string) => ({ id, header: `@@ ${id} @@`, lines: [{ kind: 'add' as const, new: 1, text }] });
const file = (path: string, hunks: ReturnType<typeof hunk>[]): DiffFile => ({ path, status: 'modified', add: hunks.length, del: 0, hunks });

const diff: Diff = {
  ev: 'diff',
  project: 'acme-1a2b3c4d',
  agent: 'pr-access',
  base: 'main',
  files: [
    file('apps/desktop/src/agents/SidebarRow.tsx', [hunk('a1', 'row'), hunk('a2', 'row2')]),
    file('apps/desktop/src/agents/rowEnd.ts', [hunk('b1', 'end')]),
    file('apps/desktop/src/agents/sidebar.css', [hunk('c1', 'css')]),
    file('docs/desktop-bar.md', [hunk('d1', 'doc')]),
    file('apps/desktop/src/proto/words_side.gen.ts', [hunk('e1', 'gen')]),
  ],
  hash: '9f2c41d07a3be815',
};

const guide: Guide = {
  overview: 'the row end gets one owner',
  parts: [
    { area: 'Sidebar', title: "Give the row's end one owner", summary: 'rowEndOf lists it', hunks: ['b1', 'a2', 'a1'], weight: 'critical' },
    { area: 'Sidebar', title: 'Room for the pin', summary: 'flex', hunks: ['c1'], weight: 'context' },
  ],
  rest: ['d1', 'e1'],
};

describe('the guide as a column (page review-guided, pick B)', () => {
  it('its parts in order, each its hunks in reading order as runs of one file; then the rest; then Generated, folded', () => {
    const cols = guideColumn(diff, guide, ['generated'], new Set(), W);
    expect(cols.map((s) => [s.key, s.kind, s.label, s.title, s.weight, s.folded])).toEqual([
      ['part-1', 'part', 'Sidebar', "Give the row's end one owner", 'critical', false],
      ['part-2', 'part', 'Sidebar', 'Room for the pin', 'context', false],
      ['rest', 'rest', 'The rest', null, null, false],
      ['generated', 'generated', 'Generated', null, null, true],
    ]);
    // b1, then a2 and a1 (one run of SidebarRow.tsx, in the guide's order), never git's order
    const runs = cols[0]?.blocks[0]?.cards.map((c) => [c.path.split('/').pop(), c.lines.filter((l) => l.head === true).map((l) => l.text)]);
    expect(runs).toEqual([
      ['rowEnd.ts', ['@@ b1 @@']],
      ['SidebarRow.tsx', ['@@ a2 @@', '@@ a1 @@']],
    ]);
    // a context part's code is open: the weight is a label, never a fold (architect m_57259)
    expect(cols[1]?.blocks[0]?.cards[0]?.lines.length).toBeGreaterThan(0);
    expect(numberOf(cols, 'part-2')).toBe('02 / 02');
    expect(numberOf(cols, 'rest')).toBeNull();
    // every hunk drawn once, Generated's when he opens it
    const open = shownCards(guideColumn(diff, guide, [], new Set(), W));
    const heads = open.flatMap((c) => c.lines.filter((l) => l.head === true).map((l) => l.text));
    expect(heads.sort()).toEqual(['@@ a1 @@', '@@ a2 @@', '@@ b1 @@', '@@ c1 @@', '@@ d1 @@', '@@ e1 @@']);
  });

  it('a guide that names no hunk of a file still leaves nothing out: the hub put it in rest', () => {
    expect(guidable(diff)).toBe(true);
    expect(guidable({ ...diff, files: [{ path: 'a.ts', status: 'modified', add: 0, del: 0, hunks: [{ header: '@@', lines: [] }] }] })).toBe(false);
  });

  it('a viewed file folds its cards (his fold) and counts; a push that changes it un-views it', () => {
    const f = diff.files[1];
    if (f === undefined) throw new Error('no file');
    const viewed = new Set([fileKey(f)]);
    const cols = kindColumn(diff, [], viewed, W);
    const card = shownCards(cols).find((c) => c.path === f.path);
    expect(card?.viewed).toBe(true);
    expect(card?.lines).toEqual([]);
    expect(viewedCount(diff.files, viewed)).toBe(1);
    const pushed = { ...diff, files: diff.files.map((x) => (x.path === f.path ? { ...x, hunks: [hunk('b9', 'end2')] } : x)) };
    expect(viewedCount(pushed.files, viewed)).toBe(0);
  });

  it('By kind numbers the kinds; tokens read about 5k', () => {
    const cols = kindColumn(diff, ['generated'], new Set(), W);
    // Code, Words and docs, Generated (no test here)
    expect(cols.map((s) => numberOf(cols, s.key))).toEqual(['01 / 03', '02 / 03', '03 / 03']);
    expect(cols[0]?.blocks[0]?.folder).toBe('apps/desktop/src/agents');
    expect(aboutTokens(5200)).toBe('5k');
    expect(aboutTokens(800)).toBe('800');
  });
});
