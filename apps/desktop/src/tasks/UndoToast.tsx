// The short Undo after his gesture (roma idea 4): the Task tab's footer and a Board card offer it
// for UNDO_MS after Pause, Resume and Mark done; its click sends the inverse gesture (undo.ts),
// through the hub's task_act like any of his. Owner: tasks.
import { useEffect, useRef, useState } from 'react';
import { Toast } from '@/ds';
import type { Task as TaskReq, WorkRow } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { didWords, inverse, undoable, UNDO_MS, type Did } from './undo';

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
export function UndoToast({ did, works, name, send, done, testId }: {
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
  return (
    <div className="tk-undo">
      {/* no lead mark: nothing passed or failed (designer m_65397) */}
      <Toast action={{ text: tasks.undo, testId: `${testId}.undo`, onClick: () => {
        // the inverse at his click (a Resume undone keeps its end only if it is still to come)
        const now = inverse(did, Date.now());
        if (now !== null) send(did.project, now);
        done();
      } }} testId={testId}>{didWords(did, name)}</Toast>
    </div>
  );
}
