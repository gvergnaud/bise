// src/app/panel · ⌘T's and ⌘L's omnibox, its pure parts (page newtab-omnibox-flow, designer
// m_64584): one list of what his words find, his web pages (bise's web history: the links he
// opened from bise, main/webhistory.ts) and his artifacts mixed, ranked as Chrome's: a prefix
// match first (his words start the address, scheme and www. aside, or the title), then every word
// starting a word, then every word inside; within each, frecency (each of the last visits weighs
// by its age, a typed one twice; an artifact: its last change, and once more when it is in Recent).
// Row 1 is the top hit when it completes inline (a prefix match), else what he typed (the address,
// or 'Search Google for "…"'); what he typed comes second after a hit, then the other hits, then
// New ›'s rows. Owner: newtab-omnibox.
import type { Item } from '@/artifacts/model';
import type { WebHistory, WebPage, WebVisit } from '@/shared/api';
import type { MakeOpt } from './make';

/** How many hits the list shows, web and artifacts together. */
export const HITS_MAX = 8;

const DAY_MS = 86_400_000;

/** A hit: a web page or an artifact, its match class (3 prefix, 2 word start, 1 inside) and its frecency. */
export type OmniHit =
  | { readonly kind: 'web'; readonly page: WebPage; readonly cls: number; readonly score: number }
  | { readonly kind: 'found'; readonly item: Item; readonly cls: number; readonly score: number };

/** A row of the omnibox's list. */
export type OmniRow =
  | { readonly kind: 'web'; readonly page: WebPage }
  | { readonly kind: 'found'; readonly item: Item }
  | { readonly kind: 'url'; readonly url: string }
  | { readonly kind: 'search'; readonly q: string; readonly url: string }
  | { readonly kind: 'new'; readonly make: MakeOpt };

/** A visit's weight by its age (Chrome's buckets: 4 days, 2 weeks, a month, 3 months). Pure. */
export function ageWeight(ageMs: number): number {
  const d = ageMs / DAY_MS;
  return d <= 4 ? 100 : d <= 14 ? 70 : d <= 31 ? 50 : d <= 90 ? 30 : 10;
}

/** A page's frecency: its visits' weights, a typed one twice. Pure. */
export function frecency(visits: readonly WebVisit[], now: number): number {
  return visits.reduce((s, v) => s + ageWeight(now - v.at) * (v.typed ? 2 : 1), 0);
}

