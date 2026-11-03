// Esc's one order (r3-escape: Gabriel m_44667, designer m_44693): what one esc does in the window,
// innermost first, main's order for main's cases, native-mac's surfaces where designer put them.
//
//   1. a menu, a dialog (⌘K, the keys sheet, a bubble's card, a status popover), a composer list
//      (/ @ $): theirs, they close (a bubble's card gives the focus back to its bubble)
//   2. the sidebar slid over (narrow): it closes                                  layer 'over'
//   3. the focus in the find bar: theirs, find closes; the focus in a page's search field
//      (ds ListSearch, data-esc-own): theirs, its words clear, then it gives the focus back
//   4. an inbox item open in place: theirs, it folds; an outer card's ask open in place: it folds,
//      the focus back on its row (designer m_47321)                               layer 'ask'
//   5. the focus in the unfolded floating composer: theirs (its lists, then it folds)
//   6. a zoomed image: back to fit                                                'zoom'
//   7. an artifact in front of the open panel: its tab closes, the panel as it was before it came,
//      and the route back when the artifact came by its route (artifacts/<p>/<id>); before the
//      composer reads the esc, so his words and the answering mode stay           'artifact'
//   8. the panel as a sheet over the thread (narrow): it folds                    layer 'sheet'
//   9. the floating composer unfolded, the focus outside it: it folds             layer 'float'
//  10. below: the composer (a dictation, his held words with bise, words to history), the screen's
//      own esc, then esc-up.ts (nobody took it: up the hierarchy)
//
// escapeAct is the order, pure. One window capture listener, installed when this module loads (so
// before any other window listener), reads the facts and runs the act of 2, 4's ask and 6-9 itself, then
// stops the esc: nothing below sees it, the route never also goes back. 1, 3-5 and 10 are the
// handlers' own: the listener leaves the esc to them. The surfaces of 2, 4's ask, 8 and 9 hand their act in
// with useEscLayer; 6 and 7 read their own state (zoomKeys.ts, the panel's). An esc from inside an
// artifact's page never reaches this listener: its frame forwards it and frameEsc runs 6 then 7
// for it (panel/Browser.tsx). Esc goes up, never
// back down (designer m_44802): the library's esc to a parent replaces the step, and at its root
// libraryExit gives the way out to where he was before it (artifacts/Screen.tsx). Owner: r3-escape.
import { useEffect } from 'react';
import { isZoomed, takeZoomKey } from '@/artifacts/zoomKeys';
import { takeFindEsc } from '@/artifacts/find';
import { useStore, type Store } from '@/store';
import { closeTabBack, panelState } from './panel/state';
import { MAIN, type Route } from './route';
import type { Router } from './router';

/** The layers a surface hands in (useEscLayer). */
export type EscLayer = 'over' | 'ask' | 'sheet' | 'float';

/** What is open when an esc comes. */
export interface EscFacts {
  /** a menu, a dialog, ⌘K, the keys sheet, a bubble's card, a composer list */
  readonly popup: boolean;
  /** the sidebar slid over the window */
  readonly over: boolean;
  /** the focus in a find bar: the thread's, an artifact's (artifacts/FindBar.tsx) */
  readonly find: boolean;
  /** the focus in a page's search field that takes esc itself (data-esc-own: ds ListSearch) */
  readonly field: boolean;
  /** an inbox item open in place */
  readonly inboxItem: boolean;
  /** an ask open in place in the outer card */
  readonly ask: boolean;
  /** the focus in the unfolded floating composer */
  readonly inFloat: boolean;
  readonly zoomed: boolean;
  /** an artifact's tab in front of the open panel */
  readonly artifact: boolean;
  /** the panel is a sheet over the thread */
  readonly sheet: boolean;
  /** the floating composer is unfolded */
  readonly float: boolean;
}

/** Who acts: 'below' leaves the esc to the handlers (1, 3-5, 10); the others the listener runs. */
export type EscAct = 'below' | 'over' | 'ask' | 'zoom' | 'artifact' | 'sheet' | 'float';

/** The order, innermost first. Pure. */
export function escapeAct(f: EscFacts): EscAct {
  if (f.popup) return 'below';
  if (f.over) return 'over';
  if (f.find || f.field || f.inboxItem || f.inFloat) return 'below';
  if (f.ask) return 'ask';
  if (f.zoomed) return 'zoom';
  if (f.artifact) return 'artifact';
  if (f.sheet) return 'sheet';
  if (f.float) return 'float';
  return 'below';
}

/** Open, these take esc first (esc-up.ts's POPUP: a menu, a dialog, a composer list). */
const POPUP = '[role="dialog"],[role="alertdialog"],[role="menu"],[aria-modal="true"],.app-menu,.b-composer-pick,.b-picker-pop';

const layers = new Map<EscLayer, (() => void)[]>();

