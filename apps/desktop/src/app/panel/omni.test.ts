// ⌘T's omnibox, its ranking and its rows (omni.ts; page newtab-omnibox-flow, designer m_64584), on
// a fake web history and fake artifacts.
import { describe, expect, it } from 'vitest';
import type { Item } from '@/artifacts/model';
import type { WebHistory, WebPage } from '@/shared/api';
import { addressOf, ageWeight, completionOf, frecency, hostOf, HITS_MAX, looksLikeUrl, omniHits, omniRows, rowText, searchUrl, shownUrl, typedRow } from './omni';

const DAY = 86_400_000;
const NOW = 1_800_000_000_000;

const page = (url: string, ages: readonly number[], typed = false): WebPage => ({ url, n: ages.length, visits: ages.map((d) => ({ at: NOW - d * DAY, typed })) });
const web = (...pages: WebPage[]): WebHistory => ({ keep: true, pages });
const item = (id: string, title: string, ageDays: number, kind = 'page', agent = 'perf') =>
  ({ project: 'acme', projectName: 'acme', a: { id, title, kind, agent, at_ms: NOW - ageDays * DAY, version: 1, url: '', new: false } }) as unknown as Item;
const NONE = new Set<string>();
const ids = (hs: ReturnType<typeof omniHits>) => hs.map((h) => (h.kind === 'web' ? h.page.url : h.item.a.id));

describe('frecency (Chrome buckets)', () => {
  it('weighs each visit by its age, a typed one twice', () => {
    expect([0, 4, 5, 14, 20, 31, 60, 90, 120].map((d) => ageWeight(d * DAY))).toEqual([100, 100, 70, 70, 50, 50, 30, 30, 10]);
    expect(frecency(page('https://a.com/', [1, 10]).visits, NOW)).toBe(170);
    expect(frecency(page('https://a.com/', [1], true).visits, NOW)).toBe(200);
  });
});

describe('the shown URL', () => {
  it('drops the scheme, www. and a lone slash', () => {
    expect(shownUrl('https://www.github.com/')).toBe('github.com');
    expect(shownUrl('http://github.com/gvergnaud/bise')).toBe('github.com/gvergnaud/bise');
    expect(shownUrl('https://a.com/x/')).toBe('a.com/x/');
    expect(hostOf('https://www.github.com/gvergnaud?x=1')).toBe('github.com');
  });
});

describe('the hits: prefix, then word start, then inside; frecency within', () => {
  const gh = page('https://github.com/gvergnaud/bise', [1, 2]);
  const ghOld = page('https://github.com/old', [60]);
  const docs = page('https://docs.github.com/actions', [0, 0, 0]);
  const blog = page('https://blog.example.com/about-github', [0]);
  it('a prefix of the address beats a word start and an inside match, whatever their frecency', () => {
    expect(ids(omniHits('gith', web(docs, blog, ghOld, gh), [], NONE, NOW))).toEqual(['https://github.com/gvergnaud/bise', 'https://github.com/old', 'https://docs.github.com/actions', 'https://blog.example.com/about-github']);
  });
  it('www. and the scheme are aside, and match too', () => {
    const w = page('https://www.lemonde.fr/', [1]);
    for (const q of ['lemo', 'www.lem', 'https://www.lem']) expect(ids(omniHits(q, web(w), [], NONE, NOW)), q).toEqual(['https://www.lemonde.fr/']);
  });
  it('artifacts by title prefix, words of the title, kind or maker; mixed with pages', () => {
    const arts = [item('n', 'Perf notes', 1), item('b', 'bench perf', 50), item('d', 'Launch deck', 1, 'deck', 'perf')];
    const pf = page('https://perfetto.dev/', [0]);
    expect(ids(omniHits('perf', web(pf), arts, NONE, NOW))).toEqual(['https://perfetto.dev/', 'n', 'd', 'b']);
    expect(ids(omniHits('perf notes', web(pf), arts, NONE, NOW))).toEqual(['n']);
  });
  it('an artifact in Recent counts one more visit of today', () => {
    const arts = [item('a', 'Plan alpha', 1), item('b', 'Plan beta', 1)];
    expect(ids(omniHits('plan', web(), arts, new Set(['acme/b']), NOW))).toEqual(['b', 'a']);
  });
  it(`no words, no hit; ${String(HITS_MAX)} hits at most`, () => {
    expect(omniHits('  ', web(gh), [item('a', 'x', 1)], NONE, NOW)).toEqual([]);
    const many = Array.from({ length: 20 }, (_, i) => page(`https://site${String(i)}.com/`, [i]));
    expect(omniHits('site', web(...many), [], NONE, NOW)).toHaveLength(HITS_MAX);
  });
});

