// src/app/side3d · SideHost: the right side, folded (page native-mac §7, variant A): the bims'
// world floating at the top right (World), the 3D bubbles under it (Bubbles) and their 360 floats
// (Flyout*). Mounted at the shell's root by App.tsx, position fixed under the window's bar,
// outside .app-main and .app-screen, so no parent's overflow clips the world's or an object's
// shadow (plan §2; ds/noclip.ts checks it). It computes the window's sideLayout (place.ts: at a
// resize's end, a sidebar or panel fold): the coins at the window's edge, between the thread and
// the open panel (the side's right on the panel's left edge), or as minis portalled into the
// header's or the sheet's tab row's CoinSlot (Minis.tsx; their float under the header, folded-side
// v9); ⌥ held alone 300 ms or a hover 150 ms shows the coins' cards (Cards.tsx). It hides on a page
// that is no project's (layout sideShows: the inbox, scheduled, plugins, artifacts...; the world's
// office unmounted: no loop while hidden). The project is the route's (bise's home on a page with
// none): the world and the acts (+ new agent, + add) are its; the floats list the Overview's
// scope (side.ts panelProject, lists.ts: that project's own, bise's own every project's, his
// m_45581). One float open at a time, one per bubble (designer m_44975): a bubble's click, ⌥1-⌥4
// (folded) or ⌥⌘1-⌥⌘4 (keys.ts) toggle it (by its key, its first row takes the focus); from one
// bubble to another the same float moves to its new place and size in 240 ms, its content
// crossfading (moveFrames, designer m_44990; reduced motion: a 120 ms fade in place); ⌥ held shows each bubble's key; ↑ ↓ walk the rows,
// ⏎ opens one; esc closes it (the focus back on its bubble), a click outside too, and a click in it
// that navigates (a thread, a page, a tab opened: native-lead m_48053). Places, in px
// under the bar: the world 12 from the top and the right edge; the bubbles 12 from the right, 152
// from the top, 80 apart (44 + 36), never moving (the big world floats over them, designer
// m_42536); with the world folded into a bubble, it is the stack's first, at 40; a card 68 from
// the right, its top always its bubble's, at most the room under it less 16 and 560 (layout
// cardMaxH, designer m_44499): nothing measured places it, so it never moves nor runs past the
// window. Under every palette, sheet and menu
// (z 15: over the composer's 10, under the palette's 20). Owner: nm-side.
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type MouseEvent } from 'react';
import { openPr as openPrTab } from '@/review/openPr';
import { openChange } from '@/review/openChange';
import { createPortal } from 'react-dom';
import { useNowFor } from '../clock';
import { useRoute, useRouter } from '../router';
import { MAIN, projectOf } from '../route';
import { useMenuAction } from '../menu';
import { useModHeld, useModHeldFor } from '../held';
import { CARD_GRACE_MS, CARD_HOLD_MS, CARD_HOVER_MS, CARD_OUT_MS, CARD_WARM_MS, CoinCardView } from './Cards';
import { coinCard, type CardFacts } from './coincard';
import { inRank, agentCounts, bubbleBadge, bubbleCount, bubbleName, bubbleSays, bubbleTip, answerFrom, type BubbleCount, type Counts } from './model';
import { Minis } from './Minis';
import { BUBBLES, type BubbleId } from './objects';
import { Bubble, WORLD_NAME } from './Bubbles';
import { noGrow, WorldThumb } from './World';
import { Empty, Float, FloatLayer, HeadButton, type FloatContent } from './Flyout';
import { ArchivedFold, ArchivedRows, FloatAgents } from './FloatAgents';
import { OuterAgents, OuterCard, OuterList, OuterSection } from './OuterCard';
import { OuterTile, TileHandle } from './OuterWorld';
import { clampTile, OUTER_GAP, OUTER_TOP, outerWorld, TILE_GAP, useOuterWorld } from './tilestate';
import { outerWidth } from './outerwidth';
import { Splitter } from '../Splitter';
import { useAgentRows } from '../agentrows';
import type { BarRow } from '../barrows';
import { ArtifactRow, ProcRow, TaskRow, WatchRows } from './FlyoutLists';
import { pinFirst, pinnedIn, procPinId, usePins } from '../pins';
import { usePinOrder } from '../pinhold';
import { onResizeEnd, useCoinSlot, useLayoutDriver, useSideLayout } from './place';
import { CARD_MAX, cardMaxH, OUTER_W, sideShows, THREAD_KEEPS, type CoinsAt, type PanelIn } from './layout';

