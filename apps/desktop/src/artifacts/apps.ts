// The gallery by app (contract docs/plugin-apps.md A3, A6b; designer's page plugin-apps-mocks,
// blocks 1-2): which app a card follows (the hub's opener on its row, A2: the window never
// matches), the filter's tabs (one per app that has something here, the most first, five at most,
// the rest under More; Pull requests stay a tab: their app is bise's review), a card's lead word
// ('Sheets', else 'opens on your Mac') and its type tip ('Excel workbook · opens in Sheets', no
// app: 'ZIP file · opens on your Mac', the ending said by the window: formatting, not matching).
// Pure: the toolbar, Card.tsx and gallery.ts lineOf draw from it. Owner: apps-gallery.
import { common } from '@/proto/words_common.gen';
import { library } from '@/proto/words_library.gen';

/** What the filter and the card read of a row's opener (bise_proto `Opener`, A3). */
export interface AppOpener {
  readonly plugin: string;
  /** its tool, for the id's fallback */
  readonly tool?: string;
  /** the app's own id, the `app:` key (A1's app_id; none: `<plugin>-<tool>`) */
  readonly app_id?: string | null;
  /** the app's title, 'Sheets' */
  readonly app: string;
  /** the matched entry's title, 'Excel workbook' */
  readonly type_title: string;
}

/** What the filter and the card read of a row (an artifact, a folder row of ~/bise). */
export interface AppRow {
  /** a link's address (no file: its app is his browser or the panel's) */
  readonly url?: string | null;
  readonly opener?: AppOpener | null;
  /** a PR's state: its app is bise's review */
  readonly pr?: unknown;
  /** the file on disk, its ending when no app names its type */
  readonly path?: string | null;
  /** what the agent pointed at (a file row with no path says its file here or in its url) */
  readonly target?: string | null;
}

/** The row's file name, for its ending: its path, else its target, else a url that is no web address. Pure. */
export function fileNameOf(a: AppRow): string {
  for (const x of [a.path, a.target, a.url]) if (x != null && x !== '' && !/^(https?:|mailto:)/iu.test(x)) return x;
  return '';
}

/** The app a row follows in the filter: an app, bise's review (PRs), a link (his browser), none (his Mac). */
export interface AppTag {
  /** the `app:` key: the opener's plugin, 'prs', 'links', 'mac' */
  readonly id: string;
  readonly word: string;
}

export const PRS: AppTag = { id: 'prs', word: library.kind_prs };
const LINKS: AppTag = { id: 'links', word: sentence(library.kind_links) };
const MAC: AppTag = { id: 'mac', word: common.your_mac };

function sentence(w: string): string {
  return w === '' ? w : `${(w[0] ?? '').toUpperCase()}${w.slice(1)}`;
}

/**
 * The app a row follows: its opener's app, else a PR's review, a link's browser, his Mac. Its id
 * is the `app:` key, stable (architect m_63490, m_63530): the opener's app id (A1's app_id, else
 * `<plugin>-<tool>`), else `prs`, `links`, `mac`, never a word a rewording changes; the hub's
 * `sb artifact list --app` keys the same (artifacts_filter.rs app_of, both on
 * tests/fixtures/artifacts-filter.json). Pure.
 */
export function appOf(a: AppRow): AppTag {
  if (a.pr != null) return PRS;
  const o = a.opener ?? null;
  // an empty app_id (a row from an older hub): A1's fallback, Rust's app_id_fallback, pinned on
  // both sides by the shared fixture's opener row (architect m_63779)
  if (o !== null) return { id: o.app_id != null && o.app_id !== '' ? o.app_id : `${o.plugin}-${o.tool ?? ''}`, word: o.app };
  return isLink(a) ? LINKS : MAC;
}

/**
 * What he typed after `app:` as a key: a key itself, or the word of one of `tags` ('your mac' →
 * `mac`, 'sheets' → the Sheets plugin's), any case; none: what he typed, lowercased. The hub
 * reads it the same (artifacts_filter.rs app_key_for). Pure.
 */
