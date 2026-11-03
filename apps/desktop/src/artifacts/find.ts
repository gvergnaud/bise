// ⌘F in an artifact (artifact-find, his m_52682: it opened the thread's find even with his focus in
// a page). One rule for who takes ⌘F, with search-placement (m_52687) and designer (m_52674):
//   1. the focus inside an artifact that has words (a page, a site, a doc, a text file): its find bar;
//   2. the focus on nothing (the body: he clicked some words) and the pointer on such an artifact: its find bar;
//   3. anything else (a field, the composer, a list): the screen's own search (the thread's history,
//      a page's search field: app/searchkeys.ts), as before.
// An image, a pdf and a video have no find (findable): their ⌘F stays the screen's.
// Both ways ⌘F comes: the window's key (capture, before any screen; useArtifactFindKey) and
// Edit › Find, which the menu sends even while the focus is inside the artifact's frame, where the
// window never sees the key (app/menu.ts useActionTaker). Each shown artifact registers here
// (useArtifactFind, ArtifactOpen). The bar's cursor is pure (stepAt, counterOf). Owner: artifact-find.
import { useEffect, useRef, type RefObject } from 'react';
import { useActionTaker } from '@/app/menu';
import { extOf, type KindSource } from '@/ds';

/** The kinds bise's viewer draws without words to find: an image, a pdf (pdf.js draws its pages
 *  as they come into view), a video. Their ⌘F stays the screen's. */
const NO_WORDS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'heic', 'svg', 'pdf', 'mp4', 'mov', 'webm', 'm4v']);

/** Whether an artifact's page has words ⌘F can find (a page, a site, a doc, a sheet, a deck, markdown, a text file). Pure. */
export function findable(a: KindSource & { readonly kind: string }): boolean {
  const k = a.kind.toLowerCase();
  if (k === 'shot' || k === 'shots' || k === 'image' || k === 'png') return false;
  return !NO_WORDS.has(extOf(a));
}

export interface FindPlace {
  /** the focus is inside it (its frame, its head, its bar) */
  readonly focus: boolean;
  /** the pointer is on it */
  readonly hover: boolean;
}

/**
 * The artifact ⌘F searches, if any (the rule above): the focused one; else, with the focus on
 * nothing (`elsewhere` false), the one under the pointer. Pure.
 */
export function findTarget<T extends FindPlace>(fs: readonly T[], elsewhere: boolean): T | undefined {
  const focused = fs.filter((f) => f.focus);
  if (focused.length > 0) return focused[focused.length - 1];
  if (elsewhere) return undefined;
  const hovered = fs.filter((f) => f.hover);
  return hovered[hovered.length - 1];
}

/** ⌘F alone. Pure. */
export const isFindKey = (e: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>): boolean =>
  e.key.toLowerCase() === 'f' && e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey;

/** The current match after ⏎ (on) or ⇧⏎ (back), around the ends; -1 with no match. Pure. */
export function stepAt(n: number, at: number, back: boolean): number {
  if (n <= 0) return -1;
  if (at < 0 || at >= n) return back ? n - 1 : 0;
  return (at + (back ? n - 1 : 1)) % n;
}

/** The count the bar shows: '3/12', '0/0' for a needle with no match, nothing before a needle. Pure. */
export function counterOf(needle: string, n: number, at: number): string {
  if (needle === '') return '';
  return `${String(n > 0 && at >= 0 ? at + 1 : 0)}/${String(n)}`;
}

/** The page's head bottom a frame says (a bise page's pinned frame, designer m_52698), in px: 0 when none or off. Pure. */
export function headPx(v: unknown): number {
  return typeof v === 'number' && Number.isFinite(v) && v > 0 && v < 400 ? Math.round(v) : 0;
}

// ---- the window's artifacts that have a find ----

interface Entry {
  readonly el: () => HTMLElement | null;
  readonly open: () => void;
}

const entries: Entry[] = [];

const hovered = (el: HTMLElement): boolean => {
  try {
    return el.matches(':hover');
  } catch {
    return false;
  }
};

/** The focus is on something (not the body), outside every artifact with a find. */
function focusElsewhere(entriesNow: readonly HTMLElement[]): boolean {
  const active = document.activeElement;
  if (active === null || active === document.body || active === document.documentElement) return false;
  return !entriesNow.some((el) => el.contains(active));
}

/** The find to open now (the rule above), if an artifact takes ⌘F. */
export function findOwner(): (() => void) | undefined {
  const active = document.activeElement;
  const live = entries.flatMap((x) => {
    const el = x.el();
    if (el === null) return [];
    const focus = active !== null && active !== document.body && el.contains(active);
    return [{ el, focus, hover: hovered(el), x }];
  });
  return findTarget(live, focusElsewhere(live.map((l) => l.el)))?.x.open;
}

/** While shown with words to find: this artifact (`ref`, its whole box) takes ⌘F by the rule above; `open` shows its bar. */
export function useArtifactFind(ref: RefObject<HTMLElement | null>, open: (() => void) | null): void {
  const latest = useRef(open);
  useEffect(() => {
    latest.current = open;
  });
  const on = open !== null;
  useEffect(() => {
    if (!on) return;
    const e: Entry = { el: () => ref.current, open: () => latest.current?.() };
    entries.push(e);
    return () => {
      const i = entries.indexOf(e);
      if (i >= 0) entries.splice(i, 1);
    };
  }, [ref, on]);
}

/**
 * In the shell: ⌘F goes to the artifact that takes it, before any screen hears it (the window's key
 * in the capture phase, Edit › Find before the screens' useMenuAction('find')). The key's menu
 * echo finds the same owner (the focus is in its bar then) and opens the same bar: a no-op.
 */
export function useArtifactFindKey(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isFindKey(e) || e.defaultPrevented) return;
      const open = findOwner();
      if (open === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      open();
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
    };
  }, []);
  useActionTaker('find', () => {
    const open = findOwner();
    if (open === undefined) return false;
    open();
    return true;
  });
}

// ---- esc from inside the page ----

const escs: (() => void)[] = [];

/** While the bar shows: an esc the page forwards (app/escape.ts frameEsc) closes it before the artifact. */
export function claimFindEsc(close: () => void): () => void {
  escs.push(close);
  return () => {
    const i = escs.indexOf(close);
    if (i >= 0) escs.splice(i, 1);
  };
}

/** An open find bar closes (the last one opened): true when one did. */
export function takeFindEsc(): boolean {
  const close = escs.at(-1);
  if (close === undefined) return false;
  close();
  return true;
}
