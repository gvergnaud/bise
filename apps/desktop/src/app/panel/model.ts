// src/app/panel · the tabbed panel's rules, pure (bise fait mac, page native-mac §4 #2 and §6, plan
// native-mac-plan §1 nm-panel): which tabs are open and in which order, which one is in front, what
// opening, closing, reordering and stepping do, which six show in the bar (HIG: six at most, the
// rest in the overflow menu), the panel's width for what is in front, and how the kept record reads
// back. No React, no store, no words: state.ts keeps one of these per window, the views draw it.
// Owner: nm-panel.
import { entityKey, type Entity } from '@/shared/entity';

/** What a tab shows. */
export type TabWhat =
  /** New tab: the launcher (search, Open, Recent); `artifacts`: its picker of the latest artifacts (⌘P) */
  | { readonly kind: 'launcher'; readonly n: number; readonly artifacts?: true }
  /** a browser tab with no page yet: its address field takes an artifact's name or a URL (⌘L) */
  | { readonly kind: 'browser'; readonly n: number }
  /** a shell in a project's folder; n 0 is the project's first (the bottom panel's too) */
  | { readonly kind: 'terminal'; readonly project: string; readonly n: number }
  /** a thing he opened: an artifact, a PR, a commit, a diff, a process, a plugin, an agent's thread */
  | { readonly kind: 'thing'; readonly e: Entity };

export interface Tab {
  /** one per thing: 'new:2', 'term:<project>#1', or the entity's key */
  readonly id: string;
  readonly what: TabWhat;
}

export interface Tabs {
  readonly list: readonly Tab[];
  /** the id of the tab in front (one of list's; '' when the panel has no tab) */
  readonly active: string;
}

/** The tabs in the bar at most (HIG: « Avoid providing more than six tabs in a tab view »). */
export const MAX_SHOWN = 6;

/** A fresh panel: no tab (the Overview left the panel for the outer card, page outer-side v2); ▯ opens a New tab. */
export const FIRST: Tabs = { list: [], active: '' };

/** A tab's id from what it shows: the same thing is one tab. Pure. */
export function tabIdOf(what: TabWhat): string {
  switch (what.kind) {
    case 'launcher':
      return `new:${String(what.n)}`;
    case 'browser':
      return `web:${String(what.n)}`;
    case 'terminal':
      return `term:${what.project}#${String(what.n)}`;
    case 'thing':
      return entityKey(what.e);
  }
}

/** A tab for `what`. Pure. */
export function tabOf(what: TabWhat): Tab {
  return { id: tabIdOf(what), what };
}

/**
 * Opens `what`: an open one comes to the front; a new one goes right after the tab in front, or in
 * the place of `instead` (the launcher he picked it from, as a browser's new tab becomes the page).
 * Pure.
 */
export function openIn(t: Tabs, what: TabWhat, instead?: string): Tabs {
  const tab = tabOf(what);
  const has = t.list.some((x) => x.id === tab.id);
  if (has) {
    // the launcher he picked an open thing from goes: it did its job; the open tab takes the new what
    const list = (instead !== undefined && instead !== tab.id ? t.list.filter((x) => x.id !== instead) : t.list).map((x) => (x.id === tab.id ? tab : x));
    return { list, active: tab.id };
  }
  const at = instead === undefined ? -1 : t.list.findIndex((x) => x.id === instead);
  if (at >= 0) return { list: t.list.map((x, i) => (i === at ? tab : x)), active: tab.id };
  const i = t.list.findIndex((x) => x.id === t.active);
  const list = [...t.list.slice(0, i + 1), tab, ...t.list.slice(i + 1)];
  return { list, active: tab.id };
}

/** The next free New tab; `artifacts`: on its picker of the latest artifacts (⌘P). Pure. */
export function nextLauncher(t: Tabs, artifacts = false): TabWhat {
  let n = 1;
  while (t.list.some((x) => x.id === `new:${String(n)}`)) n += 1;
  return artifacts ? { kind: 'launcher', n, artifacts: true } : { kind: 'launcher', n };
}

