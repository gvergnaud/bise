import { describe, expect, it } from 'vitest';
import { asksBise, flatHits, marks, matchScore, nextScope, searchList, sentenceNames, wordScore, type SearchData } from './rank';

const NOW = Date.UTC(2026, 9, 14, 14, 30);
const MIN = 60_000;
const D: SearchData = {
  projects: [
    { id: 'home', name: 'home', home: true },
    { id: 'sf', name: 'storefront' },
    { id: 'docs', name: 'docs' },
  ],
  agents: [
    { project: 'home', name: 'main', main: true, archived: false, status: 'idle', title: '', waits: 0, since_ms: NOW - 90 * MIN },
    { project: 'sf', name: 'main', main: true, archived: false, status: 'idle', title: '', waits: 0, since_ms: NOW - 3 * MIN, atWork: 2 },
    { project: 'sf', name: 'checkout', main: false, archived: false, status: 'working', title: 'apple pay on checkout', waits: 0, since_ms: NOW - 6 * MIN },
    { project: 'sf', name: 'checkout-v1', main: false, archived: true, status: 'done', title: 'first stripe flow', waits: 0, since_ms: NOW - 3 * 1440 * MIN },
    { project: 'sf', name: 'search', main: false, archived: false, status: 'blocked', title: 'search index', waits: 1, since_ms: NOW - 2 * MIN, ask: 'rebuild the search index tonight?' },
    { project: 'docs', name: 'check-links', main: false, archived: false, status: 'done', title: 'dead links in the docs', waits: 0, since_ms: NOW - 1440 * MIN },
    { project: 'sf', name: 'perf', main: false, archived: false, status: 'working', title: 'product page speed', waits: 0, since_ms: NOW - 14 * MIN },
  ],
  artifacts: [
    { project: 'sf', id: 'shots', title: 'checkout on mobile', kind: 'image', what: '2 shots', agent: 'checkout', at_ms: NOW - 200 * MIN },
    { project: 'sf', id: 'pp', title: 'product page, before / after', kind: 'page', what: 'page · v2', agent: 'perf', at_ms: NOW - 28 * MIN },
  ],
  scheduled: [{ project: 'sf', id: 48, agent: 'checkout', words: 're-run the flaky checkout test', every: 'every 30 min', at_ms: NOW - 60 * MIN }],
};

