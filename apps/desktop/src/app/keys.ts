// The window's own keys (settings/keys.ts lists them; tui-parity K14, V1,
// W14, F1, F5, F7, V3, K16): what a key does, as a pure decision, and the
// one listener in the shell that acts on it. Owner: amb-win.
//
// ⌘B the sidebar · ⌥⌘B the panel ·
// the rail's places (rail-keys, designer m_45492 / m_45497; PLACE_KEYS): ⌘0 bise · ⇧⌘I the inbox
// page · ⌘Y scheduled · ⇧⌘A the artifacts library · ⇧⌘P processes · ⇧⌘X plugins · ⌘, settings ·
// ⌘1-9 a project in his order (on the artifacts library: its rows, All artifacts, Recent, Unfiled,
// then the folders) · ⌥0 the project's main · ⌥1-9
// its agents in the panel's order (the right side's coins: ⌥⌘1-4) · ⌘K the agent palette · ⌘D the agent's
// changes · ⌘E fold/unfold everything · ⌘/ the key
// list · end back to the bottom · ctrl+y copy a code
// block · ⌥↑/⌥↓ the previous / next agent (empty composer).
// Keys are read by `code` (⌥1 types '¡' on a Mac keyboard) and taken in the
// capture phase, so the composer never gets the character.
import type { Agent, ProjectRow } from '@/proto';
import type { At } from '@/artifacts/folders';
import { liveOf, rankedOf } from '@/agents/model';
import { MAIN, projectOf, type Route } from './route';
import type { Change } from '@/review/openChange';
import { projectsOf } from './siderows';
import { sideShows } from './sideshows';
import { panelKeyOf, type PanelKey } from './panel/keys';

/** A window event that opens the key list (the composer's /help and /shortcuts). */
export const HELP_EVENT = 'bise:help';

/** What a screen does for a key (the thread listens with useAppAction). */
export type ScreenAction = 'fold-all' | 'stop' | 'to-end' | 'copy-code' | 'side-1' | 'side-2' | 'side-3' | 'side-4';

/** What the window's frame does: show or hide the sidebar (⌘B), the right panel (⌥⌘B), the panel full width (⇧⌥⌘B, panel-full). */
export type FrameAction = 'sidebar' | 'panel' | 'panelFull';

/** The keys the two sides show: settings/keys.ts's (the key list, a leaf module), re-exported here for the shell. */
export { FRAME_KEYS, PLACE_KEYS } from '@/settings/keys';

/** How long after a window key the View menu's echo of the same key is dropped (Electron gives
 *  both: the page's keydown, then the menu item's accelerator), as the terminal's ⌃`. */
export const MENU_ECHO_MS = 300;

/** The View menu's item for a side, at `now`, after the window's own key toggled it at `keyAt`:
 *  its echo (dropped) or his pick from the menu (acted on). Pure. */
export function menuEcho(keyAt: number | undefined, now: number): boolean {
  return keyAt !== undefined && now - keyAt < MENU_ECHO_MS;
}

export type KeyAct =
  | { readonly go: Route }
  /** ⌘D on an agent's thread: its change in the panel (review/openChange.ts: its PR if it has one) */
  | { readonly change: { readonly project: string; readonly what: Change } }
  | { readonly palette: true }
  | { readonly help: true }
  | { readonly screen: ScreenAction }
  | { readonly frame: FrameAction }
  /** ⌘1-9 on the artifacts library: its place, as a click on its row (artifacts/library.tsx goLibrary) */
  | { readonly library: At }
  /** ⌘N: New project, the add-a-project sheet (addProject.ts; his m_44225) */
  | { readonly newProject: true }
  /** ⇧⌘N: New agent in the project in view, else bise's own (newagent.tsx) */
  | { readonly newAgent: string }
  /** the tabbed panel's keys (app/panel/keys.ts): ⌘T, ⌘W, ⌘⇧[ ], ⌘P, ⌘L, ⌥⌘D */
  | { readonly panel: PanelKey };