/**
 * ⌘T and '+' with a New tab already open (his m_46959, designer m_46965): the one to bring back,
 * a launcher on its Open list (the ⌘P picker is no longer one); the one in front if it is one,
 * else the nearest to the right of it, round the end. undefined: none, a new one opens. Pure.
 */
export function launcherToReuse(t: Tabs): TabWhat | undefined {
  const n = t.list.length;
  const i = Math.max(0, t.list.findIndex((x) => x.id === t.active));
  for (let k = 0; k < n; k += 1) {
    const what = (t.list[(i + k) % n] as Tab).what;
    if (what.kind === 'launcher' && what.artifacts !== true) return what;
  }
  return undefined;
}

/** The next free browser tab (⌘L, the launcher's Browser). Pure. */
export function nextBrowser(t: Tabs): TabWhat {
  let n = 1;
  while (t.list.some((x) => x.id === `web:${String(n)}`)) n += 1;
  return { kind: 'browser', n };
}

/** The next free shell number of `project` (0 first). Pure. */
export function nextShell(t: Tabs, project: string): number {
  let n = 0;
  while (t.list.some((x) => x.id === `term:${project}#${String(n)}`)) n += 1;
  return n;
}

/** Closes `id`: the tab in front goes to its right neighbour, else its left one; the last one leaves none. Pure. */
export function closeIn(t: Tabs, id: string): Tabs {
  const i = t.list.findIndex((x) => x.id === id);
  if (i < 0) return t;
  const list = t.list.filter((x) => x.id !== id);
  if (t.active !== id) return { list, active: t.active };
  return { list, active: list[Math.min(i, list.length - 1)]?.id ?? '' };
}

/** What was in front before a tab came to the front (esc on an artifact goes back to it, app/escape.ts). */
export interface Before {
  readonly open: boolean;
  readonly active: string;
}

/**
 * Esc on the tab `id` (r3-escape, designer m_44693): it closes and the panel goes back to how it
 * was before the tab came, the tab then in front if it is still there, folded if it was. Pure.
 */
export function closeBack(t: Tabs, id: string, before: Before | undefined): { readonly tabs: Tabs; readonly open: boolean } {
  const tabs = closeIn(t, id);
  if (before === undefined) return { tabs, open: true };
  return { tabs: activate(tabs, before.active), open: before.open };
}

/** Every tab but `id` closed (a tab's menu: Close others). Pure. */
export function closeOthers(t: Tabs, id: string): Tabs {
  const keep = t.list.find((x) => x.id === id);
  if (keep === undefined) return t;
  return { list: [keep], active: keep.id };
}

/** A tab he closed and where it stood (its index then): ⇧⌘T (⌃⇧T) puts it back there, as Chrome. */
export interface Closed {
  readonly tab: Tab;
  readonly at: number;
}

/** The closed tabs a panel remembers, the last closed on top (Chrome keeps 25). */
export const CLOSED_MAX = 25;

/**
 * The closed stack after `ids` closed from `t` (one tab, or a menu's Close others): each with its
 * index, the leftmost on top so they come back in their order. An id not open: none.
 * Pure.
 */
export function pushClosed(stack: readonly Closed[], t: Tabs, ids: readonly string[]): readonly Closed[] {
  const gone: Closed[] = [];
  t.list.forEach((tab, at) => {
    if (ids.includes(tab.id)) gone.push({ tab, at });
  });
  if (gone.length === 0) return stack;
  return [...stack, ...gone.reverse()].slice(-CLOSED_MAX);
}

/**
 * ⇧⌘T (⌃⇧T): the last closed tab back in its place (its index, or the end), in front; one open
 * again since (by another door) only comes to the front. Nothing closed: undefined. Pure.
 */
export function reopenIn(t: Tabs, stack: readonly Closed[]): { readonly tabs: Tabs; readonly stack: readonly Closed[] } | undefined {
  const top = stack.at(-1);
  if (top === undefined) return undefined;
  const rest = stack.slice(0, -1);
  if (t.list.some((x) => x.id === top.tab.id)) return { tabs: activate(t, top.tab.id), stack: rest };
  const at = Math.max(0, Math.min(t.list.length, top.at));
  return { tabs: { list: [...t.list.slice(0, at), top.tab, ...t.list.slice(at)], active: top.tab.id }, stack: rest };
}

