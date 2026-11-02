// src/app/side3d · sideLayout: where the thread's column, the world and the bubbles go across the
// window's width (plan native-mac-plan v4 §2, decision d3). The panel folded and the room holding a
// readable column beside it: the outer card (page outer-side v2, his pick), 292 wide by default
// and 260-480 by its left border (outerFit, outerwidth.ts), 12 from the window's edge, the column centred in what it leaves, unless he picked the coins by hand
// (sidepick.ts). Else the column is centred when both sides
// leave the world's band (148 + 20 + 12 = 180); else it slides left (24 from the sidebar at
// least); else the world folds into a 5th bubble and the band drops to 100 (68 + 20 + 12); last,
// the column narrows. The tabbed panel open (folded-side v9, the width rule): the coins stay
// between the thread and the panel (a 68 band, the world a 5th coin) while the window holds them,
// else they go in the thread's header as minis, and over a sheet or a full-width panel in its tab
// row (CoinsAt); the column is then centred in what is left. A route that is no project's
// (sideShows: the inbox, scheduled, processes, plugins, settings, every artifacts screen...): no
// coins anywhere. Recomputed at the end of a resize or a fold only (place.ts).
// Pure: widths in, places out; nm-thread and nm-composer read it through place.ts. Owner: nm-side.

import type { Route } from '../route';

/**
 * The folded side (the world and its bubbles) shows inside a project only (r2-side item 7,
 * Gabriel m_44266, designer m_44292): a thread (bise's own too), its agents, archived, changes,
 * PRs, a review, a feature, a commit, a branch, environments, debug. Hidden on the app-wide pages:
 * the inbox, scheduled, processes, plugins, settings, report, onboarding, lab, not found and the
 * artifacts library (a project's own too). An artifact's route (artifacts/<project>/<id>) shows its
 * maker's thread with the artifact in a panel tab: the side shows there (ui-review m_45670, designer
 * m_45674: an artifact open keeps the minis). Pure.
 */
export function sideShows(r: Route): boolean {
  switch (r.screen) {
    case 'bise':
    case 'thread':
    case 'agents':
    case 'changes':
    case 'prs':
    case 'review':
    case 'feature':
    case 'commit':
    case 'branch':
    case 'environments':
    case 'debug':
      return true;
    case 'artifacts':
      return r.project !== '' && r.id !== undefined;
    case 'scheduled':
    case 'running':
    case 'plugins':
    case 'inbox':
    case 'report':
    case 'settings':
    case 'onboarding':
    case 'lab':
    case 'notfound':
      return false;
  }
}

/** How the bims' world shows: 'big' (384, hovered), 'small' (148, folded), 'bubble' (a 5th bubble, no room). */
export type WorldSize = 'big' | 'small' | 'bubble';

/** Where things go, in px from the window's left edge. */
export interface SideLayout {
  /** the column's left edge */
  readonly colLeft: number;
  /** the column's width (--w-col 720 at most) */
  readonly colW: number;
  /** how the world shows (never 'big' here: the hover grows it over the page, the layout stays) */
  readonly world: WorldSize;
  /** the band the right side keeps free at the window's right edge (180, or 100 with the world as a bubble: the coins 20 from the edge, his m_46261; 0 when the panel is open) */
  readonly gutter: number;
  /** the world and the bubbles show at the right of the thread (at the window's edge, or between the thread and the panel) */
  readonly side: boolean;
  /** where the coins are (folded-side v9): at the window's edge, between the thread and the panel,
   *  in the thread's header (no room beside it), in the sheet's tab row (the panel over the thread), or nowhere */
  readonly coins: CoinsAt;
  /** the coins' column from the window's right edge, px: 0 at the edge, the panel's width between */
  readonly right: number;
  /** the outer card's and its tile's width when the coins are 'card' (outerFit of his width) */
  readonly outerW?: number;
  /** the left edge of the band the right side keeps (its coins, its card), px from the window's left: nothing of a page goes right of it (pagecol.ts pageBand); the window's width when no side is drawn */
  readonly bandL: number;
}

