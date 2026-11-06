// The composer's / list in the window (bar 2.1, his bug 1, amb-core's C/A/B
// 40f14a12): the TUI's commands in its order, from the core (useCommands),
// each ready or greyed 'not yet', never a silent gap. How one runs:
// - runs 'window': it opens its screen here (the window owns readiness:
//   ready when the screen exists), or with its words does what that screen's
//   switch does (/theme light, /plugins off x, /computer-use off: the
//   same AppCmd settings sends), or quits (/quit);
// - runs 'hub': his line as he typed it, HubCmd slash (the TUI's router and
//   handlers run it in the hub; architect m_11011).
// Owner: amb-win. Pure.
import type { ArgKind, PickCommand } from '@/composer';
import type { AppCmd, HubCmd, PickArg, PickCommand as CoreCommand } from '@/proto';
import { MAIN, type Route } from '@/app/route';
import { composer } from '../proto/words.gen';
import { mapOf, mapPrefKey } from '@/bims/maps/ids';

/** The window's own commands the TUI has no row for, after the core's. */
export const windowCommands: readonly PickCommand[] = [
  { name: 'agents', desc: composer.cmd_agents, args: false, ready: true },
  { name: 'archived', desc: composer.cmd_archived, args: false, ready: true },
  { name: 'switch', desc: composer.cmd_switch, args: true, argKinds: ['agent'], ready: true },
  { name: 'diff', desc: composer.cmd_diff, args: true, argKinds: ['agent'], ready: true },
  { name: 'prs', desc: composer.cmd_prs, args: false, ready: true },
  { name: 'artifacts', desc: composer.cmd_artifacts, args: false, ready: true },
  { name: 'scheduled', desc: composer.cmd_scheduled, args: false, ready: true },
  { name: 'bims', desc: composer.cmd_bims, args: true, argKinds: ['map'], ready: true },
];

/**
 * What a window command does here: a screen, the key list over the window,
 * one of the core's own commands (the one settings sends for the same
 * switch: /plugins off x, /theme light, /computer-use off), or quit.
 */
export type WindowRun = { readonly go: Route } | { readonly inbox: true } | { readonly help: true } | { readonly app: AppCmd } | { readonly quit: true };

const at0 = (args: string): string => args.trim().split(/\s+/)[0] ?? '';
const at1 = (args: string): string => args.trim().split(/\s+/)[1] ?? '';

/** The TUI's /theme words (auto = follow the system) as the window's prefs theme. */
const THEMES: Readonly<Record<string, string>> = { light: 'light', dark: 'dark', auto: 'system', system: 'system' };

/**
 * /plugins <sub> <name> as the plugins place sends it (rust/tui/src/plugins.rs's
 * subcommands); 'list', a bare login/logout or an unknown word: none (the screen
 * or the hub's usage line).
 */
function pluginsRun(args: string): AppCmd | undefined {
  const [sub, name] = [at0(args), at1(args)];
  if (name === '') return undefined;
  switch (sub) {
    case 'on':
    case 'off':
      return { cmd: 'plugin_set', name, on: sub === 'on' };
    case 'login':
      return { cmd: 'plugin_login', name };
    case 'logout':
      return { cmd: 'plugin_logout', name };
    default:
      return undefined;
  }
}