export interface KeyLike {
  readonly key: string;
  readonly code: string;
  readonly metaKey: boolean;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
}

export interface KeyCtx {
  readonly route: Route;
  readonly projects: readonly ProjectRow[];
  readonly agents: (project: string) => readonly Agent[];
  /** focus is in a field with text (the composer, find, a setting) */
  readonly typing: boolean;
  /** the project's ⌥ numbers as the sidebar shows them (agents/model numbersOf); none: the hub's rank */
  readonly numbers?: (project: string) => ReadonlyMap<string, number>;
  /** the tabbed panel is open: ⌘⇧[ ] are its (⌘W is the main window's open or folded) */
  readonly panelOpen?: boolean;
  /** a window of its own (app/own.ts): its ⌘W is the menu's, which closes it */
  readonly own?: boolean;
  /** the artifacts library's places in its order (artifacts/library.tsx libraryPlaces), none: no library */
  readonly library?: () => readonly At[] | undefined;
  /** where bise's tab goes from here, as its rail button (railback.ts biseGo); none: bise's thread */
  readonly bise?: () => Route;
}

const digit = (code: string): number | undefined => (/^Digit[0-9]$/.test(code) ? Number(code.slice(5)) : undefined);

/** The project a key acts on: the route's, else bise's own. */
function here(c: KeyCtx): string | undefined {
  return projectOf(c.route) ?? c.projects.find((p) => p.home)?.project;
}

/** The agents of the panel, numbered for ⌥1-9: live, main out, in the panel's order. */
export function panelAgents(agents: readonly Agent[]): Agent[] {
  return liveOf(agents).filter((a) => !a.main);
}

