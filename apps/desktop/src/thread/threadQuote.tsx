// A selection in the thread's list becomes a quote in the composer (his bug 3, parity M6/B7): the
// selection pop and the keys that turn it into a quote chip. Split out of ThreadScreen.tsx (the
// file cap). Owner: amb-win.
import { useCallback, useEffect, useState, type RefObject } from 'react';
import type { ComposerActions } from '@/composer';
import { COL_PAD, type ColumnPlace } from './column';
import { quoteFrom, quoteKey, quoteText } from './quote';
import { copyText } from '@/quote/QuotePop';

/**
 * Text selected in this thread's list: quoting while there is some (the
 * composer's key bar says typing asks about it) and the selection pop under it
 * (src/quote); a printable key (Space too) then puts it in the composer as a
 * quote chip at the caret followed by that key, ⌘' as the chip alone
 * (quote.ts), a click on the pop's ask words as the chip alone. Capture phase
 * on the window, before the window's own keys.
 */
export function useThreadQuote(section: RefObject<HTMLElement | null>, composer: RefObject<ComposerActions | null>, agent: string, column: ColumnPlace | null): ThreadQuote {
  const [quoting, setQuoting] = useState(false);
  const [pop, setPop] = useState<PopAt | null>(null);
  const inList = useCallback((): Selection | undefined => {
    const sel = document.getSelection();
    if (sel === null || sel.isCollapsed || sel.rangeCount === 0) return undefined;
    const box = section.current?.querySelector('[data-testid="thread.list"]');
    return box != null && box.contains(sel.getRangeAt(0).commonAncestorContainer) ? sel : undefined;
  }, [section]);
  /** The selection into the composer as a quote chip at its caret; true when it went in. */
  const take = useCallback((): boolean => {
    const sel = inList();
    const acts = composer.current;
    if (sel === undefined || acts === null) return false;
    const text = quoteText(String(sel));
    if (text === '') return false;
    const at = sel.anchorNode instanceof Element ? sel.anchorNode : (sel.anchorNode?.parentElement ?? null);
    const row = at?.closest('.th-row');
    const from = quoteFrom(row?.getAttribute('data-kind'), row?.getAttribute('data-from'), agent);
    // the thread's selection goes first: cleared after, it would take the field's caret with it
    sel.removeAllRanges();
    setQuoting(false);
    setPop(null);
    acts.quote(text, from);
    return true;
  }, [inList, composer, agent]);
  useEffect(() => {
    const place = () => {
      const sel = inList();
      setQuoting(sel !== undefined && quoteText(String(sel)) !== '');
      const screen = section.current;
      const list = screen?.querySelector('[data-testid="thread.list"]');
      if (sel === undefined || screen == null || list == null) {
        setPop(null);
        return;
      }
      const rects = Array.from(sel.getRangeAt(0).getClientRects());
      const box = list.getBoundingClientRect();
      setPop(popAt(rects, box, popRoom(box, screen.getBoundingClientRect().left, column)));
    };
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      const k = quoteKey(e);
      if (k === undefined || inList() === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      // a letter, a digit, Space: the quote, then that key (lead m_11968: Space starts it too)
      if (take() && k === 'type') composer.current?.insertText(e.key);
    };
    document.addEventListener('selectionchange', place);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('scroll', place, true);
    window.addEventListener('resize', place);
    return () => {
      document.removeEventListener('selectionchange', place);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('scroll', place, true);
      window.removeEventListener('resize', place);
    };
    // a new column (the window resized, the side moved): the pop goes where the column now is
  }, [section, composer, inList, take, column]);
  const copy = useCallback(() => {
    const sel = inList();
    if (sel !== undefined) copyText(quoteText(String(sel)));
  }, [inList]);
  return { quoting, pop, ask: take, copy };
}

export interface PopAt {
  readonly x: number;
  readonly y: number;
}

interface ThreadQuote {
  readonly quoting: boolean;
  /** where the selection pop sits (fixed, the window's coordinates), or null: none */
  readonly pop: PopAt | null;
  readonly ask: () => boolean;
  readonly copy: () => void;
}

/** Height of the pop with its gap to the text (12 px words, 3 px padding). */
const POP_H = 30;
/** The room kept for the pop's width (its words are ~250 px at 12 px; 'type to ask … about it' grows them). */
export const POP_W = 280;

/** A box in the window's coordinates (a DOMRect's fields the pop reads). */
export interface Box {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
  readonly width: number;
  readonly height: number;
}

/** Where the pop may sit across the window (px): its left and right ends. */
export interface Room {
  readonly left: number;
  readonly right: number;
}

/**
 * The pop's room across: the reading column (the composer's box, 24 out of the text each side)
 * where the window's sideLayout puts it, never the list's whole width: the list runs under the
 * right side (the bims' bubbles, the outer card), which is drawn over the screen (Lorenzo's shot,
 * m_52529: a word picked at the column's end put its pop under the card). Without a place (a
 * panel's tab, beside an artifact: no side over that screen) the list's box. Pure.
 */
export function popRoom(list: Box, screenLeft: number, column: ColumnPlace | null): Room {
  if (column === null) return { left: list.left, right: list.right };
  return { left: screenLeft + column.x - COL_PAD, right: screenLeft + column.x + column.w + COL_PAD };
}

/**
 * Under the selection's last line, at its left; over its first line when the list has no room
 * under it (the thread opens at its end); kept inside the room across (popRoom) and the list
 * down; null when the selection is out of view. Pure.
 */
export function popAt(clientRects: readonly Box[], list: Box, room: Room): PopAt | null {
  const rects = clientRects.filter((x) => x.width > 0 && x.height > 0);
  const first = rects[0];
  const last = rects.at(-1);
  if (first === undefined || last === undefined) return null;
  if (last.bottom < list.top || first.top > list.bottom) return null;
  const under = last.bottom + 6;
  const y = under + POP_H <= list.bottom ? under : Math.max(list.top + 4, first.top - POP_H);
  const line = under + POP_H <= list.bottom ? last : first;
  const left = Math.min(...rects.filter((x) => Math.abs(x.bottom - line.bottom) < 2).map((x) => x.left));
  return { x: Math.max(room.left + 8, Math.min(left, room.right - POP_W)), y };
}