/** The artifact in front of the open panel, if any. */
function frontArtifact(): { tab: string; project: string; id: string } | undefined {
  const s = panelState.get();
  if (!s.open) return undefined;
  const front = s.tabs.list.find((t) => t.id === s.tabs.active);
  if (front?.what.kind !== 'thing' || front.what.e.kind !== 'artifact') return undefined;
  return { tab: front.id, project: front.what.e.project, id: front.what.e.id };
}

/** The facts now, from the page and the layers handed in. */
export function escFacts(doc: Document = document): EscFacts {
  const el = doc.activeElement;
  const inside = (sel: string) => el instanceof Element && el.closest(sel) !== null;
  return {
    popup: doc.querySelector(POPUP) !== null,
    over: (layers.get('over')?.length ?? 0) > 0,
    find: inside('[data-testid="thread.find"], [data-testid="artifact.find"]'),
    field: inside('[data-esc-own]'),
    inboxItem: doc.querySelector('[data-testid="inbox.block"][data-open]') !== null,
    ask: (layers.get('ask')?.length ?? 0) > 0,
    inFloat: (layers.get('float')?.length ?? 0) > 0 && inside('[data-testid="composer.float"]'),
    zoomed: isZoomed(),
    artifact: frontArtifact() !== undefined,
    sheet: (layers.get('sheet')?.length ?? 0) > 0,
    float: (layers.get('float')?.length ?? 0) > 0,
  };
}

let router: Pick<Router, 'get' | 'back' | 'go' | 'canBack'> | null = null;
let store: Pick<Store, 'artifacts'> | null = null;

/**
 * 7: the artifact's tab closes (the panel as it was before it came); when the window is on its route,
 * back where he was, else (nothing behind) the maker's thread. Also the artifact page's own esc
 * (its frame forwards it, panel/Browser.tsx).
 */
export function closeArtifactTab(tab: string): void {
  const what = panelState.get().tabs.list.find((t) => t.id === tab)?.what;
  closeTabBack(tab);
  if (router === null || what?.kind !== 'thing' || what.e.kind !== 'artifact') return;
  const { project, id } = what.e;
  const r = router.get();
  if (r.screen !== 'artifacts' || r.project !== project || r.id !== id) return;
  if (router.canBack()) router.back();
  else router.go({ screen: 'thread', project, agent: store?.artifacts(project).find((a) => a.id === id)?.agent ?? MAIN });
}

const ESC_KEY = { key: 'Escape', code: 'Escape', metaKey: false, ctrlKey: false, altKey: false, shiftKey: false } as const;

/**
 * The artifact page's own esc (its frame forwards the one it didn't use, guard.ts ESC_FORWARD;
 * panel/Browser.tsx hears it): the same order as the window's 6 then 7. A zoomed page or image
 * goes back to fit first (his focus is in the page after a pinch, so the window's keydown never
 * sees that esc: web-pinch, ui-review m_45656); the next esc closes the tab `tab`.
 */
export function frameEsc(tab: string): void {
  if (takeZoomKey(ESC_KEY)) return;
  // ⌘F's bar over the page (artifact-find): it closes first, the artifact stays
  if (takeFindEsc()) return;
  closeArtifactTab(tab);
}

function run(act: Exclude<EscAct, 'below'>, e: KeyboardEvent): void {
  if (act === 'zoom') takeZoomKey(e);
  else if (act === 'artifact') {
    const a = frontArtifact();
    if (a !== undefined) closeArtifactTab(a.tab);
  } else layers.get(act)?.at(-1)?.();
}

function onKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || e.repeat || e.isComposing || e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
  const act = escapeAct(escFacts());
  if (act === 'below') return;
  e.preventDefault();
  e.stopImmediatePropagation();
  run(act, e);
}

let installed = false;
function install(): void {
  if (installed || typeof window === 'undefined') return;
  installed = true;
  window.addEventListener('keydown', onKey, true);
}
install();

/** The window's router and store for 7's way back (mounted once, from esc-up.ts's useHistoryWays). */
export function useEscape(r: Router): void {
  const s = useStore();
  useEffect(() => {
    router = r;
    store = s;
    return () => {
      if (router === r) router = null;
      if (store === s) store = null;
    };
  }, [r, s]);
}

/**
 * Esc at the library's root (designer m_44802: esc goes up, never back down): how many steps back
 * is where he was before he opened the library, past every library step (`back`: the router's
 * places behind, the nearest first); null: none, the window's esc-up decides. Pure.
 */
export function libraryExit(back: readonly Route[]): number | null {
  const i = back.findIndex((r) => !(r.screen === 'artifacts' && r.id === undefined));
  return i < 0 ? null : i + 1;
}

/** A surface's esc (2 the slid-over sidebar, 4 the outer card's ask, 8 the sheet, 9 the float), while `on`; the newest acts. */
export function useEscLayer(layer: EscLayer, on: boolean, act: () => void): void {
  useEffect(() => {
    if (!on) return;
    const list = layers.get(layer) ?? [];
    list.push(act);
    layers.set(layer, list);
    return () => {
      const i = list.indexOf(act);
      if (i >= 0) list.splice(i, 1);
    };
  }, [layer, on, act]);
}
