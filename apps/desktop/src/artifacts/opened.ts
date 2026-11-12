// His opens and his keeps, as the hub's commands (artifacts_marks.rs; page artifacts-signal v3):
// an artifact that opens (its tab: app/panel/tabs.ts openTab; his browser: leave.ts) tells its hub,
// which moves its `opened_ms` and the version he saw (`seen_v`) forward, so the sections order it by
// its last touch. One function for both doors (on plugin-apps the hub's view/open does it itself:
// this is the one spot that moves at the sync). Keep / Let go from a card's menu: one command per
// project. Pure but for the store it is handed. Owner: artifacts-keep.
import type { Artifact } from '@/proto';
import type { Store } from '@/store';
import type { Item } from './model';

/** He opened artifact `id` of `project`: its hub hears it, with the version he sees when the list knows it. */
export function sayOpened(store: Pick<Store, 'artifacts' | 'command'>, project: string, id: string, known?: Partial<Pick<Artifact, 'version'>>): void {
  const a = store.artifacts(project).find((x) => x.id === id) ?? known;
  const v = a?.version;
  store.command(v === undefined || v === 0 ? { cmd: 'artifacts_opened', project, id } : { cmd: 'artifacts_opened', project, id, v });
}

/** His Keep (`keep`) or Let go of these items: one command per project. Pure. */
export function keepCmds(items: readonly Item[], keep: boolean): { cmd: 'artifacts_keep'; project: string; ids: string[]; keep: boolean }[] {
  const by = new Map<string, string[]>();
  for (const it of items) {
    const ids = by.get(it.project);
    if (ids === undefined) by.set(it.project, [it.a.id]);
    else ids.push(it.a.id);
  }
  return [...by].map(([project, ids]) => ({ cmd: 'artifacts_keep', project, ids, keep }));
}
