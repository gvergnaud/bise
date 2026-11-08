// The rail's places and the bise tab's way back (his m_49841): the ':*' button, ⌘0 and Go › bise
// bring him back to the project or chat he was in before he went to a page (inbox, scheduled,
// artifacts, processes, plugins, settings), never to a default screen. The window remembers his last place
// in bise's tab (a project, a chat, its changes...) in its keep (keep.ts, no age: a relaunch
// finds it too); from bise's tab itself the button is the tab's root, bise's own thread, as a
// Mac tab bar's. A remembered place that is gone comes back as what of it is still there: an
// agent dropped → its project's main, a project removed → bise's thread. Nothing remembered (the
// first launch): bise's thread. Owner: nm-frame (the rail); the keep is amb-win's.
import type { Agent, ProjectRow } from '@/proto';
import { keep, type Keep } from './keep';
import { formatRoute, MAIN, parseRoute, type Route } from './route';

/** A rail place: the route it goes to and the key that does too. */
export type RailPlace = 'bise' | 'inbox' | 'scheduled' | 'artifacts' | 'processes' | 'plugins' | 'settings';

/** Which rail place a route sits in: the pages are theirs (every processes route, all, scoped or one
 *  open, is the processes place's), every thread and project screen bise's. Pure. */
export function railPlaceOf(route: Route): RailPlace {
  switch (route.screen) {
    case 'inbox':
    case 'scheduled':
    case 'artifacts':
    case 'plugins':
    case 'settings':
      return route.screen;
    case 'running':
      return 'processes';
    default:
      return 'bise';
  }
}

/** Where each place's button goes from another place; bise's is its root, the click goes back (biseGo). */
export const RAIL_GO: Record<RailPlace, Route> = {
  bise: { screen: 'bise' },
  inbox: { screen: 'inbox' },
  scheduled: { screen: 'scheduled' },
  artifacts: { screen: 'artifacts', project: '' },
  processes: { screen: 'running' },
  plugins: { screen: 'plugins' },
  settings: { screen: 'settings' },
};

/** The keep's view key of his last place in bise's tab (the main window's only). */
export const BISE_AT_KEY = 'route.bise';

/** A place of bise's tab worth coming back to: never a page's, the first run, a lab or a wrong link. Pure. */
export function backTo(r: Route): boolean {
  return railPlaceOf(r) === 'bise' && r.screen !== 'onboarding' && r.screen !== 'lab' && r.screen !== 'notfound';
}

/** What the window knows of his projects and their agents (the store's). */
export interface Here {
  readonly projects: readonly ProjectRow[];
  readonly agents: (project: string) => readonly Agent[];
}

/**
 * What of `r` is still there: its project removed → bise's thread; its agent dropped (archived or
 * gone from a list that came) → its project's main. A list not come yet counts as there (the
 * start, before the hub's rows). Pure.
 */
export function stillThere(r: Route, h: Here): Route {
  if (!('project' in r) || r.project === '') return r;
  if (h.projects.length > 0 && !h.projects.some((p) => p.project === r.project)) return RAIL_GO.bise;
  if (!('agent' in r) || r.agent === MAIN) return r;
  const agents = h.agents(r.project);
  if (agents.length === 0) return r;
  const a = agents.find((x) => x.name === r.agent);
  return a === undefined || a.archived ? { screen: 'thread', project: r.project, agent: MAIN } : r;
}

/**
 * Where the bise tab goes from `now`: in bise's tab already, its root (bise's thread); from a
 * page, his kept place in it (`kept`, its route string), what of it is still there; nothing
 * kept, bise's thread. Pure.
 */
export function biseGo(now: Route, kept: unknown, h: Here): Route {
  if (railPlaceOf(now) === 'bise') return RAIL_GO.bise;
  const r = typeof kept === 'string' && kept !== '' ? parseRoute(kept) : undefined;
  return r === undefined || !backTo(r) ? RAIL_GO.bise : stillThere(r, h);
}

/** The bise tab's memory over a keep: `saw` each route the window shows, `go` where the tab goes. */
export interface BiseBack {
  saw(r: Route): void;
  go(now: Route, h: Here): Route;
}

export function createBiseBack(keep: Pick<Keep, 'view' | 'setView'>): BiseBack {
  return {
    saw(r) {
      if (backTo(r)) keep.setView(BISE_AT_KEY, formatRoute(r));
    },
    go: (now, h) => biseGo(now, keep.view(BISE_AT_KEY), h),
  };
}

/** The main window's: App.tsx tells it each route; the rail's button, ⌘0 and Go › bise ask it. */
export const biseBack: BiseBack = createBiseBack(keep);
