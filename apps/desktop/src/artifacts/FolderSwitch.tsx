// A folder's For you · Everything switch (his condition on his q-library pick, artifacts-signal v13;
// designer m_62289 3b, his lib1-switch block): at the right of the place's head line, never a row
// over the list, For you by default and kept across folders and visits (Screen.tsx keeps it). For
// you: the folder's files for him, then one quiet row '14 from agents · Show everything' that flips
// it; Everything: every file in place, an agent's with its name on the card's line. Not in ~/bise's
// folders (all his: a slot's place, slots.ts) nor in the column's views. The split is the hub's
// section (sections.ts), never re-decided. Owner: artifacts-keep.
import { Press, Seg } from '@/ds';
import { library } from '@/proto/words_library.gen';
import type { Item } from './model';
import { isHis } from './folders';

export type FolderMode = 'you' | 'all';

/** What a folder shows in this mode: the items, and how many the switch hides (the agents'). Pure. */
export function folderView(items: readonly Item[], mode: FolderMode): { shown: Item[]; hidden: number } {
  if (mode === 'all') return { shown: [...items], hidden: 0 };
  const shown = items.filter(isHis);
  return { shown, hidden: items.length - shown.length };
}

const MODES: readonly FolderMode[] = ['you', 'all'];

export function FolderSwitch({ mode, onMode, testId }: { mode: FolderMode; onMode: (m: FolderMode) => void; testId: string }) {
  return (
    <span className="art-fswitch" aria-label={library.folder_switch}>
      <Seg items={[library.for_you, library.everything]} on={MODES.indexOf(mode)} onPick={(i) => { onMode(MODES[i] ?? 'you'); }} testId={testId} />
    </span>
  );
}

/** Under a folder on For you: '14 from agents · Show everything', which flips the switch. */
export function HiddenRow({ n, onShow, testId }: { n: number; onShow: () => void; testId: string }) {
  return (
    <div className="art-quiet" data-testid={testId}>
      <span className="art-hidden-n">{library.n_from_agents({ n })}</span>
      <Press look="art-quiet-row" onClick={onShow} data-testid={`${testId}.show`}>{library.show_everything}</Press>
    </div>
  );
}
