// The sections' split, order, route and commands (sections.ts, opened.ts, Menus keepRows; page
// artifacts-signal v3, designer m_60689): the hub's section read as sent, never re-decided.
import { describe, expect, it, vi } from 'vitest';
import type { Artifact } from '@/proto';
import { library } from '@/proto/words_library.gen';
import { cardsOf, lineOf, rowsOf } from './gallery';
import { keepRows } from './Menus';
import type { Item } from './model';
import { keepCmds, sayOpened } from './opened';
import { sectionHeads } from './SectionRows';
import { byTouched, groupsOf, itemsIn, keptByHim, sectionCounts, sectionOf, sectionPath, splitSectionPath } from './sections';

const art = (id: string, extra: Partial<Artifact> = {}): Artifact => ({ id, kind: 'doc', title: id, agent: 'perf', version: 1, at_ms: 10, url: '', new: false, ...extra });
const item = (a: Artifact, project = 'shop-1'): Item => ({ project, projectName: 'storefront', a });

describe('the sections', () => {
  const items = [
    item(art('plan', { section: 'open', touched_ms: 50 })),
    item(art('notebook', { section: 'kept', kept: { by: 'learnings', at_ms: 1 }, touched_ms: 20 })),
    item(art('deck', { section: 'kept', kept: { by: 'you', at_ms: 2 }, touched_ms: 70 })),
    item(art('shots', { section: 'agents', touched_ms: 90 })),
    item(art('old-plan', { section: 'done', touched_ms: 5 })),
    // an older hub's row: no section, no touch: open work at its version's time
    item(art('legacy', { at_ms: 60 })),
  ];

  it("reads the hub's section; an older hub's row is open work", () => {
    expect(sectionOf(art('x'))).toBe('open');
    expect(sectionOf(art('x', { section: 'unknown' }))).toBe('open');
    expect(sectionCounts(items)).toEqual({ kept: 2, open: 2, done: 1, agents: 1 });
  });

  it('For you is Kept then Open work, each newest touch first; Done and Between agents one list each; a search keeps them all', () => {
    expect(groupsOf(items, 'you').map((g) => [g.id, g.items.map((it) => it.a.id)])).toEqual([
      ['kept', ['deck', 'notebook']],
      ['open', ['legacy', 'plan']],
    ]);
    expect(itemsIn(items, 'done').map((it) => it.a.id)).toEqual(['old-plan']);
    expect(itemsIn(items, 'agents').map((it) => it.a.id)).toEqual(['shots']);
    expect(groupsOf(items, 'all').map((g) => g.id)).toEqual(['kept', 'open', 'done', 'agents']);
    // an empty section is left out
    expect(groupsOf(items.filter((it) => it.a.section !== 'kept'), 'you').map((g) => g.id)).toEqual(['open']);
    // ties: by id, one order for every client
    expect([item(art('b', { touched_ms: 1 })), item(art('a', { touched_ms: 1 }))].sort(byTouched).map((it) => it.a.id)).toEqual(['a', 'b']);
  });

  it("the grid's heads say each section and what it shows; a stack never spans two sections", () => {
    const you = itemsIn(items, 'you');
    const rows = rowsOf(cardsOf(you), 4, 0, sectionHeads(sectionCounts(you)));
    expect(rows.flatMap((r) => (r.kind === 'day' ? [r.title] : []))).toEqual([`${library.sect_kept} · 2`, `${library.sect_open} · 2`]);
    const turn = { turn: 7, agent: 'ui-review' };
    const shots = [item(art('shot 1', { ...turn, section: 'kept', kept: { by: 'you', at_ms: 1 } })), item(art('shot 2', { ...turn, section: 'agents' })), item(art('shot 3', { ...turn, section: 'agents' }))];
    expect(cardsOf(shots).map((c) => (c.kind === 'stack' ? c.items.length : 1))).toEqual([1, 2]);
  });

  it("◆ is his in ink, an agent's names it on the card's line", () => {
    expect(keptByHim(art('x', { kept: { by: 'you', at_ms: 1 } }))).toBe(true);
    expect(keptByHim(art('x', { kept: { by: 'perf', at_ms: 1 } }))).toBe(false);
    const c = cardsOf([item(art('x', { kept: { by: 'learnings', at_ms: 1 } }))])[0];
    expect(c && lineOf(c, { kind: 'search' }, 10)).toMatch(new RegExp(`^${library.kept_by({ agent: 'learnings' })} · `));
  });

  it('Done and Between agents are route steps in any place, and back', () => {
    for (const [view, place, path] of [['you', '', ''], ['you', 'f/f-1', 'f/f-1'], ['done', '', 'done'], ['agents', 'f/f-1', 'agents/f/f-1'], ['done', 'unfiled', 'done/unfiled']] as const) {
      expect(sectionPath(view, place)).toBe(path);
      expect(splitSectionPath(path)).toEqual({ view, rest: place });
    }
  });

  it('Keep / Let go: one command per project; the menu says Let go only when every one is kept', () => {
    const picked = [item(art('a')), item(art('b'), 'acme-2'), item(art('c'))];
    expect(keepCmds(picked, true)).toEqual([
      { cmd: 'artifacts_keep', project: 'shop-1', ids: ['a', 'c'], keep: true },
      { cmd: 'artifacts_keep', project: 'acme-2', ids: ['b'], keep: true },
    ]);
    expect(keepRows(false)).toEqual([{ id: 'keep', name: library.keep }]);
    expect(keepRows(true)).toEqual([{ id: 'let_go', name: library.let_go }]);
  });

  it('an open tells its hub, with the version he sees when the list knows it', () => {
    const command = vi.fn();
    const store = { artifacts: () => [art('deck', { version: 3 })], command };
    sayOpened(store, 'shop-1', 'deck');
    sayOpened(store, 'shop-1', 'gone');
    expect(command.mock.calls).toEqual([[{ cmd: 'artifacts_opened', project: 'shop-1', id: 'deck', v: 3 }], [{ cmd: 'artifacts_opened', project: 'shop-1', id: 'gone' }]]);
  });
});
