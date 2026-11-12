// The thread's list: virtualized, keyed, newest at the bottom. Owner: amb-win.
//
// What it guarantees (the reset's typing and flicker rules):
// - rows are keyed by their own key ('e:<pos>' an entry, 'p:<cid>' a send
//   not confirmed yet), so a new entry mounts one row and an entry replaced
//   at its pos re-renders that row only (the row reads its entry with the
//   store's per-key hook, not from this list's props);
// - rows that come or go before the one he reads don't move it: an older
//   page in front (firstItemIndex goes down), far rows the store trimmed
//   (it goes up; trim.ts, told the row he reads by onRead), a window's gap
//   filled above him; the row is held at its y by its key (thread-perf);
// - a jump of his (⌘F's match, his answer far back) wins over that hold; a reveal of a row not
//   listed yet (⌘F's match in a fold that opens for it) waits for it, HOLD_MS at most;
// - a row he opens or closes keeps its y, or the part of it marked data-hold around his press (a
//   part that changes its own height: the thread's view card going to the panel and back);
// - at the bottom it follows new rows; scrolled up, it stays where he is;
// - the list never holds the composer: it sits beside it in the screen.
import { useCallback, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type ReactNode, type Ref } from 'react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { keep } from '@/app/keep';
import './thread.css';
import { Press } from '@/ds';

// the list's own end, inside the scroller (designer's composer-strip A): the 12 px the bottom fade
// lies on at the end, so it never fades the last row (thread.css .th-list-end)
// the tail (his sends on their way) is the scrollable list's own end, in Virtuoso's footer, never a
// band pinned under the list (his « directement dans l'historique en bas, dans la partie
// scrollable », queue-drawer-fix); given as Virtuoso's own `context` (a React context set
// around it never reached its footer), so the footer component stays the same one
function ListEnd({ context: tail }: { context?: ReactNode }) {
  return (
    <>
      {tail}
      <div className="th-list-end" aria-hidden="true" />
    </>
  );
}
// Virtuoso reads its `components` object (never undefined): the end always, the head when it has one
const END = { Footer: ListEnd };
// Virtuoso measures its own box: it must fill the list's flex area
const FILL = { flex: '1 1 auto', minHeight: 0 };
// it opens on the newest row's end (a long answer's last line, not its
// first): at the bottom, so new rows follow
const LAST = { index: 'LAST', align: 'end' } as const;

/** Far from 0: room for every older row that may come in front. */
const START = 1_000_000_000;

export interface ThreadListHandle {
  /** bring a row into view (⌘F's current match, a card's entry) */
  scrollTo(key: string): void;
  /** bring a row into view only when it isn't whole in it already (answered-link: his answer);
   * `shown` runs once the row is drawn and whole in it (his answer's flash); `top`: its top goes
   * that many px under the list's top edge, not the view's centre (a tick of his, msg-dots) */
  reveal(key: string, shown?: () => void, top?: number): void;
  /** to the newest row */
  scrollToEnd(): void;
  /**
   * Keep the view still while rows open or close (⌘E opens everything): the row at the top of
   * the view keeps its y, the thread grows below it (designer m_17092).
   */
  holdView(): void;
}

export interface ThreadListProps {
  /** the rows' keys, oldest first */
  keys: readonly string[];
  /** one row (its own hooks read its data) */
  row: (key: string) => ReactNode;
  /** older rows exist before the first one */
  more: boolean;
  /** the older page's cursor (the view's `before`): a new one is asked again, the same one once */
  before: number | null;
  /** ask the page before the first row (called again only once `before` changed) */
  onOlder: () => void;
  /** above the oldest row while `more` (a loading line) */
  head?: ReactNode;
  /** under the newest row, the scrollable list's end (its footer): drawn in
   * the same frame as the change that adds it (his sends before they land) */
  tail?: ReactNode;
  /** over the list's bottom edge, under its last row, out of the layout (thread-loading's cue):
   * nothing moves when it comes or goes */
  cue?: ReactNode;
  testId?: string;
  ref?: Ref<ThreadListHandle>;
  /** where his scroll is kept across a reload (keep.ts 'scroll:<it>', 10 minutes); none: not kept */
  keepKey?: string;
  /** the part of a row held still when an older page comes in front (a
   * selector inside the row: its words, not a divider above them that goes
   * once the row before it arrives); none: the whole row */
  holdPart?: string;
  /** a row's React key when not its own (the live row and the entry that takes its place share one) */
  reactKey?: (key: string) => string;
  /**
   * He scrolled up to read (H10, amb-home: the inbox block folds to one line, the composer's keys
   * line hides), or he is back at the end. Told on each change.
   */
  onReading?: (reading: boolean) => void;
  /** the key of the row he reads, scrolled up, when it changes (the store keeps the rows near it: trim.ts) */
  onRead?: (key: string) => void;
}

