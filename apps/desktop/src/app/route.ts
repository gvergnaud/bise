// The window's routes: which screen shows, for which project and agent.
// Owner: amb-win. Pure: no React, no store, no Electron; the router
// (router.ts) holds the current one and the shell (App.tsx) draws it.
//
// A route has a string form, the one deep links, the core's `open` and the
// test ids use (`route:<string>`), kept from the old window so his links
// and amb-tools' scenarios carry over:
//   bise                       bise's own thread (the home project's main)
//   project/<p>                a project's main thread
//   agent/<p>/<a>              one agent's thread
//   agents/<p>                 a project's agents list (its live ones)
//   agents/<p>/archived        its archived agents (/archived)
//   changes/<p>                a project's changes (branches, merged today)
//   review/<p>/<a>             an agent's change, to review and merge
//   debug/<p>/<a>[/completion[/<provider>]]  /debug: an agent's raw session, or a request as sent
//   feature/<p>/<name>         a feature: its branch, agents, try / merge / drop
//   prs/<p>                    its open pull requests (the TUI's /prs)
//   commit/<p>/<sha>           one commit already on main, read-only
//   branch/<p>/<branch>        a branch's diff by its name ('/diff sb/x'; the branch keeps its slashes)
//   artifacts/<p>[/<id>]       a project's artifacts (one opened)
//   artifacts[/<p>]/~/<place>  the library at a place: recent, unfiled, f/<folder id> (artifacts/folders.ts atPath)
//   environments/<p>           its worktrees and dev servers
//   settings[/<page>] | onboarding    amb-home's screens
//   plugins[/skills | /p/<id>] the plugins place (every kind, or skills only: /skills), a plugin's page
//   inbox/all                  the inbox screen: what waits on him, then what he answered (inbox-screen)
//   ('inbox' and 'inbox?card=<p>/<id>' stay asks to open the block above the composer in place,
//   read by inboxAtOf, never routes; 'tab?artifact=<p>/<id>[&at=<block>]' asks to open an
//   artifact in the panel's tab, shared/asks.ts artifactAskOf)
//   lab/<composer|spotlight|pearl>             amb-web's labs, test mode only
// Anything else parses to `notfound` with the string kept, never to bise:
// a wrong link says so.

export type Route =
  | { screen: 'bise' }
  | { screen: 'thread'; project: string; agent: string }
  /** its live agents (/agents); `archived`: only its archived ones (/archived, as the TUI) */
  | { screen: 'agents'; project: string; archived?: true }
  | { screen: 'changes'; project: string }
  | { screen: 'prs'; project: string }
  | { screen: 'review'; project: string; agent: string }
  /** /debug: an agent's raw session (history) or a request as its provider got it (completion,
      as `provider` would get it when named), the TUI's logview.rs */
  | { screen: 'debug'; project: string; agent: string; tab?: 'completion'; provider?: string }
  | { screen: 'feature'; project: string; name: string }
  | { screen: 'commit'; project: string; sha: string }
  | { screen: 'branch'; project: string; branch: string }
  /** `at`: the library's place (artifacts/folders.ts atPath: 'recent', 'unfiled', 'f/<id>'); none: all artifacts */
  | { screen: 'artifacts'; project: string; id?: string; at?: string }
  | { screen: 'environments'; project: string }
  | { screen: 'scheduled' }
  /**
   * the plugins place (page plugins v48): the list (`kind: 'skills'`: /skills, the same list on
   * skills), or one plugin's page (`id`: its PluginId as written, `mistral:gmail`)
   */
  | { screen: 'plugins'; kind?: 'skills'; id?: string }
  /**
   * the processes his agents left running (designer's running-procs): every project's, or the bar's
   * scope (`scope`: a project's, an agent's; its 'all ›', designer m_38642); `open`: that one's
   * output beside the list
   */
  | { screen: 'running'; scope?: { project: string; agent?: string }; open?: { project: string; agent: string; id: string } }
  /** every project's waiting items, then what he answered or what was withdrawn (inbox-history pick A) */
  | { screen: 'inbox' }
  /** the last crash report, opened in the window (designer's voice-window part 2); `log: 'restart'`
   * the last update's restart.log (settings › general's 'show the log', designer's app-swap v2) */
  | { screen: 'report'; log?: 'restart' }
  | { screen: 'settings'; page?: string }
  | { screen: 'onboarding' }
  | { screen: 'lab'; lab: Lab }
  | { screen: 'notfound'; path: string };

