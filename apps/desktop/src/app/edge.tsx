// The app's edge for the store's machine (fast-try p4, architect m_29189/m_29867): the one place
// that runs its effects. The machine decides (store/machine/), this only executes: a window move
// (an artifact's view beside the thread, at its block) through the router, a window.bise call
// (typed from BiseApi, preload.ts's API) as the effect names it. Mounted once inside the router
// (App.tsx); a test mounts it the same way, or records the effects with store.onEffect.
// Owner: amb-core.
import { useEffect } from 'react';
import { openAtAnchor } from '@/artifacts/Open';
import { openTab } from '@/app/panel/tabs';
import { useStore } from '@/store';
import type { Effect, FxName } from '@/store/machine/types';
import type { BiseApi } from '@/shared/api';
import { artifactAskOf, branchAskOf } from '@/shared/asks';
import { openChange, type ChangeStore } from '@/review/openChange';
import { routeOfOpen, type Route } from './route';
import { useRouter, type Router } from './router';

/** The route a move to an artifact goes to: the artifact's view beside its maker's thread. Pure. */
export function navRoute(to: Extract<Extract<Effect, { out: 'nav' }>['to'], { artifact: string }>): Route {
  return routeOfOpen({ project: to.project, what: 'artifact', id: to.artifact });
}

/**
 * The effect another window's ask runs here (route.ts artifactAsk: the capsule's card chip, its
 * option that opens an artifact; branchAsk: its try card's diff): the artifact or the branch's change
 * in the panel's tab, the window stays where it is, as the chip in this window does. Anything else:
 * undefined. Pure.
 */
export function askEffect(route: string): Extract<Effect, { out: 'nav' }> | undefined {
  const a = artifactAskOf(route);
  if (a !== undefined) return { out: 'nav', to: a, tab: true };
  const b = branchAskOf(route);
  return b === undefined ? undefined : { out: 'nav', to: b, tab: true };
}

/**
 * A move: the artifact's view at its block, in the panel's tab (`tab`) or through `router`; a
 * branch's change in the panel (page pr-guided V2), through the door when the store is here (its
 * agent's thread in the centre, its PR if it has one), else its own tab.
 */
export function runNav(e: Extract<Effect, { out: 'nav' }>, router: Pick<Router, 'go'> | null, store?: ChangeStore): void {
  if ('branch' in e.to) {
    if (store === undefined) openTab({ kind: 'branch', project: e.to.project, branch: e.to.branch });
    else openChange(store, router, e.to.project, { branch: e.to.branch });
    return;
  }
  if (e.to.at !== null) openAtAnchor(`${e.to.project}/${e.to.artifact}`, e.to.at);
  // a card's chip: the panel's tab only, the thread and its open card stay (his m_47572)
  if (e.tab === true) openTab({ kind: 'artifact', project: e.to.project, id: e.to.artifact });
  else router?.go(navRoute(e.to));
}

/** Runs one effect: a move through `router`, a call on `bise` (none: a test with no window.bise). */
export function runEffect(e: Effect, router: Router, bise: Pick<BiseApi, FxName> | undefined, store?: ChangeStore): void {
  switch (e.out) {
    case 'nav':
      runNav(e, router, store);
      return;
    case 'fx':
      void bise?.[e.fx.fx](...e.fx.args);
      return;
  }
}

/** The edge, mounted inside the router: every effect of the store's machine runs here. */
export function StoreEdge(): null {
  const store = useStore();
  const router = useRouter();
  useEffect(
    () =>
      store.onEffect((e) => {
        runEffect(e, router, (globalThis as { bise?: BiseApi }).bise, store);
      }),
    [store, router],
  );
  return null;
}
