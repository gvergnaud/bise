// The gallery's filter bar (his pick 'A + jetons' on page artifacts-signal v8; designer's
// artifacts-filters-options v4 and m_61790; architect m_61786). One state: the search field's text,
// `in:acme since:today by:perf pricing`. The When chip, the by: tokens and the app tabs
// (apps-gallery, on plugin-apps) are views of it that write their token back (withToken), so the
// chips and the field never drift and the filtered view's address is that string.
//
// Each dimension is one row of DIMENSIONS: its token, how a value reads (parse), whether an item
// matches a set of values (OR inside a dimension), and its facet (the counts with every OTHER
// dimension applied). AND across dimensions. Values may be quoted (`app:"Pull requests"`),
// tokens and values are case-insensitive. `in:` is the project scope (the Screen resolves it),
// the rest are words (folders.ts searchItems).
//
// The rule is the hub's too: rust/switchboard/src/artifacts_filter.rs (`sb artifact list --since
// --agent`), both tested on tests/fixtures/artifacts-filter.json. Pure. Owner: artifacts-keep.
import { appCompletions, appKeyFor, appOf } from './apps';
import type { Item } from './model';
import { touchedOf } from './sections';

/** When it was last touched: the When chip's values, as `sb artifact list --since` says them. */
export type When = 'last' | 'today' | 'week' | 'older';
export const WHENS: readonly When[] = ['last', 'today', 'week', 'older'];

/** The local offset east of UTC (minutes) at a time: each time its own (a DST night counts right). */
export type Tz = (ms: number) => number;
export const localTz: Tz = (ms) => -new Date(ms).getTimezoneOffset();

/** What a match needs besides the item: now and the zone. */
export interface Ctx {
  readonly now: number;
  readonly tz: Tz;
}

/** A dimension of the bar. */
export interface Dimension {
  readonly token: string;
  /** a typed value's key, null when it means nothing (it stays a word) */
  readonly parse: (value: string) => string | null;
  readonly match: (it: Item, keys: ReadonlySet<string>, ctx: Ctx) => boolean;
  /** the keys an item counts for, for the facet */
  readonly keysOf: (it: Item, ctx: Ctx) => readonly string[];
}

const DAY_MS = 86_400_000;

/** The local day (days since 1970-01-01) of `ms` at `offsetMin` minutes east of UTC. */
export const dayOf = (ms: number, offsetMin: number): number => Math.floor((ms + offsetMin * 60_000) / DAY_MS);

/** Which When values a touch is in at `ctx` (last: the hub's `new`, never recomputed). */
export function whensOf(it: Item, ctx: Ctx): When[] {
  const today = dayOf(ctx.now, ctx.tz(ctx.now));
  const t = touchedOf(it.a);
  const day = dayOf(t, ctx.tz(t));
  const out: When[] = [];
  if (it.a.new) out.push('last');
  if (day === today) out.push('today');
  out.push(day > today - 7 ? 'week' : 'older');
  return out;
}

/** Who an item is by for `by:`: `you` when he added it, else its agent. */
export const whoOf = (it: Item): string => (it.a.by === 'you' ? 'you' : it.a.agent.toLowerCase());

const since: Dimension = {
  token: 'since',
  parse: (v) => (WHENS as readonly string[]).includes(v) ? v : null,
  match: (it, keys, ctx) => whensOf(it, ctx).some((w) => keys.has(w)),
  keysOf: (it, ctx) => whensOf(it, ctx),
};

const by: Dimension = {
  token: 'by',
  parse: (v) => {
    const n = v.replace(/^@/u, '');
    return n === '' ? null : n;
  },
  match: (it, keys) => keys.has(whoOf(it)),
  keysOf: (it) => [whoOf(it)],
};

/**
 * The app an item follows for `app:` (apps-gallery, apps.ts appOf: the hub's opener on its row, A2;
 * a PR's review, a link's browser, his Mac), by its stable key: the opener's plugin, `prs`,
 * `links`, `mac` (architect m_63490). The app tabs write the same token; a typed word reads as its
 * key against the row's own tag (apps.ts appKeyFor): `app:"your mac"` is `app:mac`.
 */
export const appKeyOf = (it: Item): string => appOf(it.a).id;

const app: Dimension = {
  token: 'app',
  parse: (v) => (v === '' ? null : v),
  match: (it, keys) => {
    const t = appOf(it.a);
    return [...keys].some((k) => appKeyFor(k, [t]) === t.id);
  },
  keysOf: (it) => [appKeyOf(it)],
};