export function appKeyFor(typed: string, tags: Iterable<AppTag>): string {
  const t = typed.trim().toLowerCase();
  const all = [...tags];
  return (all.find((x) => x.id.toLowerCase() === t) ?? all.find((x) => x.word.toLowerCase() === t))?.id ?? t;
}

/** The fixed tags (no opener), for appKeyFor: `app:"pull requests"`, `app:"your mac"`. */
export const FIXED_TAGS: readonly AppTag[] = [PRS, LINKS, MAC];

/** A link: no file, a web or mail address (its marks say where it opens). Pure. */
const isLink = (a: AppRow): boolean => (a.path ?? '') === '' && /^(https?:\/\/|mailto:)/iu.test(a.url ?? '');

/** One tab of the filter: its app and how many here follow it. */
export interface AppTab {
  readonly id: string;
  readonly word: string;
  readonly n: number;
}

/** How many tabs show before More (the mocks: 'past five, More ▾ holds the rest'). */
export const MAX_TABS = 5;

/**
 * The filter's tabs for these rows: one per app that has something here (no zero counts, but a tag
 * in `keep`: a tab he picked, so he can take it off, and the Pull requests tab while archived PRs
 * are here, prSideItems), the most first, ties by word; `max` shown, the rest under More. Pull
 * requests never fold into More. Faceted (his pick A, rule 3): `tags` are the rows the other
 * filters keep, the app filter left out, so each count says what a click would show. Pure.
 */
export function appTabs(tags: readonly AppTag[], keep: readonly AppTag[] = [], max = MAX_TABS): { tabs: AppTab[]; more: AppTab[] } {
  const n = new Map<string, { word: string; n: number }>();
  for (const t of tags) {
    const c = n.get(t.id);
    if (c === undefined) n.set(t.id, { word: t.word, n: 1 });
    else c.n += 1;
  }
  for (const t of keep) if (!n.has(t.id)) n.set(t.id, { word: t.word, n: 0 });
  const all = [...n].map(([id, c]) => ({ id, word: c.word, n: c.n })).sort((x, y) => y.n - x.n || x.word.localeCompare(y.word) || x.id.localeCompare(y.id));
  const room = Math.max(1, max);
  if (all.length <= room) return { tabs: all, more: [] };
  const tabs = all.slice(0, room);
  const more = all.slice(room);
  const pr = more.findIndex((t) => t.id === PRS.id || t.id === keyOfTag(PRS));
  if (pr >= 0) {
    // PRs keep their tab: the last shown app moves under More, at its head (it has the most there)
    const [prTab] = more.splice(pr, 1);
    const last = tabs.pop();
    if (last !== undefined) more.unshift(last);
    if (prTab !== undefined) tabs.push(prTab);
  }
  return { tabs, more };
}

/** A tag's key in the `app:` token (filters.ts appKeyOf): its id, `prs`. Pure. */
export const keyOfTag = (t: AppTag): string => t.id;

/**
 * The bar's app tabs by the `app:` token's keys (filters.ts): `rows` the ones every other filter
 * keeps (the facet), `picked` the keys in the field (kept at 0 so he can take them off),
 * `keepPrs` the Pull requests tab even at 0 (archived PRs one click away). Pure.
 */
export function appTabsOf(rows: readonly AppRow[], picked: ReadonlySet<string>, keepPrs: boolean, max = MAX_TABS, firstParty: ReadonlySet<string> = new Set()): { tabs: AppTab[]; more: AppTab[] } {
  const words = appWords(rows, firstParty);
  const tags = rows.map((r) => {
    const id = keyOfTag(appOf(r));
    return { id, word: words.get(id) ?? id };
  });
  const keep: AppTag[] = [...picked].map((k) => ({ id: k, word: words.get(k) ?? k }));
  if (keepPrs) keep.push({ id: keyOfTag(PRS), word: PRS.word });
  return appTabs(tags, keep, max);
}

/**
 * Each app key's word, its tab's label: two plugins may declare the same app id (they share a tab,
 * designer m_63527), whose label is a first-party opener's title when one of them is first-party
 * (`firstParty`: the built-in plugins' names), else the first in plugin order (architect m_63530).
 * Pure.
 */
