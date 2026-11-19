// Move to task… where the native menu doesn't reach (tasks-ux m_65591): a visible ⋯ on an inbox
// card's row (the inbox page, the thread's strip, the Task tab's Needs you) and on the Task tab's
// artifact rows. It shows on the row's hover and keyboard focus, its space kept (nothing shifts),
// tip 'More', beside the row's own acts, never over them (taste 36). A pick opens the picker
// (moveTo.ts askMove) where he clicked; Remove from <task> applies at once with its toast. Owner: tasks.
import { Dropdown, type PickerRow } from '@/ds';
import type { Artifact } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { useWorks } from '@/store';
import { cutAtWord } from './model';
import { copyText } from '@/quote/QuotePop';
import { askMove, lastAt, movedWords, NAME_MAX, tellMoved, worksWithout, type Thing } from './moveTo';
import { useSendMove } from './MoveHost';
import type { MouseEvent } from 'react';
import './tasks.css';

/** The tasks whose cards hold this card (one, or none). */
function useCardWorks(project: string, card: number): readonly string[] {
  return useWorks(project).works.filter((w) => (w.cards ?? []).includes(card)).map((w) => w.id);
}

/** A card row's right click: the ⋯'s one item, Move to task…, so its picker opens where he clicked. */
export function useCardMoveMenu(project: string, card: number, title: string): (e: MouseEvent) => void {
  const now = useCardWorks(project, card);
  return (e) => {
    e.preventDefault();
    askMove({ thing: { kind: 'card', project, card, title, works: now }, mode: 'move', x: e.clientX, y: e.clientY });
  };
}

/** An inbox card's ⋯: Move to task… (a card belongs to one task: the one whose cards hold it). */
export function CardMoveMore({ project, card, title, testId }: { project: string; card: number; title: string; testId: string }) {
  const now = useCardWorks(project, card);
  return (
    // a sibling of the row's own button, never in it: a click on it is its own (open, answer stay theirs)
    <span className="tk-more-slot">
      <Dropdown look="more" label="⋯" name={tasks.act_more_move} tip={tasks.act_more_move} testId={testId}
        groups={[{ id: 'move', rows: [{ id: 'move', name: tasks.move_to, testId: `${testId}.move` }] }]}
        onPick={() => {
          askMove({ thing: { kind: 'card', project, card, title, works: now }, mode: 'move', x: lastAt.x, y: lastAt.y });
        }} />
    </span>
  );
}

/** An artifact row's ⋯: Move to task…, Also add to task…, and Remove from <task> with 2+ tasks
 *  (the native menu's items, its words). */
export function ArtMoveMore({ project, a, testId, out }: { project: string; a: Pick<Artifact, 'id' | 'kind' | 'title' | 'works'>; testId: string; out?: { readonly url: string; readonly forge: string } }) {
  const works = useWorks(project).works;
  const send = useSendMove();
  const now = a.works ?? [];
  const thing: Thing = { kind: 'art', project, id: a.id, page: a.kind === 'page', title: a.title, works: now };
  const nameOf = (id: string): string => works.find((w) => w.id === id)?.name ?? id;
  const rows: PickerRow[] = [
    { id: 'move', name: tasks.move_to, testId: `${testId}.move` },
    { id: 'also', name: tasks.move_also, testId: `${testId}.also` },
    ...(now.length >= 2 ? now.map((w) => ({ id: `rm:${w}`, name: tasks.move_remove({ name: cutAtWord(nameOf(w), NAME_MAX) }), testId: `${testId}.rm.${w}` })) : []),
  ];
  return (
    <span className="tk-more-slot">
      <Dropdown look="more" label="⋯" name={tasks.act_more_move} tip={tasks.act_more_move} testId={testId}
        // a PR's row: 'Open on GitHub ↗' and 'Copy link' above the Move items (tasks-ux m_67602, taste 36)
        groups={[...(out === undefined ? [] : [{ id: 'out', rows: [
          { id: 'out:open', name: tasks.pr_more_open({ forge: out.forge }), testId: `${testId}.open` },
          { id: 'out:copy', name: tasks.pr_more_copy, testId: `${testId}.copy` },
        ] }]), { id: 'move', rows }]}
        onPick={(id) => {
          if (out !== undefined && id === 'out:open') {
            void (window as Partial<Window>).bise?.openExternal(out.url);
            return;
          }
          if (out !== undefined && id === 'out:copy') {
            copyText(out.url);
            return;
          }
          if (id === 'move' || id === 'also') {
            askMove({ thing, mode: id, x: lastAt.x, y: lastAt.y });
            return;
          }
          const off = id.slice('rm:'.length);
          send(thing, worksWithout(now, off));
          tellMoved({ thing, before: now, words: movedWords('removed', a.title, nameOf(off)), at: Date.now() });
        }} />
    </span>
  );
}
