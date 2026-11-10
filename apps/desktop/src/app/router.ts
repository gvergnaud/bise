// The router: the current route and his way back and forward, like a
// browser (designer's desktop-history: ‹ › in the titlebar and their menu,
// ⌘← ⌘→ and ⌘[ ⌘], Go › Back / Forward, the mouse's side buttons, esc up the hierarchy in
// esc-up.ts). A step may also hold the panel as it was there (history-keys, architect m_58553):
// an artifact opened or closed in the panel marks a step on the same route (mark), and back or
// forward hands that step's panel to app/panel/steps.ts (onRestore); the router never reads it.
// Owner: amb-win; the history part amb-web (m_19808). The route lives here
// and nowhere else; screens read it with useRoute() and move with go(). A
// move to the route already shown does nothing (no new history step, no
// listener call), so a deep link sent twice never stacks. 50 places per
// window; App.tsx keeps the stack in the window's sessionStorage, so a
// reload gives it back and a relaunch starts fresh.
import { createContext, useContext, useSyncExternalStore } from 'react';
import { formatRoute, parseRoute, type Route, sameRoute } from './route';

export type Side = 'back' | 'forward';

/**
 * A step of his history as the ‹ › menu names it: its route, and what made it when it is not a
 * route of its own (`step`: the panel's JSON, an artifact opened or closed; app/panel/history.ts).
 */
export interface Place {
  readonly route: Route;
  readonly step?: unknown;
}

export interface Router {
  get(): Route;
  /** show `r`; a new step of his history (the forward steps go), or in place of this one when only a section changes (stepOf) */
  go(r: Route): void;
  /** show `r` in place of the current step: no new step, the forward ones stay */
  replace(r: Route): void;
  back(): void;
  forward(): void;
  /** `n` steps on that side at once (the ‹ › menu's rows: 1 is the nearest) */
  jump(side: Side, n: number): void;
  canBack(): boolean;
  canForward(): boolean;
  /** the places on that side, the nearest first, at most `max` (the ‹ › menu, 10) */
  places(side: Side, max: number): Route[];
  /** the same with what made each step (the ‹ › menu's rows) */
  steps(side: Side, max: number): Place[];
  /**
   * A step on the same route (history-keys, his m_58541: an artifact opened or closed in the panel;
   * architect m_58553): the current step keeps `before`, the panel as he leaves it, and the new one
   * holds `after` and `step` (what made it). All three are the panel's own JSON, never read here.
   */
  mark(before: unknown, after: unknown, step: unknown): void;
  /** The current step's panel is now `panel` (a tab brought to the front: no new step). */
  replacePanel(panel: unknown): void;
  /** Back, forward or a jump lands on a step that holds a panel: `restore` gets it, before the listeners hear the move. */
  onRestore(restore: (panel: unknown) => void): () => void;
  /** the stack as strings, for the window's session (sessionStorage: a reload keeps it, a relaunch does not) */
  saved(): SavedHistory;
  subscribe(listener: () => void): () => void;
}

/** A step as the session keeps it: its route's string, with the panel's JSON when it holds one. */
export type SavedStep = string | { readonly r: string; readonly panel?: unknown; readonly step?: unknown };

/** The stack as its route strings: what a reload gives back. */
export interface SavedHistory {
  past: SavedStep[];
  now: SavedStep;
  future: SavedStep[];
}

/** A step of the stack: its route, the panel as it was there and what made it (mark), both opaque. */
export interface Entry {
  readonly r: Route;
  readonly panel?: unknown;
  readonly step?: unknown;
}

/** How many places he can go back to (designer's desktop-history: 50 per window). */
export const DEPTH = 50;

/**
 * How a move from `from` to `to` enters his history (designer's desktop-history, 'what enters
 * history'): the same place adds nothing; one settings section to another replaces the step;
 * anything else is a new step. Pure.
 */
export function stepOf(from: Route, to: Route): 'none' | 'replace' | 'push' {
  if (sameRoute(from, to)) return 'none';
  if (from.screen === 'settings' && to.screen === 'settings') return 'replace';
  return 'push';
}

const savedOf = (e: Entry): SavedStep => (e.panel === undefined && e.step === undefined ? formatRoute(e.r) : { r: formatRoute(e.r), ...(e.panel === undefined ? {} : { panel: e.panel }), ...(e.step === undefined ? {} : { step: e.step }) });

