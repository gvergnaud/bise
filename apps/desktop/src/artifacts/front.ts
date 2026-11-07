// ⌘⏎ (his ask via side-ux m_45436): the primary action of the artifact in front, its head's first
// button ('↗ browser', else '⤓ save a copy'; Open.tsx primaryOf). Each place that shows one
// registers here while it shows (useArtifactFront): the open view and a panel tab's page (always),
// the folded float's row (only while the pointer is on it); the library's selected row is its
// list's own ⌘⏎ (List.tsx rowKey). ⌘⏎ picks one (frontOf): the one that holds the focus, else the
// one under the pointer, else the open view before a panel tab. Its key is the window's last word,
// in the bubble phase: a screen that owns ⌘⏎ (the inbox, the palette, the folder picker) takes it
// first; it fires only with the focus in an artifact or on nothing, never with the caret in a field
// (a search, the composer with words: it sends) or the inbox in front (primaryKeyFires, designer
// m_45455); the empty composer leaves it to the artifact (leaveToFront, his m_54852). File › Open in
// Browser ⌘⏎ (main/menu.ts) sends 'browser': the key's own echo is dropped (keys.ts menuEcho), a
// pick from the menu (or ⌘⏎ inside a page's frame, which the window never sees) runs the front.
// Owner: artifact-shortcut.
import { useEffect, useRef, useSyncExternalStore, type RefObject } from 'react';
import { menuEcho } from '@/app/keys';
import { useMenuAction } from '@/app/menu';

/** The key, as drawn (the tips, the keycap, the key list). */
export const PRIMARY_KEY = '⌘⏎';

/** Where an artifact shows: the open view (rank 0) before a panel tab (rank 1); a row counts only under the pointer. */
export type FrontPlace = 'view' | 'panel' | 'row';

export interface FrontLike {
  readonly place: FrontPlace;
  /** the focus is inside it */
  readonly focus: boolean;
  /** the pointer is on it */
  readonly hover: boolean;
}

const RANK: Readonly<Record<FrontPlace, number>> = { view: 0, panel: 1, row: 2 };

/** The artifact in front: the focused one, else the hovered one, else the view before the panel; a row only when hovered. Pure. */
export function frontOf<T extends FrontLike>(fs: readonly T[]): T | undefined {
  const focused = fs.filter((f) => f.focus);
  // nested (a row inside a view): the last registered is the inner one
  if (focused.length > 0) return focused[focused.length - 1];
  const hovered = fs.filter((f) => f.hover && f.place === 'row');
  if (hovered.length > 0) return hovered[hovered.length - 1];
  const hoverAny = fs.filter((f) => f.hover);
  if (hoverAny.length > 0) return hoverAny[hoverAny.length - 1];
  return [...fs].filter((f) => f.place !== 'row').sort((a, b) => RANK[a.place] - RANK[b.place])[0];
}

export interface PrimaryKeyLike {
  readonly key: string;
  readonly metaKey: boolean;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly repeat: boolean;
  readonly defaultPrevented: boolean;
}

/** ⌘⏎ alone (a repeat too: the echo is still his). Pure. */
export const isPrimaryKey = (e: Omit<PrimaryKeyLike, 'repeat' | 'defaultPrevented'>): boolean =>
  e.key === 'Enter' && e.metaKey && !e.altKey && !e.ctrlKey && !e.shiftKey;

/**
 * Whether ⌘⏎ is the artifact's: no screen took it, no repeat, the caret in no field (a search, the
 * composer with words), the focus in an artifact or on nothing (`elsewhere`: a list, a card, ⌘K
 * keep their ⌘⏎; designer m_45455), the inbox not in front (its screen, or an item open in its
 * block). `left`: the field with the caret had nothing to do with it and left it here (the empty
 * composer, leaveToFront): field, focus and the field's own preventDefault no longer count. Pure.
 */
export function primaryKeyFires(e: PrimaryKeyLike, c: { readonly field: boolean; readonly elsewhere: boolean; readonly inbox: boolean; readonly left?: boolean }): boolean {
  // a key left by its field comes prevented: ProseMirror prevents every ⏎ it sees (captureKeyDown)
  if (c.left === true) return isPrimaryKey(e) && !e.repeat && !c.inbox;
  return isPrimaryKey(e) && !e.repeat && !e.defaultPrevented && !c.field && !c.elsewhere && !c.inbox;
}