/**
 * The first item's index for Virtuoso: down by the number of rows that came
 * in front of the previous first one (an older page), up by the number of
 * rows that left the front (the store's trim: far entries leave the view,
 * `before` the keys the list had), unchanged otherwise. Pure, exported for the tests.
 */
export function nextFirstIndex(prev: { first: string | undefined; index: number }, keys: readonly string[], before: readonly string[] = []): { first: string | undefined; index: number } {
  const first = keys[0];
  if (first === prev.first) return prev;
  const k = prev.first === undefined ? -1 : keys.indexOf(prev.first);
  if (k > 0) return { first, index: prev.index - k };
  const j = first === undefined ? -1 : before.indexOf(first);
  return { first, index: k < 0 && j > 0 ? prev.index + j : prev.index };
}

/** Rows still above the drawn range when the older page is asked (a screen or two of short rows). */
export const OLDER_AHEAD = 60;

/**
 * Whether the older page goes now: the drawn range (Virtuoso's, with firstItemIndex: `start`) is
 * within OLDER_AHEAD rows of the first row (`first`, the list's firstItemIndex). Pure.
 */
export function asksOlder(start: number, first: number): boolean {
  return start - first < OLDER_AHEAD;
}

/**
 * Whether the older page may be asked: there is more, and its cursor (the view's `before`) is not
 * the one asked last. Keyed on the cursor, never on the first row: a page that folds to no entry
 * moves the cursor and leaves the rows as they were (architect m_59149, thread-older). Pure.
 */
export function olderAsk(more: boolean, before: number | null, asked: number | null): boolean {
  return more && before !== null && before !== asked;
}

/**
 * The row to hold still while an older page comes in front: the first one
 * whose bottom is under the list's top edge (the line he reads), as
 * Virtuoso's data-item-index (with firstItemIndex: stable across a
 * prepend, unlike data-index, the position in `keys`). None when no row shows. Pure.
 */
export function anchorRow(rows: readonly { index: number; top: number; bottom: number }[], edge: number): { index: number; top: number } | undefined {
  const r = rows.find((x) => x.bottom > edge);
  return r === undefined ? undefined : { index: r.index, top: r.top };
}

/** A row whole inside the list's box (reveal: no scroll then). Pure. */
export function rowWhole(row: { top: number; bottom: number }, view: { top: number; bottom: number }): boolean {
  return row.top >= view.top && row.bottom <= view.bottom;
}

/**
 * His place in a thread as keep.ts holds it: the key of the row he reads (the first one under the
 * top edge) and its top's px from that edge, so it comes back at the same y, back on the thread
 * after an artifact, ‹ › or a reload. Pure.
 */
export function placeOf(v: unknown): { key: string; off: number } | undefined {
  if (typeof v !== 'object' || v === null) return undefined;
  const { key, off } = v as { key?: unknown; off?: unknown };
  return typeof key === 'string' ? { key, off: typeof off === 'number' && Number.isFinite(off) ? off : 0 } : undefined;
}

/**
 * Scrolled up, the row he reads stays where his own scroll put it (scroll-steady, his « ce que je
 * vois ne glitch pas »): `at` is that row's y in the list's box and the list's scrollTop when it
 * was read, `now` the same now. His scroll since then moves it by the scrollTop's change; anything
 * else that moved it (a row above measured at last, a page in front) is the px to scroll by to put
 * it back. Pure.
 */
export function steadyBy(at: { y: number; st: number }, now: { y: number; st: number }): number {
  return now.y - (at.y - (now.st - at.st));
}

/** How long after a prepend the anchor row is held (Virtuoso measures the new rows over a few frames). */
const HOLD_MS = 1500;
/** How many frames a reveal nudges its row toward the view's centre at most (~1.5 s at 60 fps). */
const HOLD_FRAMES = 90;

/**
 * A row's own block, what a reveal with a `top` puts at that top (designer m_60782, m_60809): its
 * head line (the day's, or its turn's time line) when it has one, else its entry, else the row (a
 * row's room above its words not counted).
 */
export function blockOf(row: HTMLElement): HTMLElement {
  return row.querySelector<HTMLElement>('.th-day, .th-turn, [data-testid^="thread.entry."]') ?? row;
}

/** The part of a row held still: `sel` inside it when given and there, else the row. */
function partOf(row: HTMLElement, sel: string | undefined): HTMLElement {
  return (sel === undefined ? null : row.querySelector<HTMLElement>(sel)) ?? row;
}

/**
 * What his press holds still while its row opens or closes: the row of the toggle he pressed (a
 * button, a fold), and the part of it kept at its y: the element marked `data-hold` around the
 * toggle (a part that changes its own height, mcp-apps-hold), else the row's `holdPart`. None for a
 * press on no toggle of a row. Reads the DOM, writes nothing.
 */
