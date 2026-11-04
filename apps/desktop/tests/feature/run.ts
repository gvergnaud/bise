// A feature test's runner (fast-try p5, architect m_29407): plumbing only. given() feeds the core's
// and the hubs' events (and main's window-local answers) to the window, does() his gestures,
// sent() the commands that left, view.<screen>() what that screen shows, read through the app's
// own function for it (app.ts), never a derivation of ours. reload() is a new window on the same
// keep, given main's kept state events again (src/shared/latest.ts), as a real reload is.
//
// Today the window is the Store: its port is an outbox (a sink: it answers nothing and holds no
// state; the hub's answers come in through given()). When state-machine's step lands (fast-try
// p4), the outbox becomes step's outputs and only this file changes, never a test.
import {
  allItems,
  artifactGo,
  createBiseBack,
  createKeep,
  createOuterWidth,
  RAIL_GO,
  type BiseBack,
  leftWidthOf,
  sideLayout,
  SIDE,
  type CoinsAt,
  type KeepStore,
  type OuterWidth,
  centreOf,
  chipOpen,
  goneLinkOf,
  goneRoute,
  type ArtCentre,
  type GoneLink,
  galleryView,
  libraryPage,
  libraryView,
  Latest,
  navRoute,
  projectsOf,
  projectRowOf,
  projectWaiting,
  rowBars,
  sideShows,
  Store,
  threadKey,
  type AnswerFrom,
  type CardView,
  type Effect,
  type InboxItem,
  type LibraryView,
  type Pending,
  type Place,
  type Route,
  type SideShows,
  FOLDED,
  boxOf,
  closeBackKept,
  closeKept,
  fitOf,
  openKept,
  pageBand,
  THREAD_KEEPS,
  rightSideShows,
  type Before,
  type PanelKept,
} from './app';
import type { Cmd, Gesture, Here, Input } from './inputs';

export interface Feature {
  /** The core, a hub or main said these, in order. */
  given(...inputs: Input[]): Feature;
  /** He did these, in order. */
  does(...gestures: Gesture[]): Feature;
  /** Time passes (the window's clock; nothing comes in). */
  later(ms: number): Feature;
  /** He reloads the window: a new one on the same keep, main's kept state events replayed. */
  reload(): Feature;
  /** Every command that left the window, oldest first (since the start, across reloads). */
  sent(): readonly Cmd[];
  /** Every effect the window ran (a move to an artifact's view, a window.bise call), oldest first. */
  effects(): readonly Effect[];
  /** Each screen's "what I show". */
  readonly view: {
    /** The inbox: every open item he hasn't acted on, in its order, each with its refusal (the store's inbox view, machine/cards.ts inboxView). */
    inbox(): readonly InboxItem[];
    /** A message of his is still on its way (a pending row sending: store.sending, the app's update waits on it). */
    sending(): boolean;
    /** A thread's rows of his messages not echoed yet, oldest first: sending, or failed with why (the store's pending, ThreadRows' usePending). */
    pending(project: string, agent: string): readonly Pending[];
    /** The artifacts gallery's cards in this place (one project's with `only`), in order: name, line, a stack's count, new (artifacts/gallery.ts galleryView over hooks.ts allItems). */
    gallery(place: Place, only?: string): readonly CardView[];
    /** The artifacts library under a project scope ('' every project): the hero's facts, each folder's count ('14 of 74') and new ones, unfiled's count (artifacts/folders.ts libraryView over the store's folders and hooks.ts allItems). */
    library(project?: string, projectName?: string): LibraryView;
    /** Where the window is: the route its moves went to (an option's nav, a click on an artifact's name), the inbox at the start. */
    route(): Route;
    /** The left sidebar under its head for that route (app/sideshows.ts): his projects' names, the library's rows ('All artifacts', 'Recent', 'Unfiled', its folders; none while the library page is not mounted: its rows are that page's, artifacts/library.tsx), or the settings nav. */
    sidebar(): SidebarView;
    /** The sidebar's project rows (app/siderows.ts projectRowOf, as Sidebar.tsx draws them), `seen`: when he last looked at each project (none: never). */
    projectRows(seen?: Readonly<Record<string, number>>): readonly ProjectRowView[];
    /** The outer card at the window's right on a project's thread, the panel folded: where the coins are ('card' when it shows) and its width (side3d/layout.ts sideLayout over his kept width, outerwidth.ts). */
    outerCard(): OuterCardView;
    /** The right side's coins (or card) against what the window draws beside them, window px: the band they keep, the panel's column, and the content (the thread's column on a thread, the page's box on /debug) (side3d/layout.ts sideLayout over the panel's box, panel/model.ts boxOf; app/pagecol.ts pageBand). */
    side(): SideView;
    /** An artifact opened by id: the centre (its maker's thread, the project's main when the list doesn't have it, nothing before the list came) and the panel (its view, or the gone lines' link and where it goes, or nothing yet) (artifacts/gone.ts). */
    artifact(project: string, id: string): ArtifactView;
  };
}