/** The screen a window command opens here; none: not one of the window's (or no screen yet). */
export function windowRun(name: string, args: string, at: { project: string; agent: string }): WindowRun | undefined {
  const arg = at0(args);
  const go = (r: Route): WindowRun => ({ go: r });
  switch (name.replace(/^\//, '')) {
    // as the TUI: /agents lists the live ones, /archived only the archived ones
    case 'agents':
      return go({ screen: 'agents', project: at.project });
    case 'archived':
      return go({ screen: 'agents', project: at.project, archived: true });
    case 'switch':
      return go(arg === '' ? { screen: 'agents', project: at.project } : { screen: 'thread', project: at.project, agent: arg });
    case 'diff': {
      // a branch (it has a slash: sb/x, feat/y) opens its diff, an agent on it or none (R7, amb-win's route)
      if (arg.includes('/')) return go({ screen: 'branch', project: at.project, branch: arg });
      const who = arg === '' ? at.agent : arg;
      return go(who === MAIN ? { screen: 'changes', project: at.project } : { screen: 'review', project: at.project, agent: who });
    }
    case 'prs':
      return go({ screen: 'prs', project: at.project });
    case 'debug': {
      // /debug [history | completion [provider]] on the agent in view (the TUI's logview.rs
      // parse): its raw session, or its last request as sent (as `provider` would get it)
      const [sub, prov, extra] = arg.split(/\s+/u).filter((w) => w !== '');
      const base = { screen: 'debug', project: at.project, agent: at.agent } as const;
      if (sub === undefined || (sub === 'history' && prov === undefined)) return go(base);
      if (sub !== 'completion' || extra !== undefined) return undefined;
      return go(prov === undefined ? { ...base, tab: 'completion' } : { ...base, tab: 'completion', provider: prov });
    }
    // a window command with words of its own (/artifacts add <url>, /plugins on <x>) is the hub's
    case 'artifacts':
      return arg === '' ? go({ screen: 'artifacts', project: at.project }) : undefined;
    case 'inbox':
      // one inbox (designer m_19631): /inbox opens the block in place, as ⌘I
      return { inbox: true };
    case 'help':
    case 'shortcuts':
      return { help: true };
    case 'voice':
      // Setup at Dictation and fn (page models-setup): the model that listens, its key, the mic
      return go({ screen: 'settings', page: 'dictation' });
    case 'models':
    case 'provider':
      return go({ screen: 'settings', page: 'models' });
    case 'theme': {
      // '/theme light' switches as settings · general does; bare or unknown: the page
      const t = THEMES[arg];
      return t === undefined ? go({ screen: 'settings', page: 'general' }) : { app: { cmd: 'prefs_set', key: 'theme', value: t } };
    }
    case 'setup':
      return go({ screen: 'settings', page: 'setup' });
    case 'plugins': {
      // '/plugins off x' acts as the plugins place's switch; 'list' or bare: the place;
      // anything else goes to the hub, whose line says the usage
      if (arg === '' || arg === 'list') return go({ screen: 'plugins' });
      const c = pluginsRun(args);
      return c === undefined ? undefined : { app: c };
    }
    case 'skills': {
      // /skills: the plugins place on skills; '/skills off x' as /plugins off x
      if (arg === '' || arg === 'list') return go({ screen: 'plugins', kind: 'skills' });
      const c = pluginsRun(args);
      return c === undefined ? undefined : { app: c };
    }
    case 'computer-use':
      // '/computer-use off|uninstall' acts as the settings page's buttons; bare: the page (it sets up)
      return arg === 'off' || arg === 'uninstall' ? { app: { cmd: 'computer_use', act: arg } } : go({ screen: 'settings', page: 'computer' });
    case 'scheduled':
      return go({ screen: 'scheduled' });
    case 'processes':
      // every project's processes, as bise's thread's 'all ›' (Gabriel, main m_38648)
      return go({ screen: 'running' });
    case 'keychain':
      // N1 (the TUI's /keychain [on|off]): on/off moves his keys and sign-ins as settings · keys'
      // switch does; bare: that page, which says where they are
      return arg === 'on' || arg === 'off' ? { app: { cmd: 'keychain', to: arg } } : go({ screen: 'settings', page: 'keys' });
    case 'quit':
      // the window and the capsule quit; the agents keep running (the hub is its own process)
      return { quit: true };
    case 'welcome':
      return go({ screen: 'onboarding' });
    case 'bims':
      // '/bims kyoto' draws this project's office as that map, in every window (his prefs,
      // architect m_28589); bare: none, the composer's list of maps is the menu
      return arg === '' || at.project === '' ? undefined : { app: { cmd: 'prefs_set', key: mapPrefKey(at.project), value: mapOf(arg) } };
    default:
      return undefined;
  }
}

/** The route a window command opens (the screens only). */
export function runWindowCommand(name: string, args: string, at: { project: string; agent: string }): Route | undefined {
  const r = windowRun(name, args, at);
  return r !== undefined && 'go' in r ? r.go : undefined;
}

/** A window command has its screen here (the key list counts). */
const hasScreen = (name: string): boolean => windowRun(name, '', { project: '', agent: MAIN }) !== undefined;

function argKind(a: PickArg): ArgKind {
  switch (a.kind) {
    case 'words':
      return { words: a.words };
    case 'note':
      return 'text';
    default:
      return a.kind;
  }
}

/**
 * The composer's / list: the core's commands in the TUI's order, ready when
 * the window can run them, then the window's own the TUI has no row for.
 * Before the core answers, the window's own alone.
 */
/** What bar 2.1 leaves out of the window's / list: /version is a dev tool of the terminal. */
export const LEFT_OUT: ReadonlySet<string> = new Set(['version']);

export function pickCommands(core: readonly CoreCommand[]): PickCommand[] {
  if (core.length === 0) return [...windowCommands];
  const rows = core.filter((c) => !LEFT_OUT.has(c.name.replace(/^\//, ''))).map(
    (c): PickCommand => ({
      name: c.name,
      desc: c.desc,
      args: c.args.length > 0,
      ...(c.args.length > 0 ? { argKinds: c.args.map(argKind) } : {}),
      // the hub runs every typed line of its own (slash); a window one needs its screen here
      ready: c.runs === 'window' ? hasScreen(c.name) : true,
    }),
  );
  const known = new Set(core.map((c) => c.name.replace(/^\//, '')));
  return [...rows, ...windowCommands.filter((w) => !known.has(w.name))];
}

/**
 * A line he typed that isn't one of the window's screens goes to the hub as
 * he typed it (architect m_11011, amb-feed's HubCmd slash cd021d67): the
 * TUI's router parses it and runs the TUI's handler; no parser here. The
 * typed HubCmds (new, rename, model...) stay for buttons, pickers and forms.
 */
/**
 * A sent line that is a slash command the composer's list doesn't know
 * (qa-flows d50c36c5: /undo, /cancel, /flow pr went out as messages): '/word
 * args' with a plain command word, one line. It goes to the hub as typed
 * slash like the known ones, and the hub's line (a notice or its refusal)
 * shows as the TUI's. A path ('/usr/bin is odd') or a multi-line text stays
 * a message. Pure.
 */
export function slashLine(text: string): { name: string; args: string } | null {
  const m = /^\/([a-z][a-z0-9_-]*)(?:[ \t]+([^\n]*))?$/u.exec(text.trim());
  if (m === null) return null;
  return { name: `/${m[1] ?? ''}`, args: (m[2] ?? '').trim() };
}

export function slashOf(name: string, args: string, at: { project: string; agent: string }): Extract<HubCmd, { cmd: 'slash' }> {
  const word = name.startsWith('/') ? name : `/${name}`;
  const line = args.trim() === '' ? word : `${word} ${args.trim()}`;
  return { cmd: 'slash', project: at.project, agent: at.agent, line };
}
