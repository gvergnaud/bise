// The filter bar's one rule (filters.ts) against the table the hub's filter reads too
// (rust/switchboard/src/artifacts_filter.rs, tests/fixtures/artifacts-filter.json): the two sides
// cannot drift. Then the field's grammar: tokens, withToken, facets, the by: completion.
import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import TABLE from '../../../../tests/fixtures/artifacts-filter.json';
import {
  applyFilters, cleared, completeAt, facetOf, parseFilters, tokenAt, waysOut, whenFacet, whoOptions, withToken, type Ctx, type Filters,
} from './filters';
import type { Item } from './model';

interface Row {
  id: string;
  title: string;
  agent: string;
  kind: string;
  by: string;
  new: boolean;
  touched_ms: number;
}
interface Case {
  name: string;
  now: number;
  offset_min: number;
  tz?: number[][];
  rows: Row[];
  filter: { since?: string; by?: string[]; words?: string[] };
  want: string[];
}

const item = (r: Row): Item => ({
  project: 'p',
  projectName: 'p',
  a: { id: r.id, kind: r.kind, title: r.title, agent: r.agent, by: r.by, new: r.new, touched_ms: r.touched_ms, version: 1, at_ms: r.touched_ms, url: '' } satisfies Artifact,
});

/** The typed filter of a case as the field would say it. */
const queryOf = (f: Case['filter']): string =>
  [...(f.since === undefined ? [] : [`since:${f.since}`]), ...(f.by ?? []).map((b) => `by:${b}`), ...(f.words ?? [])].join(' ');

/** The case's zone: its step table, else its one offset. */
const tzOf = (c: Case) => (ms: number): number => {
  const steps = c.tz ?? [[0, c.offset_min]];
  return [...steps].reverse().find((s) => ms >= (s[0] ?? 0))?.[1] ?? 0;
};

/** The words as the hub reads them (title, agent, kind, id); the gallery's hay adds the folder. */
const withWords = (items: Item[], f: Filters): Item[] =>
  items.filter((it) => f.words.every((w) => `${it.a.title} ${it.a.agent} ${it.a.kind} ${it.a.id}`.toLowerCase().includes(w)));

describe('the filter, one table for both sides', () => {
  for (const c of (TABLE as { cases: Case[] }).cases) {
    it(c.name, () => {
      const f = parseFilters(queryOf(c.filter));
      const ctx: Ctx = { now: c.now, tz: tzOf(c) };
      expect(withWords(applyFilters(c.rows.map(item), f, ctx), f).map((it) => it.a.id)).toEqual(c.want);
    });
  }
});

const ctx: Ctx = { now: 10 * 86_400_000, tz: () => 0 };
const row = (id: string, agent: string, by: string, day: number, isNew = false): Item =>
  item({ id, title: id, agent, kind: 'doc', by, new: isNew, touched_ms: day * 86_400_000 + 1 });
const items = [row('a', 'perf', 'perf', 10, true), row('b', 'perf', 'perf', 9), row('c', 'docs', 'docs', 1), row('d', 'main', 'you', 10)];

describe("the field's grammar", () => {
  it('reads in:, since:, by: (quoted values, any case); a value that means nothing stays a word', () => {
    const f = parseFilters('In:Acme since:TODAY by:@perf by:"Pull requests" pricing since:soon');
    expect(f.project).toBe('acme');
    expect([...(f.keys.get('since') ?? [])]).toEqual(['today']);
    expect([...(f.keys.get('by') ?? [])]).toEqual(['perf', 'pull requests']);
    expect(f.words).toEqual(['pricing', 'since:soon']);
  });

  it('withToken: a click sets one alone, ⌘-click adds or takes out, × takes all; the words stay', () => {
    expect(withToken('pricing', 'since', 'today')).toBe('since:today pricing');
    expect(withToken('since:today pricing', 'since', 'week')).toBe('since:week pricing');
    expect(withToken('by:perf x', 'by', 'docs', 'toggle')).toBe('by:perf by:docs x');
    expect(withToken('by:perf by:docs x', 'by', 'perf', 'toggle')).toBe('by:docs x');
    expect(withToken('in:acme by:perf by:docs x', 'by', null)).toBe('in:acme x');
    expect(withToken('', 'app', 'Pull requests')).toBe('app:"pull requests"');
  });

  it('Clear takes every filter and word out, never the project', () => {
    expect(cleared('in:acme since:today by:perf pricing')).toBe('in:acme');
  });

  it('AND across dimensions, OR inside one', () => {
    const f = parseFilters('since:today by:perf by:you');
    expect(applyFilters(items, f, ctx).map((it) => it.a.id)).toEqual(['a', 'd']);
  });

  it("a facet counts with every other dimension applied; a picked value at 0 stays so it can go", () => {
    const f = parseFilters('since:older by:main');
    expect(facetOf(items, f, ctx, 'by')).toEqual([{ key: 'docs', n: 1 }, { key: 'main', n: 0 }]);
    expect(whenFacet(items, parseFilters('by:perf'), ctx)).toEqual([
      { key: 'last', n: 1 }, { key: 'today', n: 1 }, { key: 'week', n: 2 }, { key: 'older', n: 0 },
    ]);
  });

  it('by: completes people first, then agents by their count in scope; typed narrows by prefix then substring', () => {
    expect(whoOptions(items, parseFilters(''), ctx, '').map((x) => x.key)).toEqual(['you', 'perf', 'docs']);
    expect(whoOptions(items, parseFilters(''), ctx, 'o').map((x) => x.key)).toEqual(['you', 'docs']);
  });

  it('the token at the caret, and its completion', () => {
    const q = 'pricing by:pe';
    const t = tokenAt(q, q.length);
    expect(t).toEqual({ key: 'by', typed: 'pe', start: 8 });
    expect(completeAt(q, 8, q.length, 'by', 'perf')).toBe('pricing by:perf ');
    expect(tokenAt('pricing', 7)).toBeNull();
  });

  it('the empty view shows only the ways out that find something', () => {
    expect(waysOut({ everywhere: 64, fromAgents: 3, filtered: true })).toEqual([{ kind: 'everywhere', n: 64 }, { kind: 'agents', n: 3 }, { kind: 'clear' }]);
    expect(waysOut({ everywhere: null, fromAgents: 0, filtered: false })).toEqual([]);
  });
});