/** Brings `id` to the front. Pure. */
export function activate(t: Tabs, id: string): Tabs {
  return t.list.some((x) => x.id === id) ? { ...t, active: id } : t;
}

/** Moves `id` to index `to` (a drag in the bar), the others keeping their order. Pure. */
export function moveIn(t: Tabs, id: string, to: number): Tabs {
  const tab = t.list.find((x) => x.id === id);
  if (tab === undefined) return t;
  const rest = t.list.filter((x) => x.id !== id);
  const at = Math.max(0, Math.min(rest.length, Math.round(to)));
  return { ...t, list: [...rest.slice(0, at), tab, ...rest.slice(at)] };
}

/** ⌘⇧] (+1) and ⌘⇧[ (-1): the next or previous tab, round the ends. Pure. */
export function stepIn(t: Tabs, dir: 1 | -1): Tabs {
  const n = t.list.length;
  if (n < 2) return t;
  const i = t.list.findIndex((x) => x.id === t.active);
  const next = t.list[(i + dir + n) % n] as Tab;
  return { ...t, active: next.id };
}

/**
 * A tab is sized to its title (designer m_43050 #5): 132 at least (designer m_50832: its icon, ~6
 * characters, the hover pin's slot and the ×), 180 at most, its title cut with '…' past it;
 * panel.css .pn-tab's min-width and max-width. TAB_GAP: the row's gap between two.
 */
export const TAB_MIN_W = 132;
export const TAB_MAX_W = 180;
export const TAB_GAP = 4;

/**
 * How many whole tabs fit `room` px, each at its own width (`widths`, in the tabs' order, as
 * measured at their titles), the one in front (`active`) always shown in the last place past the
 * rest (shownOf): only what really doesn't fit goes to the overflow menu, six at most, one at
 * least. Pure.
 */
export function fitTabs(widths: readonly number[], active: number, room: number, max = MAX_SHOWN): number {
  const w = (i: number) => Math.min(TAB_MAX_W, Math.max(TAB_MIN_W, widths[i] ?? TAB_MIN_W));
  const total = (n: number) => {
    let t = (n - 1) * TAB_GAP + w(active >= n - 1 ? active : n - 1);
    for (let i = 0; i < n - 1; i++) t += w(i);
    return t;
  };
  let n = Math.min(widths.length, max);
  while (n > 1 && total(n) > room) n--;
  return Math.max(1, n);
}

/**
 * The tabs the row shows (`max` at most, six by default, in their order) and the rest for the
 * overflow menu. The tab in front always shows: past the last place, it takes it. Pure.
 */
export function shownOf(t: Tabs, max = MAX_SHOWN): { readonly shown: readonly Tab[]; readonly over: readonly Tab[] } {
  if (t.list.length <= max) return { shown: t.list, over: [] };
  const i = t.list.findIndex((x) => x.id === t.active);
  if (i < max) return { shown: t.list.slice(0, max), over: t.list.slice(max) };
  const shown = [...t.list.slice(0, max - 1), t.list[i] as Tab];
  return { shown, over: t.list.filter((x) => !shown.includes(x)) };
}

/** The agent a tab hangs on, when it has one (its thread, its changes, its process). Pure. */
function agentOf(what: TabWhat): { readonly project: string; readonly agent: string } | undefined {
  if (what.kind !== 'thing') return undefined;
  const e = what.e;
  return e.kind === 'agent' || e.kind === 'diff' || e.kind === 'process' ? { project: e.project, agent: e.agent } : undefined;
}

/**
 * Drops the tabs of agents no longer in the rows (architect m_42089: an agent is keyed by its name,
 * a rename changes its key, a dead tab never shows). `known(project)` is the names of a project
 * whose rows came, undefined while they did not (its tabs wait). Pure.
 */
export function dropGone(t: Tabs, known: (project: string) => ReadonlySet<string> | undefined): Tabs {
  let out: Tabs = t;
  for (const x of t.list) {
    const a = agentOf(x.what);
    if (a === undefined) continue;
    const names = known(a.project);
    if (names !== undefined && !names.has(a.agent)) out = closeIn(out, x.id);
  }
  return out;
}

