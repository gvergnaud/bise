// The gallery's app tabs and the app: token (docs/plugin-apps.md A6b, his pick A on page
// artifacts-signal v8): one more dimension of the filter bar (filters.ts), the tabs its facet
// (apps.ts appTabsOf), the toolbar's two folding steps (Toolbar.tsx nextStage).
import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import { common } from '@/proto/words_common.gen';
import { library } from '@/proto/words_library.gen';
import { appTabsOf } from './apps';
import { appKeyOf, appOptions, applyFilters, facetOf, parseFilters, withToken, type Ctx } from './filters';
import type { Item } from './model';
import { nextStage } from './Toolbar';

const ctx: Ctx = { now: 1_794_405_600_000, tz: () => 0 };
const opener = (plugin: string, app: string) => ({ plugin, tool: 'open', app, type: '*.x', type_title: 'X' });

function item(id: string, over: Partial<Artifact> = {}, agent = 'perf'): Item {
  return { project: 'p', projectName: 'p', a: { id, kind: 'file', title: id, agent, by: agent, new: false, version: 1, at_ms: ctx.now, touched_ms: ctx.now, url: '', ...over } };
}

const items: Item[] = [
  item('q3', { opener: opener('sheets', 'Sheets'), path: '/w/q3.xlsx' }),
  item('budget', { opener: opener('sheets', 'Sheets'), path: '/w/budget.xlsx' }, 'main'),
  item('deck', { opener: opener('media', 'Preview'), path: '/w/deck.pdf' }),
  item('logs', { path: '/w/logs.zip' }, 'main'),
  item('pr-1', { kind: 'pr', pr: { number: 1 } as never, url: 'https://github.com/a/b/pull/1' }),
];

const ids = (xs: readonly Item[]) => xs.map((x) => x.a.id);

describe('the app: token, one dimension of the bar', () => {
  it("keys an item by a stable key, never a word: the opener's plugin, a PR's review, his Mac (architect m_63490)", () => {
    expect(items.map(appKeyOf)).toEqual(['sheets', 'sheets', 'media', 'mac', 'prs']);
  });

  it('OR inside (two apps), AND across (by:), quoted values, any case; a word reads as its key', () => {
    expect(ids(applyFilters(items, parseFilters('app:sheets'), ctx))).toEqual(['q3', 'budget']);
    expect(ids(applyFilters(items, parseFilters('app:Sheets app:preview'), ctx))).toEqual(['q3', 'budget', 'deck']);
    expect(ids(applyFilters(items, parseFilters('app:media'), ctx))).toEqual(['deck']);
    expect(ids(applyFilters(items, parseFilters('app:sheets by:main'), ctx))).toEqual(['budget']);
    expect(ids(applyFilters(items, parseFilters(`app:"${common.your_mac}"`), ctx))).toEqual(['logs']);
    expect(ids(applyFilters(items, parseFilters('app:mac'), ctx))).toEqual(['logs']);
    expect(ids(applyFilters(items, parseFilters(`app:"${library.kind_prs}"`), ctx))).toEqual(['pr-1']);
  });

  it('faceted: the app counts apply every other filter, never the app one', () => {
    const f = parseFilters('app:sheets by:main');
    expect(facetOf(items, f, ctx, 'app')).toEqual([{ key: 'mac', n: 1 }, { key: 'sheets', n: 1 }]);
  });

  it('a tab writes the token: a click alone, ⌘-click toggles, All takes it out', () => {
    expect(withToken('', 'app', 'sheets')).toBe('app:sheets');
    expect(withToken('app:sheets', 'app', 'preview', 'toggle')).toBe('app:sheets app:preview');
    expect(withToken('app:sheets app:preview pricing', 'app', 'sheets', 'toggle')).toBe('app:preview pricing');
    expect(withToken('app:sheets', 'app', 'pull requests')).toBe('app:"pull requests"');
    expect(withToken('in:acme app:sheets', 'app', null)).toBe('in:acme');
  });

  it('completes app: from the apps in scope, what he typed first, the most first', () => {
    expect(appOptions(items, parseFilters(''), ctx, 'p').map((o) => o.word)).toEqual(['Preview', library.kind_prs]);
    expect(appOptions(items, parseFilters(''), ctx, '').map((o) => [o.key, o.n])[0]).toEqual(['sheets', 2]);
    // by:main narrows the counts (AND across)
    expect(appOptions(items, parseFilters('by:main'), ctx, 's').map((o) => [o.word, o.n])).toEqual([['Sheets', 1]]);
  });
});

describe('the tabs', () => {
  it("one per app, the most first, by the token's keys; a picked app at 0 stays so he can take it off", () => {
    const { tabs } = appTabsOf(items.map((x) => x.a), new Set(['writer']), false);
    expect(tabs.map((t) => [t.id, t.word, t.n])).toEqual([
      ['sheets', 'Sheets', 2],
      ['media', 'Preview', 1],
      ['prs', library.kind_prs, 1],
      ['mac', common.your_mac, 1],
      ['writer', 'writer', 0],
    ]);
  });

  it('Pull requests keep their tab past five, and show at 0 when archived ones are one click away', () => {
    const many = ['A', 'B', 'C', 'D', 'E', 'F'].flatMap((w, i) => Array.from({ length: 6 - i }, (_, k) => item(`${w}${String(k)}`, { opener: opener(w, w) }).a));
    const { tabs, more } = appTabsOf([...many, items[4]?.a as Artifact], new Set(), false);
    expect(tabs.map((t) => t.word)).toEqual(['A', 'B', 'C', 'D', library.kind_prs]);
    expect(more.map((t) => t.word)).toEqual(['E', 'F']);
    expect(appTabsOf([items[0]?.a as Artifact], new Set(), true).tabs.map((t) => [t.word, t.n])).toEqual([['Sheets', 1], [library.kind_prs, 0]]);
  });
});

describe("the toolbar folds in two steps: the tabs into 'All ▾', then every filter into one menu that says its app ('All apps ▾', designer m_63720)", () => {
  it('folds one step while too narrow, unfolds one step once the less folded one fits', () => {
    expect(nextStage(0, 900, [800, 0, 0])).toBe(0);
    expect(nextStage(0, 700, [800, 0, 0])).toBe(1);
    expect(nextStage(1, 700, [800, 600, 0])).toBe(1);
    expect(nextStage(1, 500, [800, 600, 0])).toBe(2);
    expect(nextStage(2, 650, [800, 600, 400])).toBe(1);
    expect(nextStage(1, 820, [800, 600, 400])).toBe(0);
    // not measured yet: stays
    expect(nextStage(0, 0, [0, 0, 0])).toBe(0);
  });
});