/** The outer card (page outer-side v2) and the tile over it: their width by default (292, 12 more for the titles: his m_47018, designer m_47025; set inline), and the band it keeps with its two 12 */
export const OUTER_W = 292;
export const OUTER_BAND = OUTER_W + 12 + 12;
/** the least column the card leaves beside it; under it the coins come back at the window's edge */
export const OUTER_COL_MIN = 480;
/**
 * The card's widths by its left border (his m_48003, designer m_48013): 292 by default, 260 at
 * least (its rows drop their PR and ψ ends whole, the name cut last), 480 at most (past it a row's
 * name and its end no longer read as one row; a big screen's room goes to the thread's air).
 */
export const OUTER = { def: OUTER_W, min: 260, max: 480 } as const;

/**
 * The card's width in `room` px (the window less the rail and the sidebar) for the width he wants:
 * in OUTER's limits and never more than leaves the column its OUTER_COL_MIN and two MIN_LEFT; null
 * when even OUTER.min does not (the coins come back). Pure.
 */
export function outerFit(room: number, want: number): number | null {
  const most = Math.min(OUTER.max, room - 2 * 12 - OUTER_COL_MIN - 2 * MIN_LEFT);
  if (most < OUTER.min) return null;
  const w = Number.isFinite(want) ? Math.round(want) : OUTER.def;
  return Math.min(most, Math.max(OUTER.min, w));
}

/** Where the coins are (folded-side v9, Gabriel's pick). */
export type CoinsAt = 'card' | 'edge' | 'between' | 'header' | 'tabs' | 'none';

/** The coins' band between the thread and the panel: 12 + the 44 coin + 12 (the world a 5th coin there) */
export const BETWEEN_BAND = 68;
/** What the thread keeps beside the panel, from panel/model.ts: THREAD_MIN beside rows or a shell, ART_THREAD_MIN beside an artifact */
export const THREAD_KEEPS = { rows: 560, artifact: 400 } as const;

/** The tabbed panel as the layout reads it: open beside the thread at `width`, over it as a sheet, or full width; `threadMin` what the thread keeps beside it. */
export interface PanelIn {
  readonly open: boolean;
  readonly width: number;
  readonly over: boolean;
  readonly full: boolean;
  readonly threadMin: number;
}

/** --w-col: the column at most */
export const COL_W = 720;
/** the least room left of the column once it slid */
export const MIN_LEFT = 24;
/** the world's band: 148 + 20 (the coins' column 8 in, his m_46261) + 12 */
export const WORLD_BAND = 180;
/** the band with the world as a 5th bubble: its 68 object + 20 + 12 */
export const BUBBLE_BAND = 100;
/** the panel's width when its owner says none (--w-panel) */
export const PANEL_W = 384;

/** The column in `room` px with `band` kept on the right, or null when it needs to narrow. Pure. */
function fit(room: number, band: number): { left: number; w: number } | null {
  const w = Math.min(COL_W, room);
  const centred = (room - w) / 2;
  if (centred >= band) return { left: centred, w };
  const slid = room - w - band;
  if (slid >= MIN_LEFT) return { left: slid, w };
  return null;
}

/**
 * The layout for a window `winW` wide, a sidebar (with the rail) `sideW` wide, the tabbed panel
 * open or not (`panelW` its width, --w-panel when its owner did not say), the route showing the
 * side or not (`shows`, sideShows: off, the column is centred in the whole room, no band), and how
 * the open panel stands (`beside`: a sheet over the thread, full width, what the thread keeps).
 * The panel open (folded-side v9, the width rule): the coins stay between the thread and the panel
 * (the world a 5th coin, the panel's left edge their right) while the window holds the left, the
 * thread's least, their 68 band and the panel; under that they go in the thread's header in place
 * of its status line; a sheet or a full-width panel covers the thread: the coins go in its tab row
 * (under a sheet the thread keeps the whole room, its width as with the panel folded's room).
 * `compact` (side-mini, his m_46698): he picked the coins by hand, so the card never shows, even
 * with room for it; the rest falls back as without room. `outerWant`: the card's width he dragged
 * (outerwidth.ts), drawn as outerFit lets it. Pure.
 */
