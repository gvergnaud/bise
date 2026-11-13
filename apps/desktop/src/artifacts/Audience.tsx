// For you | All (his ask m_63198, after his q-library pick; designer m_63213): a toggle in the
// artifacts page's filter bar, between the project and When, in every place (the library's Home,
// Recent, Kept, a folder, Unfiled, Archived, a search), For you by default, kept across places and
// visits (Screen.tsx keeps it). For you: his files (every section but the agents'), then one quiet
// row '14 from agents · Show all' that flips it; All: every file in place, an agent's leading with
// its name on the card's line. Hidden in a slot's places (~/bise, Shared: all his). The split is
// the hub's section (sections.ts), never re-decided. Owner: artifacts-keep.
import { Filters, Press } from '@/ds';
import { library } from '@/proto/words_library.gen';
import type { Item } from './model';
import { isHis } from './folders';

export type Audience = 'you' | 'all';

/** What a place shows for this side: its items, and how many the toggle hides (the agents'). Pure. */
export function audienceView(items: readonly Item[], aud: Audience): { shown: Item[]; hidden: number } {
  if (aud === 'all') return { shown: [...items], hidden: 0 };
  const shown = items.filter(isHis);
  return { shown, hidden: items.length - shown.length };
}

/** 'For you | All' as a 2-pill group, the app tabs' shape and height (designer m_63680: one control
 *  shape in the bar; underline tabs stay for a page's own sections). Its parts: `<testId>.you|all`. */
export function AudienceToggle({ aud, onAud, testId }: { aud: Audience; onAud: (a: Audience) => void; testId: string }) {
  return (
    <Filters label={library.audience_switch} on={aud} testId={testId}
      items={[{ id: 'you', label: library.for_you }, { id: 'all', label: library.all_audience }]}
      onPick={(id) => { onAud(id === 'all' ? 'all' : 'you'); }} />
  );
}

/** Under the list on For you: '14 from agents · Show all', which flips the toggle. */
export function HiddenRow({ n, onShow, testId }: { n: number; onShow: () => void; testId: string }) {
  return (
    <div className="art-quiet" data-testid={testId}>
      <span className="art-hidden-n">{library.n_from_agents({ n })}</span>
      <Press look="art-quiet-row" onClick={onShow} data-testid={`${testId}.show`}>{library.show_all}</Press>
    </div>
  );
}
