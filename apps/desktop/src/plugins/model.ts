// The plugins place's pure part (page plugins v48, plugins-ui's screens; docs/plugins.md): what a
// row says and which control it gets, the list's groups and filters, the band's items and the
// sidebar row's words. Every number comes from the core (`plugins`, `plugins_summary`:
// bend_plugins::catalog::rows and ::summary); this file only picks words and groups rows, it
// adds nothing up (architect m_40158). Buttons read typed fields (`action`, `login`,
// `parts[].login`, `parts[].error`), never the status words. Pure. Owner: plug-desktop.
import type { Plugin, PluginPart, PluginSuggest, PluginsSummary } from '@/proto';
import { settings } from '@/proto/words.gen';

/** The list's kind filter. */
export type KindFilter = 'all' | 'plugin' | 'mistral' | 'mcp' | 'skills';
export const KINDS: readonly KindFilter[] = ['all', 'plugin', 'mistral', 'mcp', 'skills'];

/** The kinds in the list's order, each its group. */
const GROUPS: readonly Exclude<KindFilter, 'all'>[] = ['plugin', 'mistral', 'mcp', 'skills'];

/** A row's mark on its icon: '?' waits on him (a login), '✗' failed (a server), or none. */
export type Badge = '?' | '✗' | null;

/** The one control at a row's right end. */
export type Control =
  /** bise's own skills: 'always on', no switch */
  | { kind: 'locked' }
  /** a built-in that needs its setup first (computer, off until set up): 'not set up' + set up */
  | { kind: 'set_up_cu' }
  /** a connector whose sign-in needs doing again: 'sign-in expired' + connect */
  | { kind: 'connect'; id: string }
  /** a remote server that waits on a login: 'needs a login' + log in (its part's name) */
  | { kind: 'log_in'; name: string; pending: boolean }
  /** a server that failed: its red line, details, and the switch */
  | { kind: 'failed'; why: string }
  /** the switch */
  | { kind: 'toggle' };

/** Installed (what his agents have) or not: a connector to connect is in browse. */
export function installed(p: Plugin): boolean {
  return p.state === 'loaded' || p.state === 'disabled' || p.state === 'invalid';
}

/** Which kind group a row goes in (an unknown kind with the plugins). */
export function groupOf(p: Plugin): Exclude<KindFilter, 'all'> {
  return p.kind === 'mistral' || p.kind === 'mcp' || p.kind === 'skills' ? p.kind : 'plugin';
}

/** What he reads as its name. */
export const titleOf = (p: Plugin): string => (p.title !== '' ? p.title : p.name);

/** The parts of a kind, and their tools (an MCP server's) flattened. */
function partsOf(p: Plugin, kind: PluginPart['kind']): PluginPart[] {
  return (p.parts ?? []).filter((x) => x.kind === kind);
}

/** An MCP part's tool count: the listed tools, else the counted ones (none: not known yet). */
export function toolsOf(x: PluginPart): number | null {
  if ((x.tools ?? []).length > 0) return (x.tools ?? []).length;
  return x.tool_count ?? null;
}

/** What it gives, at the row's right: '1 skill · 23 tools', '18 skills', '1 skill · 1 server'. */
export function gives(p: Plugin): string {
  const skills = partsOf(p, 'skill').length;
  const servers = partsOf(p, 'mcp');
  const counts = servers.map(toolsOf);
  const out: string[] = [];
  if (skills > 0) out.push(settings.plugins_n_skills({ n: skills }));
  if (servers.length > 0) {
    // the tools once every server listed them, else the servers
    const known = counts.filter((c): c is number => c !== null);
    if (known.length === counts.length) {
      const n = known.reduce((a, c) => a + c, 0);
      if (n > 0) out.push(settings.plugins_n_tools({ n }));
    } else out.push(settings.plugins_n_servers({ n: servers.length }));
  }
  // a connector is its own server: its tools are its parts
  const tools = partsOf(p, 'tool').length;
  if (tools > 0) out.push(settings.plugins_n_tools({ n: tools }));
  // his accounts, said from 2 (one account: nothing changes)
  const accounts = (p.accounts ?? []).length;
  if (accounts > 1) out.push(settings.plugins_n_accounts({ n: accounts }));
  return out.join(' · ');
}