/**
 * Drops the tabs of artifacts no longer in their project's list (panel-scope: a tab whose artifact
 * is gone goes, quietly). `known(project)` is the ids of a project whose artifacts came, undefined
 * while they did not (its tabs wait). Pure.
 */
export function dropGoneArts(t: Tabs, known: (project: string) => ReadonlySet<string> | undefined, here?: { readonly project: string; readonly id: string }): Tabs {
  let out: Tabs = t;
  for (const x of t.list) {
    if (x.what.kind !== 'thing' || x.what.e.kind !== 'artifact') continue;
    // the route's own artifact stays: its tab says it isn't in the list any more (artifacts/gone.ts)
    if (here !== undefined && x.what.e.project === here.project && x.what.e.id === here.id) continue;
    const ids = known(x.what.e.project);
    if (ids !== undefined && !ids.has(x.what.e.id)) out = closeIn(out, x.id);
  }
  return out;
}

/** The width each kind of tab asks, kept apart: the rows, a shell (mockup 2), a page (mockup 4). */
export type WidthOf = 'rows' | 'shell' | 'page';

/** The widths each kind asks (page native-mac §6: 384; mockup 2: 480; mockup 4: 640): 384 is where the panel starts, 640 what a page widens it to (widenFor). */
export const WIDTHS: Readonly<Record<WidthOf, number>> = { rows: 384, shell: 480, page: 640 };
/** The panel's border drags it from 320 to 640 (native-mac-qa §7). */
export const MIN_W = 320;
export const MAX_W = 640;

/** Which width a tab asks: a shell's, a page's (an artifact, an office, a thread), or the rows'. Pure. */
export function widthOf(what: TabWhat): WidthOf {
  if (what.kind === 'terminal') return 'shell';
  if (what.kind === 'thing' || what.kind === 'browser') return 'page';
  return 'rows';
}

/** What the thread keeps beside the panel (ui-review m_42509: a 640 tab at 1100 left it 220). */
export const THREAD_MIN = 560;

/** Where the panel stands: beside the thread (it takes its column) or over it, as a sheet. */
export interface PanelFit {
  readonly width: number;
  /** a sheet over the thread: the window has no room for MIN_W beside a THREAD_MIN thread */
  readonly over: boolean;
}

/**
 * The panel in a window: what its tab asks, capped so the thread keeps THREAD_MIN beside the left
 * (rail and sidebar). Where even MIN_W leaves the thread under THREAD_MIN (1100 with a sidebar),
 * it goes over the thread as a sheet, at what its tab asks within the window: never a thread
 * under 560 (ui-review m_42619 #3). Pure.
 */
export function fitW(asked: number, win: number, left: number, threadMin = THREAD_MIN): PanelFit {
  const room = win - left - threadMin;
  if (room >= MIN_W) return { width: Math.min(asked, room), over: false };
  return { width: Math.max(MIN_W, Math.min(asked, win - left)), over: true };
}

/**
 * An artifact in front (r3-escape item 2, Gabriel m_44728, designer m_44757, main's ArtifactView
 * rule): the panel takes a share of the window, the thread beside it keeps ART_THREAD_MIN (not
 * THREAD_MIN), the sidebar stays. 55% for every kind, 60% for a sheet and a web page; his drag
 * keeps that kind's share within 43-70% until a double-click on the border. 43, not 50 (his
 * m_48889): the drag takes a web page 100 px narrower than before at 1440 (619, was 720), so the
 * thread beside it shows 100 px more.
 */
export const ART_THREAD_MIN = 400;
export const SHARE_MIN = 0.43;
export const SHARE_MAX = 0.7;
const SHARE_WIDE: ReadonlySet<string> = new Set(['sheet', 'link']);

/** A share in its limits. Pure. */
export function clampShare(s: number): number {
  return Math.min(SHARE_MAX, Math.max(SHARE_MIN, s));
}

