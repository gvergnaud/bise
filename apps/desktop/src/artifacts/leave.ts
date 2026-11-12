// A browser-only artifact (designer m_21714, his feedback main m_21706): a PR, a link, any kind
// with no in-app view (model.ts leaves). A click or ⏎ on its row (the artifacts list, the right
// bar, ⌘K, its thread chip) or space opens his browser at once: no panel, no history entry, the
// list keeps its selection. Every other kind opens in the window as before. Owner: amb-web.
// The rule is one (model.ts leaves) but its dispatch is in two places for now: an inbox option's
// act decides it in the store's machine (store/machine/cards.ts opens, fast-try p4 1d); the rows
// here (ThreadRows, Peek, RightBar, Palette, rows-news) still call useGoArtifact. Each of those
// screens' slices moves its call to store.do (a nav intent through opens()); then useGoArtifact
// is deleted (architect m_29955, rule 7).
import { useCallback } from 'react';
import { routeOfOpen, type Route } from '@/app/route';
import { useRouter } from '@/app/router';
import { useStore } from '@/store';
import { leaves, type Leaving } from './model';
import { sayOpened } from './opened';
import { artifactGesture, countGesture } from '@/app/usage';

export { inApp, leaves, LEAVES, type Leaving } from './model';

const openExternal = (): ((u: string) => unknown) | undefined =>
  (globalThis as { bise?: { openExternal?: (u: string) => unknown } }).bise?.openExternal;

/**
 * Where one artifact opens: `browser`, the url his browser takes for a browser-only one (nothing
 * in the window moves), and the window's route for it (its view beside the maker's thread), the
 * fallback with no browser to hand it to. `a`: the store's, else the row's own idea of it. Pure.
 */
export function artifactGo(a: Leaving | undefined, project: string, id: string): { route: Route; browser?: string } {
  const route = routeOfOpen({ project, what: 'artifact', id });
  return a !== undefined && leaves(a) ? { route, browser: a.url ?? '' } : { route };
}

/**
 * Open one artifact from any list: a browser-only one in his browser (nothing in the window
 * moves), else its route. `known` is the row's own idea of it, for an artifact the store
 * doesn't hold (a thread chip's `made`).
 */
export function useGoArtifact(): (project: string, id: string, known?: Leaving) => void {
  const router = useRouter();
  const store = useStore();
  return useCallback(
    (project, id, known) => {
      const a = store.artifacts(project).find((x) => x.id === id) ?? known;
      const plan = artifactGo(a, project, id);
      const go = openExternal();
      if (plan.browser !== undefined && go !== undefined) {
        countGesture(artifactGesture(a?.kind ?? ''));
        // his open in his browser: its hub moves its last touch (opened.ts; a tab's is openTab's)
        sayOpened(store, project, id);
        void go(plan.browser);
        return;
      }
      router.go(plan.route);
    },
    [router, store],
  );
}
