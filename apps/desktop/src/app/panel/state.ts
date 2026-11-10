// src/app/panel · the tabbed panel's live state, one per project of the window (scope.ts, his
// m_45500): folded or open (folded by default, page native-mac §7), its tabs (model.ts) and its
// width; the project in front is App.tsx's (panelScope, from the route), his Recent and artifact
// shares the window's. Kept across a reload, an update swap or the next day (keep.ts, no age: a Mac
// app reopens as he left it). Outside React's tree, as the terminal's panel.ts: a tab opened or a
// width dragged re-renders the panel and the bar's tabs, never the shell or the composer. Every
// door in: openTab (tabs.ts), the bar's ▯ and ⌥⌘B (⌥⌘0) (panelToggle), ⌃` (openShell), the
// launcher's picks, ⇧⌘T (⌃⇧T) the last closed tab back (reopenClosed: a stack of closed tabs per
// project's panel, not kept, as Chrome's per window). A thing opened or closed is a step of the
// window's history (bindPanelHistory, steps.ts; history-keys): back gives the panel as it was.
// Owner: nm-panel; the per-project scope panel-scope.
import { useEffect, useLayoutEffect, useState, useSyncExternalStore } from 'react';
import { keep } from '@/app/keep';
import { useRoute } from '@/app/router';
import { useProjects } from '@/store';
import { useLeftWidth } from '@/app/frame-state';
import { iconKindOf } from '@/ds';
import { useAllArtifacts } from '@/artifacts';
import { armHandoff } from '@/composer/handoff';
import { activate, boxOf, clampShare, closeBackKept, closeKept, closeOthers, fitOf, launcherToReuse, moveIn, nextBrowser, nextLauncher, nextShell, openKept, pushClosed, reopenIn, stepIn, clampW, withOpen, FOLDED, dropGone, dropGoneArts, type Before, type Closed, type PanelBox, type PanelFit, type PanelKept, type TabWhat, type Tabs } from './model';
import { parseRec, pruneScopes, putView, scopeOf, switchTo, viewOf, type PanelRec } from './scope';
import { focusBackOf, noteFocus } from './focusback';
import { parseSnap, restoreKept, sameSnap, snapOf, type PanelStep } from './steps';
import type { Router } from '@/app/router';
import type { Entity } from '@/shared/entity';

const KEY = 'panel';

let rec: PanelRec = parseRec(keep.view(KEY));
let state: PanelKept = withOpen(viewOf(rec));
const listeners = new Set<() => void>();

function emit(): void {
  for (const l of listeners) l();
}

function set(raw: PanelKept): void {
  if (raw === state) return;
  const next = withOpen(raw);
  // the thread's composer and the floating one swap (full width on or off): his caret goes across
  if (state.open && next.full !== state.full) armHandoff();
  state = next;
  rec = putView(rec, next);
  keep.setView(KEY, rec);
  emit();
}

/**
 * The project in front of the window (App.tsx, scope.ts scopeOf): its own panel shows, as he left
 * it; one that never opened a panel has none. '' (none known yet) changes nothing.
 */
export function panelScope(project: string): void {
  if (project === '' || project === rec.at) return;
  // what he opened before any project was in front (a first launch) is the first project's
  if (rec.at === '' && rec.byProject[project] === undefined) rec = putView(switchTo(rec, project), state);
  else rec = switchTo(rec, project);
  state = withOpen(viewOf(rec));
  keep.setView(KEY, rec);
  emit();
}

/** The project whose panel is in front ('' before the first). */
export function panelAt(): string {
  return rec.at;
}

/**
 * The panel just became another project's (Host.tsx data-snap): true for the frame of the switch,
 * so its width snaps instead of easing from the other project's (ui-review m_45602).
 */
export function useScopeSnap(): boolean {
  usePanelState();
  const at = rec.at;
  const [shown, setShown] = useState(at);
  const snap = shown !== at;
  useEffect(() => {
    if (!snap) return;
    const id = requestAnimationFrame(() => {
      setShown(at);
    });
    return () => {
      cancelAnimationFrame(id);
    };
  }, [snap, at]);
  return snap;
}

/** The projects rows came: the panels of projects no longer in them go (never on an empty list). */
export function prunePanels(projects: readonly string[]): void {
  const next = pruneScopes(rec, projects);
  if (next === rec) return;
  rec = next;
  keep.setView(KEY, rec);
}

