// The application menu's actions for this window (⌘, ⌘I ⌘F: amb-mac's
// src/main/menu.ts sends them through window.bise.onMenu), and its Agent menu kept on the agent in
// view (app/menus/use.ts useAgentMenuBar; its ⌘. Stop asks the thread with emitAction('stop')).
// Owner: nm-menus.
//
// One subscription to the preload for the whole window (useMenuRouting,
// in the shell); 'settings' and the Go menu's places are routes, 'inbox' opens the block in place,
// anything else goes to the screens that listen for it (useMenuAction: the thread's ⌘F). A Go item
// right after the window took a key (noteWindowKey) is that key's echo: dropped, as the View menu's
// (keys.ts menuEcho): a zoomed image's ⌘0 stays its own, never 'bise' as well.
import { useEffect, useRef } from 'react';
import type { MenuAction } from '@/shared/api';
import type { Router } from './router';
import { inboxAtOf, parseRoute, projectOf, type Route } from './route';
import { askInbox, inboxFromMenu } from './inboxAsk';
import { askEffect, runNav } from './edge';
import { draftAskOf } from '@/shared/asks';
import { handIn } from '@/composer/handin';
import { keptStorage } from './keep';
import { addProject } from './addProject';
import { newAgent } from './newagent';
import { biseBack } from './railback';
import { useStore } from '@/store';
import { menuEcho, type ScreenAction } from './keys';
import { panel } from '@/terminal/panel';
import { closeActive } from './panel/state';
import { useAgentMenuBar } from './menus/use';

/** What a screen may be asked: a menu item's ('find') or a window key's ('fold-all', 'stop'...). */
export type AppAction = MenuAction | ScreenAction;

type Listener = (action: AppAction) => void;
const listeners = new Set<Listener>();

/** Runs the action and says true when it is its own now (an artifact in front takes ⌘F: artifacts/find.ts). */
type Taker = { readonly action: AppAction; readonly take: () => boolean };
const takers: Taker[] = [];

/** Ask the shown screens (the shell's keys call it); a taker that takes it first: the screens never hear it. */
export function emitAction(action: AppAction): void {
  if (takers.some((t) => t.action === action && t.take())) return;
  for (const l of [...listeners]) l(action);
}

/** While mounted: `take` is asked before the screens hear `action`, and keeps it when it says true. */
export function useActionTaker(action: AppAction, take: () => boolean) {
  const latest = useRef(take);
  useEffect(() => {
    latest.current = take;
  });
  useEffect(() => {
    const t: Taker = { action, take: () => latest.current() };
    takers.push(t);
    return () => {
      const i = takers.indexOf(t);
      if (i >= 0) takers.splice(i, 1);
    };
  }, [action]);
}

/** The route a menu action opens, if it is one: settings, and the Go menu's places (the rail's). Pure. */
export function routeOfMenu(action: MenuAction): Route | undefined {
  switch (action) {
    case 'settings':
    case 'bise':
    case 'scheduled':
    case 'plugins':
      return { screen: action };
    case 'inboxPage':
      return { screen: 'inbox' };
    case 'artifacts':
      return { screen: 'artifacts', project: '' };
    case 'processes':
      return { screen: 'running' };
    default:
      return undefined;
  }
}

/** When the window's own key listener last took a key (App.tsx useWindowKeys). */
let windowKeyAt: number | undefined;

/** The window took a key: the menu's echo of it, if any, comes within MENU_ECHO_MS. */
export function noteWindowKey(now: number): void {
  windowKeyAt = now;
}

/** In the shell: the menu's routes go to the router, the rest to the screens. */
export function useMenuRouting(router: Router) {
  // the menu bar's Agent menu follows the agent in view (bise fait mac, d4)
  useAgentMenuBar();
  const store = useStore();
  useEffect(() => {
    // the page outside Electron (a component test) has no preload
    const bise = (window as Partial<Window>).bise;
    if (bise === undefined) return;
    return bise.onMenu((action) => {
      // b1/b2: ⌘I opens the inbox block in place, on the screen he is on; none there: the inbox place
      if (action === 'inbox') {
        inboxFromMenu((to) => {
          router.go(to);
        });
        return;
      }
      // File › New Project… / New Agent… (r2-pointer): the same sheets as ⌘N / ⇧⌘N (keys.ts); the
      // key's own echo opens the same sheet again, a no-op
      if (action === 'newProject') {
        addProject(router);
        return;
      }
      if (action === 'newAgent') {
        const p = projectOf(router.get()) ?? store.projects().find((x) => x.home)?.project;
        if (p !== undefined) newAgent(p);
        return;
      }
      // File › Close Tab ⌘W (cmd-w-artifact): the panel's tab in front closes, never the window. It
      // comes alone when his focus is in an artifact's frame (the window never sees that key); the
      // echo of a ⌘W the window took itself (app/panel/keys.ts) is dropped: one press, one tab
      if (action === 'closeTab') {
        if (!menuEcho(windowKeyAt, Date.now())) closeActive();
        return;
      }
      // Go › Back ⌘[ / Forward ⌘] (history-keys): the window's history; the echo of the ⌘[ ⌘] the
      // window took itself (esc-up.ts) is dropped, one press, one step
      if (action === 'back' || action === 'forward') {
        if (menuEcho(windowKeyAt, Date.now())) return;
        if (action === 'back') router.back();
        else router.forward();
        return;
      }
      // Go › bise as the rail's ':*': back to his last project or chat from a page (railback.ts)
      const to = action === 'bise' ? biseBack.go(router.get(), { projects: store.projects(), agents: (p) => store.agents(p) }) : routeOfMenu(action);
      if (to === undefined) emitAction(action);
      else if (!menuEcho(windowKeyAt, Date.now())) router.go(to);
    });
  }, [router, store]);
}

/**
 * In the shell: a route main sends this live window (the capsule's
 * openWindow(route) while the window is open, amb-mac b9f1fc04) goes to the
 * router; a wrong one shows notfound, as a link.
 */
export function useMainRoutes(router: Router) {
  useEffect(() => {
    const bise = (window as Partial<Window>).bise;
    if (bise === undefined) return;
    return bise.onRoute((route) => {
      // View > Terminal while another window had the keys: the terminal here, where it lives
      if (route === 'terminal') {
        panel.reveal();
        return;
      }
      // the capsule's card chip (or its option that opens an artifact): it in the panel's tab
      // at its block, the window stays where it is (edge.tsx askEffect)
      const art = askEffect(route);
      if (art !== undefined) {
        runNav(art, router);
        return;
      }
      // the spotlight's ⏎ on an agent's row under a message: its thread, his words its draft
      const draft = draftAskOf(route);
      if (draft !== undefined) {
        handIn(keptStorage, `${draft.project}/${draft.agent}`, draft.entry);
        router.go(parseRoute(`agent/${draft.project}/${draft.agent}`));
        return;
      }
      // b1/b2: 'inbox' / a notification's 'inbox?card=<project>/<id>' open the block in place
      const inbox = inboxAtOf(route);
      if (inbox === undefined) router.go(parseRoute(route));
      else if (inbox.at === undefined)
        inboxFromMenu((to) => {
          router.go(to);
        });
      else askInbox(inbox.at);
    });
  }, [router]);
}

/** In a screen: `handler` runs on that menu action while the screen is shown. */
export function useMenuAction(action: AppAction, handler: () => void) {
  const latest = useRef(handler);
  useEffect(() => {
    latest.current = handler;
  });
  useEffect(() => {
    const l: Listener = (a) => {
      if (a === action) latest.current();
    };
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  }, [action]);
}
