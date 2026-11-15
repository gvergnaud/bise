// Move to task… (F9; spec tasks-ux m_65409, look designer m_65406): the picker and its toast,
// drawn once for the whole window (App.tsx). An artifact's or a card's menu asks for it (moveTo.ts
// askMove) where he clicked; picking a row sends the hub's task_move at once (exactly the new
// works, his for good) and shows '<thing> moved to <task> · Undo' for 6 s; Undo sends the works it
// had before, exactly (a new gesture, never a rewind). Owner: tasks.
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Input, PickerPop, Toast, tipAttrs, type PickerGroup, type PickerRow } from '@/ds';
import type { WorkRow } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { useStore, useWorks } from '@/store';
import { useNow } from '@/app/clock';
import { cutAtWord } from './model';
import { StatusMark, statusWord } from './Marks';
import { useSeenTasks } from './seen';
import { UNDO_MS } from './undo';
import { askMove, filtered, lastAt, movedWords, NAME_MAX, NO_TASK, offered, tellMoved, useMoveAsk, useMoved, worksAfter, type Ask, type Thing } from './moveTo';

/** The hub's task_move for `thing` onto exactly `works`. */
export function useSendMove(): (thing: Thing, works: readonly string[]) => void {
  const store = useStore();
  return (thing, works) => {
    if (thing.kind === 'art') store.command({ cmd: 'task_move', project: thing.project, art: thing.id, page: thing.page, works: [...works] });
    else store.command({ cmd: 'task_move', project: thing.project, card: thing.card, works: [...works] });
  };
}

/** The picker and the toast, once for the window. */
export function MoveHost() {
  const ask = useMoveAsk();
  // where he last pressed: a native menu's pick opens the picker there
  useEffect(() => {
    const keep = (e: MouseEvent): void => {
      lastAt.x = e.clientX;
      lastAt.y = e.clientY;
    };
    window.addEventListener('pointerdown', keep, true);
    window.addEventListener('contextmenu', keep, true);
    return () => {
      window.removeEventListener('pointerdown', keep, true);
      window.removeEventListener('contextmenu', keep, true);
    };
  }, []);
  return (
    <>
      {ask !== null && <MovePicker key={`${String(ask.x)}:${String(ask.y)}:${ask.mode}`} ask={ask} />}
      <MoveToast />
    </>
  );
}

/** A task's row: its Board mark (with its tip) and its name cut at a word, the whole name in the
 *  tip; the current task ✓ and disabled, never hidden (designer m_65406 1-2). */
function row(w: WorkRow, current: boolean, faded: boolean): PickerRow {
  const name = cutAtWord(w.name, 36);
  return {
    id: w.id,
    name: faded ? <span className="tk-move-done">{name}</span> : name,
    text: w.name,
    glyph: <StatusMark status={w.status} tip={statusWord(w.status)} />,
    current,
    disabled: current,
    tip: current ? tasks.move_current_tip : name === w.name ? undefined : w.name,
    testId: `tasks.move.row.${w.id}`,
  };
}