export function appWords(rows: readonly AppRow[], firstParty: ReadonlySet<string>): Map<string, string> {
  const best = new Map<string, { word: string; first: boolean; plugin: string }>();
  for (const r of rows) {
    const t = appOf(r);
    const plugin = r.pr == null ? (r.opener?.plugin ?? '') : '';
    const c = { word: t.word, first: firstParty.has(plugin), plugin };
    const was = best.get(t.id);
    if (was === undefined || (c.first && !was.first) || (c.first === was.first && c.plugin < was.plugin)) best.set(t.id, c);
  }
  return new Map([...best].map(([id, c]) => [id, c.word]));
}

/** The rows of the apps he picked: one of them (OR inside the dimension); none picked: every row. Pure. */
export function inApps<T>(rows: readonly T[], tagOf: (r: T) => AppTag, picked: ReadonlySet<string>): T[] {
  return picked.size === 0 ? [...rows] : rows.filter((r) => picked.has(tagOf(r).id));
}

/**
 * A click on a tab (his pick A): a plain click shows that app alone ('all' clears), ⌘-click adds it
 * to the picked ones or takes it off (OR). Pure.
 */
export function pickApp(picked: ReadonlySet<string>, id: string, add: boolean): Set<string> {
  if (id === 'all') return new Set();
  if (!add) return picked.size === 1 && picked.has(id) ? new Set() : new Set([id]);
  const next = new Set(picked);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** The app: token's completion (his pick A): the apps in scope whose word starts with, then holds, what he typed, the most first. Pure. */
export function appCompletions(tabs: readonly AppTab[], typed: string): AppTab[] {
  const t = typed.trim().toLowerCase();
  const at = (x: AppTab) => x.word.toLowerCase().indexOf(t);
  return tabs.filter((x) => at(x) >= 0).sort((x, y) => Number(at(x) !== 0) - Number(at(y) !== 0) || y.n - x.n || x.word.localeCompare(y.word));
}

/**
 * A card's lead word, before its who and when: 'Sheets'; whatever no app opens: 'opens on your
 * Mac' (one rule, path or not: designer m_61626); a PR or a link: none (their own marks say it). Pure.
 */
export function leadOf(a: AppRow): string | null {
  const t = appOf(a);
  if (t.id === PRS.id || t.id === LINKS.id) return null;
  return t.id === MAC.id ? library.opens_mac : t.word;
}

/** The lead says his Mac opens it (the line then drops who made it: the mocks' 'opens on your Mac · 3 d'). Pure. */
export const onMac = (a: AppRow): boolean => leadOf(a) === library.opens_mac;

/**
 * The ☰ list's app column: the app ('Sheets'), 'your Mac' when no app opens it (File types' word,
 * which fits the column; the card's line keeps 'opens on your Mac': designer m_61645); a PR or a
 * link: null (the list says its kind). Pure.
 */
export function appWord(a: AppRow): string | null {
  const lead = leadOf(a);
  if (lead === null) return null;
  return onMac(a) ? common.your_mac : lead;
}

/** The ending of a name, as a type's word: 'logs.zip' → 'ZIP', a folder or no ending → null. Pure. */
export function endingOf(name: string): string | null {
  const base = name.replace(/\/+$/u, '').split('/').pop() ?? '';
  if (name.endsWith('/')) return null;
  const dot = base.lastIndexOf('.');
  if (dot <= 0 || dot === base.length - 1) return null;
  return base.slice(dot + 1).toUpperCase();
}

/**
 * A card's type tip: 'Excel workbook · opens in Sheets'; no app, by its ending: 'ZIP file · opens
 * on your Mac', a folder: 'Folder · opens on your Mac'; a PR or a link: null (their marks keep
 * theirs). `name`: the file's name when the row has no path (a folder row of ~/bise). Pure.
 */
export function opensTip(a: AppRow, name = fileNameOf(a)): string | null {
  const t = appOf(a);
  if (t.id === PRS.id || t.id === LINKS.id) return null;
  const o = a.opener ?? null;
  if (o !== null) return library.tip_opens({ kind: o.type_title, app: o.app });
  if (name === '') return null;
  const ending = endingOf(name);
  return library.tip_opens_mac({ kind: ending === null ? library.folder_kind : library.file_of({ ending }) });
}
