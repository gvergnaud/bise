// The sidebar's words and signs (designer's desktop-screens v6/v7, signed): one
// sign per agent in a 15 px mono column, the same signs as the TUI's, what
// a project's row says on its right, which done agents fold into 'n more
// today', and the window's widths (the sidebar slides over under 1000 px,
// the frame is wide from 1100 px). Owner: amb-win. Pure:
// Sidebar.tsx and App.tsx draw these.
import type { Agent, ComputerAgent, ProjectRow } from '@/proto';
import { hero } from '@/proto/words_hero.gen';
import { panel } from '@/proto/words_panel.gen';
import { running } from '@/proto/words_running.gen';
import { MAIN, type Route } from './route';
import { pageOf } from '@/settings/pages';
import { shortAge } from '@/agents/model';
import { pauseLine, shownPause } from '@/thread/wake';

/** A sign's tone: ink (at work), accent (asks him), err (failed), dim (the rest). */
export type Tone = 'ink' | 'acc' | 'err' | 'dim';

export interface Sign {
  /** the mono glyph: ∿ ? ⋯ ✗ ◷ ✓ – */
  readonly g: string;
  readonly tone: Tone;
  /** the row's end words ('14m', 'done', '14:20'); '' when a count says it */
  readonly end: string;
  readonly endTone: 'dim' | 'err';
  /** the pink count of what it asks him (0: none) */
  readonly count: number;
  /** the hover words: its state in full (never by color alone) */
  readonly title: string;
}

const pad = (n: number) => String(n).padStart(2, '0');
const hm = (ms: number) => {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
};

/** A turn's length, in the one short age (ambient m_12439): 'now', '6 min', '2 h', '3 d'. */
export function turnShort(ms: number): string {
  return shortAge(ms);
}

/**
 * An agent's sign: what it asks him first (? and a pink count), then at work
 * (∿ and its turn time), waiting on someone (⋯, who on hover), failed (✗),
 * a next wake (◷ and its time), done (✓), at rest (–). `wake`: the next run
 * of a scheduled task that wakes it. Pure.
 */
export function signOf(a: Agent, now: number, wake?: number): Sign {
  const what = a.title !== '' ? a.title : a.purpose;
  if (a.waits > 0) return { g: '?', tone: 'acc', end: '', endTone: 'dim', count: a.waits, title: `${panel.asks_you} · ${what}` };
  // R9/S3: whom it waits on, typed by the hub (the TUI's panel: ? when it is him, … another agent)
  if (a.waiting_on?.who === 'you') return { g: '?', tone: 'acc', end: '', endTone: 'dim', count: 0, title: `${panel.asks_you} · ${what}` };
  // a pause (agent-waiting-states A): it continues when a command ends or another agent answers:
  // '…' in ink, how long on the right ('3 min'), the pause line on hover; never the accent (it
  // doesn't wait on him). An alarm leaves it idle: it is free.
  // whom it waits on by the name the window gives it ('bise answers', never 'main answers')
  const raw = pauseLine(a, now);
  const pause = raw === null ? null : shownPause(raw);
  if (pause !== null) return { g: '…', tone: 'ink', end: pause.len, endTone: 'dim', count: 0, title: `${pause.title} · ${what}` };
  switch (a.status) {
    case 'working':
      return { g: '∿', tone: 'ink', end: turnShort(a.turn_ms ?? now - a.since_ms), endTone: 'dim', count: 0, title: `${panel.state_working} · ${what}` };
    case 'waiting':
    case 'blocked':
      return { g: '⋯', tone: 'dim', end: panel.state_waiting, endTone: 'dim', count: 0, title: `${panel.state_waiting} · ${what}` };
    case 'failed':
      return { g: '✗', tone: 'err', end: panel.state_failed, endTone: 'err', count: 0, title: `${panel.state_failed} · ${what}` };
    default:
      break;
  }
  if (wake !== undefined) return { g: '◷', tone: 'dim', end: hm(wake), endTone: 'dim', count: 0, title: `${panel.wakes_at({ at: hm(wake) })} · ${what}` };
  if (a.status === 'done') return { g: '✓', tone: 'dim', end: panel.state_done, endTone: 'dim', count: 0, title: `${panel.state_done} · ${what}` };
  // at rest: the hub's word, the one the agent's head chip says too (designer m_19784 (5): '– stopped'
  // here and 'idle' there disagreed); the TUI's ○ for idle
  return { g: '○', tone: 'dim', end: panel.state_idle, endTone: 'dim', count: 0, title: `${panel.state_idle} · ${what}` };
}