export const sideLayout = (winW: number, sideW: number, panelOpen: boolean, panelW: number = PANEL_W, shows = true, beside: Omit<PanelIn, 'open' | 'width'> = { over: false, full: false, threadMin: THREAD_KEEPS.rows }, compact = false, outerWant: number = OUTER_W): SideLayout => {
  const l = placed(winW, sideW, panelOpen, panelW, shows, beside, compact, outerWant);
  // the band's left edge: the coins' (or the card's) band ends at the panel's left edge, or the window's
  return { ...l, bandL: l.side ? winW - l.right - l.gutter : winW };
};

/** Placed = SideLayout less its band's edge (sideLayout adds it). */
type Placed = Omit<SideLayout, 'bandL'>;

/** sideLayout's places, its band's edge aside. Pure. */
function placed(winW: number, sideW: number, panelOpen: boolean, panelW: number, shows: boolean, beside: Omit<PanelIn, 'open' | 'width'>, compact: boolean, outerWant: number): Placed {
  // a sheet goes over the thread: the thread keeps the whole room under it (ui-review m_47216)
  const room = Math.max(0, winW - sideW - (panelOpen && !beside.over ? panelW : 0));
  const centred = (coins: CoinsAt): Placed => {
    const w = Math.min(COL_W, Math.max(0, room - 2 * MIN_LEFT));
    return { colLeft: sideW + (room - w) / 2, colW: w, world: 'small', gutter: 0, side: false, coins, right: 0 };
  };
  if (!shows) return centred('none');
  if (panelOpen) {
    if (beside.over || beside.full) return centred('tabs');
    if (winW < sideW + beside.threadMin + BETWEEN_BAND + panelW) return centred('header');
    const at = fit(room, BETWEEN_BAND) ?? { left: MIN_LEFT, w: Math.max(0, room - BETWEEN_BAND - MIN_LEFT) };
    return { colLeft: sideW + at.left, colW: at.w, world: 'bubble', gutter: BETWEEN_BAND, side: true, coins: 'between', right: panelW };
  }
  const edge = (l: { left: number; w: number }, world: WorldSize, gutter: number): Placed =>
    ({ colLeft: sideW + l.left, colW: l.w, world, gutter, side: true, coins: 'edge', right: 0 });
  // the outer card (page outer-side v2): at his width while the room holds it (outerFit), the
  // column, at least 480, centred in what it leaves
  const card = compact ? null : outerFit(room, outerWant);
  if (card !== null) {
    const band = card + 12 + 12;
    const left = room - band;
    const w = Math.min(COL_W, left - 2 * MIN_LEFT);
    return { colLeft: sideW + (left - w) / 2, colW: w, world: 'small', gutter: band, side: true, coins: 'card', right: 0, outerW: card };
  }
  const small = fit(room, WORLD_BAND);
  if (small !== null) return edge(small, 'small', WORLD_BAND);
  const bubble = fit(room, BUBBLE_BAND);
  if (bubble !== null) return edge(bubble, 'bubble', BUBBLE_BAND);
  return edge({ left: MIN_LEFT, w: Math.max(0, room - BUBBLE_BAND - MIN_LEFT) }, 'bubble', BUBBLE_BAND);
}

/** the room a card keeps above the window's bottom (designer m_44499) */
export const CARD_ROOM = 16;
/** a card's height at most, however tall the window */
export const CARD_MAX = 560;

/**
 * A card's height at most: its top is always its bubble's (`bubbleTop`, side px, the side
 * `sideTop` under the window's top); it ends 16 above the window's bottom at the lowest, 560 at
 * most; its rows scroll inside, its head and foot stay whole (designer m_44499, which replaces the
 * lift of r2-side item 3). Nothing measured: a card can never run past the window, nor move. Pure.
 */
export const cardMaxH = (sideTop: number, bubbleTop: number, winH: number): number =>
  Math.max(0, Math.min(CARD_MAX, winH - CARD_ROOM - sideTop - bubbleTop));