/** amb-web's test-only labs (qa-type m_9587): drawn in test mode only. */
export const LABS = ['composer', 'spotlight', 'pearl'] as const;
export type Lab = (typeof LABS)[number];
const isLab = (x: string | undefined): x is Lab => LABS.some((l) => l === x);

export type Screen = Route['screen'];

/** The agent every project has: its main. */
export const MAIN = 'main';

const SHA = /^[0-9a-f]{4,40}$/;
// a project id or an agent name: no slash, no space, not empty
const NAME = /^[^/\s]+$/;

/** A route's string form (the inverse of `parseRoute`). */
export function formatRoute(r: Route): string {
  switch (r.screen) {
    case 'bise':
    case 'scheduled':
    case 'onboarding':
      return r.screen;
    case 'running':
      // its output open: 3 names; a scope: 'in/<project>[/<agent>]'
      if (r.open !== undefined) return `running/${r.open.project}/${r.open.agent}/${r.open.id}`;
      if (r.scope === undefined) return 'running';
      return r.scope.agent === undefined ? `running/in/${r.scope.project}` : `running/in/${r.scope.project}/${r.scope.agent}`;
    case 'report':
      return r.log === 'restart' ? 'report/restart' : 'report';
    case 'plugins':
      if (r.id !== undefined) return `plugins/p/${encodeURIComponent(r.id)}`;
      return r.kind === 'skills' ? 'plugins/skills' : 'plugins';
    case 'inbox':
      return 'inbox/all';
    case 'thread':
      return r.agent === MAIN ? `project/${r.project}` : `agent/${r.project}/${r.agent}`;
    case 'agents':
      return r.archived === true ? `agents/${r.project}/archived` : `agents/${r.project}`;
    case 'changes':
    case 'prs':
    case 'environments':
      return `${r.screen}/${r.project}`;
    case 'review':
      return `review/${r.project}/${r.agent}`;
    case 'debug': {
      const base = `debug/${r.project}/${r.agent}`;
      if (r.tab === undefined) return base;
      return r.provider === undefined ? `${base}/completion` : `${base}/completion/${r.provider}`;
    }
    case 'feature':
      return `feature/${r.project}/${r.name}`;
    case 'commit':
      return `commit/${r.project}/${r.sha}`;
    case 'branch':
      return `branch/${r.project}/${r.branch}`;
    case 'artifacts':
      // project '': every project's (the sidebar's '↗ artifacts', amb-kit)
      if (r.id !== undefined) return `artifacts/${r.project}/${r.id}`;
      if (r.at !== undefined) return r.project === '' ? `artifacts/~/${r.at}` : `artifacts/${r.project}/~/${r.at}`;
      return r.project === '' ? 'artifacts' : `artifacts/${r.project}`;
    case 'settings':
      return r.page === undefined ? 'settings' : `settings/${r.page}`;
    case 'lab':
      return `lab/${r.lab}`;
    case 'notfound':
      return r.path;
  }
}