/** The caret is in a field: an input, a textarea, an editable (the composer). */
export function inField(el: Element | null): boolean {
  return el instanceof HTMLElement && el.closest('input, textarea, select, [contenteditable="true"], [contenteditable=""]') !== null;
}

// ---- the window's registered artifacts ----

interface Entry {
  readonly place: FrontPlace;
  readonly el: () => HTMLElement | null;
  /** its primary action now; null: it has none (no '↗ browser', nothing to save) */
  readonly run: () => (() => void) | null;
}

const entries: Entry[] = [];
const subs = new Set<() => void>();
let count = 0;
const changed = (): void => {
  count = entries.filter((x) => x.place !== 'row' && x.run() !== null).length;
  for (const f of [...subs]) f();
};

const hovered = (el: HTMLElement): boolean => {
  try {
    return el.matches(':hover');
  } catch {
    return false;
  }
};

/** The focus is on something that is no shown artifact (nothing focused: false). */
function focusElsewhere(): boolean {
  const active = document.activeElement;
  if (active === null || active === document.body || active === document.documentElement) return false;
  return !entries.some((x) => x.el()?.contains(active) === true);
}

/** The front's primary action now, if any. */
export function frontAction(): (() => void) | undefined {
  const active = document.activeElement;
  const live = entries.flatMap((x) => {
    const el = x.el();
    if (el === null) return [];
    const focus = active !== null && active !== document.body && el.contains(active);
    return [{ place: x.place, focus, hover: hovered(el), x }];
  });
  return frontOf(live)?.x.run() ?? undefined;
}

// the keys a field left to the artifact in front (leaveToFront), read by the window's ⌘⏎
const left = new WeakSet<Event>();

/**
 * A field's ⌘⏎ it has nothing to do with (the composer, empty: nothing to send; his m_54852, the
 * composer takes the focus when an artifact opens beside the thread): the artifact in front's, when
 * one shows with a primary action. true: left to it (the field takes nothing, the window's ⌘⏎
 * runs it); false: no artifact to give it to, the field keeps it.
 */
export function leaveToFront(e: KeyboardEvent): boolean {
  if (!isPrimaryKey(e) || frontAction() === undefined) return false;
  left.add(e);
  return true;
}

/**
 * While shown: this artifact may be in front. `run` gives its primary action at the time of the
 * key (null: none). A 'row' counts only under the pointer.
 */
export function useArtifactFront(ref: RefObject<HTMLElement | null>, place: FrontPlace, run: (() => void) | null): void {
  const latest = useRef(run);
  useEffect(() => {
    const was = latest.current;
    latest.current = run;
    if ((was === null) !== (run === null)) changed();
  });
  useEffect(() => {
    const e: Entry = { place, el: () => ref.current, run: () => latest.current };
    entries.push(e);
    changed();
    return () => {
      const i = entries.indexOf(e);
      if (i >= 0) entries.splice(i, 1);
      changed();
    };
  }, [ref, place]);
}

/** Some artifact with a primary action shows (a view or a panel tab): File › Open in Browser is lit. */
export function useHasFront(): boolean {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => {
        subs.delete(f);
      };
    },
    () => count > 0,
  );
}

/**
 * In the shell: ⌘⏎ and File › Open in Browser run the front's primary action. `inbox()`: the inbox
 * is in front now (its screen, or an item open in its block).
 */
export function usePrimaryKey(inbox: () => boolean): void {
  const keyAt = useRef<number | undefined>(undefined);
  const inboxNow = useRef(inbox);
  useEffect(() => {
    inboxNow.current = inbox;
  });
  useEffect(() => {
    // capture: every ⌘⏎ the window sees, whoever takes it, so its menu echo is dropped
    const seen = (e: KeyboardEvent) => {
      if (isPrimaryKey(e)) keyAt.current = Date.now();
    };
    // bubble: what no screen took
    const onKey = (e: KeyboardEvent) => {
      if (!primaryKeyFires(e, { field: inField(document.activeElement), elsewhere: focusElsewhere(), inbox: inboxNow.current(), left: left.has(e) })) return;
      const run = frontAction();
      if (run === undefined) return;
      e.preventDefault();
      run();
    };
    window.addEventListener('keydown', seen, true);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', seen, true);
      window.removeEventListener('keydown', onKey);
    };
  }, []);
  useMenuAction('browser', () => {
    if (menuEcho(keyAt.current, Date.now())) return;
    frontAction()?.();
  });
}