/** Its own login or one of its on servers' says it needs one (typed fields only). */
export function needsLogin(p: Plugin): boolean {
  return p.login === 'needs' || (p.parts ?? []).some((x) => x.on && x.login === 'needs');
}

/** The first on server that failed, with its line. */
export function failedPart(p: Plugin): PluginPart | null {
  return (p.parts ?? []).find((x) => x.on && x.error != null) ?? null;
}

/** The mark on its icon: only an on row waits on him. */
export function badgeOf(p: Plugin): Badge {
  if (!p.on || !installed(p)) return null;
  if (needsLogin(p) || p.state === 'invalid') return p.state === 'invalid' ? '✗' : '?';
  // a setting it needs to work, not given: it waits on him like a login (designer m_51514)
  if (needsSetting(p)) return '?';
  return failedPart(p) !== null ? '✗' : null;
}

/** A setting it needs to work (required) has no value. */
export function needsSetting(p: Plugin): boolean {
  return (p.settings ?? []).some((s) => s.required === true && !s.set);
}

/** The built-in computer: off until /computer-use set it up (it is opt-in). */
export function isComputer(p: Plugin): boolean {
  return p.id === 'computer' && p.scope === 'built-in';
}

/** Its control (page plugins §1: a row that waits on his account has its button, no switch). */
export function controlOf(p: Plugin): Control {
  if (p.locked === true) return { kind: 'locked' };
  if (isComputer(p) && !p.on) return { kind: 'set_up_cu' };
  if (p.on && p.kind === 'mistral' && p.login === 'needs') return { kind: 'connect', id: p.id };
  if (p.on) {
    const l = (p.parts ?? []).find((x) => x.kind === 'mcp' && x.on && (x.login === 'needs' || x.login === 'pending'));
    if (l !== undefined) return { kind: 'log_in', name: l.name, pending: l.login === 'pending' };
    const f = failedPart(p);
    if (f !== null) return { kind: 'failed', why: f.error ?? '' };
  }
  return { kind: 'toggle' };
}

/** The tag after its name: built-in, Mistral, MCP, skills (a plain plugin has none). */
export function tagsOf(p: Plugin): string[] {
  const out: string[] = [];
  if (p.kind === 'mistral') out.push(settings.plugins_tag_mistral);
  else if (p.kind === 'mcp') out.push(settings.plugins_tag_mcp);
  else if (p.kind === 'skills') out.push(settings.plugins_tag_skills);
  if (p.scope === 'built-in') out.push(settings.plugins_built_in);
  return out;
}

/** He can remove it: a plugin of his own folder (plug-tidy's wire refuses the rest). */
export function removable(p: Plugin): boolean {
  return p.scope === 'user' && (p.kind === 'plugin' || p.kind === 'mcp');
}

/** What a search found inside a row: one of its parts (a tool, a skill), else the row itself. */
export interface Hit {
  readonly p: Plugin;
  /** the part that matched, when the row's own words didn't */
  readonly part?: PluginPart;
}

const has = (s: string | null | undefined, q: string): boolean => s != null && s.toLowerCase().includes(q);

/** The row, or one of its parts or tools, matches `q` (any case); null: it doesn't. */
export function hitOf(p: Plugin, q: string): Hit | null {
  const s = q.trim().toLowerCase();
  if (s === '') return { p };
  if (has(titleOf(p), s) || has(p.name, s) || has(p.what, s)) return { p };
  for (const x of p.parts ?? []) {
    if (has(x.name, s) || has(x.what, s)) return { p, part: x };
    for (const t of x.tools ?? []) if (has(t.name, s) || has(t.what, s)) return { p, part: t };
  }
  return null;
}

/** One group of the list: its kind and its rows (hits). */
export interface Group {
  readonly kind: Exclude<KindFilter, 'all'>;
  readonly hits: readonly Hit[];
}

/** The installed part, by kind in the list's order, filtered by kind and search; empty groups go. */
export function groupsOf(rows: readonly Plugin[], kind: KindFilter, q: string): Group[] {
  const shown = rows.filter((p) => installed(p));
  return GROUPS.filter((g) => kind === 'all' || kind === g)
    .map((g) => ({
      kind: g,
      hits: shown.filter((p) => groupOf(p) === g).flatMap((p) => {
        const h = hitOf(p, q);
        return h === null ? [] : [h];
      }),
    }))
    .filter((g) => g.hits.length > 0);
}