/** A box across the window, px from its left edge. */
export interface Span {
  readonly l: number;
  readonly r: number;
}

/** The right side and what is beside it. */
export interface SideView {
  readonly coins: CoinsAt;
  /** the band the coins (or the card) take; null when they are not beside the content (minis, none) */
  readonly band: Span | null;
  /** the panel's column, null folded or a sheet */
  readonly panel: Span | null;
  /** the panel has a tab to show (an open panel with none is an empty column) */
  readonly panelShows: boolean;
  /** what the screen draws: the thread's column, or the page's box less its band */
  readonly content: Span;
}

/** What the window shows for an artifact's route. */
export interface ArtifactView {
  readonly centre: ArtCentre;
  readonly panel: 'wait' | 'view' | { readonly gone: GoneLink; readonly to: Route };
}

/** The outer card: shown ('card') or the coins instead, and its width when shown. */
export interface OuterCardView {
  readonly coins: CoinsAt;
  readonly width: number | null;
}

/** What the left sidebar lists. */
export interface SidebarView {
  readonly shows: SideShows;
  readonly rows: readonly string[];
}

/** A project row of the sidebar: its name, quiet or not, line 2's sentence and its week's bar count. */
export interface ProjectRowView {
  readonly name: string;
  readonly quiet: boolean;
  /** line 2's words ('' when the row draws no line 2) */
  readonly say: string;
  /** line 2's day bars (0 when the row draws none) */
  readonly bars: number;
}

/** The library's rows, in the column's order (artifacts/Column.tsx): its words are the column's, the test reads only their count and the folders' paths. */
const libraryRows = (v: LibraryView): string[] => ['all', 'recent', 'unfiled', ...v.folders.map((f) => f.path)];

/** Where an answer was made, as the window's views pass it to the store. */
const fromOf = (here: Here): AnswerFrom => (here === 'inbox' ? { at: null, inbox: true } : { at: here });

