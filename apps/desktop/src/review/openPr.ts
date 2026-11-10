// A PR's one door (pr-integration A rule 7; page pr-guided V2): its right-bar row, the agent's ⑂
// chip, the thread's 'PR #412' line, the artifacts' grid and list and the Pull requests screen all
// open its tab in the panel, its guided review, and put its agent's thread in the centre (where he
// asks about it; back goes where he was); ⌘-click on any of them opens the forge's page instead.
// Owner: pr-artifacts.
import { useCallback, type MouseEvent } from 'react';
import type { Artifact } from '@/proto';
import { openTab } from '@/app/panel/tabs';
import type { Route } from '@/app/route';
import { useRouter, type Router } from '@/app/router';
import { useStore } from '@/store';

/** The thread a PR's door puts in the centre: its first agent's, while that agent is here; none for a PR no agent made. Pure. */
export function prCentre(project: string, url: string, arts: readonly Artifact[], agents: readonly string[]): Route | null {
  const agent = arts.find((a) => a.pr != null && a.url === url)?.pr?.status?.agents.find((n) => agents.includes(n));
  return agent === undefined ? null : { screen: 'thread', project, agent };
}

/** The store's reads the door needs (the Store has them). */
export interface PrStore {
  artifacts(project: string): readonly Artifact[];
  agents(project: string): readonly { name: string }[];
}

/** ⌘-click: the forge's page in his browser; a plain click: the PR's tab, its agent's thread in the centre. */
export function openPr(store: PrStore, router: Pick<Router, 'go' | 'get'> | null, project: string, url: string, e?: Pick<MouseEvent, 'metaKey'>): void {
  if (e?.metaKey === true) {
    void (window as Partial<Window>).bise?.openExternal(url);
    return;
  }
  openTab({ kind: 'pr', project, url });
  const to = prCentre(project, url, store.artifacts(project), store.agents(project).map((a) => a.name));
  const now = router?.get();
  if (to !== null && to.screen === 'thread' && !(now?.screen === 'thread' && now.project === to.project && now.agent === to.agent)) router?.go(to);
}

/** openPr for a component: the store and router of its tree. */
export function useOpenPr(): (project: string, url: string, e?: Pick<MouseEvent, 'metaKey'>) => void {
  const store = useStore();
  const router = useRouter();
  return useCallback((project: string, url: string, e?: Pick<MouseEvent, 'metaKey'>) => { openPr(store, router, project, url, e); }, [store, router]);
}
