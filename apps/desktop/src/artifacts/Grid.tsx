// The gallery (page artifacts-gallery v7, block b-pages; brief part 3): the cards by day, `cols`
// across (3 at 800, 4 at 1100, 5 at 1400; gaps 16 / 24), or the list behind ☰ (List.tsx ListRow),
// virtualized over the page's one scroller (@tanstack/react-virtual: only the rows in view and a
// few around are drawn, so 893 cards cost what 30 do). The cursor (the list keys' selection) is a
// 2 px ink ring and is scrolled into view when a key moves it; his multi-pick fills the rings.
// What a click, ··· and a drag do is the screen's (Screen.tsx, art-folders). Owner: art-grid.
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type HTMLAttributes } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ArtCard, type PickHow } from './Card';
import { ListHead, ListRow } from './List';
import { usePrefetch } from './thumbs';
import { GAP_Y, itemsOf, lineOf, rowHeight, rowIndexOf, rowsOf, type Card, type Place, type Row } from './gallery';
import type { View } from './gridKeys';
import { PrWords } from './PrWords';
import './cards.css';

export interface GridProps {
  /** the cards, stacks made (useGridKeys' cards) */
  readonly cards: readonly Card[];
  /** cards across (useGridKeys' cols) */
  readonly cols: number;
  readonly view: View;
  /** the clock of the day groups and the lines (a render reads no clock) */
  readonly now: number;
  /** where they show: what the line leaves out (grid.ts Place) */
  readonly place: Place;
  /** the day groups (none: one run, as 'recent' on all artifacts) */
  readonly days?: boolean;
  /** the keyboard's selection: its ring */
  readonly cursor: string | null;
  /** his multi-pick (⌘ / shift click, the circle) */
  readonly picked: ReadonlySet<string>;
  readonly onPick: (key: string, how: PickHow) => void;
  /** ⏎ or a plain click (Card.tsx clickOf): an artifact opens, a stack opens as its sheet */
  readonly onOpen: (card: Card) => void;
  /** ··· and a right click */
  readonly onMenu?: (key: string, at: { x: number; y: number }) => void;
  /** a card's drag props (draggable, onDragStart), spread on its root */
  readonly dragOf?: (key: string) => HTMLAttributes<HTMLElement>;
  /** the page's one scroller (the hero scrolls away with the cards) */
  readonly scroller: () => HTMLElement | null;
  readonly testId: string;
}

/** Rows drawn while the scroller has no size yet (a first paint, a test's DOM). */
const UNMEASURED = 12;
const DAY_H = 24;
const LIST_ROW_H = 44;

export function ArtifactGrid({ cards, cols, view, now, place, days = true, cursor, picked, onPick, onOpen, onMenu, dragOf, scroller, testId }: GridProps) {
  const across = view === 'grid' ? cols : 1;
  const rows = useMemo(() => rowsOf(cards, across, now, days), [cards, across, now, days]);
  const all = useMemo(() => cards.flatMap(itemsOf), [cards]);
  usePrefetch(all);
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(900);
  const [margin, setMargin] = useState(0);
  // the grid's width (the estimate of a row) and where it starts in the scroller (the hero, the filters above it)
  useLayoutEffect(() => {
    const el = box.current;
    const sc = scroller();
    if (el === null) return;
    const measure = () => {
      setWidth(el.clientWidth || 900);
      if (sc !== null) setMargin(el.getBoundingClientRect().top - sc.getBoundingClientRect().top + sc.scrollTop);
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    if (sc !== null) ro.observe(sc);
    return () => {
      ro.disconnect();
    };
  }, [scroller]);
  const size = (r: Row | undefined, i: number): number => {
    if (r === undefined) return 0;
    // a head: its 16 px line and 8 below; 24 above when first or in the list (else its row above's)
    if (r.kind === 'day') return i === 0 || view === 'list' ? DAY_H + GAP_Y : DAY_H;
    return view === 'grid' ? rowHeight(width, across) : LIST_ROW_H;
  };
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer's functions are read each render, never memoized
  const v = useVirtualizer({
    count: rows.length,
    getScrollElement: scroller,
    estimateSize: (i) => size(rows[i], i),
    getItemKey: (i) => rows[i]?.key ?? i,
    overscan: 4,
    scrollMargin: margin,
  });
  const drawn = v.getVirtualItems();
  // the cursor moved by a key: its row into view (never at mount: the hero stays where it is)
  const was = useRef(cursor);
  useEffect(() => {
    if (was.current === cursor) return;
    was.current = cursor;
    const i = rowIndexOf(rows, cursor);
    if (i >= 0) v.scrollToIndex(i, { align: 'auto' });
  }, [cursor, rows, v]);
  const many = picked.size > 1;
  const range = v.range;
  const inView = (i: number) => range === null || (i >= range.startIndex && i <= range.endIndex);
  const shown =
    drawn.length > 0
      ? drawn.map((d) => ({ index: d.index, start: d.start - margin, key: d.key }))
      : rows.slice(0, UNMEASURED).map((r, index) => ({ index, start: -1, key: r.key }));
  const absolute = drawn.length > 0;
  const draw = (r: Row, i: number) => {
    if (r.kind === 'day') {
      return (
        <div className={`art-dh${i === 0 ? ' first' : ''}`} data-testid={`${testId}.day.${r.day}`}>
          {r.title}
        </div>
      );
    }
    if (view === 'list') {
      const c = r.cards[0] as Card;
      return (
        <ListRow card={c} on={cursor === c.key} picked={picked.has(c.key)} now={now} visible={inView(i)}
          onPick={onPick} onOpen={onOpen} {...(onMenu === undefined ? {} : { onMenu })} {...(dragOf === undefined ? {} : { dragOf })}
          testId={`${testId}.row.${c.key}`} />
      );
    }
    return (
      <div className="art-grow" style={{ gridTemplateColumns: `repeat(${String(across)}, minmax(0, 1fr))` }}>
        {r.cards.map((c) => (
          <ArtCard key={c.key} card={c} line={c.kind === 'one' && c.it.a.pr != null ? <PrWords a={c.it.a} fit /> : lineOf(c, place, now)} on={cursor === c.key} picked={picked.has(c.key)} many={many}
            visible={inView(i)} onPick={onPick} onOpen={onOpen} {...(onMenu === undefined ? {} : { onMenu })}
            {...(dragOf === undefined ? {} : { dragOf })} testId={`${testId}.card.${c.key}`} />
        ))}
      </div>
    );
  };
  return (
    <>
    {/* the list's head stays out of the virtual rows: they are placed from the box's top */}
    {view === 'list' && <ListHead />}
    <div ref={box} className={`art-grid ${view}`} role="listbox" aria-multiselectable="true" data-testid={testId} data-cols={across} data-n={all.length}
      style={absolute ? { height: v.getTotalSize() } : undefined}>
      {shown.map((s) => {
        const r = rows[s.index];
        if (r === undefined) return null;
        return (
          <div key={s.key} data-index={s.index} ref={absolute ? v.measureElement : undefined} className="art-vrow"
            style={absolute ? { position: 'absolute', top: 0, left: 0, right: 0, transform: `translateY(${String(s.start)}px)` } : undefined}>
            {draw(r, s.index)}
          </div>
        );
      })}
    </div>
    </>
  );
}