/** A route from its string form; an unknown or malformed one is `notfound`. */
export function parseRoute(path: string): Route {
  const parts = path.trim().replace(/^\/+|\/+$/g, '').split('/');
  const head = parts[0] ?? '';
  const args = parts.slice(1);
  const nf: Route = { screen: 'notfound', path };
  // the arguments, if there are exactly `n` and each is a name
  const take = (n: number): string[] | undefined => (args.length === n && args.every((x) => NAME.test(x)) ? args : undefined);
  const one = take(1)?.[0];
  const [a, b] = take(2) ?? [];
  switch (head) {
    case 'bise':
    case 'scheduled':
    case 'onboarding':
      return args.length === 0 ? { screen: head } : nf;
    case 'running': {
      if (args.length === 0) return { screen: 'running' };
      if (args[0] === 'in') {
        const [, p, ag] = args;
        if (p === undefined || !NAME.test(p) || args.length > 3 || (ag !== undefined && !NAME.test(ag))) return nf;
        return { screen: 'running', scope: ag === undefined ? { project: p } : { project: p, agent: ag } };
      }
      const [project, agent, id] = take(3) ?? [];
      return project !== undefined && agent !== undefined && id !== undefined ? { screen: 'running', open: { project, agent, id } } : nf;
    }
    case 'report':
      if (args.length === 0) return { screen: 'report' };
      return one === 'restart' ? { screen: 'report', log: 'restart' } : nf;
    case 'inbox':
      return one === 'all' ? { screen: 'inbox' } : nf;
    case 'plugins': {
      if (args.length === 0) return { screen: 'plugins' };
      if (one === 'skills') return { screen: 'plugins', kind: 'skills' };
      const [p, id] = args;
      if (p !== 'p' || id === undefined || id === '' || args.length !== 2) return nf;
      try {
        return { screen: 'plugins', id: decodeURIComponent(id) };
      } catch {
        return nf;
      }
    }
    case 'project':
      return one !== undefined ? { screen: 'thread', project: one, agent: MAIN } : nf;
    case 'agent':
      return a !== undefined && b !== undefined ? { screen: 'thread', project: a, agent: b } : nf;
    case 'agents':
      if (a !== undefined && b === 'archived') return { screen: 'agents', project: a, archived: true };
      return one !== undefined ? { screen: 'agents', project: one } : nf;
    case 'changes':
    case 'prs':
    case 'environments':
      return one !== undefined ? { screen: head, project: one } : nf;
    case 'review':
      return a !== undefined && b !== undefined ? { screen: head, project: a, agent: b } : nf;
    case 'debug': {
      // debug/<p>/<a>[/completion[/<provider or provider/model>]]
      const [p, ag, tab, ...prov] = args;
      if (p === undefined || ag === undefined || !NAME.test(p) || !NAME.test(ag)) return nf;
      if (tab === undefined) return { screen: 'debug', project: p, agent: ag };
      if (tab !== 'completion' || prov.length > 2 || !prov.every((x) => NAME.test(x))) return nf;
      return prov.length === 0
        ? { screen: 'debug', project: p, agent: ag, tab }
        : { screen: 'debug', project: p, agent: ag, tab, provider: prov.join('/') };
    }
    case 'feature':
      return a !== undefined && b !== undefined ? { screen: 'feature', project: a, name: b } : nf;
    case 'commit':
      return a !== undefined && b !== undefined && SHA.test(b) ? { screen: 'commit', project: a, sha: b } : nf;
    case 'branch': {
      // everything after the project is the branch, slashes kept ('sb/x')
      const [p, ...rest] = args;
      const branch = rest.join('/');
      return p !== undefined && NAME.test(p) && rest.length > 0 && rest.every((x) => NAME.test(x)) ? { screen: 'branch', project: p, branch } : nf;
    }
    case 'artifacts': {
      // the library's place after a '~' (never a project id nor an artifact's)
      const t = args.indexOf('~');
      if (t === 0 || t === 1) {
        const at = args.slice(t + 1).join('/');
        const p = t === 1 ? (args[0] ?? '') : '';
        if (at === '' || !args.slice(t + 1).every((x) => NAME.test(x)) || (t === 1 && !NAME.test(p))) return nf;
        return { screen: 'artifacts', project: p, at };
      }
      if (args.length === 0) return { screen: 'artifacts', project: '' };
      if (one !== undefined) return { screen: 'artifacts', project: one };
      return a !== undefined && b !== undefined ? { screen: 'artifacts', project: a, id: b } : nf;
    }
    case 'settings':
      if (args.length === 0) return { screen: 'settings' };
      return one !== undefined ? { screen: 'settings', page: one } : nf;
    case 'lab':
      return isLab(one) ? { screen: 'lab', lab: one } : nf;
    default:
      return nf;
  }
}