/** The share of the window an artifact of `kind` (ds's iconKindOf) takes: his for that kind, else its own. Pure. */
export function shareOf(kind: string, shares: Readonly<Record<string, number>>): number {
  const his = shares[kind];
  if (his !== undefined) return clampShare(his);
  return SHARE_WIDE.has(kind) ? 0.6 : 0.55;
}

/**
 * The panel with an artifact in front: its share of the window, capped so the thread keeps
 * ART_THREAD_MIN beside the left; a sheet over the thread where fitW makes one (under 1176 with a
 * sidebar), so the window classes stay. Pure.
 */
export function artFit(share: number, win: number, left: number): PanelFit {
  const asked = Math.round(win * share);
  if (fitW(MIN_W, win, left).over) return { width: Math.max(MIN_W, Math.min(asked, win - left)), over: true };
  return { width: Math.max(MIN_W, Math.min(asked, win - left - ART_THREAD_MIN)), over: false };
}

/**
 * Full width (panel-full, his m_45439, designer m_45445): the border dragged until the thread would
 * keep under FULL_AT goes full on release, the panel over the whole content (rail and sidebar
 * stay), the thread gone and the floating composer over the panel's bottom; in full width a drag
 * right by FULL_BACK or more brings the thread back. A sheet (no room beside the thread) never goes
 * full: it already covers the thread.
 */
export const FULL_AT = ART_THREAD_MIN;
export const FULL_BACK = 120;

/** During his drag beside the thread: `raw`, the width his pointer asks (before any clamp), leaves the thread under FULL_AT. Pure. */
export function pastFull(raw: number, win: number, left: number): boolean {
  return win - left - raw < FULL_AT;
}

/** In full width: his drag right by `dx` px (the pointer's move since the press) brings the thread back. Pure. */
export function backFromFull(dx: number): boolean {
  return dx >= FULL_BACK;
}

/** The panel in full width: the window but the left (rail and sidebar); a sheet stays a sheet. Pure. */
export function fullFit(fit: PanelFit, win: number, left: number): PanelFit {
  return fit.over ? fit : { width: Math.max(fit.width, win - left), over: false };
}

/** A width in the panel's limits. Pure. */
export function clampW(w: number): number {
  return Number.isFinite(w) ? Math.round(Math.min(MAX_W, Math.max(MIN_W, w))) : WIDTHS.rows;
}

/** The panel's whole kept state: open or folded, its tabs, its one width, what he opened last. */
export interface PanelKept {
  readonly open: boolean;
  readonly tabs: Tabs;
  /** ONE width per window, whatever tab is in front (designer m_43050 #7a): his last drag, else 384 */
  readonly width: number;
  /** he dragged it: an artifact opening no longer widens it */
  readonly dragged: boolean;
  /** the things he opened in a tab, newest first (the launcher's Recent) */
  readonly recent: readonly Entity[];
  /** his drag's share of the window per artifact kind (shareOf); none: the kind's own */
  readonly shares: Readonly<Record<string, number>>;
  /** full width (fullFit): the thread gone while the panel is open; folding ends it (withOpen) */
  readonly full: boolean;
}

export const FOLDED: PanelKept = { open: false, tabs: FIRST, width: WIDTHS.rows, dragged: false, recent: [], shares: {}, full: false };

/** The state as kept: a folded panel is never full, so it opens again beside the thread. Pure. */
export function withOpen(k: PanelKept): PanelKept {
  return !k.open && k.full ? { ...k, full: false } : k;
}

/**
 * The width after `what` opens in a new tab: a page (an artifact, an office, a thread) widens it
 * once to the page's 640 (fitW caps it so the thread keeps 560), unless his drag set it; then it
 * stays for every tab. Pure.
 */
export function widenFor(k: PanelKept, what: TabWhat): number {
  // an artifact has its own width (artFit): the one width stays for the other tabs
  if (what.kind === 'thing' && what.e.kind === 'artifact') return k.width;
  return !k.dragged && widthOf(what) === 'page' ? Math.max(k.width, WIDTHS.page) : k.width;
}

/** How many things a New tab's Recent shows (the project in front's: launch.ts recentIn). */
export const RECENT_MAX = 8;
/** How many things the window's Recent keeps, every project's: enough for each one's eight. */
export const RECENT_KEPT = 64;