export function pressHold(target: Element, holdPart: string | undefined): { row: HTMLElement; part: string | undefined } | undefined {
  const toggle = target.closest('button, [role="button"], summary, [aria-expanded]');
  const row = toggle?.closest<HTMLElement>('[data-item-index]') ?? null;
  if (toggle === null || row === null) return undefined;
  const mark = toggle.closest<HTMLElement>('[data-hold]');
  const v = mark !== null && row.contains(mark) ? mark.dataset['hold'] : undefined;
  return { row, part: v === undefined || v === '' ? holdPart : `[data-hold=${JSON.stringify(v)}]` };
}

/** How many keys came after `last` (an older page in front doesn't count). Pure. */
export function newAtEnd(last: string | undefined, keys: readonly string[]): number {
  if (last === undefined) return 0;
  const i = keys.lastIndexOf(last);
  return i < 0 ? 0 : keys.length - 1 - i;
}

/**
 * A row opened while the list stuck to its end: how far to scroll down so what it opened comes into
 * view (`toEnd`, the px left below the view) without the clicked row leaving the view's top
 * (`rowTop`, its y in the view), and whether the list still sticks to its end (all of it fits). Pure.
 */
export function followOpened(rowTop: number, toEnd: number): { by: number; stuck: boolean } {
  const room = Math.max(0, rowTop);
  if (toEnd <= room) return { by: Math.max(0, toEnd), stuck: true };
  return { by: room, stuck: false };
}