/**
 * S29/L19 W3: the core's mark beside an agent that uses his computer (amb-core m_16595): '↖' in
 * ink while it drives, '?' in the accent when he took the wheel, dim when stopped; its words on
 * hover; none without a row. Pure.
 */
export function computerDot(r: ComputerAgent | undefined): { g: string; tone: Tone; title: string } | null {
  if (r === undefined) return null;
  const tone: Tone = r.kind === 'driving' ? 'ink' : r.kind === 'paused' ? 'acc' : 'dim';
  return { g: r.mark, tone, title: r.words };
}

/** A gauge worth its place: some words, and not an empty context ('0%'). Pure. */
export function gaugeSays(short: string | null | undefined): boolean {
  const w = (short ?? '').trim();
  return w !== '' && !/(^|\s)0%$/.test(w);
}

/** How long a done agent stays in the sidebar before it folds. */
export const FOLD_AFTER_MS = 3_600_000;

/**
 * A done agent quiet for over an hour (not one that waits, works, failed or wakes). An idle one
 * stays: it is at rest, waiting for his next message, not finished. Pure.
 */
export function folds(a: Agent, now: number, wake?: number): boolean {
  if (a.waits > 0 || wake !== undefined) return false;
  if (a.status !== 'done') return false;
  return now - a.since_ms > FOLD_AFTER_MS;
}

/** The fold's words: '3 more today', or '3 more' when some are older than today. Pure. */
export function moreWords(folded: readonly Agent[], now: number): string {
  const today = new Date(now).setHours(0, 0, 0, 0);
  const n = folded.length;
  return folded.every((a) => a.since_ms >= today) ? panel.more_today({ n }) : panel.more({ n });
}

/**
 * A project row's right side: what goes wrong in words (err), else the number of its agents at
 * work ('3', dim: his m_25471, the count lost with '∿ n'), and his pink count (`waiting`: the
 * inbox's own count of that project, inbox/waiting.ts projectWaiting). Quiet says nothing:
 * its mark does (desktop-delight v2, Marks.tsx). Pure.
 */
export function projectSide(p: ProjectRow, refused = false, waiting = p.waits): { text: string; err: boolean; count: number } {
  if (p.missing === true) return { text: panel.folder_gone, err: true, count: 0 };
  if (refused) return { text: panel.refused, err: true, count: 0 };
  return { text: p.working > 0 ? String(p.working) : '', err: false, count: waiting };
}

/**
 * The project the right panel shows here: the route's; on bise's own thread every project's ('',
 * designer m_12093), none while he has no project; none on the global screens. Pure.
 */
export function panelProject(route: Route, home: string | undefined, projects: number): string | undefined {
  const all = projects > 0 ? '' : undefined;
  switch (route.screen) {
    case 'bise':
      return all;
    case 'thread':
      return route.project === home ? all : route.project;
    case 'agents':
    case 'changes':
    case 'debug':
    case 'feature':
    case 'prs':
    case 'environments':
      return route.project;
    default:
      return undefined;
  }
}

/** The titlebar's crumb: where he is, 'storefront › perf' (a project's main is the project). Pure. */
/** The library's crumb after 'artifacts': each part's words and its place ('desktop app' f/<theme>, 'agent view' f/<task>). */
export type PlaceCrumb = (at: string) => readonly { name: string; at: string }[];

/** A plugin's name in the crumb from its id: its own part, without its kind's prefix ('skills:bise' → 'bise'). Pure. */
export function pluginCrumb(id: string): string {
  return id.replace(/^(skills|mistral):/u, '');
}