/** A URL as the list and the field show it: no scheme, no www., no lone '/'. Pure. */
export function shownUrl(url: string): string {
  const s = url.replace(/^https?:\/\//iu, '').replace(/^www\./iu, '');
  const slash = s.indexOf('/');
  return slash === s.length - 1 ? s.slice(0, -1) : s;
}

/** The address with its scheme dropped (www. kept): his 'www.git' completes too. Pure. */
function noScheme(url: string): string {
  return url.replace(/^https?:\/\//iu, '');
}

/** The host a list row names under a page ('github.com'). Pure. */
export function hostOf(url: string): string {
  return shownUrl(url).split(/[/?#]/u)[0] ?? '';
}

/** The words his words may start: split at anything not a letter or a digit. Pure. */
function wordsOf(s: string): readonly string[] {
  return s.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter((w) => w !== '');
}

/** His words, lowercased, split on spaces. Pure. */
function qWords(q: string): readonly string[] {
  return q.toLowerCase().split(/\s+/u).filter((w) => w !== '');
}

/** What a prefix match of `q` completes to, of `cands` the first his words start (case aside), or null. Pure. */
function prefixOf(q: string, cands: readonly string[]): string | null {
  if (q.trim() === '' || q !== q.trimStart()) return null;
  const l = q.toLowerCase();
  return cands.find((c) => c.toLowerCase().startsWith(l)) ?? null;
}

/** The class of a match: 3 prefix, 2 every word starts a word, 1 every word inside, 0 none. Pure. */
function classOf(q: string, prefixes: readonly string[], hay: string): number {
  if (prefixOf(q, prefixes) !== null) return 3;
  const ws = qWords(q);
  if (ws.length === 0) return 0;
  const words = wordsOf(hay);
  if (ws.every((w) => words.some((x) => x.startsWith(w)))) return 2;
  const l = hay.toLowerCase();
  return ws.every((w) => l.includes(w)) ? 1 : 0;
}

/** What a page's address completes from: its shown URL, then with www., then whole. Pure. */
function addressPrefixes(p: WebPage): readonly string[] {
  return [shownUrl(p.url), noScheme(p.url), p.url];
}

/** What ranks a page as a prefix match: its address, then its title (a web tab read it). Pure. */
function pagePrefixes(p: WebPage): readonly string[] {
  return [...addressPrefixes(p), ...(p.title === undefined ? [] : [p.title])];
}

/** The whole text a hit completes `q` to (his letters kept, the rest the hit's), or null: no prefix match. Pure. */
export function completionOf(q: string, hit: OmniHit | undefined): string | null {
  if (hit === undefined) return null;
  // a page completes its address only, as Chrome: its title ranks it, never fills the field (ui-review m_64756)
  const c = prefixOf(q, hit.kind === 'web' ? addressPrefixes(hit.page) : [hit.item.a.title]);
  return c === null ? null : q + c.slice(q.length);
}

/**
 * What his words find in his web pages and artifacts, best first, HITS_MAX at most: by match class,
 * then frecency. `recent`: the artifacts in Recent ('project/id'), one more visit of today. Pure.
 */
export function omniHits(q: string, web: WebHistory, items: readonly Item[], recent: ReadonlySet<string>, now: number, max = HITS_MAX): readonly OmniHit[] {
  if (q.trim() === '') return [];
  const pages: OmniHit[] = web.pages.flatMap((page) => {
    const cls = classOf(q, pagePrefixes(page), `${page.url} ${page.title ?? ''}`);
    return cls === 0 ? [] : [{ kind: 'web' as const, page, cls, score: frecency(page.visits, now) }];
  });
  const arts: OmniHit[] = items.flatMap((item) => {
    const cls = classOf(q, [item.a.title], `${item.a.title} ${item.a.kind} ${item.a.agent}`);
    const score = ageWeight(now - item.a.at_ms) + (recent.has(`${item.project}/${item.a.id}`) ? 100 : 0);
    return cls === 0 ? [] : [{ kind: 'found' as const, item, cls, score }];
  });
  return [...pages, ...arts].toSorted((a, b) => b.cls - a.cls || b.score - a.score).slice(0, max);
}

/** His words are an address ('localhost:5173', 'https://…', 'bise.dev/x'), not a search. Pure. */
export function looksLikeUrl(q: string): boolean {
  const t = q.trim();
  if (t === '' || /\s/u.test(t)) return false;
  return /^[a-z][a-z0-9+.-]*:\/\//iu.test(t) || /^localhost(:\d+)?(\/|$)/iu.test(t) || /^[\w-]+(\.[\w-]+)+(:\d+)?(\/|$)/u.test(t);
}

/** His words as an address to open: a scheme added when they have none. Pure. */
export function addressOf(q: string): string {
  const t = q.trim();
  return t.includes('://') ? t : `https://${t}`;
}

/** Google's search for his words (designer m_64584: his browser's default today; no picker yet). Pure. */
export function searchUrl(q: string): string {
  return `https://www.google.com/search?q=${encodeURIComponent(q.trim())}`;
}

/** What he typed, as a row: his address, else the search. Pure. */
export function typedRow(q: string): OmniRow {
  return looksLikeUrl(q) ? { kind: 'url', url: addressOf(q) } : { kind: 'search', q: q.trim(), url: searchUrl(q) };
}

function hitRow(h: OmniHit): OmniRow {
  return h.kind === 'web' ? { kind: 'web', page: h.page } : { kind: 'found', item: h.item };
}

/** The list for his words: the inline hit or what he typed first, then the rest, then New ›'s rows. Pure. */
export function omniRows(q: string, hits: readonly OmniHit[], makes: readonly MakeOpt[] = []): readonly OmniRow[] {
  if (q.trim() === '') return [];
  const typed = typedRow(q);
  const made = makes.map((make) => ({ kind: 'new' as const, make }));
  const [top, ...rest] = hits;
  if (top === undefined || completionOf(q, top) === null) return [typed, ...hits.map(hitRow), ...made];
  // the address he typed is that page already: once
  const same = typed.kind === 'url' && top.kind === 'web' && shownUrl(top.page.url) === shownUrl(typed.url);
  return [hitRow(top), ...(same ? [] : [typed]), ...rest.map(hitRow), ...made];
}

/** The text the field shows for a walked-to row: a page's shown URL, an artifact's or a make's title, his words for the others. Pure. */
export function rowText(r: OmniRow, q: string): string {
  switch (r.kind) {
    case 'web':
      return shownUrl(r.page.url);
    case 'found':
      return r.item.a.title;
    case 'new':
      return r.make.title;
    case 'url':
    case 'search':
      return q;
  }
}