/** A kept step back as an entry; a bad string or shape: none. Pure. */
function entryOf(x: SavedStep): Entry | undefined {
  const raw: unknown = typeof x === 'string' ? x : (x as { r?: unknown } | null)?.r;
  if (typeof raw !== 'string') return undefined;
  const r = parseRoute(raw);
  if (r.screen === 'notfound' || r.screen === 'lab') return undefined;
  if (typeof x === 'string') return { r };
  return { r, ...(x.panel === undefined ? {} : { panel: x.panel }), ...(x.step === undefined ? {} : { step: x.step }) };
}

/**
 * The stack a window starts with (amb-win's note m_19808): keep.ts picks the start route (a
 * reload, a relaunch, a deep link); the session only adds what was behind and ahead of it. The
 * session's place is the start route: its past and future come back, with the panels its steps
 * held. Another start route (a deep link, a notification): it is a new step after the session's
 * place, nothing ahead. Bad strings drop out. Pure.
 */
export function restoreHistory(start: Route, saved: SavedHistory | undefined): { past: Entry[]; now: Entry; future: Entry[] } {
  if (saved === undefined) return { past: [], now: { r: start }, future: [] };
  const entries = (xs: readonly SavedStep[]) => xs.flatMap((x) => entryOf(x) ?? []);
  const [was] = entries([saved.now]);
  if (was !== undefined && sameRoute(was.r, start)) return { past: entries(saved.past).slice(-DEPTH), now: was, future: entries(saved.future).slice(0, DEPTH) };
  const past = was === undefined ? entries(saved.past) : [...entries(saved.past), was];
  return { past: past.slice(-DEPTH), now: { r: start }, future: [] };
}

export function createRouter(start: Route, saved?: SavedHistory): Router {
  let { past, now, future } = restoreHistory(start, saved);
  const listeners = new Set<() => void>();
  const restorers = new Set<(panel: unknown) => void>();
  const emit = () => {
    for (const l of [...listeners]) l();
  };
  const move = (side: Side, n: number) => {
    const from = side === 'back' ? past : future;
    if (n < 1 || n > from.length) return;
    if (side === 'back') {
      const to = past.length - n;
      future = [...past.slice(to + 1), now, ...future].slice(0, DEPTH);
      now = past[to] ?? now;
      past = past.slice(0, to);
    } else {
      past = [...past, now, ...future.slice(0, n - 1)].slice(-DEPTH);
      now = future[n - 1] ?? now;
      future = future.slice(n);
    }
    // the panel as it was on that step, before the screens hear the move (one call, never a mark)
    const panel = now.panel;
    if (panel !== undefined) for (const f of [...restorers]) f(panel);
    emit();
  };
  return {
    get: () => now.r,
    go(r) {
      const step = stepOf(now.r, r);
      if (step === 'none') return;
      if (step === 'push') {
        past = [...past, now].slice(-DEPTH);
        future = [];
        now = { r };
      } else now = { ...now, r };
      emit();
    },
    replace(r) {
      if (sameRoute(r, now.r)) return;
      now = { ...now, r };
      emit();
    },
    back: () => {
      move('back', 1);
    },
    forward: () => {
      move('forward', 1);
    },
    jump: move,
    canBack: () => past.length > 0,
    canForward: () => future.length > 0,
    places: (side, max) => (side === 'back' ? [...past].reverse() : future).slice(0, max).map((e) => e.r),
    steps: (side, max) => (side === 'back' ? [...past].reverse() : future).slice(0, max).map((e) => (e.step === undefined ? { route: e.r } : { route: e.r, step: e.step })),
    mark(before, after, step) {
      past = [...past, { ...now, panel: before }].slice(-DEPTH);
      future = [];
      now = { r: now.r, panel: after, step };
      emit();
    },
    replacePanel(panel) {
      now = { ...now, panel };
    },
    onRestore(f) {
      restorers.add(f);
      return () => restorers.delete(f);
    },
    saved: () => ({ past: past.map(savedOf), now: savedOf(now), future: future.map(savedOf) }),
    subscribe(l) {
      listeners.add(l);
      return () => listeners.delete(l);
    },
  };
}

export const RouterContext = createContext<Router | null>(null);

export function useRouter(): Router {
  const r = useContext(RouterContext);
  if (r === null) throw new Error('useRouter outside <RouterContext>');
  return r;
}

/** The current route; re-renders only when it changes. */
export function useRoute(): Route {
  const r = useRouter();
  return useSyncExternalStore(
    (l) => r.subscribe(l),
    () => r.get(),
  );
}