/** `plugin`: a plugin's title from its id ('mistral:linear' → 'Linear'), the plugins' rows'; pluginCrumb when unknown. */
export function crumbOf(route: Route, name: (project: string) => string, place: PlaceCrumb = () => [], plugin: (id: string) => string = pluginCrumb): string[] {
  switch (route.screen) {
    case 'bise':
      return [panel.bise];
    case 'thread':
      return route.agent === MAIN ? [name(route.project)] : [name(route.project), route.agent];
    case 'agents':
      // /archived is its own list, its crumb says so (designer m_19489)
      // the page's title, chrome in sentence case (designer m_43933 #1; nm-pages-b): its banner's
      return [name(route.project), route.archived === true ? hero.archived_title : hero.agents_title];
    case 'changes':
      return [name(route.project), hero.changes_title];
    case 'prs':
      return [name(route.project), hero.prs_title];
    case 'environments':
      return [name(route.project), hero.environments_title];
    case 'debug':
      return [name(route.project), route.agent, panel.crumb_debug];
    case 'feature':
      // its name alone, no text glyph in the bar (ui-review m_43947 #3)
      return [name(route.project), route.name];
    case 'artifacts':
      // the library's place after it: 'artifacts › desktop app › agent view' (artifacts/folders.ts crumbOf)
      return [...(route.project === '' ? [panel.artifacts] : [name(route.project), panel.artifacts]), ...(route.at === undefined ? [] : place(route.at).map((c) => c.name))];
    case 'settings':
      // the page's title, the nav's words ('computer use', never its id 'computer')
      return route.page === undefined ? [panel.crumb_settings] : [panel.crumb_settings, pageOf(route.page).title];
    case 'notfound':
      return [panel.crumb_not_found];
    case 'report':
      // the page's own words (designer m_17010)
      return [route.log === 'restart' ? hero.update_log_title : hero.crash_title];
    case 'lab':
      return [panel.crumb_lab, route.lab];
    case 'plugins':
      // a plugin's page: 'Plugins › bise', its title, never its id ('skills:bise'; designer m_43933 #2, ui-review m_43947 #1)
      return route.id === undefined ? [hero.plugins_title] : [hero.plugins_title, plugin(route.id)];
    case 'running':
      // its noun, not its route's name (designer m_38642); in a scope, whose: 'acme › perf › processes'
      if (route.scope === undefined) return [running.heading];
      return route.scope.agent === undefined ? [name(route.scope.project), running.heading] : [name(route.scope.project), route.scope.agent, running.heading];
    default:
      return [route.screen];
  }
}

/**
 * The crumb's segments before the last, as his way back along them (his ask, desktop-history's
 * third way besides ‹ and esc): each earlier one a route, the last (where he is) plain. The
 * project's name → its main's thread, an agent's name (a review's, a log's) → its thread,
 * settings' word → its root, 'plugins' → its list; a screen's own word ('agents', 'changes'…) is where he is, and the
 * words that name one thing of the project (a feature, a commit, a branch) stay plain. Pure.
 */
export function crumbGoOf(route: Route, name: (project: string) => string, place: PlaceCrumb = () => []): { c: string; go?: Route }[] {
  return crumbOf(route, name, place).slice(0, -1).map((c, i) => {
    // the library's crumb: 'artifacts' goes to all artifacts, a theme to its folder
    if (route.screen === 'artifacts' && route.at !== undefined) {
      const root = route.project === '' ? 0 : 1;
      if (i === root) return { c, go: { screen: 'artifacts', project: route.project } };
      const part = place(route.at)[i - root - 1];
      if (part !== undefined) return { c, go: { screen: 'artifacts', project: route.project, at: part.at } };
    }
    if ('project' in route && route.project !== '' && i === 0 && c === name(route.project))
      return { c, go: { screen: 'thread', project: route.project, agent: MAIN } };
    if (route.screen === 'debug' && i === 1)
      return { c, go: { screen: 'thread', project: route.project, agent: route.agent } };
    // processes in a scope: its project's main, its agent's thread
    if (route.screen === 'running' && route.scope !== undefined)
      return { c, go: { screen: 'thread', project: route.scope.project, agent: i === 0 ? MAIN : (route.scope.agent ?? MAIN) } };
    if (c === panel.crumb_settings) return { c, go: { screen: 'settings' } };
    // a plugin's page: 'plugins' → the list it came from (skills' list keeps its kind)
    if (route.screen === 'plugins' && i === 0)
      return { c, go: route.kind === undefined ? { screen: 'plugins' } : { screen: 'plugins', kind: route.kind } };
    // 'lab' alone stays plain: test-only, no lab index to go to
    return { c };
  });
}

/** Under this width the sidebar (and the right panel) slide over the window. */
export const NARROW_PX = 1000;
/** From this width the frame is 'wide' (Frame.tsx widthOf); the panel stays folded by default at any width (panel/state.ts). */
export const PANEL_OPEN_PX = 1100;

/**
 * The title bar's panel button words after its '↗': 'artifacts · 5' ('5' under 1000 px); at zero
 * the count goes ('artifacts', '' when narrow), designer m_18734. Pure.
 */
export function panelWords(narrow: boolean, count: number): string {
  if (count <= 0) return narrow ? '' : panel.artifacts;
  return narrow ? String(count) : panel.artifacts_count({ n: count });
}
