// src/app/panel · the panel's keys (page native-mac §4 #2, native-mac-qa §7 and §12): ⌘T New tab,
// ⌘W closes the tab in front (folded: nothing, never the window; cmd-w-artifact), ⌃⇧T the last closed
// tab back (⇧⌘T is File › Reopen Closed Tab's, App.tsx), ⌘⇧] and ⌘⇧[ the next and previous tab, and the launcher's rows: ⌘P Artifacts (the picker of the latest), ⌥⌘D
// Changes (nothing when the agent in view has none), ⌘L Browser (a browser
// tab). ⌥⌘0 (the panel) and ⌃` (the terminal) keep their arms in app/keys.ts and
// terminal/model.ts. panelKeyOf decides, pure (app/keys.ts's one arm calls it); doPanelKey acts.
// Owner: nm-panel.
import type { KeyLike } from '@/app/keys';
import { projectOf, type Route } from '@/app/route';
import type { Agent } from '@/proto';
import { closeActive, openArtifacts, openBrowser, openLauncher, openShell, openWhat, reopenClosed, stepTab } from './state';
import { hasChanges, type OpenRow } from './launch';

/** Full width on and off (panel-full, designer m_45445): View › Panel Full Width, app/keys.ts. */
export const PANEL_FULL_KEY = '⇧⌥⌘B';

/** The keys the panel's tips and the launcher's rows name. */
export const PANEL_KEYS = {
  toggle: '⌥⌘B',
  close: '⌘W',
  add: '⌘T',
  reopen: '⇧⌘T',
  files: '⌘P',
  terminal: '⌃`',
  changes: '⌥⌘D',
  browser: '⌘L',
} as const;

/** What a panel key does. */
export type PanelKey = 'new-tab' | 'close-tab' | 'reopen-tab' | 'next-tab' | 'prev-tab' | 'files' | 'changes' | 'browser';

/**
 * The panel's key for `e`, if it is one; ⌘⇧[ and ⌘⇧] only while the panel is open. ⌘W is the
 * main window's always (cmd-w-artifact, his m_53150: never the window): open, its tab in front
 * closes; folded, nothing. In a window of its own (`own`) ⌘W is the menu's: File › Close Tab closes
 * that window. Pure.
 */
export function panelKeyOf(e: KeyLike, open: boolean, own = false): PanelKey | undefined {
  // ⌃⇧T, as Chrome on Windows and Linux (his m_46187), folded or open; ⇧⌘T is File › Reopen Closed
  // Tab's (main/menu.ts), never the page's: one press, one tab (designer m_46216)
  if (e.ctrlKey && e.shiftKey && !e.metaKey && !e.altKey && e.code === 'KeyT') return 'reopen-tab';
  const meta = e.metaKey && !e.ctrlKey;
  if (!meta) return undefined;
  if (!e.altKey && !e.shiftKey) {
    if (e.code === 'KeyT') return 'new-tab';
    if (e.code === 'KeyW') return own ? undefined : 'close-tab';
    if (e.code === 'KeyP') return 'files';
    if (e.code === 'KeyL') return 'browser';
    return undefined;
  }
  if (e.shiftKey && !e.altKey) {
    if (e.code === 'BracketRight') return open ? 'next-tab' : undefined;
    if (e.code === 'BracketLeft') return open ? 'prev-tab' : undefined;
    return undefined;
  }
  if (e.altKey && !e.shiftKey) {
    if (e.code === 'KeyD') return 'changes';
  }
  return undefined;
}

/** The agent in view: a thread's, else none (bise's own, a page). Pure. */
export function agentInView(route: Route): { project: string; agent: string } | undefined {
  return route.screen === 'thread' ? { project: route.project, agent: route.agent } : undefined;
}

export interface PanelCtx {
  readonly route: Route;
  /** bise's own project (the home hub), for a page with no project */
  readonly home: string | undefined;
  /** a project's agents (the store's rows): whether the one in view has changes */
  readonly agents: (project: string) => readonly Agent[];
}

/** The agent in view has changes to show: the launcher's Changes row and ⌥⌘D only then. Pure. */
export function changesInView(c: PanelCtx): { project: string; agent: string } | undefined {
  const a = agentInView(c.route);
  return a !== undefined && hasChanges(c.agents(a.project), a.agent) ? a : undefined;
}

/**
 * Acts on a panel key (or a launcher row, which does the same), in the context of the route in
 * view. `instead`: the launcher tab the pick came from (it becomes what it opened).
 */
export function doPanelKey(k: PanelKey | OpenRow, c: PanelCtx, instead?: string): void {
  const project = projectOf(c.route) ?? c.home;
  switch (k) {
    case 'new-tab':
      openLauncher();
      return;
    case 'close-tab':
      closeActive();
      return;
    case 'reopen-tab':
      // off a Mac ⌃⇧T is the menu's Shift+CmdOrCtrl+T: it acts there, once
      if ((window as Partial<Window>).bise?.platform === 'darwin') reopenClosed();
      return;
    case 'next-tab':
      stepTab(1);
      return;
    case 'prev-tab':
      stepTab(-1);
      return;
    case 'terminal':
      if (project !== undefined) openShell(project, instead);
      return;
    case 'files':
    case 'artifacts':
      openArtifacts(instead);
      return;
    case 'changes': {
      const a = changesInView(c);
      if (a !== undefined) openWhat({ kind: 'thing', e: { kind: 'diff', project: a.project, agent: a.agent } }, instead);
      return;
    }
    case 'browser':
      openBrowser(instead);
      return;
    case 'office':
      if (project !== undefined) openWhat({ kind: 'thing', e: { kind: 'office', project } }, instead);
      return;
  }
}