/** One group of the not-installed part: where it comes from ('mistral', a marketplace) and its rows. */
export interface SourceGroup {
  readonly source: string;
  readonly hits: readonly Hit[];
}

/** A row's key: its id, else its name. */
export const keyOf = (p: Plugin): string => (p.id === '' ? p.name : p.id);

/**
 * The not-installed part (his m_62905: one list, installed first, then what he can add): each
 * source in the core's order (Mistral's connectors, then each marketplace), filtered by the same
 * kind chip and search as the installed part; a row he has installed already and empty sources go.
 */
export function availableOf(browse: readonly { readonly source: string; readonly items: readonly Plugin[] }[], rows: readonly Plugin[], kind: KindFilter, q: string): SourceGroup[] {
  const have = new Set(rows.filter(installed).map(keyOf));
  return browse
    .map((b) => ({
      source: b.source,
      hits: b.items.flatMap((p) => {
        if (have.has(keyOf(p)) || (kind !== 'all' && groupOf(p) !== kind)) return [];
        const h = hitOf(p, q);
        return h === null ? [] : [h];
      }),
    }))
    .filter((b) => b.hits.length > 0);
}

/** A group's head: 'plugins', 'Mistral connectors', 'MCP servers', 'skills'. */
export function groupWords(kind: Exclude<KindFilter, 'all'>): string {
  switch (kind) {
    case 'plugin':
      return settings.plugins_kind_plugins;
    case 'mistral':
      return settings.plugins_group_mistral;
    case 'mcp':
      return settings.plugins_group_mcp;
    case 'skills':
      return settings.plugins_kind_skills;
  }
}

/** A kind chip's words. */
export function kindWords(kind: KindFilter): string {
  switch (kind) {
    case 'all':
      return settings.plugins_kind_all;
    case 'plugin':
      return settings.plugins_kind_plugins;
    case 'mistral':
      return settings.plugins_kind_mistral;
    case 'mcp':
      return settings.plugins_kind_mcp;
    case 'skills':
      return settings.plugins_kind_skills;
  }
}

/** The row's second line: what it does, a server's red line, a match inside it. */
export function lineOf(h: Hit): string {
  if (h.part !== undefined) return `${h.part.kind === 'skill' ? settings.plugins_part_skill : settings.plugins_part_tool} ${h.part.name}`;
  const f = h.p.on ? failedPart(h.p) : null;
  if (f !== null) return settings.plugins_failed({ why: f.error ?? '' });
  return h.p.what ?? '';
}

// ---- the summary: the sidebar row and the band ----

/** A summary's items of a kind. */
export const itemsOf = (s: PluginsSummary | null, kind: PluginSuggest['kind']): PluginSuggest[] => (s?.suggest ?? []).filter((x) => x.kind === kind);

/** What waits on him, each in its words ('Outlook Calendar needs a login', 'postgres-dev failed to start'). */
export function waitsWords(s: PluginsSummary): string[] {
  return (s.suggest ?? []).flatMap((x) => {
    if (x.kind === 'login') return (x.titles ?? []).map((name) => settings.plugins_waits_login({ name }));
    if (x.kind === 'error') return (x.titles ?? []).map((name) => settings.plugins_waits_error({ name }));
    return [];
  });
}

/**
 * The sidebar row's end (page plugins v48, plugins-ui m_40135): the plugins on, faint; when some
 * wait on him, their number in the accent (inbox's count), no pill. Its tip says which.
 */
export function sideOf(s: PluginsSummary | null): { readonly end: string; readonly waits: number; readonly tip: string } {
  if (s === null) return { end: '', waits: 0, tip: '' };
  if (s.waiting > 0) {
    return { end: '', waits: s.waiting, tip: settings.plugins_side_tip_waits({ on: s.on, n: s.waiting, list: waitsWords(s).join(', ') }) };
  }
  return { end: s.on > 0 ? String(s.on) : '', waits: 0, tip: settings.plugins_side_tip({ n: s.on }) };
}