describe('the inline completion: a prefix match only', () => {
  const gh = page('https://github.com/gvergnaud/bise', [1]);
  const hits = omniHits('GitH', web(gh), [], NONE, NOW);
  it('his letters kept, the rest the hit’s', () => {
    expect(completionOf('GitH', hits[0])).toBe('GitHub.com/gvergnaud/bise');
    expect(completionOf('Perf', omniHits('Perf', web(), [item('n', 'perf notes', 1)], NONE, NOW)[0])).toBe('Perf notes');
  });
  it('none for a word start, a leading space, or no hit', () => {
    const docs = omniHits('github', web(page('https://docs.github.com/', [1])), [], NONE, NOW);
    expect(completionOf('github', docs[0])).toBeNull();
    expect(completionOf(' git', hits[0])).toBeNull();
    expect(completionOf('git', undefined)).toBeNull();
  });
});

describe('what he typed: an address or a Google search', () => {
  it('tells an address from a search', () => {
    for (const u of ['localhost:5173', 'https://bise.dev', 'bise.dev/docs', 'http://127.0.0.1:4000/x']) expect(looksLikeUrl(u), u).toBe(true);
    for (const q of ['perf notes', 'perf', '', 'launch deck v2']) expect(looksLikeUrl(q), q).toBe(false);
    expect(addressOf('bise.dev/docs')).toBe('https://bise.dev/docs');
    expect(addressOf('http://localhost:5173')).toBe('http://localhost:5173');
  });
  it('a search goes to Google with his words', () => {
    expect(searchUrl(' a & b ')).toBe('https://www.google.com/search?q=a%20%26%20b');
    expect(typedRow('perf notes')).toEqual({ kind: 'search', q: 'perf notes', url: 'https://www.google.com/search?q=perf%20notes' });
    expect(typedRow('bise.dev')).toEqual({ kind: 'url', url: 'https://bise.dev' });
  });
});

describe('the rows (designer m_64584)', () => {
  const gh = page('https://github.com/gvergnaud/bise', [1]);
  const make = { plugin: 'sheets', app: 'Sheets', ending: '.xlsx', title: 'Sheet' };
  it('an inline hit first, what he typed second, the other hits, New › last', () => {
    const arts = [item('g', 'git notes', 1)];
    const hits = omniHits('git', web(gh), arts, NONE, NOW);
    expect(omniRows('git', hits, [make]).map((r) => r.kind)).toEqual(['web', 'search', 'found', 'new']);
  });
  it('no inline hit: what he typed first; never empty while he has words', () => {
    const hits = omniHits('bise', web(gh), [], NONE, NOW);
    expect(omniRows('bise', hits).map((r) => r.kind)).toEqual(['search', 'web']);
    expect(omniRows('zzz', [])).toEqual([typedRow('zzz')]);
    expect(omniRows('', [])).toEqual([]);
  });
  it('his address that is the hit already: once', () => {
    const p = page('https://bise.dev/', [1]);
    const hits = omniHits('bise.dev', web(p), [], NONE, NOW);
    expect(omniRows('bise.dev', hits).map((r) => r.kind)).toEqual(['web']);
    expect(omniRows('bise.de', omniHits('bise.de', web(p), [], NONE, NOW)).map((r) => r.kind)).toEqual(['web', 'url']);
  });
  it("a walked-to row's text: a page's shown URL, an artifact's title, his words for the typed rows", () => {
    expect(rowText({ kind: 'web', page: gh }, 'g')).toBe('github.com/gvergnaud/bise');
    expect(rowText({ kind: 'found', item: item('a', 'Perf notes', 1) }, 'p')).toBe('Perf notes');
    expect(rowText(typedRow('perf x'), 'perf x')).toBe('perf x');
  });
});