describe('search: one ranking for ⌘K and the spotlight', () => {
  it('a word scores by how it matches: whole, start, a word’s start, inside, else nothing', () => {
    expect(wordScore('perf', 'perf')).toBeGreaterThan(wordScore('per', 'perf'));
    expect(wordScore('per', 'perf')).toBeGreaterThan(wordScore('pay', 'apple pay'));
    expect(wordScore('pay', 'apple pay')).toBeGreaterThan(wordScore('ppl', 'apple'));
    expect(wordScore('xyz', 'apple')).toBe(0);
    // every word must match a field; the name counts more than what it's about, which counts more than its project
    expect(matchScore('check pay', { name: ['checkout'], about: ['apple pay'] })).toBeGreaterThan(0);
    expect(matchScore('check zzz', { name: ['checkout'], about: ['apple pay'] })).toBe(-1);
    expect(matchScore('sto', { name: ['storefront'] })).toBeGreaterThan(matchScore('sto', { name: ['x'], about: ['storefront'] }));
    expect(matchScore('sto', { name: ['x'], about: ['storefront'] })).toBeGreaterThan(matchScore('sto', { name: ['x'], where: ['storefront'] }));
  });

  it("'che': agents (archived last), artifacts, scheduled, each a group; the name's match is marked", () => {
    const l = searchList(D, 'che', 'all', NOW, 'UTC');
    expect(l.mode).toBe('found');
    expect(l.groups.map((g) => g.id)).toEqual(['agents', 'artifacts', 'scheduled']);
    expect(l.groups[0]?.hits.map((h) => h.name)).toEqual(['checkout', 'check-links', 'checkout-v1']);
    const v1 = l.groups[0]?.hits[2];
    expect(v1?.archived).toBe(true);
    expect(v1?.glyph).toBe('–');
    expect(v1?.detail).toBe('archived · first stripe flow');
    expect(v1?.when).toBe('3 d');
    expect(l.groups[0]?.hits[0]).toMatchObject({ glyph: '∿', tone: 'ink', detail: 'apple pay on checkout', where: 'storefront', when: 'working · 6 min' });
    expect(l.groups[1]?.hits[0]).toMatchObject({ glyph: '◫', name: 'checkout on mobile', detail: '2 shots', where: 'checkout · storefront' });
    expect(l.groups[2]?.hits[0]).toMatchObject({ glyph: '◷', where: 'checkout · storefront', when: 'every 30 min' });
    expect(marks('checkout-v1', 'che')).toEqual([[0, 3]]);
    expect(marks('#471 checkout: apple pay flag', 'che pay')).toEqual([
      [5, 8],
      [21, 24],
    ]);
  });

  it('an archived agent is found by its old title too, after every live one', () => {
    const l = searchList(D, 'stripe', 'all', NOW, 'UTC');
    expect(flatHits(l).map((h) => h.name)).toEqual(['checkout-v1']);
    expect(flatHits(l)[0]?.open).toEqual({ to: 'thread', project: 'sf', agent: 'checkout-v1' });
  });

  it('a scope keeps one kind; tab cycles all · agents · artifacts · scheduled', () => {
    expect(searchList(D, 'che', 'agents', NOW).groups.map((g) => g.id)).toEqual(['agents']);
    expect(searchList(D, 'che', 'scheduled', NOW).groups.map((g) => g.id)).toEqual(['scheduled']);
    expect(nextScope('all')).toBe('agents');
    expect(nextScope('scheduled')).toBe('all');
    expect(nextScope('all', true)).toBe('scheduled');
  });

  it('no query: waiting for you, then recent mixed by recency, archived last', () => {
    const l = searchList(D, '', 'all', NOW, 'UTC');
    expect(l.mode).toBe('start');
    expect(l.groups.map((g) => g.label)).toEqual(['waiting for you', 'recent']);
    expect(l.groups[0]?.hits).toMatchObject([{ name: 'search', glyph: '?', tone: 'acc', detail: 'rebuild the search index tonight?', when: '2 min', waiting: true }]);
    const recent = l.groups[1]?.hits.map((h) => h.name) ?? [];
    expect(recent.slice(0, 4)).toEqual(['bise · storefront', 'checkout', 'perf', 'product page, before / after']);
    expect(recent.at(-1)).toBe('checkout-v1');
    expect(l.groups[1]?.hits[0]).toMatchObject({ tone: 'pearl', when: '2 agents at work', where: '' });
  });

  it("a bise waiting on him has the pink pearl in 'waiting for you'; idle, the grey one (designer m_13854)", () => {
    const asks: SearchData = { ...D, agents: D.agents.map((a) => (a.main && a.project === 'home' ? { ...a, waits: 1, ask: 'which repo first?' } : a)) };
    const l = searchList(asks, '', 'all', NOW, 'UTC');
    const w = l.groups.find((g) => g.id === 'waiting')?.hits ?? [];
    expect(w.find((h) => h.name === 'bise')).toMatchObject({ tone: 'bise', glyph: '', detail: 'which repo first?' });
    expect(l.groups.find((g) => g.id === 'recent')?.hits.find((h) => h.name === 'bise · storefront')).toMatchObject({ tone: 'pearl' });
  });

  it('nothing matches: none; nothing to search (only the bise rows): blank', () => {
    expect(searchList(D, 'stripe webhook', 'all', NOW).mode).toBe('none');
    const bare: SearchData = { projects: D.projects, agents: D.agents.filter((a) => a.main), artifacts: [], scheduled: [] };
    expect(searchList(bare, '', 'all', NOW).mode).toBe('blank');
  });
});