/**
 * What's new to connect, as one key: the dot shows until he has seen the band with this key
 * (the app's own view state, kept by the window). '' : nothing to suggest.
 */
export function newKey(s: PluginsSummary | null): string {
  if (s === null) return '';
  return (s.suggest ?? [])
    .filter((x) => x.kind === 'mistral' || x.kind === 'import' || x.kind === 'marketplace' || x.kind === 'workspace')
    .map((x) => `${x.kind}:${(x.ids ?? []).join(',')}`)
    .join('|');
}

/**
 * The band shows: something to connect, import or look at. Without its connector cards (nothing
 * installed: the list's Not installed rows say them; designer m_63123 #2), only for a line the list
 * can't say (an import, a project's skills, a marketplace).
 */
export function bandShows(s: PluginsSummary | null, cards = true): boolean {
  if (newKey(s) === '') return false;
  return cards || itemsOf(s, 'import').length + itemsOf(s, 'workspace').length + itemsOf(s, 'marketplace').length > 0;
}

/** A band card's words: the line under its title, and its tip. */
export interface CardWords { readonly line: string; readonly tip: string }

/**
 * The band's words (the core's `whys[i]`, the same order as its ids): a why every card would say
 * goes up to the band's sub, once (ui-review #6); each card then says what its tool does (its
 * row's `what`). The tip: the connector's full description, then where it was seen.
 */
export function bandWords(x: PluginSuggest, rows: ReadonlyMap<string, Plugin>): { readonly sub: string; readonly cards: readonly CardWords[] } {
  const ids = x.ids ?? [];
  const whys = x.whys ?? [];
  const first = whys[0]?.text ?? '';
  const shared = ids.length >= 2 && first !== '' && whys.length === ids.length && whys.every((w) => w.text === first) ? first : '';
  const cards = ids.map((id, i) => {
    const why = whys.at(i);
    const what = rows.get(id)?.what ?? '';
    const where = why?.source === 'seen' && (why.where ?? []).length > 0 ? settings.plugins_why_where({ list: (why.where ?? []).join(', ') }) : '';
    return { line: shared !== '' ? what : why?.text ?? '', tip: [what, where].filter((s) => s !== '').join('\n') };
  });
  return { sub: shared !== '' ? settings.plugins_band_sub_why({ why: shared }) : settings.plugins_band_sub, cards };
}

/**
 * How many of the band's `k` cards to show so the grid has no hole (ui-review #9: 2 across, 1 when
 * narrow): 2, 4 or 6 cells, the last one 'browse N more' when more wait than it shows (`total`,
 * the core's n). One cell: itself.
 */
export function bandCells(k: number, total: number): { readonly shown: number; readonly more: number } {
  const slots = Math.min(6, k + (total > k ? 1 : 0));
  const cells = slots <= 1 ? slots : slots - (slots % 2);
  const shown = Math.min(k, total > cells ? cells - 1 : cells);
  return { shown, more: Math.max(0, total - shown) };
}

/** A marketplace line: named when it is his one marketplace ('6 new plugins in openai/plugins'). */
export function marketWords(items: readonly PluginSuggest[], x: PluginSuggest): string {
  return items.length === 1 && x.source != null ? settings.plugins_band_market_in({ n: x.n, source: x.source }) : settings.plugins_band_market({ n: x.n });
}

/** The band's import line: '4 MCP servers found on this Mac: Claude Code, Cursor' (the core's one import item). */
export function foundWords(x: PluginSuggest | undefined): string | null {
  if (x === undefined) return null;
  return settings.plugins_band_found({ n: x.n, list: (x.titles ?? []).join(', ') });
}

/** A lead as a sentence: ends with its period ('bise's own skills' → 'bise's own skills.'; designer
 *  m_43933: every lead ends with one), but not after a path or an address ('~/.agents/plugins/linear',
 *  'mcp.linear.app': a period would read as part of it; designer m_44086); '' stays ''. Pure. */
export function leadOf(s: string): string {
  const t = s.trimEnd();
  const last = t.split(/\s/u).at(-1) ?? '';
  const pathy = last.includes('/') || last.startsWith('~') || /^[\w-]+(\.[\w-]+)+$/u.test(last);
  return t === '' || pathy || /[.!?…:]$/u.test(t) ? t : `${t}.`;
}
