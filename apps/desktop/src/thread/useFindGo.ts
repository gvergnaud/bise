// ⌘F's go-to-match (thread-perf): the current match's entry into view, however far back. Held in
// the thread's view: revealed (no scroll when it shows whole already). Not held: the store reads
// the page around it (store.jumpTo, the view's window with its gap row to the latest: inbox-open),
// and it is revealed once that page is in. One page asked at a time: ⏎ held down walks the
// matches faster than the hub answers, and a page asked for a match he already left would land in
// the view after the next jump replaced its window. The newest match he went to wins; a page that
// never comes frees the next one after ASK_MS. Owner: thread-perf.
import { useCallback, useEffect, useRef, type RefObject } from 'react';
import { threadKey, useThreadOrder, type Store } from '@/store';
import { entryKey } from './entry';
import type { ThreadListHandle } from './ThreadList';

/** A page asked and not in after this long: the next jump goes anyway. */
const ASK_MS = 3000;

/** How the entry shows once held: its top `top` px under the list's top, not centred; `shown` once it is in view (msg-dots' ticks). */
export interface GoHow {
  readonly top?: number;
  readonly shown?: (pos: number) => void;
}

/** The function FindBar's onGo calls with the current match's pos (and a tick of his, msg-dots, with `how`). */
export function useFindGo(store: Store, project: string, agent: string, list: RefObject<ThreadListHandle | null>, how?: GoHow): (pos: number) => void {
  const top = how?.top;
  const shown = how?.shown;
  const view = useThreadOrder(threadKey(project, agent));
  // the match to show, and when its page was asked (0: none on its way)
  const want = useRef<number | undefined>(undefined);
  const asked = useRef(0);
  const go = useCallback(() => {
    const pos = want.current;
    if (pos === undefined) return;
    if (asked.current > 0 && Date.now() - asked.current < ASK_MS) return;
    const at = store.jumpTo(project, agent, pos);
    if (at.kind === 'held') {
      want.current = undefined;
      asked.current = 0;
      const p = at.pos;
      list.current?.reveal(entryKey(p), shown === undefined ? undefined : () => { shown(p); }, top);
    } else asked.current = at.kind === 'asked' ? Date.now() : 0;
  }, [store, project, agent, list, top, shown]);
  // a page came (its answer, or another): the newest match he went to, now
  useEffect(() => {
    asked.current = 0;
    go();
  }, [view, go]);
  return useCallback(
    (pos: number) => {
      want.current = pos;
      go();
    },
    [go],
  );
}