describe("what ⏎ does with a typed line: main's decision m_12973, the spotlight's and ⌘K's", () => {
  it('one or two words that name something: ⏎ opens; longer, or naming nothing: ⏎ asks bise', () => {
    expect(asksBise('search', true)).toBe(false);
    expect(asksBise('checkout v1', true)).toBe(false);
    expect(asksBise('stripe webhook', false)).toBe(true);
    expect(asksBise('search ne trouve pas tshirt', true)).toBe(true);
    expect(asksBise('   ', false)).toBe(false);
  });
  it("a sentence names an agent by its name, or by two of its title's 4+ letter words", () => {
    expect(sentenceNames('search ne trouve pas tshirt', { name: 'search', title: 'search index' })).toBe(true);
    expect(sentenceNames('why is amb-web slow', { name: 'amb-web', title: '' })).toBe(true);
    expect(sentenceNames('the product page loads slowly, check its speed', { name: 'perf', title: 'product page speed' })).toBe(true);
    // one long word of the title (twice) is not enough; nor a short one
    expect(sentenceNames('the page, the page again', { name: 'perf', title: 'product page speed' })).toBe(false);
    expect(sentenceNames('pay me', { name: 'checkout', title: 'apple pay on checkout' })).toBe(false);
  });
  it("'search' opens search; 'search ne trouve pas tshirt' asks bise, search one row down", () => {
    const word = searchList(D, 'search', 'all', NOW);
    expect(word.mode).toBe('found');
    expect(flatHits(word)[0]?.id).toBe('agent.sf.search');
    const sentence = searchList(D, 'search ne trouve pas tshirt', 'all', NOW);
    expect(sentence.mode).toBe('ask');
    expect(flatHits(sentence).map((h) => h.id)).toEqual(['agent.sf.search']);
    // a sentence that names nothing still asks, with no rows under it
    const nothing = searchList(D, 'why is everything so slow today', 'all', NOW);
    expect(nothing.mode).toBe('ask');
    expect(flatHits(nothing)).toEqual([]);
  });
  it('a sentence lists the archived agents it names too, after the live ones', () => {
    const l = searchList(D, 'redo the first stripe flow for checkout', 'all', NOW);
    expect(l.mode).toBe('ask');
    const ids = flatHits(l).map((h) => h.id);
    expect(ids).toContain('agent.sf.checkout-v1');
    expect(ids.indexOf('agent.sf.checkout')).toBeLessThan(ids.indexOf('agent.sf.checkout-v1'));
  });

  it('a keystroke over many artifacts makes no new date formatter per row (cmdk-perf: 0.5 ms each)', () => {
    // mutation: dayTime back to `new Intl.DateTimeFormat` per call turns this red (thousands made)
    const many: SearchData = { ...D, artifacts: Array.from({ length: 500 }, (_, k) => ({ project: 'sf', id: `a${String(k)}`, title: `report ${String(k)}`, kind: 'page', what: 'page', agent: 'perf', at_ms: NOW - k * 3_600_000 })) };
    const real = Intl.DateTimeFormat;
    let made = 0;
    Intl.DateTimeFormat = new Proxy(real, {
      construct(t, args: ConstructorParameters<typeof Intl.DateTimeFormat>) {
        made++;
        return new t(...args);
      },
    });
    try {
      for (const q of ['r', 're', 'rep']) expect(flatHits(searchList(many, q, 'artifacts', NOW, 'Europe/Paris')).length).toBe(500);
    } finally {
      Intl.DateTimeFormat = real;
    }
    expect(made).toBeLessThanOrEqual(4);
  });
});

describe('search: the tasks (tasks-center: results, the done ones too)', () => {
  const T: SearchData = {
    ...D,
    tasks: [
      { project: 'sf', id: 'checkout-flow', name: 'Checkout flow', status: 'needs_you', what: 'needs you · 1 question', at_ms: NOW - 5 * MIN },
      { project: 'sf', id: 'old-checkout', name: 'Old checkout', status: 'done', what: 'done', at_ms: NOW - 9 * 1440 * MIN },
    ],
  };
  it("'checkout': a tasks group after the agents, a done one after the open one, ⏎ opens its tab", () => {
    const l = searchList(T, 'checkout', 'all', NOW);
    const g = l.groups.map((x) => x.id);
    expect(g.indexOf('tasks')).toBe(g.indexOf('agents') + 1);
    const hits = l.groups.find((x) => x.id === 'tasks')?.hits ?? [];
    expect(hits.map((h) => h.name)).toEqual(['Checkout flow', 'Old checkout']);
    expect(hits[0]?.open).toEqual({ to: 'task', project: 'sf', id: 'checkout-flow' });
    // its sign is its status, drawn by the tasks' TaskMark: no text glyph, no tone of its own (tasks-shine batch 2)
    expect(hits.map((h) => [h.mark, h.glyph, h.tone])).toEqual([['needs_you', '', ''], ['done', '', '']]);
  });
  it('a scope other than all leaves them out', () => {
    expect(searchList(T, 'checkout', 'agents', NOW).groups.some((x) => x.id === 'tasks')).toBe(false);
  });
});