/**
 * App.tsx: the panel of the route's project in front (scope.ts scopeOf), switched before the
 * paint so no frame shows the old project's panel; a full projects list (home in it) prunes the
 * panels of projects gone.
 */
export function usePanelScope(): void {
  const route = useRoute();
  const rows = useProjects();
  const home = rows.find((p) => p.home)?.project;
  const at = scopeOf(route, home, rec.at);
  useLayoutEffect(() => {
    panelScope(at);
  }, [at]);
  useEffect(() => {
    // the core's first rows (no home yet) are not the registry: nothing goes before it
    if (home !== undefined) prunePanels(rows.map((p) => p.project));
  }, [rows, home]);
}

function setTabs(tabs: Tabs): void {
  if (tabs !== state.tabs) set({ ...state, tabs });
}

export const panelState = {
  get: (): PanelKept => state,
  subscribe: (l: () => void): (() => void) => {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  /** tests only: back to a fresh window (`raw`: the kept record it starts from) */
  reset(raw?: unknown): void {
    rec = parseRec(raw);
    state = withOpen(viewOf(rec));
    before.clear();
    drafts.clear();
    closed.clear();
    emit();
  },
  /** tests only: the window's record as kept */
  rec: (): PanelRec => rec,
};

/** The panel's state, live. */
export function usePanelState(): PanelKept {
  return useSyncExternalStore(panelState.subscribe, panelState.get);
}

/** The window's width, live. */
function useWindowWidth(): number {
  return useSyncExternalStore(
    (cb) => {
      window.addEventListener('resize', cb);
      return () => {
        window.removeEventListener('resize', cb);
      };
    },
    () => window.innerWidth,
  );
}

/**
 * The panel in this window now: an artifact in front takes its kind's share of the window, the
 * thread keeping 400 (artFit); any other tab the one width, the thread keeping 560 (fitW).
 */
export function usePanelFit(): PanelFit {
  const s = usePanelState();
  const kind = useFrontArtKind();
  const win = useWindowWidth();
  const left = useLeftWidth();
  return fitOf(s, kind, win, left);
}

/** The artifact in front of the panel (its tab's entity, open or folded); undefined for any other tab. */
export function useFrontArt(): { readonly project: string; readonly id: string } | undefined {
  const s = usePanelState();
  const front = s.tabs.list.find((t) => t.id === s.tabs.active)?.what;
  return front?.kind === 'thing' && front.e.kind === 'artifact' ? front.e : undefined;
}

/** a PR's kind for its share of the window (ds's iconKindOf of a PR artifact) */
const PR_KIND = 'pr';

/** The kind (ds's iconKindOf) of the artifact in front of the panel, a PR's tab its 'pr' (page pr-guided: its guided review reads at 744 at 1440); undefined for any other tab. */
export function useFrontArtKind(): string | undefined {
  const e = useFrontArt();
  const s = usePanelState();
  const front = s.tabs.list.find((t) => t.id === s.tabs.active)?.what;
  // no artifact in front: a project no row has, so it reads nothing ('' would read every project's)
  const a = useAllArtifacts(e?.project ?? '\u0000').find((x) => x.a.id === e?.id)?.a;
  if (e === undefined) return front?.kind === 'thing' && front.e.kind === 'pr' ? PR_KIND : undefined;
  return a === undefined ? 'page' : iconKindOf(a);
}

/** His drag on the border with an artifact of `kind` in front: that kind's share, 43-70%, kept. */
export function setArtShare(kind: string, share: number): void {
  const v = clampShare(share);
  if (state.shares[kind] !== v) set({ ...state, shares: { ...state.shares, [kind]: v } });
}

/** A double click on the border with an artifact in front: that kind's own share again. */
export function resetArtShare(kind: string): void {
  if (!(kind in state.shares)) return;
  set({ ...state, shares: Object.fromEntries(Object.entries(state.shares).filter(([k]) => k !== kind)) });
}

/**
 * Open or folded, and the column it takes (0 folded, 0 as a sheet over the thread): what the right
 * side (nm-side's sideLayout) and the shell read. `over`: it is a sheet now.
 */
export function usePanelBox(): PanelBox {
  return boxOf(usePanelState(), usePanelFit());
}

/** Full width now (model.ts fullFit): open, full, and not a sheet. The thread hides, the floating composer comes over the panel. */
export function usePanelFull(): boolean {
  return usePanelBox().full;
}

/** The bar's ▯ and ⌥⌘B (⌥⌘0): fold or unfold, on the tab he left in front; no tab: a New tab (designer m_46177). */
export function panelToggle(): void {
  if (!state.open && state.tabs.list.length === 0) {
    openLauncher();
    return;
  }
  set({ ...state, open: !state.open });
}

/** Full width on or off (his drag past the thread's 400, a drag back, ⇧⌥⌘B, the tab row's 'show the thread'); on unfolds the panel. */
export function setFull(on: boolean): void {
  if (on === (state.open && state.full)) return;
  set({ ...state, open: state.open || on, full: on });
}

/** ⇧⌥⌘B and View › Panel Full Width: full width or back beside the thread. */
export function toggleFull(): void {
  setFull(!(state.open && state.full));
}

/** His drag on the border is past the thread's 400 now (Host.tsx Border): the thread dims as a preview. Not kept. */
let fullPreview = false;
const previewSubs = new Set<() => void>();

export function setFullPreview(on: boolean): void {
  if (on === fullPreview) return;
  fullPreview = on;
  for (const l of previewSubs) l();
}

/** The drag's preview, live (App.tsx dims the thread with it). */
export function useFullPreview(): boolean {
  return useSyncExternalStore(
    (l) => {
      previewSubs.add(l);
      return () => {
        previewSubs.delete(l);
      };
    },
    () => fullPreview,
  );
}

/** The tab he just opened (openWhat), until its field takes the keys (takeOpened); a project switch or an unfold never sets it. */
let opened: string | null = null;
/** How many times he opened a tab: ⌘T on the New tab already in front changes nothing else, yet its field takes the keys again. */
let openedN = 0;

/** The count of his opens (openedN), live: a field's focus effect runs again on each. */
export function useOpenedN(): number {
  return useSyncExternalStore(panelState.subscribe, () => openedN);
}

/**
 * True once for the tab he just opened (⌘T, '+', ⌘L, a launcher's pick): its field takes the
 * keys then, never when it comes back in front by a project switch (his m_46543). Clears it.
 */
export function takeOpened(id: string): boolean {
  if (opened !== id) return false;
  opened = null;
  return true;
}

/** The window's history (app/router.ts), once the shell bound it: a thing opened or closed is a step of it (steps.ts). */
let history: Pick<Router, 'mark' | 'replacePanel'> | undefined;
/** A thing still there (an artifact in its list): a step that holds a gone one never brings its tab back. */
let exists: (e: Entity) => boolean = () => true;

/**
 * The shell binds the window's router (history-keys, architect m_58553): opens and closes mark its
 * steps, a tab brought to the front rewrites the current one, back and forward give the panel back
 * as that step had it (restorePanel). The unbind it returns.
 */
export function bindPanelHistory(router: Pick<Router, 'mark' | 'replacePanel' | 'onRestore'>, isThere: (e: Entity) => boolean): () => void {
  history = router;
  exists = isThere;
  const off = router.onRestore(restorePanel);
  return () => {
    off();
    if (history === router) history = undefined;
  };
}

/** Back or forward landed on a step that holds a panel: its project's panel as it was there; a bad one: nothing. Never a step. */
function restorePanel(raw: unknown): void {
  const snap = parseSnap(raw);
  if (snap === undefined) return;
  panelScope(snap.project);
  if (snap.project !== '' && snap.project !== rec.at) return;
  set(restoreKept(state, snap, exists));
}

/** A thing opened or closed (designer m_58552): a new step, the current one keeping the panel as it was. */
function stepped(was: PanelKept, step: PanelStep): void {
  const before = snapOf(rec.at, was);
  const after = snapOf(rec.at, state);
  if (!sameSnap(before, after)) history?.mark(before, after, step);
}

/** A tab brought to the front: the current step remembers it, no new step (designer m_58552). */
function fronted(was: PanelKept): void {
  if (was.tabs.active !== state.tabs.active) history?.replacePanel(snapOf(rec.at, state));
}

/**
 * Opens `what` in a tab (an open one comes to the front), unfolding the panel; `instead`: the
 * launcher it replaces. A thing that opens (a new tab, or the panel folded) is a step of his
 * history unless `quiet` (an artifact's route opens its own tab: the route is the step).
 */
export function openWhat(what: TabWhat, instead?: string, quiet = false): void {
  const was = state;
  const o = openKept(state, what, instead);
  const active = o.kept.tabs.active;
  opened = active;
  openedN += 1;
  if (o.before !== null) before.set(beforeKey(active), o.before);
  // where ⌘W gives his focus back when this tab closes (focusback.ts)
  if (typeof document !== 'undefined') noteFocus(beforeKey(active));
  // a New tab that opens fresh starts with an empty field, whatever an earlier one of its number held
  if (!state.tabs.list.some((t) => t.id === active)) drafts.delete(beforeKey(active));
  set(o.kept);
  if (what.kind !== 'thing' || quiet) return;
  if (!was.open || !was.tabs.list.some((t) => t.id === active)) stepped(was, { did: 'open', e: what.e });
  else fronted(was);
}

/** The thing the tab `id` shows, if it is one (a step's name). */
function thingOf(k: PanelKept, id: string): Entity | undefined {
  const what = k.tabs.list.find((t) => t.id === id)?.what;
  return what?.kind === 'thing' ? what.e : undefined;
}

/**
 * The words in each New tab's field, per project's panel as `before`: a New tab behind another
 * unmounts, and ⌘T brings it back with his words, selected (his m_46959, ui-review m_47041). A
 * reload forgets them.
 */
const drafts = new Map<string, string>();

/** The words he left in the New tab `id`'s field ('' none). */
export function launcherDraft(id: string): string {
  return drafts.get(beforeKey(id)) ?? '';
}

/** His words in the New tab `id`'s field, kept while it is behind another tab. */
export function setLauncherDraft(id: string, q: string): void {
  if (q === '') drafts.delete(beforeKey(id));
  else drafts.set(beforeKey(id), q);
}

/**
 * '+' and ⌘T: a New tab (the launcher); one already open in this panel comes to the front instead,
 * its field taking the keys (his m_46959, model.ts launcherToReuse).
 */
export function openLauncher(): void {
  openWhat(launcherToReuse(state.tabs) ?? nextLauncher(state.tabs));
}

/**
 * ⌘P and the launcher's Artifacts: the picker of the latest artifacts, in `instead` (the launcher
 * it came from), else in the New tab in front, else in a new one.
 */
export function openArtifacts(instead?: string): void {
  const at = instead ?? state.tabs.active;
  const front = state.open ? state.tabs.list.find((t) => t.id === at) : undefined;
  if (front?.what.kind === 'launcher') openWhat({ kind: 'launcher', n: front.what.n, artifacts: true }, front.id);
  else openWhat(nextLauncher(state.tabs, true), instead);
}

/** The picker's esc on an empty field: the New tab `id` back on its Open list. */
export function launcherHome(id: string): void {
  const t = state.tabs.list.find((x) => x.id === id);
  if (t?.what.kind === 'launcher') openWhat({ kind: 'launcher', n: t.what.n }, id);
}

/** ⌘L and the launcher's Browser: a browser tab, its address field focused (Browser.tsx). */
export function openBrowser(instead?: string): void {
  openWhat(nextBrowser(state.tabs), instead);
}

/** A new shell of `project` in a tab (the launcher's Terminal, a terminal's '+'). */
export function openShell(project: string, instead?: string): void {
  openWhat({ kind: 'terminal', project, n: nextShell(state.tabs, project) }, instead);
}

/**
 * ⌃` with the terminal on the right (his pref): the project's first shell in front, or the panel
 * folded when it already is.
 */
export function toggleShell(project: string): void {
  const id = `term:${project}#0`;
  if (state.open && state.tabs.active === id) set({ ...state, open: false });
  else openWhat({ kind: 'terminal', project, n: 0 });
}

/** The tabs he closed, the last on top (model.ts Closed), per project's panel as its tabs; a reload forgets them, as `before`. */
const closed = new Map<string, readonly Closed[]>();

/** `ids` closed from the tabs now: on this panel's closed stack, for ⇧⌘T. */
function remember(ids: readonly string[]): void {
  const stack = closed.get(rec.at) ?? [];
  const next = pushClosed(stack, state.tabs, ids);
  if (next !== stack) closed.set(rec.at, next);
}

/**
 * A tab closes (its ×, its menu, ⌘W, a launcher or a browser done). `remember` false: it went
 * elsewhere (a shell moved to the bottom), ⇧⌘T never brings it back.
 */
export function closeTabId(id: string, opts: { readonly remember?: boolean } = {}): void {
  if (opts.remember !== false) remember([id]);
  const was = state;
  // the last tab closed: the panel folds (the outer card comes back), nothing left to show
  set(closeKept(state, id));
  const e = thingOf(was, id);
  if (e !== undefined) stepped(was, { did: 'close', e });
}

/**
 * ⇧⌘T (⌃⇧T), as Chrome: the last tab he closed in this panel back in its place, in front, the
 * panel unfolded; again, the one before. Nothing closed: nothing happens.
 */
export function reopenClosed(): void {
  const r = reopenIn(state.tabs, closed.get(rec.at) ?? []);
  if (r === undefined) return;
  closed.set(rec.at, r.stack);
  if (!state.open || r.tabs.active !== state.tabs.active) before.set(beforeKey(r.tabs.active), { open: state.open, active: state.tabs.active });
  set({ ...state, open: true, tabs: r.tabs });
}

/** What was in front when each tab came by openWhat (model.ts Before), per project's panel; a reload forgets it. */
const before = new Map<string, Before>();
const beforeKey = (id: string): string => `${rec.at}\n${id}`;

/** Esc on the tab `id`: it closes, the panel as it was before it came (model.ts closeBack, app/escape.ts). */
export function closeTabBack(id: string): void {
  remember([id]);
  const was = state;
  const next = closeBackKept(state, id, before.get(beforeKey(id)));
  before.delete(beforeKey(id));
  set(next);
  const e = thingOf(was, id);
  if (e !== undefined) stepped(was, { did: 'close', e });
}

export function closeOthersId(id: string): void {
  remember(state.tabs.list.filter((x) => x.id !== id).map((x) => x.id));
  setTabs(closeOthers(state.tabs, id));
}

export function activateTab(id: string): void {
  const was = state;
  setTabs(activate(state.tabs, id));
  fronted(was);
}

export function moveTabTo(id: string, to: number): void {
  setTabs(moveIn(state.tabs, id, to));
}

/** ⌘⇧] and ⌘⇧[ (only while the panel is open). */
export function stepTab(dir: 1 | -1): void {
  if (!state.open) return;
  const was = state;
  setTabs(stepIn(state.tabs, dir));
  fronted(was);
}

/**
 * ⌘W in the main window (the window's key, or File › Close Tab when the focus is in an artifact's
 * frame; cmd-w-artifact, designer m_53158): the tab in front closes (the last one folds the panel),
 * his focus back where it was before it opened (focusback.ts); no tab: the panel folds; folded:
 * nothing, never the window.
 */
export function closeActive(): void {
  if (!state.open) return;
  const id = state.tabs.active;
  if (id === '') {
    set({ ...state, open: false });
    return;
  }
  const back = typeof document === 'undefined' ? undefined : focusBackOf(beforeKey(id));
  closeTabId(id);
  back?.();
}

/** The panel's width now: its one width, whatever tab is in front (designer m_43050 #7a). */
export function widthNow(s: PanelKept): number {
  return s.width;
}

/** His drag on the panel's border: the window's panel width, kept; no artifact widens it after. */
export function setWidth(w: number): void {
  const v = clampW(w);
  if (state.width !== v || !state.dragged) set({ ...state, width: v, dragged: true });
}

/** A double click on the border: back to the width it starts with, an artifact may widen it again. */
export function resetWidth(): void {
  set({ ...state, width: FOLDED.width, dragged: false });
}


/** The rows came: the tabs of agents no longer in them go (model.ts dropGone). */
export function dropGoneTabs(known: (project: string) => ReadonlySet<string> | undefined): void {
  setTabs(dropGone(state.tabs, known));
}

/** A project's artifacts came: the tabs of its artifacts no longer in them go, quietly (model.ts dropGoneArts). */
export function dropGoneArtTabs(known: (project: string) => ReadonlySet<string> | undefined, here?: { readonly project: string; readonly id: string }): void {
  setTabs(dropGoneArts(state.tabs, known, here));
}