/** A new window, its core up, nothing given yet. `now`: the clock's start (ms). */
export function feature(now = 1_791_100_000_000): Feature {
  const outbox: Cmd[] = [];
  const ran: Effect[] = [];
  const kept = new Map<string, unknown>();
  const latest = new Latest();
  let clock = now;
  let route: Route = { screen: 'inbox' };
  // the window's own keep (keep.ts) on a storage a reload finds again; written at once
  const disk = new Map<string, string>();
  const storage: KeepStore = { getItem: (k) => disk.get(k) ?? null, setItem: (k, v) => { disk.set(k, v); } };
  // one keep per window, as keep.ts's one: the outer card's width and the bise tab's last place share it
  const keepOpen = (): { outer: OuterWidth; back: BiseBack } => {
    const k = createKeep(storage, () => clock, (f) => { f(); return () => undefined; });
    return { outer: createOuterWidth(k), back: createBiseBack(k) };
  };
  let { outer, back } = keepOpen();
  // the window shows `r`: App.tsx tells the bise tab each route
  const moveTo = (r: Route): void => {
    route = r;
    back.saw(r);
  };
  let win = 1440;
  // the tabbed panel (panel/state.ts over model.ts's pure steps): its kept state and what was in front before each tab came
  let panel: PanelKept = FOLDED;
  const befores = new Map<string, Before>();
  // the room the left takes: the rail and a docked sidebar at its default width (frame-state.ts)
  const left = (): number => leftWidthOf(win, { hidden: false, width: SIDE.def });
  const open = (): Store => {
    const s = new Store(
      { send: (cmd) => { outbox.push(cmd); } },
      { now: () => clock, asides: { read: (k) => kept.get(k), write: (k, v) => { kept.set(k, v); } } },
    );
    s.setCore('up');
    // the app's edge (app/edge.tsx) runs these; here they are recorded, as sent() records commands
    s.onEffect((e) => {
      ran.push(e);
      // a tab's open (a card's chip) leaves the route where it is (app/edge.tsx)
      if (e.out === 'nav' && e.tab !== true) moveTo(navRoute(e.to));
    });
    return s;
  };
  let store = open();

  const doOne = (g: Gesture): void => {
    switch (g.g) {
      case 'answer':
        store.do({ i: 'answer', project: g.project, card: g.card, reply: g.reply, ...(g.from === undefined ? {} : { from: fromOf(g.from) }) });
        return;
      case 'close':
        store.do({ i: 'close', project: g.project, card: g.card });
        return;
      case 'retry':
        store.do({ i: 'retry_card', project: g.project, card: g.card });
        return;
      case 'pick':
        store.do({ i: 'pick', project: g.project, card: g.card, n: g.n, ...(g.from === undefined ? {} : { from: fromOf(g.from) }) });
        return;
      case 'send':
        store.send({ project: g.project, agent: g.agent, text: g.text, ...(g.mode === undefined ? {} : { mode: g.mode }), ...(g.parts === undefined ? {} : { parts: g.parts }) });
        return;
      case 'open':
        // the app's switch: the projects shown (App.tsx) and the thread followed (ThreadScreen.tsx)
        store.shown([g.project]);
        store.subscribeThread(g.project, g.agent);
        return;
      case 'resend':
        store.retry(threadKey(g.project, g.agent), g.cid);
        return;
      case 'dismiss':
        store.dismiss(threadKey(g.project, g.agent), g.cid);
        return;
      case 'art': {
        // a click on an artifact's name (artifacts/leave.ts useGoArtifact): his browser, or the window's route
        const plan = artifactGo(store.artifacts(g.project).find((x) => x.id === g.id), g.project, g.id);
        if (plan.browser !== undefined) ran.push({ out: 'fx', fx: { fx: 'openExternal', args: [plan.browser] } });
        else moveTo(plan.route);
        return;
      }
      case 'chip': {
        // the card's chip and ⌘O (inbox/Block.tsx openPage): the app's own intent for it
        const intent = chipOpen(g.card);
        if (intent !== null) store.do(intent);
        return;
      }
      case 'go':
        moveTo(g.route);
        return;
      case 'rail':
        // a click on a rail button (app/SideRail.tsx): bise's goes back to his last project or chat
        moveTo(g.place === 'bise' ? back.go(route, { projects: store.projects(), agents: (p) => store.agents(p) }) : RAIL_GO[g.place]);
        return;
      case 'win':
        win = g.width;
        return;
      case 'tab': {
        // an artifact opened in the panel (panel/state.ts openWhat, ArtifactView.tsx ArtifactRoute's openTab)
        const o = openKept(panel, { kind: 'thing', e: { kind: 'artifact', project: g.project, id: g.id } });
        if (o.before !== null) befores.set(o.kept.tabs.active, o.before);
        panel = o.kept;
        return;
      }
      case 'untab': {
        // the tab in front closes: its × (panel/state.ts closeTabId) or esc (app/escape.ts closeArtifactTab, closeTabBack)
        const id = panel.tabs.active;
        panel = g.how === 'x' ? closeKept(panel, id) : closeBackKept(panel, id, g.forget === true ? undefined : befores.get(id));
        befores.delete(id);
        return;
      }
      case 'outer':
        // the card's border (side3d/Host.tsx): a drag clamped to the room beside the left, a double-click back
        if (g.want === null) outer.reset();
        else outer.drag(g.want, win - left());
        return;
    }
  };

  const f: Feature = {
    given(...inputs) {
      for (const i of inputs) {
        // the core's state comes from main's onState (hooks.ts setCore), never as an event
        if (i.ev === 'core_state') {
          store.setCore(i.state);
          continue;
        }
        latest.keep(i as unknown as Record<string, unknown>);
        store.apply(i);
      }
      return f;
    },
    does(...gestures) {
      for (const g of gestures) doOne(g);
      return f;
    },
    later(ms) {
      clock += ms;
      return f;
    },
    reload() {
      store = open();
      ({ outer, back } = keepOpen());
      for (const e of latest.snapshot()) store.apply(e);
      return f;
    },
    sent: () => [...outbox],
    effects: () => [...ran],
    view: {
      inbox: () => store.inboxItems(),
      sending: () => store.sending(),
      pending: (project, agent) => store.pending(threadKey(project, agent)),
      gallery: (place, only) => galleryView(allItems(store, only), place, clock),
      library: (project = '', projectName = '') => libraryView(store.folders(), allItems(store), project, projectName),
      route: () => route,
      outerCard: () => {
        const l = sideLayout(win, left(), false, undefined, true, undefined, false, outer.want());
        return { coins: l.coins, width: l.coins === 'card' ? (l.outerW ?? null) : null };
      },
      side: () => {
        const l0 = left();
        // the artifact in front takes its kind's share (an unknown kind reads as a page: state.ts useFrontArtKind)
        const front = panel.tabs.list.find((t) => t.id === panel.tabs.active)?.what;
        const art = front?.kind === 'thing' && front.e.kind === 'artifact' ? 'page' : undefined;
        const box = boxOf(panel, fitOf(panel, art, win, l0));
        // the same call as side3d/Host.tsx SideHost (useLayoutDriver)
        const lay = sideLayout(win, l0, box.open, box.open && box.width > 0 ? box.width : undefined, rightSideShows(route), { over: box.over, full: box.full, threadMin: art === undefined ? THREAD_KEEPS.rows : THREAD_KEEPS.artifact }, false, outer.want());
        const screenR = win - box.width;
        const content = route.screen === 'debug' ? { l: l0, r: screenR - pageBand(lay, screenR) } : { l: lay.colLeft, r: lay.colLeft + lay.colW };
        return {
          coins: lay.coins,
          band: lay.side ? { l: lay.bandL, r: win - lay.right } : null,
          panel: box.width > 0 ? { l: screenR, r: win } : null,
          panelShows: panel.tabs.list.length > 0,
          content,
        };
      },
      artifact: (project, id) => {
        const listed = store.artifacts(project);
        const a = listed.find((x) => x.id === id);
        const came = store.artifactsCame(project);
        const link = goneLinkOf(listed);
        return { centre: centreOf(came, a), panel: a !== undefined ? 'view' : came ? { gone: link, to: goneRoute(project, link) } : 'wait' };
      },
      sidebar: () => {
        const shows = sideShows(route);
        if (shows === 'projects') return { shows, rows: projectsOf(store.projects()).map((p) => p.name) };
        if (shows === 'settings') return { shows, rows: ['settings'] };
        const scope = route.screen === 'artifacts' ? route.project : '';
        return { shows, rows: libraryPage(route) ? libraryRows(libraryView(store.folders(), allItems(store), scope, '')) : [] };
      },
      projectRows: (seen = {}) =>
        projectsOf(store.projects()).flatMap((p, index) => {
          const waits = projectWaiting(store.inboxItems(), p);
          const r = projectRowOf({ p, agents: store.agents(p.project), waits, index, on: false, cmd: false, seen: seen[p.project], now: clock });
          return [{ name: p.name, quiet: r.facts.quiet, say: r.facts.sentence.text, bars: rowBars(r.facts.week, clock).length }];
        }),
    },
  };
  return f;
}