/** The bar's dimensions; `app:` is apps-gallery's (plugin-apps). */
export const DIMENSIONS: readonly Dimension[] = [since, by, app];

/** The field's text read: the project typed (`in:`), each dimension's keys, the words. */
export interface Filters {
  readonly project: string | null;
  readonly keys: ReadonlyMap<string, ReadonlySet<string>>;
  readonly words: readonly string[];
}

interface Tok {
  readonly key: string | null;
  readonly value: string;
  readonly raw: string;
}

/** The field's tokens: `key:value`, `key:"two words"`, or a word. */
function tokens(q: string): Tok[] {
  const out: Tok[] = [];
  // `key:` alone (nothing after it yet) is a token with no value, never a word
  for (const m of q.matchAll(/([a-z]+):"([^"]*)"?|([a-z]+):(\S*)|(\S+)/giu)) {
    const [raw, k1, v1, k2, v2, w] = m;
    if (k1 !== undefined) out.push({ key: k1.toLowerCase(), value: (v1 ?? '').toLowerCase(), raw });
    else if (k2 !== undefined) out.push({ key: k2.toLowerCase(), value: (v2 ?? '').toLowerCase(), raw });
    else out.push({ key: null, value: (w ?? raw).toLowerCase(), raw });
  }
  return out;
}

const dimOf = (token: string | null): Dimension | undefined => DIMENSIONS.find((d) => d.token === token);

/** The field's text as filters; a token whose value means nothing stays a word. Pure. */
export function parseFilters(q: string): Filters {
  let project: string | null = null;
  const keys = new Map<string, Set<string>>();
  const words: string[] = [];
  for (const t of tokens(q)) {
    if (t.key === 'in' && t.value !== '') {
      project = t.value;
      continue;
    }
    const d = dimOf(t.key);
    // a bare dimension (`app:` as he types it) narrows nothing yet: its completion offers the values
    if (d !== undefined && t.value === '') continue;
    const k = d?.parse(t.value) ?? null;
    if (d === undefined || k === null) {
      words.push(t.key === null ? t.value : `${t.key}:${t.value}`);
      continue;
    }
    const set = keys.get(d.token) ?? new Set<string>();
    set.add(k);
    keys.set(d.token, set);
  }
  return { project, keys, words };
}

/** Whether anything narrows the view (Clear shows): a dimension, or words. The project never. */
export const anySet = (f: Filters): boolean => f.keys.size > 0 || f.words.length > 0;

const quoted = (v: string): string => (/\s/u.test(v) ? `"${v}"` : v);

/**
 * The field's text with one dimension's token changed: `only` sets that value alone (a click),
 * `toggle` adds or takes it out (⌘-click: OR inside), null takes every value out (its ×). The
 * rest of the text stays as typed. Pure.
 */
export function withToken(q: string, token: string, value: string | null, how: 'only' | 'toggle' = 'only'): string {
  const all = tokens(q);
  const v = value?.toLowerCase() ?? null;
  const had = all.some((t) => t.key === token && t.value === v);
  const keep = all.filter((t) => t.key !== token || (how === 'toggle' && t.value !== v));
  const add = v !== null && !(how === 'toggle' && had) ? [`${token}:${quoted(v)}`] : [];
  const rest = keep.map((t) => t.raw);
  // the scope first, then the dimensions, then the words: the order he reads them in
  const isTok = (t: Tok): boolean => t.key !== null && (t.key === 'in' || dimOf(t.key) !== undefined);
  const toks = keep.filter(isTok).map((t) => t.raw);
  const words = rest.filter((_, i) => !isTok(keep[i] as Tok));
  return [...toks, ...add, ...words].join(' ');
}

/** The field's text with every dimension and word taken out (Clear): the project stays. Pure. */
export function cleared(q: string): string {
  return tokens(q).filter((t) => t.key === 'in').map((t) => t.raw).join(' ');
}

/** The items every dimension of `f` keeps, `skip` left out (a facet's own). AND across. Pure. */
export function applyFilters(items: readonly Item[], f: Filters, ctx: Ctx, skip: string | null = null): Item[] {
  const on = DIMENSIONS.filter((d) => d.token !== skip && (f.keys.get(d.token)?.size ?? 0) > 0);
  if (on.length === 0) return [...items];
  return items.filter((it) => on.every((d) => d.match(it, f.keys.get(d.token) ?? new Set(), ctx)));
}

/** A facet's row: a key, how many items hold it with every other dimension applied. */
export interface Facet {
  readonly key: string;
  readonly n: number;
}

