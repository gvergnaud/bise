// His ticks beside a thread (msg-dots, his ask m_60465, designer m_60490): a very thin strip in the
// column's left room (--pad-column), one tick per message of his, his last 20 (msgdots.ts). A
// click brings that message's top 16 px under the list's top, its page read when the view doesn't
// hold it (useFindGo: store.jumpTo, never the whole thread), and tints it once; a hover (or focus)
// shows his words cut short and their time in a bubble right of the strip; ↑/↓ move between the
// ticks. The tick he is in (his last message above the list's top third) is long and ink. Fewer
// ticks than fit in the list's height: the newest that fit; under two: no strip. Owner: msg-dots.
import { memo, useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import { Press } from '@/ds';
import { useStore, useThreadMarks, useThreadOrder } from '@/store';
import type { ThreadListHandle } from './ThreadList';
import { useFindGo, type GoHow } from './useFindGo';
import { currentDot, dotsOf, dotWhen, stepDot, type Dot } from './ticks';
import './msgdots.css';

/** A tick's row (px): 8 apart (designer's 4 px grid). */
export const DOT_ROW = 8;
/** Room kept above and under the strip in the list (px). */
const DOT_MARGIN = 16;
/** His message's top under the list's top after a click (px). */
const DOT_TOP = 16;
/** The bubble shows after this (ms). */
const BUBBLE_IN_MS = 150;

/** His message a tick brought up: the neutral tint once (msgdots.css .th-dot-flash), again on a second click. */
export function tintEntry(pos: number): void {
  const el = document.querySelector<HTMLElement>(`[data-testid="thread.entry.${String(pos)}"]`);
  if (el === null) return;
  el.classList.remove('th-dot-flash');
  el.getBoundingClientRect(); // a reflow: the tint runs again
  el.classList.add('th-dot-flash');
}

/** How many ticks fit in a list `h` px high. Pure. */
export function dotsFit(h: number): number {
  return Math.max(0, Math.floor((h - 2 * DOT_MARGIN) / DOT_ROW));
}

/** The pos of the entry he is in: the last entry row whose top is above the list's top third. */
function readingPos(list: HTMLElement): number | null {
  const box = list.getBoundingClientRect();
  const line = box.top + box.height / 3;
  let pos: number | null = null;
  for (const el of list.querySelectorAll<HTMLElement>('[data-testid^="thread.entry."]')) {
    if (el.getBoundingClientRect().top > line) break;
    const p = Number(el.dataset.testid?.slice('thread.entry.'.length));
    if (Number.isFinite(p)) pos = p;
  }
  return pos;
}

export const MsgDots = memo(function MsgDots({ tkey, project, agent, listRef }: { tkey: string; project: string; agent: string; listRef: RefObject<ThreadListHandle | null> }) {
  const store = useStore();
  const marks = useThreadMarks(tkey);
  const view = useThreadOrder(tkey);
  const all = useMemo(() => dotsOf(marks, view.order.map((p) => store.entry(tkey, p))), [marks, view, store, tkey]);
  const root = useRef<HTMLDivElement>(null);
  // the list's height: as many ticks as fit
  const [fit, setFit] = useState(Infinity);
  useEffect(() => {
    const host = root.current?.parentElement;
    if (host === null || host === undefined) return;
    const ro = new ResizeObserver(() => {
      setFit(dotsFit(host.clientHeight));
    });
    ro.observe(host);
    return () => {
      ro.disconnect();
    };
  }, []);
  const dots = useMemo(() => all.slice(Math.max(0, all.length - fit)), [all, fit]);
  // the tick he is in, as he scrolls (the list's scroller, read once a frame)
  const [reading, setReading] = useState<number | null>(null);
  useEffect(() => {
    const list = root.current?.parentElement?.querySelector<HTMLElement>('[data-testid="thread.list"]');
    const scroller = list?.querySelector<HTMLElement>('[data-virtuoso-scroller]') ?? list;
    if (list === null || list === undefined || scroller === null || scroller === undefined) return;
    let raf = 0;
    const read = () => {
      raf = 0;
      setReading(readingPos(list));
    };
    const on = () => {
      if (raf === 0) raf = requestAnimationFrame(read);
    };
    read();
    scroller.addEventListener('scroll', on, { passive: true });
    return () => {
      scroller.removeEventListener('scroll', on);
      if (raf !== 0) cancelAnimationFrame(raf);
    };
  }, [view, dots.length]);
  const current = currentDot(dots, reading);
  // the click: his message's top 16 px under the list's top, then a tint once (useFindGo reads its page)
  const how = useMemo<GoHow>(() => ({ top: DOT_TOP, shown: tintEntry }), []);
  const go = useFindGo(store, project, agent, listRef, how);
  // the bubble: the tick hovered or focused, 150 ms after
  const [tip, setTip] = useState<number | null>(null);
  const timer = useRef(0);
  const show = useCallback((pos: number | null) => {
    window.clearTimeout(timer.current);
    if (pos === null) setTip(null);
    else
      timer.current = window.setTimeout(() => {
        setTip(pos);
      }, BUBBLE_IN_MS);
  }, []);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  const onKeyDown = useCallback(
    (e: KeyboardEvent<HTMLButtonElement>, pos: number) => {
      if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
      e.preventDefault();
      const to = stepDot(dots, pos, e.key === 'ArrowUp' ? -1 : 1);
      if (to !== null) root.current?.querySelector<HTMLButtonElement>(`[data-pos="${String(to)}"]`)?.focus();
    },
    [dots],
  );
  if (dots.length < 2) return <div ref={root} className="th-dots" hidden data-testid="thread.dots" />;
  const tipDot: Dot | undefined = dots.find((d) => d.pos === tip);
  const tipAt = tipDot === undefined ? -1 : dots.indexOf(tipDot);
  const tabbable = current ?? dots[dots.length - 1]?.pos;
  return (
    <div ref={root} className="th-dots" data-testid="thread.dots" role="navigation" aria-label="your messages" style={{ height: dots.length * DOT_ROW }}>
      {dots.map((d) => (
        <Press
          key={d.pos}
          look="th-dot"
          data-pos={d.pos}
          data-current={d.pos === current || undefined}
          data-testid={`thread.dot.${String(d.pos)}`}
          aria-label={d.text}
          tabIndex={d.pos === tabbable ? 0 : -1}
          onClick={() => {
            go(d.pos);
          }}
          onPointerEnter={() => {
            show(d.pos);
          }}
          onPointerLeave={() => {
            show(null);
          }}
          onFocus={() => {
            show(d.pos);
          }}
          onBlur={() => {
            show(null);
          }}
          onKeyDown={(e) => {
            onKeyDown(e, d.pos);
          }}
        />
      ))}
      {tipDot !== undefined && (
        <div className="th-dot-tip" role="tooltip" data-testid="thread.dot.tip" style={{ top: tipAt * DOT_ROW + DOT_ROW / 2 }}>
          <span className="th-dot-words">
            {tipDot.text}
            {tipDot.at > 0 && <span className="th-dot-when">{` · ${dotWhen(tipDot.at, Date.now())}`}</span>}
          </span>
        </div>
      )}
    </div>
  );
});