/** `e` first in Recent, once, RECENT_KEPT at most. Pure. */
export function pushRecent(list: readonly Entity[], e: Entity, max = RECENT_KEPT): readonly Entity[] {
  const k = entityKey(e);
  return [e, ...list.filter((x) => entityKey(x) !== k)].slice(0, max);
}

const isObj = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x);

/** An entity read back from the kept record, or undefined when it isn't one. Pure. */
function entityOf(x: unknown): Entity | undefined {
  if (!isObj(x) || typeof x['kind'] !== 'string') return undefined;
  const s = (k: string) => typeof x[k] === 'string' && x[k] !== '';
  switch (x['kind']) {
    case 'agent':
      return s('project') && s('agent') ? (x as Entity) : undefined;
    case 'project':
      return s('project') ? (x as Entity) : undefined;
    case 'artifact':
      return s('project') && s('id') ? (x as Entity) : undefined;
    case 'pr':
      return s('project') && typeof x['number'] === 'number' ? (x as Entity) : undefined;
    case 'commit':
      return s('project') && s('sha') ? (x as Entity) : undefined;
    case 'diff':
      return s('project') && s('agent') ? (x as Entity) : undefined;
    case 'process':
      return s('project') && s('agent') && s('id') ? (x as Entity) : undefined;
    case 'plugin':
      return s('id') ? (x as Entity) : undefined;
    case 'scheduled':
      return s('project') && typeof x['id'] === 'number' ? (x as Entity) : undefined;
    case 'office':
      return s('project') ? (x as Entity) : undefined;
    default:
      return undefined;
  }
}

/** A tab's what read back, or undefined. Pure. */
function whatOf(x: unknown): TabWhat | undefined {
  if (!isObj(x)) return undefined;
  switch (x['kind']) {
    case 'launcher':
      if (typeof x['n'] !== 'number') return undefined;
      return x['artifacts'] === true ? { kind: 'launcher', n: x['n'], artifacts: true } : { kind: 'launcher', n: x['n'] };
    case 'browser':
      return typeof x['n'] === 'number' ? { kind: 'browser', n: x['n'] } : undefined;
    case 'terminal':
      return typeof x['project'] === 'string' && x['project'] !== '' && typeof x['n'] === 'number' ? { kind: 'terminal', project: x['project'], n: x['n'] } : undefined;
    case 'thing': {
      const e = entityOf(x['e']);
      return e === undefined ? undefined : { kind: 'thing', e };
    }
    default:
      return undefined;
  }
}

/** The kept record read back: anything unreadable is dropped, a broken record reads as FOLDED. Pure. */
export function parseKept(raw: unknown): PanelKept {
  if (!isObj(raw)) return FOLDED;
  const tabsRaw = isObj(raw['tabs']) && Array.isArray(raw['tabs']['list']) ? (raw['tabs']['list'] as unknown[]) : [];
  const seen = new Set<string>();
  const list: Tab[] = [];
  for (const x of tabsRaw) {
    const what = whatOf(isObj(x) ? x['what'] : undefined);
    if (what === undefined) continue;
    const tab = tabOf(what);
    if (seen.has(tab.id)) continue;
    seen.add(tab.id);
    list.push(tab);
  }
  const want = isObj(raw['tabs']) ? raw['tabs']['active'] : undefined;
  const tabs: Tabs = { list, active: list.some((x) => x.id === want) ? (want as string) : (list[0]?.id ?? '') };
  const width = typeof raw['width'] === 'number' ? clampW(raw['width']) : WIDTHS.rows;
  const recent = (Array.isArray(raw['recent']) ? (raw['recent'] as unknown[]) : []).map(entityOf).filter((e): e is Entity => e !== undefined).slice(0, RECENT_KEPT);
  const shares: Record<string, number> = {};
  if (isObj(raw['shares'])) for (const [k, v] of Object.entries(raw['shares'])) if (typeof v === 'number' && Number.isFinite(v)) shares[k] = clampShare(v);
  const open = raw['open'] === true;
  return { open, tabs, width, dragged: raw['dragged'] === true, recent, shares, full: open && raw['full'] === true };
}
