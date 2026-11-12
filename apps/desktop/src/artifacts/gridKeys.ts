// The gallery's keys (brief part 3): the list keys every list screen has (app/listkeys.ts: ↑↓ jk
// pgup pgdn home end ⏎, the selection kept across a reload) over the cards, stacks counted as one;
// in the grid ↑↓ (j k) move one row and ←→ one card (grid.ts gridStep). The row's own keys stay
// List.tsx rowKey's, called by the screen's onKey. Screen.tsx (art-folders) calls useGridKeys in
// place of useListKeys and hands its cards and cols to <ArtifactGrid>. Owner: art-grid.
import { useMemo, useSyncExternalStore, type KeyboardEvent } from 'react';
import { useListKeys, type ListKeys } from '@/app/listkeys';
import { cardsOf, colsFor, dirOf, gridStep, oneOf, rowsOf, type Card, type HeadOf } from './gallery';
import type { Item } from './model';

export type View = 'grid' | 'list';

const widthNow = (): number => (typeof window === 'undefined' ? 1100 : window.innerWidth);
const onResize = (cb: () => void) => {
  window.addEventListener('resize', cb);
  return () => {
    window.removeEventListener('resize', cb);
  };
};

/** Cards across for the window's width now (3 at 800, 4 at 1100, 5 at 1400), following a resize. */
export function useCols(): number {
  return colsFor(useSyncExternalStore(onResize, widthNow));
}

export interface GridKeysOpts {
  /** the items shown, newest first (filtered) */
  readonly items: readonly Item[];
  readonly view: View;
  /** the day groups (the grid's rows break at each day) */
  readonly days?: boolean | HeadOf;
  /** ⏎ on the selected card */
  readonly onOpen?: (key: string) => void;
  /** the screen's other keys, with the selected card's key */
  readonly onKey?: (e: KeyboardEvent, sel: string | null) => boolean;
  readonly keepAs?: string;
  /** make stacks (a stack's own sheet: no) */
  readonly stacks?: boolean;
}

export interface GridKeys<E extends HTMLElement> extends ListKeys<E> {
  /** the cards, stacks made (grid.ts cardsOf) */
  readonly cards: readonly Card[];
  /** cards across now */
  readonly cols: number;
  /** a card by its key */
  readonly cardOf: (key: string | null) => Card | undefined;
}

/** The cards and their keys (the header says which). */
export function useGridKeys<E extends HTMLElement = HTMLElement>({ items, view, days = true, onOpen, onKey, keepAs, stacks = true }: GridKeysOpts): GridKeys<E> {
  const cards = useMemo(() => (stacks ? cardsOf(items) : items.map(oneOf)), [items, stacks]);
  const byKey = useMemo(() => new Map(cards.map((c) => [c.key, c])), [cards]);
  const cols = useCols();
  const list = useListKeys<E>({
    order: cards.map((c) => c.key),
    ...(keepAs === undefined ? {} : { keepAs }),
    ...(onOpen === undefined ? {} : { onOpen }),
    ...(onKey === undefined ? {} : { onKey }),
  });
  const onKeyDown = (e: KeyboardEvent) => {
    const t = e.target as HTMLElement;
    const typing = t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t.isContentEditable;
    const dir = view === 'grid' && !typing && !e.defaultPrevented && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey ? dirOf(e.key) : null;
    if (dir === null) {
      list.onKeyDown(e);
      return;
    }
    e.preventDefault();
    // the grid scrolls its cursor into view itself (Grid.tsx), the card may not be drawn yet
    list.setSel(gridStep(rowsOf(cards, cols, 0, days), list.sel, dir));
  };
  return { ...list, onKeyDown, cards, cols, cardOf: (k) => (k === null ? undefined : byKey.get(k)) };
}