export function ThreadList({ keys, row, more, before, onOlder, head, tail, cue, testId, ref, keepKey, holdPart, reactKey, onReading, onRead }: ThreadListProps) {
  const virtuoso = useRef<VirtuosoHandle>(null);
  // the anchor of firstItemIndex, moved during render (React's
  // "state from the previous render" pattern): no effect, no extra frame
  const [anchor, setAnchor] = useState<{ first: string | undefined; index: number }>({ first: keys[0], index: START });
  // the keys of the last commit: rows that left the front (the store's trim) move firstItemIndex up
  const lastKeys = useRef(keys);
  const next = nextFirstIndex(anchor, keys, lastKeys.current);
  if (next !== anchor) setAnchor(next);

  // the row he reads, measured at each scroll of his (not while held): when
  // rows come or go before it (an older page in front, a gap filled above, far rows trimmed), that
  // row is held at that y (hold below). `key`: its key, which a change of the keys doesn't move
  const scroller = useRef<HTMLElement | null>(null);
  const reading = useRef<{ index: number; top: number; key?: string } | undefined>(undefined);
  const keysNow = useRef(keys);
  keysNow.current = keys;
  const tellRead = useRef(onRead);
  tellRead.current = onRead;
  const told = useRef<string | undefined>(undefined);
  // click: held because he opened or closed a row (it keeps its y even at the end: never re-stuck)
  // end: the click came while the list stuck to its end (followOpened)
  // part: the row's part held (pressHold's data-hold), else holdPart
  const held = useRef<{ index: number; top: number; until: number; click?: boolean; end?: boolean; part?: string } | undefined>(undefined);
  const holdRef = useRef<() => void>(() => undefined);
  // his place written to keep.ts (keepPlace below, once `stick` exists)
  const placeRef = useRef<() => void>(() => undefined);
  // scrolled up, the row he reads and where his own scroll put it (steadyBy): put back at that y
  // whenever something else moved it. Virtuoso's own fixes came a frame late or never (a row above
  // measured at last while a new entry came at the end: 4 to 56 px jumps). None: read again at the
  // next scroll (after a hold, a jump of his, at the end)
  const still = useRef<{ index: number; y: number; st: number } | undefined>(undefined);
  // a jump of his (⌘F, a reveal, his place put back): no steady until it lands or he moves the list
  const jumpUntil = useRef(0);
  const steady = useCallback(() => {
    const s = still.current;
    const el = scroller.current;
    if (s === undefined || el === null || stick.current || held.current !== undefined || Date.now() < jumpUntil.current) return;
    const row = el.querySelector<HTMLElement>(`[data-item-index="${String(s.index)}"]`);
    if (row === null) return;
    const d = steadyBy(s, { y: partOf(row, holdPart).getBoundingClientRect().top - el.getBoundingClientRect().top, st: el.scrollTop });
    if (Math.abs(d) < 0.5) return;
    el.scrollTop += d;
    // read again where it is now: this scroll is ours, never one of his to follow
    still.current = { index: s.index, y: partOf(row, holdPart).getBoundingClientRect().top - el.getBoundingClientRect().top, st: el.scrollTop };
  }, [holdPart]);
  const measure = useCallback(() => {
    const el = scroller.current;
    if (el === null) return;
    if (held.current !== undefined) {
      still.current = undefined;
      holdRef.current();
      return;
    }
    // what moved the row he reads since the last scroll, other than his scroll, is undone first
    steady();
    const rows = [...el.querySelectorAll<HTMLElement>('[data-item-index]')].map((r) => {
      const b = partOf(r, holdPart).getBoundingClientRect();
      return { index: Number(r.dataset.itemIndex), top: b.top, bottom: b.bottom };
    });
    const edge = el.getBoundingClientRect().top;
    const a = anchorRow(rows, edge);
    still.current = a === undefined ? undefined : { index: a.index, y: a.top - edge, st: el.scrollTop };
    const key = a === undefined ? undefined : keysNow.current[a.index - firstIndex.current];
    reading.current = a === undefined ? undefined : { ...a, ...(key === undefined ? {} : { key }) };
    // the store keeps the rows near it (trim.ts); told when it changes, while he reads up there
    if (key !== undefined && key !== told.current && !stick.current) {
      told.current = key;
      tellRead.current?.(key);
    }
    placeRef.current();
  }, [holdPart, steady]);
  const scrollerRef = useCallback(
    (el: HTMLElement | Window | null) => {
      scroller.current?.removeEventListener('scroll', measure);
      scroller.current = el instanceof HTMLElement ? el : null;
      // scrolled up, steady alone keeps what he reads still: Virtuoso's own upward fix (its
      // scrollBy, a frame after the row moved, or not at all when an entry came in the same
      // update) would move it a second time. At the end, Virtuoso's is kept
      if (el instanceof HTMLElement) {
        const own = el.scrollBy.bind(el);
        el.scrollBy = (a?: ScrollToOptions | number, b?: number) => {
          if (!stick.current && still.current !== undefined) return;
          if (typeof a === 'number') own(a, b ?? 0);
          else own(a);
        };
      }
      scroller.current?.addEventListener('scroll', measure, { passive: true });
    },
    [measure],
  );

  // one older page at a time: asked again only once its cursor moved (olderAsk)
  const asked = useRef<number | null>(null);
  const startReached = useCallback(() => {
    if (!olderAsk(more, before, asked.current)) return;
    asked.current = before;
    onOlder();
  }, [more, before, onOlder]);

  // M2 / N11: scrolled up, a bar takes him back to the newest, with how
  // many rows came meanwhile (the TUI's 'back to the bottom · N new')
  const [atBottom, setAtBottom] = useState(true);
  const [seen, setSeen] = useState({ last: keys.at(-1), count: keys.length, unseen: 0 });
  const last = keys.at(-1);
  if (last !== seen.last) {
    const added = atBottom ? 0 : newAtEnd(seen.last, keys);
    setSeen({ last, count: keys.length, unseen: atBottom ? 0 : seen.unseen + added });
  }
  // it opens on the newest row's end. Not through Virtuoso's
  // initialTopMostItemIndex: that keeps the whole list visibility:hidden
  // until a scroll event says the item was reached, and a thread shorter
  // than the screen never scrolls, so its rows stayed invisible and could
  // not be selected (his bug 3, qa-type 7a27cbab). Once, when the first rows come.
  // After a reload (keep.ts) it opens where he had scrolled up to, if that row is loaded
  // (below, once `stick` exists).
  const opened = useRef(false);
  // he moved this list (wheel, key, press): only then is a place written. The list a mount put
  // back keeps the place it was given: the narrow thread beside an artifact wraps its rows
  // taller, so its own px would bring the wide one back lower than he left it.
  const moved = useRef(false);
  // a kept place being put back (the frames after the restore): Virtuoso's first atBottom, from
  // its mount at the end, may come after it and must not stick the list back to the end
  const restoring = useRef(false);
  const scrollKey = keepKey === undefined ? undefined : `scroll:${keepKey}`;

  // it stays at its end until HE scrolls up (qa-type b37dc4c1, his bug 5):
  // rows measured after the first scroll (long answers, markdown, the real
  // core's pages and entries) grow the list under the end, so Virtuoso's
  // own atBottom goes false with nobody scrolling; while it sticks, every
  // growth goes back to the end and new rows follow. A wheel up or a key
  // that scrolls up unsticks it; at the end, or after his send, it sticks.
  const stick = useRef(true);
  const [stuck, setStuck] = useState(true);
  // a reveal's settle runs (settling), and HOLD_MS after it began (revealUntil): Virtuoso's
  // isAtBottom stays the end's until its scroll events catch up with the reveal's scroll, so a line
  // that came meanwhile followed the output back to the end and the ⌘F hit was gone (history-perf's
  // 'the current match stays put', land-reds). Ends early when he moves the list. revealGen: the
  // newest reveal's, an older settle stops
  const revealUntil = useRef(0);
  const settling = useRef(false);
  const revealGen = useRef(0);
  // a reveal asked before its row was listed (find-scroll: ⌘F's match in a fold that opens for it)
  const waiting = useRef<{ key: string; until: number; shown?: () => void } | undefined>(undefined);
  const self = useRef<ThreadListHandle | null>(null);
  const setStick = useCallback((on: boolean) => {
    stick.current = on;
    setStuck(on);
  }, []);
  // to the end by scrollTop, not Virtuoso's scrollToIndex: a scrollToIndex re-runs itself whenever
  // the list's sizes change within 150 ms of its scroll (for up to 1.2 s), and his wheel up brings
  // in unmeasured rows that change those sizes, so a pending one took him back to the end ~300 ms
  // after he scrolled up (qa-flows' type-anywhere red, 3 of 5). While it sticks, each growth comes
  // back here (onGrow, the box observer), which is all the retry it needs.
  const toEndNow = useCallback(() => {
    const el = scroller.current;
    if (el === null) virtuoso.current?.scrollToIndex(LAST);
    else el.scrollTop = el.scrollHeight;
  }, []);
  // the hold: the row he reads (its holdPart) back at the y it had before
  // the prepend, by nudging scrollTop (Virtuoso's own prepend left ~7 px,
  // up to 50 while it measures the new rows). Ends after HOLD_MS, at his
  // wheel, key or click, or when it sticks to the end.
  const hold = useCallback(() => {
    const h = held.current;
    const el = scroller.current;
    if (h === undefined || el === null) return;
    if ((stick.current && h.click !== true) || Date.now() > h.until) {
      held.current = undefined;
      // steady takes over from here: the row he reads, where the hold left it
      measure();
      return;
    }
    const row = el.querySelector<HTMLElement>(`[data-item-index="${String(h.index)}"]`);
    if (row === null) return;
    const part = partOf(row, h.part ?? holdPart);
    if (h.click === true && h.end === true) {
      // opened at the thread's end (designer m_43996): what it opened comes into view, the list
      // still at its end, never further than the clicked row at the view's top; taller than the
      // view, he reads it from its row and the list no longer follows the end
      const f = followOpened(part.getBoundingClientRect().top - el.getBoundingClientRect().top, el.scrollHeight - el.clientHeight - el.scrollTop);
      if (f.by >= 0.5) el.scrollTop += f.by;
      if (!f.stuck && stick.current) {
        stick.current = false;
        setStuck(false);
      }
      return;
    }
    const d = part.getBoundingClientRect().top - h.top;
    if (Math.abs(d) < 0.5) return;
    el.scrollTop += d;
    // he opened a row at the end: he reads it there now, the list no longer follows the end
    if (h.click === true && stick.current) {
      stick.current = false;
      setStuck(false);
    }
  }, [holdPart, measure]);
  useLayoutEffect(() => {
    holdRef.current = hold;
  }, [hold]);
  // Virtuoso draws new or grown rows over its next commits and frames: each
  // scroll it makes comes back through measure (-> hold), each DOM change
  // or resize in the list through the observers (a mutation is a
  // microtask: it runs in a window in the background too, where frames and
  // timers are throttled), and a timer covers the rest; until the hold ends
  const stopHold = useRef<() => void>(() => undefined);
  const keepHeld = useCallback((): (() => void) => {
    stopHold.current();
    hold();
    const el = scroller.current;
    let timer = 0;
    const stop = () => {
      window.clearTimeout(timer);
      mo.disconnect();
      ro.disconnect();
    };
    const again = () => {
      hold();
      if (held.current === undefined) stop();
    };
    const mo = new MutationObserver(again);
    const ro = new ResizeObserver(again);
    if (el !== null) {
      mo.observe(el, { subtree: true, childList: true, attributes: true, attributeFilter: ['style'] });
      for (const c of el.children) ro.observe(c);
    }
    const tick = () => {
      again();
      if (held.current !== undefined) timer = window.setTimeout(tick, 16);
    };
    timer = window.setTimeout(tick, 0);
    stopHold.current = stop;
    return stop;
  }, [hold]);
  useEffect(
    () => () => {
      stopHold.current();
    },
    [],
  );
  /**
   * He opens or closes a row (a click on its toggle, ⌘E): that row, else the one at the top of
   * the view, keeps its y while the list measures what changed (designer m_17092: an opened row
   * never moves, the thread grows below it; never back to the end on his click).
   */
  const holdRow = useCallback(
    (rowEl: HTMLElement | null, part?: string) => {
      const el = scroller.current;
      if (el === null) return;
      const top = el.getBoundingClientRect().top;
      const r =
        rowEl ??
        [...el.querySelectorAll<HTMLElement>('[data-item-index]')].find((x) => x.getBoundingClientRect().bottom > top) ??
        null;
      if (r === null) return;
      const p = part ?? holdPart;
      held.current = { index: Number(r.dataset.itemIndex), top: partOf(r, p).getBoundingClientRect().top, until: Date.now() + HOLD_MS, click: true, end: stick.current, ...(p === undefined ? {} : { part: p }) };
      still.current = undefined;
      keepHeld();
    },
    [holdPart, keepHeld],
  );
  const firstIndex = useRef(next.index);
  useLayoutEffect(() => {
    const before = firstIndex.current;
    firstIndex.current = next.index;
    lastKeys.current = keys;
    // rows came or went before the row he read at the last scroll (an older page in front, a gap
    // filled above, far rows trimmed): hold it. Rows added at the end move nothing
    const r = reading.current;
    if (stick.current || r === undefined) return;
    const at = r.key === undefined ? -1 : keys.indexOf(r.key);
    const index = at < 0 ? r.index : next.index + at;
    if (index === r.index && next.index === before) return;
    reading.current = { ...r, index };
    held.current = { index, top: r.top, until: Date.now() + HOLD_MS };
    still.current = undefined;
    return keepHeld();
  }, [keys, next.index, keepHeld]);
  const onGrow = useCallback(() => {
    // a row he opened stays where he opened it, even at the end
    if (held.current?.click === true) hold();
    else if (stick.current) toEndNow();
    else hold();
  }, [toEndNow, hold]);

  // H10: reading = he scrolled up himself (the same as the 'back to the bottom' bar). What is under
  // the list (the inbox block) shrinks then: scrollTop stays, so the rows he reads don't move; back at
  // the end it grows again, and the list's box gets shorter: while it sticks, it goes back to the end.
  const readingUp = !atBottom && !stuck;
  const tellReading = useRef(onReading);
  useEffect(() => {
    tellReading.current = onReading;
  });
  useEffect(() => {
    tellReading.current?.(readingUp);
  }, [readingUp]);
  useEffect(
    () => () => {
      tellReading.current?.(false);
    },
    [],
  );
  // the scroller is Virtuoso's: it exists once the first rows are drawn
  const drawn = keys.length > 0;
  useEffect(() => {
    const el = scroller.current;
    if (!drawn || el === null) return;
    let h = el.clientHeight;
    const ro = new ResizeObserver(() => {
      const now = el.clientHeight;
      if (now < h && stick.current && held.current?.click !== true) toEndNow();
      h = now;
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [toEndNow, drawn]);
  // scrolled up, every change in the list puts the row he reads back where his scroll left it
  // (steady), before the frame is drawn: a DOM change (a commit of the list, a row's new words)
  // as a mutation, a row measured at its size as a resize of the list's content
  useEffect(() => {
    const el = scroller.current;
    if (!drawn || el === null) return;
    const ro = new ResizeObserver(steady);
    const watch = () => {
      for (const c of el.children) {
        ro.observe(c);
        for (const g of c.children) ro.observe(g);
      }
    };
    const mo = new MutationObserver(() => {
      watch();
      steady();
    });
    watch();
    mo.observe(el, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['style'] });
    return () => {
      mo.disconnect();
      ro.disconnect();
    };
  }, [steady, drawn]);

  useLayoutEffect(() => {
    if (opened.current || keys.length === 0) return;
    opened.current = true;
    const at = scrollKey === undefined ? undefined : placeOf(keep.view(scrollKey));
    const i = at === undefined ? -1 : keys.indexOf(at.key);
    if (at !== undefined && i >= 0 && i < keys.length - 1) {
      // unstuck now (no growth may take it back to the end), drawn so in the next tick
      stick.current = false;
      queueMicrotask(() => {
        setStuck(false);
      });
      // that row's top back at its px from the top edge
      virtuoso.current?.scrollToIndex({ index: i, align: 'start', offset: -at.off });
      // Virtuoso lands on its estimated sizes (a few px off): that row's top, measured in the
      // DOM, is nudged back to its px over the next frames, until he moves the list
      const index = firstIndex.current + i;
      const until = Date.now() + HOLD_MS;
      jumpUntil.current = until;
      restoring.current = true;
      const settle = () => {
        if (moved.current || Date.now() >= until) {
          restoring.current = false;
          return;
        }
        if (stick.current) {
          stick.current = false;
          setStuck(false);
        }
        const el = scroller.current;
        const r = el?.querySelector<HTMLElement>(`[data-item-index="${String(index)}"]`) ?? null;
        if (el !== null && r !== null) {
          const d = r.getBoundingClientRect().top - el.getBoundingClientRect().top - at.off;
          if (Math.abs(d) >= 0.5) el.scrollTop += d;
        }
        requestAnimationFrame(settle);
      };
      requestAnimationFrame(settle);
    } else toEndNow();
  }, [keys, scrollKey, toEndNow]);
  // his place while scrolled up: the row he reads, measured in the DOM (Virtuoso's range starts
  // in the 600 px drawn above the view, so its first row is one he doesn't see); at the end,
  // nothing kept. At each scroll (measure) and each new range, once he moved it.
  const keepPlace = useCallback(() => {
    // a mount that only put his place back writes nothing, not even "at the end": Virtuoso's
    // first atBottom (before the restore lands) sticks it for a frame
    if (scrollKey === undefined || !opened.current || !moved.current) return;
    if (stick.current) {
      keep.setView(scrollKey, undefined, true);
      return;
    }
    const el = scroller.current;
    if (el === null) return;
    const edge = el.getBoundingClientRect().top;
    const rows = Array.from(el.querySelectorAll<HTMLElement>('[data-item-index]'), (r) => {
      const b = r.getBoundingClientRect();
      return { index: Number(r.dataset.itemIndex), top: b.top, bottom: b.bottom };
    });
    const a = anchorRow(rows, edge);
    const key = a === undefined ? undefined : keys[a.index - next.index];
    if (a === undefined || key === undefined) return;
    keep.setView(scrollKey, { key, off: Math.round(a.top - edge) }, true);
  }, [scrollKey, keys, next.index]);
  useLayoutEffect(() => {
    placeRef.current = keepPlace;
  }, [keepPlace]);
  // the older page is asked while OLDER_AHEAD rows are still above what he reads, not when he
  // reaches the first one (Virtuoso's startReached): it comes while he scrolls (thread-older). Only
  // scrolled up: a thread opened at its end asks nothing more of its hub
  const onRange = useCallback(
    (r: { startIndex: number }) => {
      keepPlace();
      if (!stick.current && asksOlder(r.startIndex, next.index)) startReached();
    },
    [keepPlace, next.index, startReached],
  );

  const toEnd = useCallback(() => {
    setStick(true);
    if (keys.length > 0) toEndNow();
  }, [keys, setStick, toEndNow]);

  useImperativeHandle(
    ref,
    () => (self.current = {
      scrollTo(key) {
        const i = keys.indexOf(key);
        waiting.current = undefined;
        if (i < 0) return;
        // a jump of his (⌘F, a quote's source) wins over the row held still
        held.current = undefined;
        stopHold.current();
        jumpUntil.current = Date.now() + HOLD_MS;
        virtuoso.current?.scrollToIndex({ index: i, align: 'center' });
      },
      reveal(key, shown, top) {
        const i = keys.indexOf(key);
        const align = top === undefined ? ({ index: i, align: 'center' } as const) : ({ index: i, align: 'start', offset: -top } as const);
        // not a row yet (⌘F's match in a fold that opens for it, find-scroll): revealed when it comes
        waiting.current = i < 0 ? { key, until: Date.now() + HOLD_MS, ...(shown === undefined ? {} : { shown }) } : undefined;
        if (i < 0) return;
        // a jump of his (⌘F's match, his answer far back) wins over the row held still: the window
        // that came in front for it would hold the row he read before (the keys' hold)
        held.current = undefined;
        stopHold.current();
        // a row drawn and whole in the view stays where it is (his ask: only when it is outside)
        const el = scroller.current;
        const row = el?.querySelector<HTMLElement>(`[data-item-index="${String(next.index + i)}"]`) ?? null;
        if (el !== null && row !== null && rowWhole(row.getBoundingClientRect(), el.getBoundingClientRect())) {
          shown?.();
          return;
        }
        // he reads up there now: no growth takes the list back to its end
        setStick(false);
        virtuoso.current?.scrollToIndex(align);
        // Virtuoso lands on its estimated sizes (far off in a long thread of unmeasured rows): once
        // the row is drawn, its centre is nudged to the view's over the next frames, until it shows
        // whole, he moves the list, a newer reveal takes over, or HOLD_FRAMES. Frames, not ms: under
        // load (or off-screen) frames come late, and a 1.5 s deadline gave up before the row was
        // drawn, the list left at Virtuoso's estimate (history-perf's '⏎ to the oldest hit', land-reds);
        // and an older reveal's settle went on beside the newer one, centring a row by its stale
        // index. `shown` waits for the row drawn and whole: a fixed delay missed it under load
        // (inbox-open's flash), the row not drawn yet
        revealGen.current += 1;
        const gen = revealGen.current;
        let frames = 0;
        let lastTop: number | undefined;
        moved.current = false;
        revealUntil.current = Date.now() + HOLD_MS;
        jumpUntil.current = revealUntil.current;
        settling.current = true;
        let told = false;
        const tell = (): void => {
          if (told) return;
          told = true;
          shown?.();
        };
        const settle = () => {
          if (revealGen.current !== gen) return;
          const box = scroller.current;
          const r = box?.querySelector<HTMLElement>(`[data-item-index="${String(firstIndex.current + i)}"]`) ?? null;
          frames += 1;
          if (moved.current || frames > HOLD_FRAMES) {
            settling.current = false;
            if (r !== null) tell();
            return;
          }
          if (box === null || r === null) {
            // Virtuoso's scrollToIndex can stall (find-walk-red: a window replaced, then a new one
            // in front, its sizes by index still the old ones: the list stayed at the latest rows,
            // scrollTop unmoved for all HOLD_FRAMES): then a viewport at a time toward the row
            const at0 = box?.scrollTop;
            if (box !== null && r === null && at0 === lastTop) {
              const at = [...box.querySelectorAll<HTMLElement>('[data-item-index]')].map((x) => Number(x.dataset.itemIndex));
              const step = box.clientHeight * 0.8;
              if (at.length > 0) box.scrollTop += firstIndex.current + i < Math.min(...at) ? -step : step;
            } else virtuoso.current?.scrollToIndex(align);
            lastTop = at0;
          } else {
            const b = (top === undefined ? r : blockOf(r)).getBoundingClientRect();
            const v = box.getBoundingClientRect();
            const d = top === undefined ? b.top + b.height / 2 - (v.top + v.height / 2) : b.top - (v.top + top);
            // at its top place, or whole in the view and centred: a row taller than the view only
            // ever gets its top there
            if (Math.abs(d) < 2 && (top !== undefined || rowWhole(b, v))) {
              settling.current = false;
              tell();
              return;
            }
            box.scrollTop += top === undefined && b.height > v.height ? b.top - v.top : d;
          }
          requestAnimationFrame(settle);
        };
        requestAnimationFrame(settle);
      },
      scrollToEnd: toEnd,
      holdView() {
        holdRow(null);
      },
    }),
    [keys, next.index, toEnd, holdRow, setStick],
  );
  // a reveal that waits for its row: done once the row is listed (within HOLD_MS)
  useLayoutEffect(() => {
    const w = waiting.current;
    if (w === undefined || !keys.includes(w.key)) return;
    waiting.current = undefined;
    if (Date.now() <= w.until) self.current?.reveal(w.key, w.shown);
  }, [keys]);

  return (
    <div
      className="th-list"
      data-testid={testId}
      data-stuck={stuck}
      onPointerDown={(e) => {
        moved.current = true;
        held.current = undefined;
        jumpUntil.current = 0;
        // a press on a row's toggle (a fold, a thinking row, a tool line): that row keeps its y
        // while it opens or closes, or the part marked data-hold around it (pressHold)
        const p = pressHold(e.target as Element, holdPart);
        if (p !== undefined) holdRow(p.row, p.part);
      }}
      onWheel={(e) => {
        moved.current = true;
        held.current = undefined;
        jumpUntil.current = 0;
        if (e.deltaY < 0 && stick.current) setStick(false);
      }}
      onKeyDown={(e) => {
        moved.current = true;
        if (['PageUp', 'PageDown', 'Home', 'End', 'ArrowUp', 'ArrowDown'].includes(e.key)) {
          held.current = undefined;
          jumpUntil.current = 0;
        }
        if (['PageUp', 'Home', 'ArrowUp'].includes(e.key) && stick.current) setStick(false);
      }}
    >
      <Virtuoso
        ref={virtuoso}
        context={tail}
        scrollerRef={scrollerRef}
        style={FILL}
        data={keys}
        firstItemIndex={next.index}
        computeItemKey={(_, key) => (reactKey === undefined ? key : reactKey(key))}
        itemContent={(_, key) => row(key)}
        // Virtuoso never follows the output itself: the list follows its end only while it sticks,
        // through onGrow and the box observer (toEndNow). Virtuoso's following decided on its own
        // at-the-bottom, a frame behind his wheel up, and with a followOutput set at all it also
        // scrolled to the end on a size increase or a smaller viewport up to 100 ms after a new
        // entry, whatever the function said: his first notches up were taken back to the end
        // (scroll-steady: 20 to 186 px pulled down)
        followOutput={false}
        totalListHeightChanged={onGrow}
        rangeChanged={onRange}
        alignToBottom
        startReached={startReached}
        atBottomStateChange={(b) => {
          if (b && restoring.current) return;
          setAtBottom(b);
          if (b) {
            setSeen((x) => ({ ...x, unseen: 0 }));
            setStick(true);
          }
        }}
        components={head !== undefined && more ? { Header: () => head, Footer: ListEnd } : END}
        increaseViewportBy={{ top: 600, bottom: 300 }}
      />
      {cue}
      {!atBottom && !stuck && (
        <Press look="th-to-end" onClick={toEnd} data-testid="thread.to-end">
          ↓ back to the bottom{seen.unseen > 0 ? ` · ${String(seen.unseen)} new` : ''}
        </Press>
      )}
    </div>
  );
}