/** half the border's 8 px hit area: it sits centred on the card's left edge */
const SPLIT_HALF = 4;
import { newAgent, openTab, panelFront, panelProject, useFrontArtKind, useLeftWidth, usePanelBox } from './ports';
import { acrossSays, busyProjects, useListed } from './lists';
import { barScope, inScope, scopeWords } from '../barscope';
import { alarms } from '@/thread/wake';
import type { Agent } from '@/proto';
import { useAgents, useCards, useProjects, useStore } from '@/store';
import { useAllArtifacts } from '@/artifacts';
import { AddButton, useAddDropOn, useAddPlace } from '@/artifacts/Drops';
import { useAllScheduled } from '@/scheduled';
import { followOf } from '@/bims/focus';
import { mapPrefKey } from '@/bims/maps/ids';
import { isRunning, useAllProcesses } from '@/running';
import { side } from '@/proto/words_side.gen';
import { useWelcoming } from '@/welcome/shown';
import './side3d.css';

/** a float's right edge from the side's right edge: 12 + the 44 coin + 12 (side3d.css .s3-card) */
const CARD_RIGHT = 68;
/** a mini's float: its top 8 under the header, whichever mini */
const MINI_TOP = 8;
const MINI_TOPS = { world: null, bubbles: { agents: MINI_TOP, artifacts: MINI_TOP, scheduled: MINI_TOP, processes: MINI_TOP } } as const;

/** the first bubble's top under the world (the big one floats over them: they never move), as the 5th bubble's stack */
const TOP_SMALL = 152;
const TOP_STACK = 40;
/** one bubble and the room to the next: 44 + 36 */
const STEP = 80;
/** how many rows a card lists before 'all … ›' */
const ROWS = 6;
/** the float's move from one bubble to another, ms, and its curve (designer m_44990) */
export const MOVE_MS = 240;
export const MOVE_EASE = 'cubic-bezier(.3,.7,.4,1)';
/** reduced motion: no move, the whole float fades in this long */
export const FADE_MS = 120;
/** the rows ↑ ↓ walk: the float's content now, never the leaving one */
const ROWS_NOW = '.s3-card-layer[data-now] [data-row]';

/** The float's move as Web Animations keyframes and timing: its top and height from where it is to its new bubble's; reduced motion, a fade in only. Pure. */
export function moveFrames(from: { readonly top: number; readonly height: number }, to: { readonly top: number; readonly height: number }, reduced: boolean): [Keyframe[], KeyframeAnimationOptions] {
  if (reduced) return [[{ opacity: 0 }, { opacity: 1 }], { duration: FADE_MS, easing: 'linear' }];
  const px = (n: number) => `${String(Math.round(n))}px`;
  return [[{ top: px(from.top), height: px(from.height) }, { top: px(to.top), height: px(to.height) }], { duration: MOVE_MS, easing: MOVE_EASE }];
}

/** Where each bubble's top goes, px under the bar. Pure. */
export function bubbleTops(world: 'small' | 'bubble'): { readonly world: number | null; readonly bubbles: Readonly<Record<BubbleId, number>> } {
  const first = world === 'bubble' ? TOP_STACK + STEP : TOP_SMALL;
  const tops = Object.fromEntries(BUBBLES.map((b, i) => [b, first + i * STEP])) as Record<BubbleId, number>;
  return { world: world === 'bubble' ? TOP_STACK : null, bubbles: tops };
}

/** A coin's key, its true one ('⌥⌘1' for the first, top to bottom: keys.ts; ⌥1-9 are the agents', designer m_45787). Pure. */
export const bubbleKey = (b: BubbleId): string => `⌥⌘${String(BUBBLES.indexOf(b) + 1)}`;

/** The row ↑ (-1) or ↓ (+1) goes to from `at` (-1: none focused: ↓ the first, ↑ the last), kept in the card's `n` rows. Pure. */
export function rowStep(at: number, n: number, by: 1 | -1): number {
  if (at < 0) return by === 1 ? 0 : n - 1;
  return Math.min(n - 1, Math.max(0, at + by));
}

/** The artifacts card's foot: how many, and in how many projects when more than one. Pure. */
export function artFoot(n: number, projects: number): string {
  if (n === 0) return '';
  return projects > 1 ? side.in_projects({ n, p: projects }) : side.artifact_count({ n });
}

/** The open card's height at most (null before its first layout: CARD_MAX) and whether its rows scroll (the 16 fade). */
interface CardFit {
  readonly maxH: number | null;
  readonly scrolls: boolean;
}
const NO_FIT: CardFit = { maxH: null, scrolls: false };

/** The window's height, at each resize's end only. */
function useWinH(): number {
  const [h, setH] = useState(() => window.innerHeight);
  useEffect(() => onResizeEnd(() => {
    setH(window.innerHeight);
  }), []);
  return h;
}

/** The window's width, at each resize's end only. */
function useWinW(): number {
  const [w, setW] = useState(() => window.innerWidth);
  useEffect(() => onResizeEnd(setW), []);
  return w;
}