/**
 * A dimension's counts with every OTHER dimension applied (rule 3: no dead ends): the most first;
 * a value at 0 is left out unless it is picked (so he can take it out). Pure.
 */
export function facetOf(items: readonly Item[], f: Filters, ctx: Ctx, token: string): Facet[] {
  const d = dimOf(token);
  if (d === undefined) return [];
  const n = new Map<string, number>();
  for (const it of applyFilters(items, f, ctx, token)) for (const k of d.keysOf(it, ctx)) n.set(k, (n.get(k) ?? 0) + 1);
  for (const k of f.keys.get(token) ?? []) if (!n.has(k)) n.set(k, 0);
  return [...n].map(([key, c]) => ({ key, n: c })).sort((x, y) => y.n - x.n || x.key.localeCompare(y.key));
}

/** The When chip's rows, in its own order (never by count), each with its count. Pure. */
export function whenFacet(items: readonly Item[], f: Filters, ctx: Ctx): Facet[] {
  const n = new Map(facetOf(items, f, ctx, 'since').map((x) => [x.key, x.n]));
  return WHENS.map((w) => ({ key: w, n: n.get(w) ?? 0 }));
}

/**
 * The `by:` completion (designer m_61790): people first (`you`), then the agents with the most
 * artifacts in the scope; `typed` narrows by prefix, then by substring. Pure.
 */
export function whoOptions(items: readonly Item[], f: Filters, ctx: Ctx, typed: string): Facet[] {
  const t = typed.toLowerCase().replace(/^@/u, '');
  const all = facetOf(items, f, ctx, 'by').filter((x) => x.n > 0);
  const ranked = [...all.filter((x) => x.key === 'you'), ...all.filter((x) => x.key !== 'you')];
  if (t === '') return ranked;
  return [...ranked.filter((x) => x.key.startsWith(t)), ...ranked.filter((x) => !x.key.startsWith(t) && x.key.includes(t))];
}

/**
 * The `app:` completion (his pick A): the apps in the scope with every other filter applied, whose
 * word starts with, then holds, what he typed, the most first (apps.ts appCompletions). Pure.
 */
export function appOptions(items: readonly Item[], f: Filters, ctx: Ctx, typed: string): { key: string; word: string; n: number }[] {
  const words = new Map(items.map((it) => [appKeyOf(it), appOf(it.a).word]));
  const tabs = facetOf(items, f, ctx, 'app').filter((x) => x.n > 0).map((x) => ({ id: x.key, word: words.get(x.key) ?? x.key, n: x.n }));
  return appCompletions(tabs, typed).map((t) => ({ key: t.id, word: t.word, n: t.n }));
}

/** The token being typed at the caret, for the completion: its key and what is typed of its value. */
export function tokenAt(q: string, caret: number): { key: string; typed: string; start: number } | null {
  const before = q.slice(0, caret);
  const m = /(?:^|\s)([a-z]+):("?)([^\s"]*)$/iu.exec(before);
  if (m === null) return null;
  const key = (m[1] ?? '').toLowerCase();
  if (key !== 'in' && dimOf(key) === undefined) return null;
  return { key, typed: m[3] ?? '', start: before.length - m[0].trimStart().length };
}

/** The text with the token at `start` (from tokenAt) replaced by `key:value` and a space. Pure. */
export function completeAt(q: string, start: number, caret: number, key: string, value: string): string {
  const head = q.slice(0, start);
  const tail = q.slice(caret).replace(/^\S*/u, '');
  return `${head}${key}:${quoted(value)} ${tail.trimStart()}`.trimEnd() + (tail.trim() === '' ? ' ' : '');
}

/** An empty view's way out (rule 4): only those that find something. */
export type WayOut = { readonly kind: 'everywhere'; readonly n: number } | { readonly kind: 'agents'; readonly n: number } | { readonly kind: 'clear' };

/**
 * The empty view's ways out (designer m_61790): search every project when one is set and others
 * hold matches, show the ones From agents when they hold some, Clear when a filter is set. Pure.
 */
export function waysOut(o: { readonly everywhere: number | null; readonly fromAgents: number; readonly filtered: boolean }): WayOut[] {
  const out: WayOut[] = [];
  if (o.everywhere !== null && o.everywhere > 0) out.push({ kind: 'everywhere', n: o.everywhere });
  if (o.fromAgents > 0) out.push({ kind: 'agents', n: o.fromAgents });
  if (o.filtered) out.push({ kind: 'clear' });
  return out;
}
