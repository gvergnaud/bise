// The app's pages as ⌘K finds them by name (ambient-lead m_21499, designer m_21505): inbox, help,
// scheduled, artifacts, settings and each settings page, and per project its agents, archived,
// changes, pull requests, environments, artifacts; the log of the agent in view. Owner: amb-web.
// Pure: no React, no store.
//
// The law: SCREENS is keyed by the router's Screen (route.ts), so a new screen fails the typecheck
// until it says here whether ⌘K lists it and how, or why not; pages.test.ts checks that every page
// listed is a route the router reads back. Not routes: the inbox (opened in place, ⌘I) and the key
// list (help, ⌘/); they are the ASKS below.
import type { SearchPage } from '@/search/rank';
import { running } from '@/proto/words_running.gen';
import { panel } from '@/proto/words_panel.gen';
import { PLACE_KEYS } from '@/settings/keys';
import { formatRoute, MAIN, projectOf, type Route, type Screen } from './route';

interface Named {
  readonly name: string;
  readonly words?: readonly string[];
  readonly glyph?: string;
}

/** What a screen is to ⌘K: pages for the app, for each project, for the agent in view, or none and why. */
type Rule =
  | { readonly app: Named & { readonly route: Route; readonly key?: string } }
  | { readonly project: Named & { readonly route: (p: string) => Route } }
  | { readonly agent: Named & { readonly route: (p: string, a: string) => Route } }
  | { readonly settings: true }
  | { readonly no: string };

/** Every screen of the router, said once. */
export const SCREENS: Record<Screen, readonly Rule[]> = {
  bise: [{ no: "bise's thread: the search's agents rows" }],
  thread: [{ no: "an agent's thread: the search's agents rows" }],
  agents: [
    { project: { name: 'agents', words: ['live', 'tasks'], route: (p) => ({ screen: 'agents', project: p }) } },
    { project: { name: 'archived', words: ['archived agents'], route: (p) => ({ screen: 'agents', project: p, archived: true }) } },
  ],
  changes: [{ project: { name: 'changes', words: ['branches', 'merged', 'diff'], route: (p) => ({ screen: 'changes', project: p }) } }],
  prs: [{ project: { name: 'pull requests', words: ['prs'], route: (p) => ({ screen: 'prs', project: p }) } }],
  debug: [{ no: "an agent's raw session and requests: /debug only (Gabriel's ask, debug-cmd)" }],
  feature: [{ no: 'one feature by its name: /feature' }],
  artifacts: [
    { app: { name: 'artifacts', glyph: '↗', words: ['every project'], key: PLACE_KEYS.artifacts, route: { screen: 'artifacts', project: '' } } },
    { project: { name: 'artifacts', glyph: '↗', route: (p) => ({ screen: 'artifacts', project: p }) } },
  ],
  environments: [{ project: { name: 'environments', words: ['worktrees', 'dev servers'], route: (p) => ({ screen: 'environments', project: p }) } }],
  inbox: [{ app: { name: 'inbox', glyph: '?', words: ['answered', 'history', 'questions'], key: PLACE_KEYS.inbox, route: { screen: 'inbox' } } }],
  scheduled: [{ app: { name: 'scheduled', glyph: '◷', words: ['timers', 'every'], key: PLACE_KEYS.scheduled, route: { screen: 'scheduled' } } }],
  plugins: [{ app: { name: panel.plugins, glyph: '⊞', words: ['skills', 'tools', 'MCP', 'connectors', 'Mistral'], key: PLACE_KEYS.plugins, route: { screen: 'plugins' } } }],
  running: [{ app: { name: running.heading, glyph: '∿', words: running.find_words.split(' · '), key: PLACE_KEYS.processes, route: { screen: 'running' } } }],
  report: [{ no: 'the last crash report: only after a crash, from its line' }],
  settings: [{ app: { name: 'settings', glyph: '⚙', words: ['preferences'], key: PLACE_KEYS.settings, route: { screen: 'settings' } } }, { settings: true }],
  onboarding: [{ no: "the first run's; settings › setup checks it again" }],
  lab: [{ no: 'test mode only' }],
  notfound: [{ no: 'a wrong link' }],
};

/** The pages that are not routes: opened in place, over the screen he is on. */
export const ASKS: readonly SearchPage[] = [
  { id: 'inbox', name: 'inbox', words: ['questions', 'notifications', 'cards', 'waiting'], glyph: '#', key: '⌘I', go: { inbox: true } },
  { id: 'help', name: 'help', words: ['keys', 'shortcuts'], glyph: '·', key: '⌘/', go: { help: true } },
];

export interface PagesCtx {
  /** his projects, bise's home first (searchDataOf's order) */
  readonly projects: readonly { readonly id: string; readonly name: string; readonly home?: boolean }[];
  /** the route in view: its project's pages first, its agent's log */
  readonly route: Route;
  /** settings' own pages (settings/pages.tsx PAGES) */
  readonly settings: readonly { readonly id: string; readonly title: string }[];
}


/** Every page ⌘K lists, in the app's order: the asks, the app's, then each project's (the one in view first), the agent in view's. */
export function appPages(c: PagesCtx): SearchPage[] {
  const route = (r: Route, n: Named, extra: Partial<SearchPage> = {}): SearchPage => ({
    id: formatRoute(r),
    name: n.name,
    words: n.words ?? [],
    glyph: n.glyph ?? '·',
    key: '',
    go: { route: formatRoute(r) },
    ...extra,
  });
  const rules = Object.values(SCREENS).flat();
  const here = projectOf(c.route);
  const pname = (id: string): string => {
    const p = c.projects.find((x) => x.id === id);
    return p === undefined ? id : p.home === true ? 'bise' : p.name;
  };
  // ⌘D from a project's thread opens its changes (from an agent's: its review)
  const dKey = (p: string): string => (p === here && c.route.screen === 'thread' && c.route.agent === MAIN ? '⌘D' : '');
  const out: SearchPage[] = [...ASKS];
  for (const r of rules) {
    if ('app' in r) out.push(route(r.app.route, r.app, { key: r.app.key ?? '' }));
    if ('settings' in r)
      for (const s of c.settings) out.push(route({ screen: 'settings', page: s.id }, { name: s.title, words: ['settings', s.id], glyph: '⚙' }, { parent: 'settings' }));
  }
  const ids = c.projects.map((p) => p.id);
  const order = here !== undefined && ids.includes(here) ? [here, ...ids.filter((p) => p !== here)] : ids;
  for (const p of order)
    for (const r of rules)
      if ('project' in r) {
        const to = r.project.route(p);
        // a project's own page has no key but ⌘D (⇧⌘A is every project's library, the rail's)
        const key = to.screen === 'changes' ? dKey(p) : '';
        out.push(route(to, r.project, { of: pname(p), key }));
      }
  if (c.route.screen === 'thread')
    for (const r of rules)
      if ('agent' in r) {
        const { project: p, agent: a } = c.route;
        out.push(route(r.agent.route(p, a), r.agent, { of: a === MAIN ? pname(p) : `${a} · ${pname(p)}` }));
      }
  return out;
}