/** What a key does here; none: not the window's key. Pure. */
export function keyAction(e: KeyLike, c: KeyCtx): KeyAct | undefined {
  const only = (m: 'meta' | 'alt' | 'ctrl' | 'none', shift = false) =>
    e.metaKey === (m === 'meta') && e.altKey === (m === 'alt') && e.ctrlKey === (m === 'ctrl') && e.shiftKey === shift;
  const n = digit(e.code);
  if (n !== undefined && only('meta')) {
    if (n === 0) return { go: c.bise?.() ?? { screen: 'bise' } };
    // the artifacts library in view: ⌘1-9 are its rows, as the projects' in their place (designer m_45492)
    const lib = sideShows(c.route) === 'library' ? c.library?.() : undefined;
    if (lib !== undefined) {
      const at = lib[n - 1];
      return at === undefined ? undefined : { library: at };
    }
    const p = projectsOf(c.projects)[n - 1];
    return p ? { go: { screen: 'thread', project: p.project, agent: MAIN } } : undefined;
  }
  if (n !== undefined && only('alt')) {
    const p = here(c);
    if (p === undefined) return undefined;
    if (n === 0) return { go: { screen: 'thread', project: p, agent: MAIN } };
    // ⌥0-9 are main and the agents, always; the coins are ⌥⌘1-4 only, so the two never share a key (designer m_45787)
    const nums = c.numbers?.(p) ?? rankedOf(liveOf(c.agents(p)));
    const name = [...nums].find(([, k]) => k === n)?.[0];
    return name !== undefined ? { go: { screen: 'thread', project: p, agent: name } } : undefined;
  }
  if (only('meta') && e.code === 'KeyK') return { palette: true };
  const pk = panelKeyOf(e, c.panelOpen === true, c.own === true);
  if (pk !== undefined) return { panel: pk };
  // the two sides (his m_44999, designer m_45008 / m_45164): ⌘B the sidebar, ⌥⌘B the panel (VS Code,
  // Cursor, Zed); ⌃⌘S (macOS's Show Sidebar), ⌘\ and ⌥⌘0 (the old ones) still work, shown nowhere.
  // A capture-phase ⌘B never reaches the composer, so it never bolds text there
  const metaAlt = e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey;
  if (only('meta') && (e.code === 'KeyB' || e.code === 'Backslash')) return { frame: 'sidebar' };
  if (e.metaKey && e.ctrlKey && !e.altKey && !e.shiftKey && e.code === 'KeyS') return { frame: 'sidebar' };
  if (metaAlt && (e.code === 'KeyB' || e.code === 'Digit0')) return { frame: 'panel' };
  if (e.metaKey && e.altKey && e.shiftKey && !e.ctrlKey && e.code === 'KeyB') return { frame: 'panelFull' };
  // bise fait mac (page native-mac §6): ⌘N New project, ⇧⌘N New agent (his m_44225), ⌘, settings
  // ⌥⌘1-⌥⌘4: the right side's coins, agents, artifacts, scheduled, processes (native-mac d1; app/side3d; designer m_45787: never ⌥1-4, the agents' keys)
  if (metaAlt && n !== undefined && n >= 1 && n <= 4) return { screen: `side-${String(n)}` as ScreenAction };
  if (only('meta') && e.code === 'KeyN') return { newProject: true };
  if (only('meta', true) && e.code === 'KeyN') {
    const p = here(c);
    return p === undefined ? undefined : { newAgent: p };
  }
  if (only('meta') && e.code === 'Comma') return { go: { screen: 'settings' } };
  // the rail's places, each the key its button shows (PLACE_KEYS): ⇧⌘I the inbox page (⌘I stays the
  // block in place, the Go menu's), ⌘Y scheduled, ⇧⌘A the artifacts library, ⇧⌘P processes (⌘P
  // alone is the panel's picker), ⇧⌘X plugins
  if (only('meta', true) && e.code === 'KeyI') return { go: { screen: 'inbox' } };
  if (only('meta') && e.code === 'KeyY') return { go: { screen: 'scheduled' } };
  if (only('meta', true) && e.code === 'KeyA') return { go: { screen: 'artifacts', project: '' } };
  if (only('meta', true) && e.code === 'KeyP') return { go: { screen: 'running' } };
  if (only('meta', true) && e.code === 'KeyX') return { go: { screen: 'plugins' } };
  if (only('meta') && e.code === 'KeyD') {
    const r = c.route;
    if (r.screen === 'thread' && r.agent !== MAIN) return { change: { project: r.project, what: { agent: r.agent } } };
    const p = here(c);
    return p ? { go: { screen: 'changes', project: p } } : undefined;
  }
  if (only('meta') && e.code === 'KeyE') return { screen: 'fold-all' };
  // the TUI's ctrl+o (ctrlhint.rs: open everything closed, else close what is open), the composer
  // focused or not: the window's capture listener takes it before the field (macOS's 'open line')
  if (only('ctrl') && e.code === 'KeyO') return { screen: 'fold-all' };
  // the key list over the window (amb-web 1e60fbd6, src/help): ⌘/ only; '?' is his to type, a
  // message may start with it (main m_22096)
  if ((only('meta') || only('meta', true)) && e.code === 'Slash') return { help: true }; // ⌘/, ⌘? (designer m_22100)
  if (only('ctrl') && e.code === 'KeyY') return { screen: 'copy-code' };
  if (only('none') && e.key === 'End' && !c.typing) return { screen: 'to-end' };
  if (only('alt') && (e.key === 'ArrowDown' || e.key === 'ArrowUp') && !c.typing) {
    const r = c.route;
    const p = here(c);
    if (p === undefined) return undefined;
    const list = [MAIN, ...panelAgents(c.agents(p)).map((a) => a.name)];
    const at = r.screen === 'thread' && r.project === p ? list.indexOf(r.agent) : -1;
    const next = list[Math.min(list.length - 1, Math.max(0, at + (e.key === 'ArrowDown' ? 1 : -1)))];
    return next !== undefined ? { go: { screen: 'thread', project: p, agent: next } } : undefined;
  }
  return undefined;
}

/** Focus is in a field that has text: plain keys (end, ⌥↑↓) are its own. */
export function typingIn(el: Element | null): boolean {
  if (el === null) return false;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return el.value !== '';
  if (el instanceof HTMLElement && el.isContentEditable) return el.textContent.trim() !== '';
  return false;
}
