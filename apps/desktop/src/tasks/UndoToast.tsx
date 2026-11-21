// The short Undo after his gesture (roma idea 4): the Task tab's footer and a Board card offer it
// for UNDO_MS after Pause, Resume and Mark done; its click sends the inverse gesture (undo.ts),
// through the hub's task_act like any of his. Owner: tasks.
import { useEffect, useRef, useState } from 'react';
import { Toast, tipAttrs, WordCut } from '@/ds';
import type { Task as TaskReq, WorkRow } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { cutAtWord, type ProjWork } from './model';
import { didWords, inverse, undoable, UNDO_MS, type Did } from './undo';

/** The most letters of a task's name the toast keeps on its one line ('Speed up the signup flow…'). */
const TOAST_NAME_MAX = 25;
/** Where the name sits in the toast's words ('<name> marked done'): what follows it is the verb. */
const MARK = '\u0001';

/** The gesture the Undo is offered for, null none; `offer` starts a new one (the old one goes). */
export function useUndo(): [Did | null, (d: Did | null) => void] {
  const [did, setDid] = useState<Did | null>(null);
  const t = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (t.current !== null) clearTimeout(t.current);
  }, []);
  return [
    did,
    (d) => {
      if (t.current !== null) clearTimeout(t.current);
      setDid(d);
      if (d !== null)
        t.current = setTimeout(() => {
          setDid(null);
        }, UNDO_MS);
    },
  ];
}

/** The toast: what he did and its Undo; nothing when the gesture has no inverse (yet) or its
 *  task is gone from the works (folded: Undo is not a way back from a tomb). */
export function UndoToast({ did, works, name, send, done, testId, bar = false }: {
  /** in the right bar's card (designer m_67367): the card's inner padding, a name cut so the
   *  line fits the card at its narrowest (260), Undo always whole */
  bar?: boolean;
  did: Did | null;
  works: readonly Pick<WorkRow, 'id'>[];
  name: string;
  send: (project: string, req: TaskReq) => void;
  done: () => void;
  testId: string;
}) {
  if (did === null || !undoable(did, works)) return null;
  // whether it has an inverse doesn't depend on the time, only its end does (taken at his click)
  if (inverse(did, 0) === null) return null;
  // one rule everywhere (designer m_67404): Undo whole, then the verb whole, then the name, which
  // gives way first (cut at a word to its room) and drops whole when not even one word fits; the
  // whole name always in the line's tip
  const cut = cutAtWord(name, TOAST_NAME_MAX);
  // the words around the name: before it ('vb-architect removed from'), after it ('marked done')
  const [pre = '', post = ''] = didWords(did, MARK).split(MARK).map((x) => x.trim());
  return (
    <div className={bar ? 'tk-undo bar' : 'tk-undo'}>
      {/* no lead mark: nothing passed or failed (designer m_65397) */}
      <Toast action={{ text: tasks.undo, testId: `${testId}.undo`, onClick: () => {
        // the inverse at his click (a Resume undone keeps its end only if it is still to come)
        const now = inverse(did, Date.now());
        if (now !== null) send(did.project, now);
        done();
      } }} testId={testId}>
        {/* one line: the name cut at a word with one '…' (or gone), the verb whole, the whole name in its tip (designer m_65519, m_67404) */}
        <span className="tk-undo-what" {...tipAttrs(name)} data-testid={`${testId}.what`}>
          {pre !== '' && <span className="tk-undo-verb" data-testid={`${testId}.pre`}>{pre}</span>}
          <WordCut className="tk-undo-name" text={cut} drop />
          {post !== '' && <span className="tk-undo-verb" data-testid={`${testId}.verb`}>{post}</span>}
        </span>
      </Toast>
    </div>
  );
}

/** Mark all done's Undo (his pick B on page tasks-true): the tasks he marked in one gesture, null
 *  none; `offer` starts a new one for UNDO_MS (the old one goes). */
export function useBatchUndo(): [readonly ProjWork[] | null, (rows: readonly ProjWork[] | null) => void] {
  const [rows, setRows] = useState<readonly ProjWork[] | null>(null);
  const t = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (t.current !== null) clearTimeout(t.current);
  }, []);
  return [
    rows,
    (r) => {
      if (t.current !== null) clearTimeout(t.current);
      setRows(r);
      if (r !== null)
        t.current = setTimeout(() => {
          setRows(null);
        }, UNDO_MS);
    },
  ];
}

/** One toast for one gesture: '12 tasks marked done · Undo'; its Undo reopens each of them. */
export function BatchToast({ rows, send, done, testId }: { rows: readonly ProjWork[] | null; send: (project: string, req: TaskReq) => void; done: () => void; testId: string }) {
  if (rows === null || rows.length === 0) return null;
  return (
    <div className="tk-undo bar">
      <Toast action={{ text: tasks.undo, testId: `${testId}.undo`, onClick: () => {
        for (const x of rows) send(x.project, { step: 'reopen', id: x.w.id });
        done();
      } }} testId={testId}>
        <span className="tk-undo-what" data-testid={`${testId}.what`}>{tasks.undo_done_all({ n: rows.length })}</span>
      </Toast>
    </div>
  );
}
