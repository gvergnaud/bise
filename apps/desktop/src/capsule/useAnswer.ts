// bise's answer to what he asked from the spotlight or a talk: main's thread of bise's home,
// subscribed in the capsule's window too (qa-flows: the capsule's store never had it), streamed
// (the hub's partial, typing) or arriving whole (a new agent entry after his question). What it
// shows is answer.ts's: a gap in the stream (a tool call, a cleared partial) never ends it while
// main's turn runs, and the whole entry takes its place when it lands (page spotlight-reponse v2).
// CapsuleRoot owns the view; this hook owns the answer's state.
import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import type { Store } from '@/store';
import { threadKey } from '@/store';
import type { CorePort } from '@/shared/api';
import { followPartial, type LivePartial } from '@/shared/partial';
import type { SpotAnswer } from '@/spotlight';
import { answerOf } from './answer';

/** His ask being answered: entries after `after` are the answer's; `rebase` waits for the follow's fresh page. */
export interface Asked {
  after: number;
  started: boolean;
  rebase: boolean;
}

export interface MainAnswer {
  readonly last: SpotAnswer | null;
  readonly setLast: (a: SpotAnswer | null) => void;
  /** his ask went to main of `home`: its answer is what comes after now */
  readonly followAsk: (home: string) => void;
  /** the ask being answered (null: none; route_done drops it when the ask went elsewhere) */
  readonly asked: RefObject<Asked | null>;
  /** the exchange ended: no ask followed, no answer shown */
  readonly forget: () => void;
}

/**
 * Follows main of `home` for his ask's answer. `onStart` runs once per ask, with its first words
 * (the capsule's view opens the answer then). `port`: the raw core port the partials come on.
 */
export function useMainAnswer(store: Store, home: string | null, port: Pick<CorePort, 'onEvent'> | undefined, onStart: () => void): MainAnswer {
  const [last, setLast] = useState<SpotAnswer | null>(null);
  // what shows, for the store's callbacks (a gap keeps it until the entry or the turn's end)
  const lastRef = useRef(last);
  useEffect(() => {
    lastRef.current = last;
  });
  const startRef = useRef(onStart);
  useEffect(() => {
    startRef.current = onStart;
  });
  // rebase: his ask follows main's thread again (R18: another window's unsubscribe used to drop
  // this one's follow, and the answer never came); until that fresh page lands, an entry already
  // in it is never his answer, so `after` moves past the page first
  const asked = useRef<Asked | null>(null);
  const followAsk = useCallback(
    (h: string): void => {
      const order = store.threadOrder(threadKey(h, 'main')).order;
      // (the answer view opens with its first reply, which replaces the last answer then)
      asked.current = { after: order.length === 0 ? -1 : Math.max(...order), started: false, rebase: true };
      store.subscribeThread(h, 'main', 20);
    },
    [store],
  );
  // main's reply while it is written (the hub's partial, 250 ms at most; shared/partial.ts): it
  // shows from its first words, and the entry that lands after it takes its place
  const liveRef = useRef<LivePartial | null>(null);
  useEffect(() => {
    if (home === null) return;
    const key = threadKey(home, 'main');
    store.subscribeThread(home, 'main', 20);
    const read = (): void => {
      const a = asked.current;
      if (a === null) return;
      const reply = a.rebase
        ? undefined
        : store
            .threadOrder(key)
            .order.filter((pos) => pos > a.after)
            .map((pos) => store.entry(key, pos))
            .filter((e) => e?.kind === 'agent')
            .at(-1);
      const next = answerOf({
        partial: liveRef.current?.text ?? null,
        typing: store.typing(key),
        reply: reply?.text,
        working: store.agents(home).find((x) => x.main)?.status === 'working',
        shown: lastRef.current,
      });
      if (next === null) return;
      if (!a.started) {
        a.started = true;
        startRef.current();
      }
      lastRef.current = next;
      setLast(next);
    };
    const offs = [
      store.subscribe(`typing:${key}`, read),
      store.subscribe(`order:${key}`, read),
      // main's turn ending (its status leaves working) ends the answer
      store.subscribe(`agents:${home}`, read),
      // the ask's follow is live again: its fresh page (always before the ask's own entries, one
      // connection, in order) moves `after` past what it holds; a live entry first (no page: a
      // core that never dropped the follow) just ends the wait
      store.onEvent((ev) => {
        const a = asked.current;
        if (a === null || !a.rebase || (ev.ev !== 'thread' && ev.ev !== 'entry') || ev.project !== home || ev.agent !== 'main') return;
        a.rebase = false;
        if (ev.ev === 'thread') a.after = Math.max(a.after, ...ev.entries.map((e) => e.pos));
        else read();
      }),
    ];
    if (port !== undefined) {
      offs.push(
        // cleared (its entry landed, a gap, a tool call, a stop): what came stays, with its '∿'
        // while main's turn runs; the entry, when it lands, takes its place
        followPartial(port, home, 'main', (live) => {
          liveRef.current = live;
          if (live !== null || asked.current?.started === true) read();
        }),
      );
    }
    return () => {
      offs.forEach((off) => {
        off();
      });
      store.unsubscribeThread(home, 'main');
    };
  }, [store, home, port]);
  const set = useCallback((a: SpotAnswer | null): void => {
    lastRef.current = a;
    setLast(a);
  }, []);
  const forget = useCallback((): void => {
    asked.current = null;
    lastRef.current = null;
    setLast(null);
  }, []);
  return { last, setLast: set, followAsk, asked, forget };
}