/** The world, the bubbles and their cards, over the window's right edge. */
export function SideHost() {
  const winW = useWinW();
  const leftW = useLeftWidth();
  const box = usePanelBox();
  const art = useFrontArtKind();
  const panel: PanelIn = { ...box, threadMin: art === undefined ? THREAD_KEEPS.rows : THREAD_KEEPS.artifact };
  const route = useRoute();
  useLayoutDriver(winW, leftW, panel, sideShows(route));
  const layout = useSideLayout();
  const rows = useProjects();
  const home = rows.find((p) => p.home)?.project ?? '';
  const project = projectOf(route) ?? home;
  if (layout.coins === 'none' || project === '') return null;
  // whose items it lists: the Overview's scope (a project's own; bise's own every project's)
  const scope = panelProject(route, home, rows.filter((p) => !p.home).length) ?? project;
  return <Side key={`${project}\u0000${scope}`} project={project} scope={scope} home={home} world={layout.world} at={layout.coins} right={layout.right} outerW={layout.outerW ?? OUTER_W} room={winW - leftW} />;
}

/** `project`: where its acts go (the world, + new agent, + add); `scope`: whose items it lists ('' every project's); `at`: where the coins are, `right` their column's distance from the window's right edge; `outerW` the card's and its tile's width, `room` the window less the left (a drag on the card's border is clamped to it). */
function Side({ project, scope, home, world, at: coinsAt, right, outerW, room }: { project: string; scope: string; home: string; world: 'small' | 'big' | 'bubble'; at: CoinsAt; right: number; outerW: number; room: number }) {
  const router = useRouter();
  const route = useRoute();
  const store = useStore();
  const rows = useProjects();
  const all = scope === '';
  const name = rows.find((p) => p.project === project)?.name || project;
  // the world is the project's office: its own agents and asks, whatever the lists' scope
  const own = useAgents(project);
  const ownCards = useCards(project);
  const ownLive = useMemo(() => inRank(own), [own]);
  const listed = useListed(scope, home);
  const { agents, live } = listed;
  // on a sub-agent's thread the artifacts float lists what it made, with its scope line (the Overview's, barscope.ts)
  const bar = barScope(route, scope);
  const artScope = scopeWords(bar, 'artifacts', name);
  const allArtifacts = useAllArtifacts(scope);
  // what he pinned leads each list, a stable partition (pins.ts pinFirst)
  const artPins = usePins('artifact');
  const schedPins = usePins('schedule');
  const procPins = usePins('process');
  const artPinned = (it: { project: string; a: { id: string } }) => pinnedIn(artPins, it.project, it.a.id);
  const schedPinned = (x: { project: string; t: { id: number } }) => pinnedIn(schedPins, x.project, String(x.t.id));
  const procPinned = (x: { project: string; p: { agent: string; id: string } }) => pinnedIn(procPins, x.project, procPinId(x.p.agent, x.p.id));
  // their order holds under his pointer after a hover pin's click, until it leaves the card (pinhold.ts)
  const artOrder = usePinOrder('artifact');
  const schedOrder = usePinOrder('schedule');
  const procOrder = usePinOrder('process');
  const schedLeads = (x: { project: string; t: { id: number } }) => pinnedIn(schedOrder, x.project, String(x.t.id));
  const artifacts = pinFirst(bar.level === 'agent' ? allArtifacts.filter((it) => inScope(bar, it.a.agent)) : allArtifacts, (it) => pinnedIn(artOrder, it.project, it.a.id));
  const tasks = pinFirst(useAllScheduled(scope), schedLeads);
  const procs = pinFirst(useAllProcesses(scope), (x) => pinnedIn(procOrder, x.project, procPinId(x.p.agent, x.p.id)));
  // the processes are asked once here, as the old right bar did; the hub sends them again on each change
  const asked = all ? rows.map((p) => p.project).join('\n') : scope;
  useEffect(() => {
    for (const p of asked.split('\n')) if (p !== '') store.command({ cmd: 'processes', project: p });
  }, [store, asked]);
  const now = useNowFor(() => false);
  const watching = live.filter((a) => alarms(a, now).length > 0);
  const projectOfAgent = (a: Agent) => listed.projectOf.get(a) ?? project;
  const counts: Counts = {
    ...agentCounts(agents),
    artifacts: artifacts.length,
    newArtifacts: artifacts.filter((x) => x.a.new).length,
    timers: tasks.length,
    watches: live.reduce((n, a) => n + alarms(a, now).length, 0),
    running: procs.filter((x) => isRunning(x.p)).length,
    listening: procs.filter((x) => isRunning(x.p) && (x.p.ports ?? []).length > 0).length,
    inbox: listed.waits,
  };
  const [open, setOpen] = useState<BubbleId | null>(null);
  // the bims' tile over the outer card: hidden or not, its height (tilestate.ts, kept per window)
  const tile = useOuterWorld();
  // a new project's welcome in main's thread holds the office (one on screen): the tile keeps its frame, empty, until it docks there
  const tilePhase = useWelcoming(project);
  // no room beside the thread: the coins are minis in the thread's header or the sheet's tab row,
  // their floats under the header (folded-side v9)
  // (his m_45864: both in the title bar, right after the thread's crumb, never in the panel's tab row)
  const mini = coinsAt === 'header' || coinsAt === 'tabs';
  const slot = useCoinSlot(mini);
  const tops = mini ? MINI_TOPS : bubbleTops(world === 'bubble' ? 'bubble' : 'small');
  // the one float (designer m_44990): from one bubble to another its frame moves (its place now,
  // read mid-move too, kept in `from`) and the bubble it leaves crossfades out (`leaving`)
  const root = useRef<HTMLDivElement | null>(null);
  const frame = useRef<HTMLElement | null>(null);
  const from = useRef<{ readonly top: number; readonly height: number } | null>(null);
  const [leaving, setLeaving] = useState<BubbleId | null>(null);
  const toggle = (b: BubbleId) => {
    if (open === b) {
      setOpen(null);
      setLeaving(null);
      return;
    }
    const el = frame.current;
    const side = root.current;
    if (open !== null && el !== null && side !== null) {
      const r = el.getBoundingClientRect();
      from.current = { top: r.top - side.getBoundingClientRect().top, height: r.height };
      setLeaving(noGrow() ? null : open);
    }
    setOpen(b);
  };
  // a card opened by its key (⌥1-4, ⌥⌘1-4): its first row takes the focus, so ⏎ opens it at once
  const keyed = useRef(false);
  const byKey = (b: BubbleId) => {
    keyed.current = true;
    toggle(b);
  };
  useMenuAction('side-1', () => {
    byKey('agents');
  });
  useMenuAction('side-2', () => {
    byKey('artifacts');
  });
  useMenuAction('side-3', () => {
    byKey('scheduled');
  });
  useMenuAction('side-4', () => {
    byKey('processes');
  });
  // ⌥ held alone 300 ms: each coin shows its key and its card, all four at once (held.ts; keys.ts
  // gives ⌥1-4 to the folded side; page folded-side v9); any other key or the release hides them
  const alt = useModHeldFor('Alt', CARD_HOLD_MS);
  // the coins' caps show their true key, ⌥⌘N: with ⌥ held beside their cards, with ⌥⌘ held alone (no cards; designer m_45787)
  const altMeta = useModHeld('AltMeta');
  const caps = alt || altMeta;
  // a hover 150 ms: that coin's card alone (information only; designer m_46248, his m_46245: 400 was
  // too slow); warm for 1 s once one showed, the next coin's at once, so sliding down the coins never
  // waits; on leave a 100 ms grace (crossing the gap between coins doesn't blink), then an 80 ms fade
  const [hover, setHover] = useState<BubbleId | null>(null);
  const [fading, setFading] = useState<BubbleId | null>(null);
  const hoverT = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const fadeT = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const warmUntil = useRef(0);
  const hoverNow = useRef<BubbleId | null>(null);
  const onHover = (b: BubbleId) => (on: boolean) => {
    clearTimeout(hoverT.current);
    if (!on) {
      hoverT.current = setTimeout(() => {
        if (hoverNow.current !== null) {
          warmUntil.current = Date.now() + CARD_WARM_MS;
          setFading(hoverNow.current);
          clearTimeout(fadeT.current);
          fadeT.current = setTimeout(() => {
            setFading(null);
          }, CARD_OUT_MS);
        }
        hoverNow.current = null;
        setHover(null);
      }, CARD_GRACE_MS);
      return;
    }
    const show = () => {
      clearTimeout(fadeT.current);
      setFading(null);
      hoverNow.current = b;
      setHover(b);
    };
    if (hoverNow.current !== null || Date.now() < warmUntil.current) {
      show();
      return;
    }
    hoverT.current = setTimeout(show, CARD_HOVER_MS);
  };
  useEffect(() => () => {
    clearTimeout(hoverT.current);
    clearTimeout(fadeT.current);
  }, []);
  const facts: CardFacts = { live, counts, artifacts, tasks, procs, watching: watching.map((a) => a.name), now };
  // no card over an open float: it is the coin's whole say
  const carded: readonly BubbleId[] = open !== null || mini ? [] : alt ? BUBBLES : hover !== null ? [hover] : [];
  const miniCounts = Object.fromEntries(BUBBLES.map((b) => [b, bubbleCount(b, counts)])) as Record<BubbleId, BubbleCount>;
  const miniTips = Object.fromEntries(BUBBLES.map((b) => [b, bubbleTip(b, counts)])) as Record<BubbleId, string>;
  // a mini's float hangs under the header, its right edge on the minis' (the side's right moved
  // there: a float stands CARD_RIGHT left of the side's right edge)
  const anchor = () => {
    const el = root.current;
    if (el === null || slot === null) return;
    el.style.right = `${String(Math.max(0, window.innerWidth - slot.getBoundingClientRect().right - CARD_RIGHT))}px`;
  };
  // esc closes the open card and gives the focus back to its bubble, ↑ ↓ walk its rows (the
  // focus ring), ⏎ opens the focused one (its button's own); a click outside closes it
  useEffect(() => {
    if (open === null) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        const rows = [...(root.current?.querySelectorAll<HTMLElement>(ROWS_NOW) ?? [])];
        if (rows.length === 0) return;
        e.preventDefault();
        e.stopPropagation();
        rows[rowStep(rows.indexOf(document.activeElement as HTMLElement), rows.length, e.key === 'ArrowDown' ? 1 : -1)]?.focus();
        frame.current?.setAttribute('data-kb', '');
        return;
      }
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      setOpen(null);
      (slot ?? root.current)?.querySelector<HTMLElement>(`[data-testid="app.${slot !== null ? 'mini' : 'bubble'}.${open}"]`)?.focus();
    };
    const down = (e: PointerEvent) => {
      // the pointer in the card: its rows' focus is the pointer's again (no ring on a clicked row)
      frame.current?.removeAttribute('data-kb');
      // in the side, or on a mini in the header (its own click toggles the float)
      if (e.target instanceof Node && (root.current?.contains(e.target) === true || slot?.contains(e.target) === true)) return;
      setOpen(null);
    };
    // opened by its key: its first row focused, its ring shown at once (data-kb: a focus() after a
    // menu accelerator is not :focus-visible in Chromium)
    if (keyed.current) {
      frame.current?.setAttribute('data-kb', '');
      root.current?.querySelector<HTMLElement>(ROWS_NOW)?.focus();
    }
    keyed.current = false;
    window.addEventListener('keydown', key, true);
    window.addEventListener('pointerdown', down, true);
    return () => {
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('pointerdown', down, true);
    };
  }, [open, slot]);
  // a card's top is always its bubble's, its height at most the room under it less 16
  // (cardMaxH: nothing measured, it never moves nor runs past the window); its rows scroll inside
  // under a 16 fade, set once they overflow (a measure that finds the same sets nothing)
  const winH = useWinH();
  const [fit, setFit] = useState<CardFit>(NO_FIT);
  const fitted = useRef<CardFit>(NO_FIT);
  const put = (next: CardFit) => {
    if (next.maxH === fitted.current.maxH && next.scrolls === fitted.current.scrolls) return;
    fitted.current = next;
    setFit(next);
  };
  useLayoutEffect(() => {
    const el = frame.current;
    const side = root.current;
    if (el === null || side === null || open === null) {
      put(NO_FIT);
      return;
    }
    const maxH = cardMaxH(side.getBoundingClientRect().top, tops.bubbles[open], winH);
    const measure = () => {
      const body = el.querySelector('.s3-card-layer[data-now] .s3-card-body');
      put({ maxH, scrolls: body !== null && body.scrollHeight > body.clientHeight });
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- its bubble's top follows `open` and the layout
  }, [open, world, winH]);
  // the move (designer m_44990): the frame goes from where it is to its new bubble's place and
  // height in MOVE_MS; a switch mid-move starts from where it is; reduced motion: no move, the
  // whole float fades in over FADE_MS. The leaving layer goes at the move's end.
  const moving = useRef<Animation | null>(null);
  useLayoutEffect(() => {
    const el = frame.current;
    const was = from.current;
    from.current = null;
    if (el === null || was === null || open === null || typeof el.animate !== 'function') return;
    moving.current?.cancel();
    const side = root.current;
    if (side !== null) el.style.maxHeight = `${String(cardMaxH(side.getBoundingClientRect().top, tops.bubbles[open], winH))}px`;
    const to = { top: tops.bubbles[open], height: el.getBoundingClientRect().height };
    moving.current = el.animate(...moveFrames(was, to, noGrow()));
    const t = setTimeout(() => {
      setLeaving(null);
    }, MOVE_MS);
    return () => {
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a move is one switch of `open`
  }, [open]);
  const maxH = fit.maxH ?? CARD_MAX;
  const here = route.screen === 'thread' ? { project: route.project, agent: route.agent } : { project: null, agent: null };
  // the tile follows the open thread's bim; an ask opened in place in the card wins while it is open (designer m_47300)
  const [askOpen, setAskOpen] = useState<{ readonly project: string; readonly agent: string } | null>(null);
  const tileFollow = followOf(askOpen === null ? route : { screen: 'thread', ...askOpen }, project, ownLive);
  const at = answerFrom(here, home);
  // a row opens its agent in its own project (bise's own lists every project's)
  const openAgentIn = (p: string) => (agent: string) => {
    router.go({ screen: 'thread', project: p, agent });
  };
  const addPlace = useAddPlace(project);
  // a drop of files on the artifacts coin adds them there, as on the list (folded-side v9)
  const artDrop = useAddDropOn(addPlace);
  // the agents float: the Overview's AGENTS and ARCHIVED rows (folded-side v9: A's floats take Overview's rows)
  const ov = useAgentRows(scope, alt);
  const [archivedOpen, setArchivedOpen] = useState(false);
  // a row opens its agent (main's on bise's own: bise's view); a mark or a head its PR's review, an
  // orphan its branch, each change in the panel (review/openChange.ts)
  const openRow = (r: BarRow) => {
    if (all && r.main === true) router.go({ screen: 'bise' });
    else if (r.kind === 'orphan' && r.branch !== undefined) openChange(store, router, r.project, { branch: r.branch });
    else if (r.kind === 'group' && r.feature !== undefined) router.go({ screen: 'feature', project: r.project, name: r.feature });
    else if (r.agent !== undefined) router.go({ screen: 'thread', project: r.project, agent: r.agent });
  };
  // the ⑂ #412 chip: that PR's tab (⌘: the forge's page); a mark with no PR (a land, a worktree) as before
  const openPr = (r: BarRow, e?: MouseEvent) => {
    if (r.mark?.url != null && r.mark.url !== '') openPrTab(store, router, r.project, r.mark.url, e);
    else if (r.kind === 'orphan' && r.branch !== undefined) openChange(store, router, r.project, { branch: r.branch });
    else if (r.agent !== undefined) openChange(store, router, r.project, { agent: r.agent });
  };
  // a click in the float that navigates (a row opens a thread, a page, an artifact or a task's tab; a
  // foot's 'all … ›') closes it, so his next esc goes to what it opened, never to the float left
  // over it (native-lead m_48053); a click that acts in place (an answer, a restore, the archived
  // fold) keeps it. One rule here, for every float's rows: where he is before the click, then after
  const clickFrom = useRef<string | null>(null);
  const whereNow = () => `${JSON.stringify(router.get())} ${panelFront() ?? ''}`;
  const floatClicked = {
    before: () => {
      clickFrom.current = whereNow();
    },
    after: () => {
      const from = clickFrom.current;
      clickFrom.current = null;
      if (from === null || from === whereNow()) return;
      setOpen(null);
      setLeaving(null);
    },
  };
  const running = procs.filter((x) => isRunning(x.p));
  // bise's own lists every project's (designer m_45706): each row says its project ('acme › perf'),
  // each head counts across them ('6 working in 3 projects'); in a project, neither
  const nameOf = (p: string) => rows.find((r) => r.project === p)?.name || p;
  const groupOf = (p: string): string | undefined => (all ? nameOf(p) : undefined);
  const across = (b: BubbleId): number => {
    if (!all) return 1;
    switch (b) {
      case 'agents':
        return busyProjects(listed);
      case 'artifacts':
        return new Set(artifacts.map((x) => x.project)).size;
      case 'scheduled':
        return new Set([...tasks.map((x) => x.project), ...watching.map(projectOfAgent)]).size;
      case 'processes':
        return new Set(running.map((x) => x.project)).size;
    }
  };
  const saysOf = (b: BubbleId) => acrossSays(bubbleSays(b, counts), across(b));
  /** Scheduled's first `n` rows: what he pinned first (its timers, then its watches), then the other timers, then the other watches */
  const schedRows = (n: number) => {
    const pinnedTasks = tasks.filter(schedLeads);
    const restTasks = tasks.filter((x) => !schedLeads(x));
    const watches = (pinned: boolean, list: readonly Agent[]) => list.map((a) => (
      <WatchRows key={`${projectOfAgent(a)}/${a.name}/${String(pinned)}`} project={projectOfAgent(a)} a={a} now={now} onOpen={openAgentIn(projectOfAgent(a))}
        group={groupOf(projectOfAgent(a))} pins={schedPins} order={schedOrder} pinned={pinned} />
    ));
    const task = (x: (typeof tasks)[number]) => <TaskRow key={`${x.project}/${x.t.id}`} x={x} now={now} group={groupOf(x.project)} pinned={schedPinned(x)} />;
    return (
      <>
        {pinnedTasks.map(task)}
        {watches(true, watching)}
        {restTasks.slice(0, Math.max(0, n - pinnedTasks.length)).map(task)}
        {watches(false, watching.slice(0, Math.max(0, n - tasks.length)))}
      </>
    );
  };
  /** each bubble's float: its head, rows and foot (designer m_44975: one per bubble) */
  const floatOf = (b: BubbleId): FloatContent => {
    switch (b) {
      case 'agents':
        return {
          id: b, name: bubbleName(b),
          // in a project the waiting part goes in the accent (page agents-float v1); across projects one line
          ...(across(b) < 2 ? { says: counts.working > 0 ? side.working({ n: counts.working }) : '', loud: counts.asks > 0 ? side.waiting_you({ n: counts.asks }) : '' } : { says: saysOf(b) }),
          add: <HeadButton icon="plus" tip={side.new_agent} onClick={() => { newAgent(project); }} testId="app.flyout.agents.add" />,
          foot: {
            left: '',
            fold: <ArchivedFold n={ov.archived.length} open={archivedOpen} onToggle={() => { setArchivedOpen((o) => !o); }} />,
            links: [{ text: side.all_agents, testId: 'app.flyout.agents.all', onClick: () => { router.go(all ? { screen: 'bise' } : { screen: 'agents', project }); } }],
          },
          // Overview's rows (folded-side v9, A's floats): the same as its AGENTS (app/agentrows.ts)
          rows: (
            <>
              {ov.live.length === 0 ? <Empty>{side.no_agents}</Empty> : <FloatAgents live={ov.live} agents={ov.agents} cards={listed.cards} at={at} onOpen={openRow} onOpenPr={openPr} groupOf={groupOf} onHandoff={() => { setOpen(null); }} />}
              {archivedOpen && ov.archived.length > 0 && <ArchivedRows rows={ov.archived} onOpen={openRow} />}
            </>
          ),
        };
      case 'artifacts':
        return {
          id: b, name: bubbleName(b), says: saysOf(b),
          add: <AddButton place={addPlace} testId="app.flyout.artifacts.add" />,
          ...(artScope === null ? {} : { scope: { words: artScope.words, tip: artScope.tip } }),
          foot: { left: artFoot(artifacts.length, new Set(artifacts.map((x) => x.project)).size), links: [{ text: side.all_artifacts, testId: 'app.flyout.artifacts.all', onClick: () => { router.go({ screen: 'artifacts', project: scope }); } }] },
          rows: artifacts.length === 0 ? <Empty>{artScope?.empty ?? side.no_artifacts}</Empty> : artifacts.slice(0, ROWS).map((it) => <ArtifactRow key={`${it.project}/${it.a.id}`} it={it} now={now} group={groupOf(it.project)} pinned={artPinned(it)} />),
        };
      case 'scheduled':
        return {
          id: b, name: bubbleName(b), says: saysOf(b),
          foot: { left: '', links: [{ text: side.all_scheduled, testId: 'app.flyout.scheduled.all', onClick: () => { router.go({ screen: 'scheduled' }); } }] },
          rows: tasks.length === 0 && watching.length === 0 ? <Empty>{side.no_scheduled}</Empty> : (
            <>
              {schedRows(ROWS)}
            </>
          ),
        };
      case 'processes':
        return {
          id: b, name: bubbleName(b), says: saysOf(b),
          foot: { left: '', links: [{ text: side.all_processes, testId: 'app.flyout.processes.all', onClick: () => { router.go(all ? { screen: 'running' } : { screen: 'running', scope: { project } }); } }] },
          rows: running.length === 0 ? <Empty>{side.no_processes}</Empty> : running.slice(0, ROWS).map((x) => <ProcRow key={`${x.project}/${x.p.agent}/${x.p.id}`} x={x} now={now} group={groupOf(x.project)} pinned={procPinned(x)} />),
        };
    }
  };
  // the panel folded with room for it: the outer card, the Overview out of the panel (page outer-side v2)
  if (coinsAt === 'card') {
    const live = ov.live.filter((r) => r.kind === 'agent').length;
    // the tile (outer-world v2): its height in the window's bounds, and the room it and its handle take
    const tileH = clampTile(tile.h, winH);
    const tileRoom = tile.hidden ? 0 : tileH + TILE_GAP;
    const sched = tasks.length + watching.length;
    // each section's count goes back to its first rows when the scope changes (designer m_46693)
    const scopeKey = all ? '' : scope;
    return (
      <>
        {!tile.hidden && (
          <>
            <OuterTile project={project} name={name} agents={ownLive} cards={ownCards} h={tileH} w={outerW} follow={tileFollow} phase={tilePhase}
              onOpen={(agent) => {
                router.go({ screen: 'thread', project, agent });
              }}
              onPickMap={(m) => {
                store.command({ cmd: 'prefs_set', key: mapPrefKey(project), value: m });
              }}
              onPanel={() => {
                openTab({ kind: 'office', project });
              }}
              onHide={() => {
                outerWorld.hide(true);
              }}
              // on bise's own the town: every project's office side by side (lost with the RightBar in 6d9d07180)
              town={all ? {
                onOpenProject: (p) => {
                  router.go({ screen: 'thread', project: p, agent: MAIN });
                },
                onOpenAgent: (p, agent) => {
                  router.go({ screen: 'thread', project: p, agent });
                },
              } : null} />
            <TileHandle h={tileH} w={outerW} winH={winH} onHeight={(h) => {
              outerWorld.height(h, winH);
            }} />
          </>
        )}
      {/* his m_48003: the card and its tile resize together by their left border (designer m_48013) */}
      <Splitter edge="outer" width={outerW} label={side.outer_width} testId="app.split.outer"
        style={{ top: OUTER_TOP, right: OUTER_GAP + outerW - SPLIT_HALF }}
        onWidth={(w) => {
          outerWidth.drag(w, room);
        }}
        onReset={() => {
          outerWidth.reset();
        }} />
      <OuterCard key={scopeKey} tile={tileRoom} w={outerW}>
        <OuterSection id="agents" name={bubbleName('agents')} count={String(live)} {...(counts.asks > 0 ? { loud: side.head_waits({ n: counts.asks }) } : {})} onAll={() => {
          router.go(all ? { screen: 'bise' } : { screen: 'agents', project });
        }} {...(tile.hidden ? { onCube: () => {
          outerWorld.hide(false);
        } } : {})}>
          <OuterAgents live={ov.live} agents={ov.agents} cards={ov.cards} at={at} here={here} now={now} groupOf={groupOf} onOpen={openRow} onOpenPr={openPr} bims={tile.hidden ? null : project} onAsk={setAskOpen} />
        </OuterSection>
        <OuterList id="artifacts" name={bubbleName('artifacts')} count={String(artifacts.length)} onAll={() => {
          router.go({ screen: 'artifacts', project: scope });
        }} total={artifacts.length} empty={<Empty>{artScope?.empty ?? side.no_artifacts}</Empty>}
          rows={(n) => artifacts.slice(0, n).map((it) => <ArtifactRow key={`${it.project}/${it.a.id}`} it={it} now={now} group={groupOf(it.project)} pinned={artPinned(it)} />)} />
        <OuterList id="scheduled" name={bubbleName('scheduled')} count={String(sched)} onAll={() => {
          router.go({ screen: 'scheduled' });
        }} total={sched} empty={<Empty>{side.no_scheduled}</Empty>} rows={(n) => (
          <>
            {schedRows(n)}
          </>
        )} />
        <OuterList id="processes" name={bubbleName('processes')} count={String(running.length)} onAll={() => {
          router.go(all ? { screen: 'running' } : { screen: 'running', scope: { project } });
        }} total={running.length} empty={<Empty>{side.no_processes}</Empty>}
          rows={(n) => running.slice(0, n).map((x) => <ProcRow key={`${x.project}/${x.p.agent}/${x.p.id}`} x={x} now={now} group={groupOf(x.project)} pinned={procPinned(x)} />)} />
      </OuterCard>
      </>
    );
  }
  return (
    <div className="s3-side" ref={root} data-world={world} data-at={coinsAt} style={coinsAt === 'between' ? { right } : undefined} data-testid="app.side">
      {mini && slot !== null && createPortal(
        <Minis counts={miniCounts} badge={bubbleBadge('agents', counts)} tips={miniTips} open={open} keys={caps ? bubbleKey : null} onClick={(b) => {
          anchor();
          toggle(b);
        }} />,
        slot,
      )}
      {!mini && tops.world !== null && (
        <Bubble id="world" count={null} tip={WORLD_NAME} open={false} top={tops.world}
          pic={<WorldThumb project={project} agents={ownLive} cards={ownCards} />} onClick={() => {
            openTab({ kind: 'office', project });
          }} />
      )}
      {!mini && BUBBLES.map((b) => (
        <Bubble key={b} id={b} count={bubbleCount(b, counts)} badge={bubbleBadge(b, counts)} tip={bubbleTip(b, counts)} open={open === b} top={tops.bubbles[b]}
          hint={caps ? bubbleKey(b) : undefined} onHover={onHover(b)} drop={b === 'artifacts' ? artDrop : undefined} onClick={() => {
          clearTimeout(hoverT.current);
          setHover(null);
          toggle(b);
        }} />
      ))}
      {carded.map((b) => <CoinCardView key={b} id={b} card={coinCard(b, facts)} top={tops.bubbles[b]} />)}
      {fading !== null && !carded.includes(fading) && open === null && !mini && <CoinCardView key={`out-${fading}`} id={fading} card={coinCard(fading, facts)} top={tops.bubbles[fading]} out />}
      {open !== null && (
        <Float id={open} name={bubbleName(open)} top={tops.bubbles[open]} maxH={maxH} moving={leaving !== null} frameRef={frame} onClicked={floatClicked}>
          <FloatLayer key={open} c={floatOf(open)} scrolls={fit.scrolls} />
          {leaving !== null && leaving !== open && <FloatLayer key={`leaving-${leaving}`} c={floatOf(leaving)} leaving />}
        </Float>
      )}
    </div>
  );
}

/** main's name as a route's agent, for the world's legend */
export const LEAD = MAIN;