function MovePicker({ ask }: { ask: Ask }) {
  const { thing, mode } = ask;
  const works = useWorks(thing.project).works;
  const seen = useSeenTasks();
  const send = useSendMove();
  const anchor = useRef<HTMLSpanElement>(null);
  const [q, setQ] = useState('');
  const [active, setActive] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    field.current?.focus();
  }, []);
  const now = useNow();
  const { open, done } = offered(thing, works, (id) => seen.seen(thing.project, id)?.at ?? 0, now);
  const shownOpen = filtered(open, q);
  const shownDone = filtered(done, q);
  const cur = new Set(thing.works);
  const groups: PickerGroup[] = [
    { id: 'open', rows: shownOpen.map((w) => row(w, cur.has(w.id), false)) },
    ...(shownDone.length > 0 ? [{ id: 'done', rows: shownDone.map((w) => row(w, cur.has(w.id), true)) }] : []),
    // 'No task' last, after a divider (artifacts only: a card belongs to one task)
    ...(thing.kind === 'art' && thing.works.length > 0 ? [{ id: 'none', look: 'tk-move-none', rows: [{ id: `none:${NO_TASK}`, name: tasks.move_no_task, testId: 'tasks.move.none' }] }] : []),
  ];
  const ids = groups.flatMap((g) => g.rows).filter((r) => r.disabled !== true).map((r) => r.id);
  const on = active !== null && ids.includes(active) ? active : (ids[0] ?? null);
  const close = (): void => {
    askMove(null);
  };
  const pick = (id: string, how = mode): void => {
    const target = id.startsWith('none:') ? NO_TASK : id;
    const next = worksAfter(thing.works, target, thing.kind === 'card' ? 'move' : how);
    send(thing, next);
    const name = works.find((w) => w.id === target)?.name ?? '';
    const what = target === NO_TASK ? 'none' : how === 'also' && thing.kind === 'art' ? 'added' : 'moved';
    tellMoved({ thing, before: thing.works, words: movedWords(what, thing.title, name), at: Date.now() });
    close();
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>): void => {
    const i = on === null ? -1 : ids.indexOf(on);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (ids.length === 0) return;
      const n = e.key === 'ArrowDown' ? (i + 1) % ids.length : (i - 1 + ids.length) % ids.length;
      setActive(ids[n] ?? null);
    } else if (e.key === 'Enter' && on !== null) {
      e.preventDefault();
      // ⌘⏎ on a row while moving: Also add (artifacts)
      pick(on, (e.metaKey || e.ctrlKey) && thing.kind === 'art' ? 'also' : mode);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  };
  const nowIn = thing.works.map((id) => works.find((w) => w.id === id)?.name ?? id);
  return (
    <>
      <span ref={anchor} className="tk-move-at" style={{ left: ask.x, top: ask.y }} aria-hidden="true" />
      <PickerPop anchor={anchor} callerKeys active={on} onClose={close} testId="tasks.move"
        title={
          <span className="tk-move-head">
            <span className="tk-move-title">{mode === 'also' ? tasks.move_also_head : tasks.move_head}</span>
            <span className="tk-move-now" data-testid="tasks.move.now">
              {nowIn.length === 0 ? tasks.move_now_none : `${tasks.move_now_in} `}
              {nowIn.slice(0, 2).map((n) => (
                <span key={n} className="tk-move-chip" {...tipAttrs(n)}>{cutAtWord(n, NAME_MAX)}</span>
              ))}
              {nowIn.length > 2 && <span className="tk-move-chip">+{String(nowIn.length - 2)}</span>}
            </span>
            <Input ref={field} look="tk-move-filter" placeholder={tasks.move_filter} value={q} onKeyDown={onKey}
              onChange={(e) => {
                setQ(e.target.value);
                setActive(null);
              }} data-testid="tasks.move.filter" />
          </span>
        }
        groups={groups}
        empty={tasks.move_no_match}
        keyline={thing.kind === 'art' && mode === 'move' ? <span className="tk-move-hint">{tasks.move_hint}</span> : undefined}
        onPick={(id) => {
          pick(id);
        }} />
    </>
  );
}

/** '<thing> moved to <task> · Undo', 6 s, at the window's foot: Undo sends its works before, exactly. */
function MoveToast() {
  const d = useMoved();
  const send = useSendMove();
  useEffect(() => {
    if (d === null) return;
    const t = setTimeout(() => {
      tellMoved(null);
    }, UNDO_MS);
    return () => {
      clearTimeout(t);
    };
  }, [d]);
  if (d === null) return null;
  return (
    <div className="tk-move-toast">
      <Toast action={{ text: tasks.undo, testId: 'tasks.move.toast.undo', onClick: () => {
        send(d.thing, d.before);
        tellMoved(null);
      } }} testId="tasks.move.toast">
        <span className="tk-undo-what">{d.words}</span>
      </Toast>
    </div>
  );
}