/**
 * b1/b2: a route main sends that asks to open the inbox IN PLACE, never a screen: 'inbox' (⌘I with
 * another window in front) opens its first item, 'inbox?card=<project>/<id>' (a notification's
 * click, amb-mac 97a5939a) that card. Anything else: undefined. Pure.
 */
export function inboxAtOf(path: string): { at?: { project: string; card: number } } | undefined {
  const m = /^\/?inbox(?:\?card=([^/\s]+)\/(\d+))?\/?$/u.exec(path.trim());
  if (m === null) return undefined;
  const [, project, id] = m;
  return project === undefined || id === undefined ? {} : { at: { project: decodeURIComponent(project), card: Number(id) } };
}

/** The project a route is about (none: bise's, the inbox, settings...). */
export function projectOf(r: Route): string | undefined {
  return 'project' in r ? r.project : undefined;
}

/** What the core's `open` asks for (a capsule's or a card's ⏎), as a route. */
export function routeOfOpen(o: { project: string; what: 'thread' | 'review' | 'artifact' | 'worktree'; agent?: string | null; id?: string | null }): Route {
  const agent = o.agent ?? MAIN;
  switch (o.what) {
    case 'thread':
      return { screen: 'thread', project: o.project, agent };
    case 'review':
      return { screen: 'review', project: o.project, agent };
    case 'artifact':
      return o.id == null ? { screen: 'artifacts', project: o.project } : { screen: 'artifacts', project: o.project, id: o.id };
    case 'worktree':
      return { screen: 'environments', project: o.project };
  }
}

/** Two routes show the same thing. */
export function sameRoute(a: Route, b: Route): boolean {
  return formatRoute(a) === formatRoute(b);
}

/**
 * Where esc goes up from `r` (designer's desktop-history, the esc list; esc-up.ts asks): an
 * agent opened large → its project's main, a project's main → bise, a list or a tool screen
 * (agents, archived, artifacts, scheduled, settings, changes, /debug, the report…) → 'back',
 * the place before (as ‹; with nothing behind, the project's main or bise). bise, the first
 * run and the labs: null, esc does nothing. An open artifact is its view's own esc. Pure.
 */
export function upOf(r: Route): Route | 'back' | null {
  switch (r.screen) {
    case 'thread':
      return r.agent === MAIN ? { screen: 'bise' } : { screen: 'thread', project: r.project, agent: MAIN };
    case 'bise':
    case 'onboarding':
    case 'lab':
      return null;
    default:
      return 'back';
  }
}

/** Where 'back' lands with nothing behind: the route's project's main, else bise. Pure. */
export function homeOf(r: Route): Route {
  const p = projectOf(r);
  return p === undefined || p === '' ? { screen: 'bise' } : { screen: 'thread', project: p, agent: MAIN };
}

/**
 * The start route of a test run ("show me desktop <route>", a check that
 * opens a screen directly): `#route=<route string>` in the page's hash
 * (amb-mac's main sets it from BISE_DESKTOP_ROUTE in test mode). None
 * without it; a wrong one is notfound, as a link.
 */
export function startRouteOf(hash: string): Route | undefined {
  const m = /^#?route=(.*)$/.exec(hash);
  if (m === null) return undefined;
  let path = m[1] ?? '';
  try {
    path = decodeURIComponent(path);
  } catch {
    // a broken escape: the raw words, which parse to notfound
  }
  return parseRoute(path);
}

/** The shown set: home first, then the route's project, deduped (empty before the first rows). Pure. */
export function shownOf(home: string | undefined, route: Route, open: readonly string[] = []): string[] {
  const out: string[] = [];
  for (const p of [home, projectOf(route), ...open]) if (p !== undefined && !out.includes(p)) out.push(p);
  return out;
}
