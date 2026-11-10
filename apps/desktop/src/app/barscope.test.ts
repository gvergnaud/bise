import { describe, expect, it } from 'vitest';
import { barScope, inScope, scopeCountTip, scopeWords } from './barscope';

describe("the right bar's scope (main m_25024, designer m_25031)", () => {
  it("bise's thread: every project, no filter, no words", () => {
    const s = barScope({ screen: 'bise' }, '');
    expect(s).toEqual({ level: 'bise' });
    expect(inScope(s, 'perf')).toBe(true);
    expect(scopeWords(s, 'artifacts', 'bise')).toBeNull();
  });
  it("a project's main, or a project screen that is no agent's: the project's, no words", () => {
    for (const r of [
      { screen: 'thread', project: 'shop', agent: 'main' },
      { screen: 'agents', project: 'shop' },
      { screen: 'artifacts', project: 'shop' },
    ] as const) {
      const s = barScope(r, 'shop');
      expect(s).toEqual({ level: 'project' });
      expect(inScope(s, 'perf')).toBe(true);
      expect(scopeWords(s, 'scheduled', 'storefront')).toBeNull();
    }
  });
  it("a sub-agent's thread or log: only its own, 'by perf' / 'for perf', their tips and lines at 0", () => {
    for (const screen of ['thread', 'debug'] as const) {
      const s = barScope({ screen, project: 'shop', agent: 'perf' }, 'shop');
      expect(s).toEqual({ level: 'agent', agent: 'perf' });
      expect(inScope(s, 'perf')).toBe(true);
      expect(inScope(s, 'main')).toBe(false);
    }
    const s = barScope({ screen: 'thread', project: 'shop', agent: 'perf' }, 'shop');
    expect(scopeWords(s, 'artifacts', 'storefront')).toEqual({ words: 'by perf', tip: "only what perf made · all › shows storefront's", empty: 'perf made nothing yet' });
    expect(scopeWords(s, 'scheduled', 'storefront')).toEqual({
      words: 'for perf',
      tip: "only perf's scheduled tasks and alarms · all › shows storefront's",
      empty: 'nothing wakes perf',
    });
  });
  it("a folded scoped head's count says the scope (designer m_26425)", () => {
    const s = barScope({ screen: 'thread', project: 'shop', agent: 'perf' }, 'shop');
    const art = scopeWords(s, 'artifacts', 'storefront');
    const sch = scopeWords(s, 'scheduled', 'storefront');
    if (art === null || sch === null) throw new Error('scoped');
    expect(scopeCountTip('scheduled', 0, sch)).toBe('0 scheduled for perf');
    expect(scopeCountTip('artifacts', 1, art)).toBe('1 artifact by perf');
    expect(scopeCountTip('artifacts', 3, art)).toBe('3 artifacts by perf');
  });
  it("on bise's thread an agent's route never scopes the bar (the bar is every project's)", () => {
    expect(barScope({ screen: 'thread', project: 'home', agent: 'perf' }, '')).toEqual({ level: 'bise' });
  });
});
