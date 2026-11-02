// The office in the right bar's AGENTS section (page bims-office v7; spec les-bims-sc-narios-du-bureau
// §1-§2): the canvas above the section's rows at the office's natural height (the renderer fits it
// to the width), a DOM overlay for the labels, the hover card and the '?' / '✗' pills over the
// bims' heads, and the loop. His camera (zoom.ts): a pinch (a ctrl wheel in Chromium) zooms around
// the pointer, two fingers or a drag move the view while zoomed, '⤢ fit', esc or a double-click on
// the floor go back to the whole office; each move draws one frame. The renderer (three.js) comes with import('./render') only once the office shows: the list
// never asks for it. The loop draws on demand (loop.ts): a kick (new rows, his message, a hover,
// ctrl held, a size or theme change) draws one frame, the next comes only while something moves;
// none while the window is away. A lost WebGL context (or a renderer that fails to load) shows one
// line over the rows, never a blank section. Hover: the bim under the pointer (renderer.pick) shows
// its card after the tip's 400 ms (at once for 1 s after one closed, the ds tips' warmth); a click
// opens it as its row's click does, and pokes it (pokes.ts: it reacts; the first click's poke
// lands as the camera arrives). The sky (sky.ts, bims-sky): the clock's sky blends in the renderer, a
// blend due mid-drag or pinch waits until he lets go (Look.hold); the rain or snow is a CSS layer
// outside the building (falling.ts). Owner: bims-wire.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { Press, tipAttrs } from '@/ds';
import { reducedMotion, textWidth } from '@/ds/hooks';
import type { Agent, Card, Place, Plugin } from '@/proto';
import { shownName } from '@/agents/model';
import { bims } from '@/proto/words_bims.gen';
import { useModHeld } from '@/app/held';
import type { ActivitiesFn, ActivityKind, Label, LabelsFn, LayoutFn, MakeRenderer, NewTimelineFn, Office as OfficeMap, OfficeRenderer } from './model';
import { CHECK_MS, HOVER_MS, nextAfter } from './loop';
import { awayNow, nightNow, testMode } from './room';
import { pastimeWake } from './pastimes';
import { useOfficeEvents } from './events';
import { bimsView, type BimsView } from './view';
import { cutLines, cutWords, hoverCardAt, placeTags, TAG_INSET, type KeepRect, type TagIn } from './tags';
import { MapChip } from './MapChip';
import { FitChip, NameLabel, Pill } from './chips';
import { atFit, DRAG_PX, FIT, panBy, sameCam, wheelCam, type Cam } from './zoom';
import { konamiDone, localOf, type Forced, type Local } from './eggs';
import { OPEN_DELAY_S, soonest } from './pokes';
import { cropped, growOf, tipOf, unlockLabel } from './growth';
import { makeDriver, type Clock } from './office-driver';
import { growHigh } from './grow-keep';
import { FOLD_MS } from './Fold';
import { Pins, pinPoints, placePins } from './pins';
import { placeZzz, sleepers } from './zzz';
import { fallingLook, fallingSig, type FallingLook } from './falling';
import { WEATHERS, type Weather } from './sky';
import { aimAt, bimsLit, easeCam, edgeOf, followZoomOf, useBimsLit } from './focus';
import type { Growth } from '@/proto';
import { DEFAULT_MAP, type MapId, mapOf } from './maps/ids';
import type { PluginObject } from './plugin-objects';
import type { Corners } from './corners';
import { bimBox, deskBox, objPillsOf, objTipOf, placeObjTip, type ObjPill, type ObjTip, type Rect } from './objpills';
import { extKept, keepExt } from './ext-keep';
import { planOf } from './maps';
import './office.css';

/** a sky blend waits this long after his last wheel (a pinch, a pan) */
const HOLD_MS = 400;

/**
 * A check's sky (test mode): none unless it sets window.__biseSky (the other checks keep the theme's
 * fixed light: a slot's blend never lands in their frame counts); { weather } pins the weather, its
 * time is window.__biseLocal's.
 */
const NO_OBJECTS: readonly PluginObject[] = [];

function pinnedSky(): { sky?: false; weather?: Weather } {
  const pin = (window as { __biseSky?: { weather?: string } }).__biseSky;
  if (pin === undefined) return { sky: false };
  const w = WEATHERS.find((x) => x === pin.weather);
  return w === undefined ? {} : { weather: w };
}

/** the rain or snow layer (falling.ts): its kind, its density, the building cut out */
function placeFalling(el: HTMLElement, l: FallingLook): void {
  if (l.kind === null) {
    delete el.dataset.kind;
    el.style.opacity = '';
    el.style.clipPath = '';
    return;
  }
  el.dataset.kind = l.kind;
  el.style.opacity = String(l.opacity);
  el.style.clipPath = l.clip;
}

/** The pure modules and the renderer's loader: feed.ts's, or fakes in a test. */
export interface Parts {
  readonly activities: ActivitiesFn;
  readonly layoutOf: LayoutFn;
  readonly newTimeline: NewTimelineFn;
  readonly labelsOf: LabelsFn;
  readonly load: () => Promise<MakeRenderer>;
}

export type OfficeState = 'loading' | 'on' | 'lost';

/** the office's last width / height in this window: the fold grows to it before the renderer has loaded (Fold.tsx) */
let lastRatio = 1.4;

/** The AGENTS head's '320 | wide' while the office shows (view.ts keeps it per window); the cube after it opens and closes the office (Cube.tsx). */
export function BimsSwitch({ view }: { view: BimsView }) {
  const opt = (id: string, label: string, tip: string, on: boolean, pick: () => void) => (
    <Press look="bims-opt" aria-pressed={on} {...tipAttrs(tip)} onClick={pick} data-testid={`app.right.agents.${id}`}>
      {label}
    </Press>
  );
  return (
    <>
      {view.view === 'bims' && !view.lost && (
        <span className="bims-switch" data-testid="app.right.agents.width">
          {opt('width.narrow', bims.width_narrow, bims.width_narrow_tip, !view.wide, () => {
            bimsView.setWide(false);
          })}
          <span className="bims-sep" aria-hidden="true">·</span>
          {opt('width.wide', bims.width_wide, bims.width_wide_tip, view.wide, () => {
            bimsView.setWide(true);
          })}
          {/* hiding it: the cube after it (Cube.tsx), pressed while the office shows (designer m_28913) */}
        </span>
      )}
    </>
  );
}

/** What a bim's tag shows: its line (null: none) and its pill; the hover card's first line is its name. Pure. */
export function tagOf(l: Label): { readonly text: string | null; readonly pill: Label['pill'] } {
  // 'pill': the '?' / '✗' alone at rest, no name; hover, ctrl or an event give it its line
  if (l.mode === 'pill') return { text: null, pill: l.pill };
  // main's name as the window says it, 'bise', as its AGENTS row (designer m_28987)
  const name = shownName(l.name);
  if (l.mode === 'hover') return { text: name, pill: l.pill };
  // an easter egg's words stand alone ('1:47', 'combo ×2', '!'): the egg is the bim's, not news
  if (l.mode === 'egg') return { text: l.words, pill: l.pill };
  return { text: l.words === '' ? name : `${name} · ${l.words}`, pill: l.pill };
}

/** A bim's place in line for a label's room (designer m_27799): with a pill first, then working, then quiet. Pure. */
export function tagRank(k: ActivityKind): number {
  if (k === 'asks' || k === 'failed') return 1;
  if (k === 'work' || k === 'hot' || k === 'main' || k === 'starting' || k === 'landing' || k === 'visit' || k === 'report') return 2;
  return 3;
}

/** px a label's chip adds to its words (its padding), and a pill beside it with its gap */
const CHIP_PAD = 8;
const PILL_ROOM = 20;
/** the hover card: its widest (css px), its padding each side (--s-2), its title's lines */
const CARD_W = 240;
const CARD_PAD = 8;
const CARD_LINES = 3;
/** after a card closes, the next one shows at once for this long (the ds tips' warmth) */
const WARM_MS = 1000;
/** from this width the camera fits a little ground past the building (render/index.ts), under it the building only */
const WIDE_FIT_PX = 480;

/** The labels' font for measuring a cut: the ds's sans at 12 px (office.css .bims-label). */
function labelFont(weight = ''): string {
  const ui = typeof document === 'undefined' ? '' : getComputedStyle(document.documentElement).getPropertyValue('--font-ui').trim();
  return `${weight}12px ${ui === '' ? 'sans-serif' : ui}`;
}

/** The labels' content, to re-render the overlay only when it changes (not at each frame). Pure. */
export function labelsSig(labels: ReadonlyMap<string, Label>): string {
  let s = '';
  for (const l of labels.values()) s += `${l.name}\u0000${l.mode}\u0000${l.words}\u0000${l.title}\u0000${l.pill ?? ''}\u0001`;
  return s;
}


/** What the loop reads of his pointer, kept fresh by React and read by the frames (the rows go to the driver). */
interface Live {
  hover: string | null;
  held: boolean;
  /** the agent whose row the pointer is on (focus.ts): a halo under its bim */
  lit: string | null;
  /** the plugin object under his pointer once the tip's delay passed (its key): its name's tip (objpills.ts objTipOf) */
  obj: string | null;
}

/** The follow camera (focus.ts): the bim it aims at ('fit': back to the whole office, then null), and its last step's time. */
interface Follow {
  target: { readonly name: string } | 'fit' | null;
  at: number;
}

/** the map's crossfade (/bims): the old office over the new one, faded out (office.css .bims-xfade, ~300 ms) */
function crossfade(r: OfficeRenderer, cv: HTMLCanvasElement, fx: HTMLCanvasElement): void {
  fx.width = cv.width;
  fx.height = cv.height;
  fx.style.width = `${String(cv.clientWidth)}px`;
  fx.style.height = `${String(cv.clientHeight)}px`;
  r.copyTo(fx);
  fx.dataset.on = '';
  // the next frame lets it go: the transition runs from full to none
  requestAnimationFrame(() => {
    delete fx.dataset.on;
  });
}

export function Office({
  project,
  agents,
  cards,
  places,
  objects = NO_OBJECTS,
  plugins = null,
  growth = null,
  map = DEFAULT_MAP,
  parts,
  follow = null,
  followZoom,
  fill = false,
  onOpen,
  onOpenPlugin,
  onPickMap,
  pins = false,
  transparent = false,
  cameraFrom = null,
  onState,
  snapRef,
  headRef,
  onZoomed,
  refit = 0,
}: {
  project: string;
  /** the project's live agents (main among them) */
  agents: readonly Agent[];
  cards: readonly Card[];
  places: readonly Place[];
  /** his plugins' objects (plugin-objects.ts officeObjects): the office places them by corners (corners.ts); absent: none */
  objects?: readonly PluginObject[];
  /** his plugins rows (the store's usePlugins): a working bim walks to the object of the plugin its call uses (uses.ts); absent: no trips */
  plugins?: readonly Plugin[] | null;
  /** how much the project has done (hub/growth); the office draws its high-water mark (growth.ts) */
  growth?: Growth | null;
  /** the project's map (his /bims pick, prefs bims.map.<project>); the loft when he never picked one */
  map?: MapId;
  parts: Parts;
  /** the agent whose thread is open (focus.ts followOf): the office zooms on its bim and follows it */
  follow?: string | null;
  /** how close the follow camera looks (focus.ts aimAt): by default its desk's (focus.ts followZoomOf: FOLLOW_ZOOM, main's MAIN_FOLLOW_ZOOM) */
  followZoom?: number;
  /** the office fills its holder's height (a panel tab, app/panel/Office.tsx): the canvas is the holder's box, the office whole and centred in it; off, its natural height for the width */
  fill?: boolean;
  /** a click on a bim: open that agent, as its row's click */
  onOpen: (agent: string) => void;
  /** a click on an object's '?' / '✗' pill: open its plugin (its key); absent: the pill shows, no click */
  onOpenPlugin?: (key: string) => void;
  /** his pick in the map chip's menu (MapChip.tsx, bottom left); absent (the town): no chip */
  onPickMap?: (map: MapId) => void;
  /** a new project's welcome (welcome/Welcome.tsx): the 1, 2, 3 pins over the lead, an empty desk, the tray (pins.tsx) */
  pins?: boolean;
  /** the canvas clears to nothing: the page shows under the cube (the welcome in the thread's column) */
  transparent?: boolean;
  /** another element that takes his camera's input as the office does (the right side's grown world, app/side3d/World.tsx): a pinch, two fingers, a drag while zoomed; null: none */
  cameraFrom?: HTMLElement | null;
  /** its state as it changes: the welcome shows its pictures when the office can't draw */
  onState?: (s: OfficeState) => void;
  /** set while it draws: the office as drawn now onto a 2d canvas (the welcome's dock, renderer.copyTo) */
  snapRef?: { current: ((target: HTMLCanvasElement) => void) | null };
  /** set while it draws: a bim's head on the canvas now, css px from its top left, null off view (the right side's folded world puts a beat's label there, app/side3d/World.tsx; renderer.headOf) */
  headRef?: { current: ((name: string) => { readonly x: number; readonly y: number } | null) | null };
  /** his camera leaves the fit or comes back to it (the right side's world frames a cut map) */
  onZoomed?: (zoomed: boolean) => void;
  /** a change eases the camera back to the whole office (the right side's world folding back) */
  refit?: number;
}) {
  const [state, setState] = useState<OfficeState>('loading');
  useEffect(() => {
    onState?.(state);
  }, [state, onState]);
  const pinsRef = useRef<HTMLDivElement | null>(null);
  // the scene fades in once its first frame is drawn and the fold has grown (Fold.tsx)
  const [drawn, setDrawn] = useState(false);
  const [grown, setGrown] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => {
      setGrown(true);
    }, reducedMotion() ? 0 : FOLD_MS - 40);
    return () => {
      clearTimeout(t);
    };
  }, []);
  const [labels, setLabels] = useState<ReadonlyMap<string, Label>>(new Map());
  const holder = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const xfade = useRef<HTMLCanvasElement>(null);
  const tags = useRef(new Map<string, HTMLDivElement>());
  const leaders = useRef<SVGSVGElement>(null);
  const moreRef = useRef<HTMLDivElement>(null);
  // the two corner chips (the map bottom left, '⤢ fit' bottom right): no tag draws under them
  const mapChipRef = useRef<HTMLDivElement>(null);
  const fitRef = useRef<HTMLButtonElement>(null);
  // the sleepers' z's (zzz.ts): placed by hand at each draw, a CSS loop between draws
  const zzzRef = useRef<HTMLDivElement>(null);
  const fallRef = useRef<HTMLDivElement>(null);
  // each bim's place in line for a label's room this frame (tagRank)
  const ranks = useRef(new Map<string, number>());
  // the office's width, for the cut of a long line (whole words, a glued '…')
  const [boxW, setBoxW] = useState(0);
  const measure = useMemo(() => textWidth(labelFont()), []);
  const measureBold = useMemo(() => textWidth(labelFont('600 ')), []);
  const heads = useRef(new Map<string, { x: number; y: number; feet: number; edge?: 'left' | 'right' | 'up' | 'down' }>());
  const kickRef = useRef<() => void>(() => undefined);
  // the project the draw loop keeps the extensions for (a ref: a new project never rebuilds the renderer)
  const projectRef = useRef(project);
  useEffect(() => {
    projectRef.current = project;
  }, [project]);
  // the bim under a point of the canvas, css px: the renderer's once loaded
  const pick = useRef<(x: number, y: number) => string | null>(() => null);
  // the plugin object under a point of the canvas (its key), css px: the renderer's once loaded
  const objAt = useRef<(x: number, y: number) => string | null>(() => null);
  const held = useModHeld('Control');
  // his last keys, for the konami code (m1); the driver keeps when he typed it
  const keys = useRef<string[]>([]);
  // the office's brain (office-driver.ts): its timeline, eggs, growth and labels; the town drives its offices the same way
  const driver = useMemo(() => makeDriver(parts), [parts]);
  // his camera (zoom.ts): the view the renderer draws; `fitted` re-renders only for the '⤢ fit' button
  const camRef = useRef<Cam>(FIT);
  const [fitted, setFitted] = useState(true);
  const applyCam = useRef<(c: Cam) => void>(() => undefined);
  // the office's wheel (its loop's, set while the renderer lives): cameraFrom's wheel calls it too
  const wheelRef = useRef<(e: WheelEvent) => void>(() => undefined);
  // the follow camera: his own pinch, drag, esc or '⤢ fit' (moveCam) stops it until he opens another agent
  const followRef = useRef<Follow>({ target: follow === null ? null : { name: follow }, at: 0 });
  const moveCam = useCallback((c: Cam) => {
    followRef.current = { target: null, at: 0 };
    if (sameCam(c, camRef.current)) return;
    camRef.current = c;
    setFitted(atFit(c));
    applyCam.current(c);
  }, []);
  const live = useRef<Live>({ hover: null, held, lit: null, obj: null });
  useEffect(() => {
    const was = followRef.current.target;
    if (follow === (was === null || was === 'fit' ? null : was.name)) return;
    // another agent's thread (main's too): follow it; none (bise's, a screen): back to the whole office if it followed one
    followRef.current = { target: follow !== null ? { name: follow } : was === null ? null : 'fit', at: 0 };
    kickRef.current();
  }, [follow]);
  // back to the whole office when asked (the side's world folds): eased as the follow camera's 'fit'
  const refitRef = useRef(refit);
  useEffect(() => {
    if (refitRef.current === refit) return;
    refitRef.current = refit;
    followRef.current = { target: 'fit', at: 0 };
    kickRef.current();
  }, [refit]);
  useEffect(() => {
    onZoomed?.(!fitted);
  }, [fitted, onZoomed]);
  // how close it follows (followZoom, else by its desk: main's a little wider, focus.ts followZoomOf): a change eases there
  const zoom = followZoom ?? followZoomOf(agents.find((a) => a.name === follow));
  const zoomRef = useRef(zoom);
  useEffect(() => {
    if (zoomRef.current === zoom) return;
    zoomRef.current = zoom;
    kickRef.current();
  }, [zoom]);
  // the office grows (growth.ts): its high-water mark, the last growth drawn (an unlock is a stage
  // past it), the unlock moment, its label over the piece and the ground corner's tip
  // in test mode no kept mark: the fixture's growth, or a check's own (window.__biseGrow, read when
  // the `growth` prop changes) so a check can go back to a lower stage
  const high = useMemo(() => (testMode() ? ((window as { __biseGrow?: Growth }).__biseGrow ?? growth) : growHigh(project, growth)), [project, growth]);
  const growNow = useMemo(() => growOf(high), [high]);
  const [newTag, setNewTag] = useState<{ text: string; x: number; y: number } | null>(null);
  // his plugins' objects' '?' / '✗' (plugin-objects.ts pill): over each object, where it stands
  const [objPills, setObjPills] = useState<readonly ObjPill[]>([]);
  // the hovered object's name over its top (Gabriel m_51985); it fades in when shown cold (the ds tips' rule)
  const [objTip, setObjTip] = useState<ObjTip | null>(null);
  const [objFade, setObjFade] = useState(false);
  const objTipRef = useRef<HTMLDivElement>(null);
  // the tip shown, for the frame loop's placeTipEl (set once it is drawn)
  const objTipNow = useRef<ObjTip | null>(null);
  // every bim's head and body box as drawn now (the frame loop's), for the object's tip to keep clear of
  const bimRects = useRef<readonly Rect[]>([]);
  // over the object, under its foot, or beside it, clear of the bims and the tile's chips; no room: none (objpills.ts placeObjTip)
  const placeTipEl = useCallback(() => {
    const el = objTipRef.current;
    const box = holder.current;
    const t = objTipNow.current;
    if (el === null || box === null || t === null) return;
    const at0 = box.getBoundingClientRect();
    // the tile's chips: the map chip and '⤢ fit' here, and what the tile lays over the office (data-bims-keep: its '⤢' and '×')
    const chips: Rect[] = [];
    for (const c of [mapChipRef.current, fitRef.current, ...document.querySelectorAll<HTMLElement>('[data-bims-keep]')]) {
      if (c === null) continue;
      const r = c.getBoundingClientRect();
      if (r.width > 0) chips.push({ left: r.left - at0.left, top: r.top - at0.top, right: r.right - at0.left, bottom: r.bottom - at0.top });
    }
    const at = placeObjTip(t, { w: el.offsetWidth, h: el.offsetHeight }, { w: box.clientWidth, h: box.clientHeight }, { bims: bimRects.current, chips });
    if (at === null) {
      el.style.visibility = 'hidden';
      return;
    }
    el.style.left = `${String(at.left)}px`;
    el.style.top = `${String(at.top)}px`;
    el.dataset.side = at.side;
    el.style.visibility = 'visible';
  }, []);
  useLayoutEffect(() => {
    objTipNow.current = objTip;
    placeTipEl();
  }, [placeTipEl, objTip]);
  // the growth drawn: before the rows, so the first one is no unlock moment (no mark yet: a bare room)
  useEffect(() => {
    if (high === null) return;
    driver.grow(growNow, Date.now() / 1000);
    kickRef.current();
  }, [driver, high, growNow]);

  // a fresh snapshot (the store gives new arrays only on a change) or his /bims pick (the office
  // crossfades to the new map at the next frame): the driver hears of it
  useEffect(() => {
    driver.rows({ agents, cards, places }, agents, map);
    kickRef.current();
  }, [driver, agents, cards, places, map]);
  // his plugins' objects: the next frame places and draws them (no desk moves for them)
  useEffect(() => {
    driver.objects(objects);
    kickRef.current();
  }, [driver, objects]);
  // his plugins rows: a working bim's call to one of them walks it to that plugin's object
  useEffect(() => {
    driver.plugins(plugins);
    kickRef.current();
  }, [driver, plugins]);
  // the extensions this project's map already had (ext-keep.ts): they come back, never go
  useEffect(() => {
    driver.seedNeed(map, extKept(project, map));
    kickRef.current();
  }, [driver, project, map]);
  useEffect(() => {
    live.current.held = held;
    kickRef.current();
  }, [held]);
  // hub/activity: his message is a plane to its bim, an agent's to another a visit, a report a
  // sheet to main's desk (events.ts)
  useOfficeEvents(project, (e) => {
    driver.event(e);
    kickRef.current();
  });

  // every tag over its bim, inside the office, none over another (tags.ts placeTags); a moved one
  // gets its 1 px leader, one with no room is left out and counted in the corner while ctrl is held
  // (designer m_27799). Room goes first to the hovered one, then to bims with a pill, then to
  // working ones, then to the quiet ones (tagRank), the labels' own order among equals.
  const layoutTags = useCallback(() => {
    const box = holder.current;
    if (box === null) return;
    const ins: TagIn[] = [];
    for (const [name, el] of tags.current) {
      const h = heads.current.get(name);
      el.style.visibility = h === undefined ? 'hidden' : '';
      if (h === undefined) continue;
      if ((el.dataset.edge ?? '') !== (h.edge ?? '')) {
        if (h.edge === undefined) delete el.dataset.edge;
        else el.dataset.edge = h.edge;
      }
      const w = el.offsetWidth;
      const th = el.offsetHeight;
      if (el.dataset.mode === 'hover') {
        // the card over its head, under its feet, or in the top-left corner when the bim fills the
        // office (tags.ts hoverCardAt); placed with the pills, first, never moved
        const c = hoverCardAt(h, w, th, box.clientHeight);
        ins.push({ name, x: c.x, y: c.y, w, h: th, pill: true, rank: 0 });
        continue;
      }
      const rank = ranks.current.get(name) ?? 3;
      ins.push({ name, x: h.x, y: h.y, w, h: th, pill: el.dataset.mode === 'pill', rank });
    }
    const at0 = box.getBoundingClientRect();
    const keep: KeepRect[] = [];
    for (const el of [mapChipRef.current, fitRef.current]) {
      if (el === null) continue;
      const r = el.getBoundingClientRect();
      if (r.width > 0) keep.push({ left: r.left - at0.left, top: r.top - at0.top, w: r.width, h: r.height });
    }
    const { at, left } = placeTags(ins, box.clientWidth, box.clientHeight, keep);
    const more = left > 0 && live.current.held ? bims.held_left_out({ n: left }) : '';
    if (moreRef.current !== null && moreRef.current.textContent !== more) moreRef.current.textContent = more;
    let lines = '';
    for (const t of ins) {
      const p = at.get(t.name);
      const el = tags.current.get(t.name);
      if (el === undefined) continue;
      el.style.visibility = p === undefined ? 'hidden' : '';
      if (p === undefined) continue;
      el.style.transform = `translate(${p.left.toFixed(1)}px, ${p.top.toFixed(1)}px)`;
      if (!p.lead) continue;
      const lx = Math.min(Math.max(t.x, p.left), p.left + t.w);
      const ly = p.top + t.h <= t.y ? p.top + t.h : p.top;
      lines += `<line x1="${lx.toFixed(1)}" y1="${ly.toFixed(1)}" x2="${t.x.toFixed(1)}" y2="${t.y.toFixed(1)}"/>`;
    }
    if (leaders.current !== null && leaders.current.innerHTML !== lines) leaders.current.innerHTML = lines;
  }, []);
  // '⤢ fit' shows or goes after a frame's layout (the follow camera's last step sets it): the tags
  // again at once, clear of it, even if no frame follows (designer m_35455)
  useLayoutEffect(layoutTags, [layoutTags, fitted]);

  useEffect(() => {
    const cv = canvas.current;
    const box = holder.current;
    if (cv === null || box === null) return undefined;
    let gone = false;
    let r: OfficeRenderer | null = null;
    // the renderer is loaded; the map last drawn (another one crossfades)
    let started = false;
    let drawnMap: OfficeMap['map'] | null = null;
    /** the objects' spots the renderer has (the driver's Step.corners: a new value when they change) */
    let drawnCorners: Corners | null = null;
    let lost = false;
    let raf = 0;
    let tick: ReturnType<typeof setTimeout> | undefined;
    let lastLabels = '';
    let lastNew = 'null';
    let lastPills = '[]';
    let lastTip = 'null';
    let lastGrown = '';
    let reduced = reducedMotion();
    let firstDrawn = false;
    /** his last wheel (a pinch, a two-finger pan), ms: a sky blend waits HOLD_MS past it */
    let lastWheel = 0;
    let lastFall = '';

    const cancel = () => {
      if (raf !== 0) cancelAnimationFrame(raf);
      raf = 0;
      if (tick !== undefined) clearTimeout(tick);
      tick = undefined;
    };
    const fail = () => {
      lost = true;
      cancel();
      if (!gone) setState('lost');
      bimsView.setLost(true);
    };
    const resize = () => {
      if (r === null) return;
      const w = box.clientWidth;
      if (w <= 0) return;
      // filling: the box's own height (100% of its holder, office.css), the canvas that whole box
      const full = fill ? box.clientHeight : 0;
      const h = full > 0 ? r.setSize(w, full, true) : r.setSize(w, 4000);
      if (!fill) box.style.height = `${String(Math.round(h))}px`;
      if (h > 0) lastRatio = w / h;
      setBoxW(w);
    };
    // the clock a frame is drawn at; in test mode only a check's forced eggs (window.__biseEggs), never a
    // random roll, and its pinned calendar day (window.__biseLocal: a check never meets the Sunday pool)
    const clockNow = (t: number): Clock => ({
      t,
      local: { ...localOf(new Date()), ...(testMode() ? (window as { __biseLocal?: Partial<Local> }).__biseLocal : undefined) },
      night: nightNow(),
      reduced,
      ...(testMode() ? { forced: (window as { __biseEggs?: Forced }).__biseEggs ?? null, ...pinnedSky() } : {}),
    });
    const draw = () => {
      raf = 0;
      tick = undefined;
      if (r === null || !started || gone || lost) return;
      const L = live.current;
      const t = Date.now() / 1000;
      const ft = followRef.current.target;
      const fol = ft === null || ft === 'fit' ? null : ft.name;
      const step = driver.step(clockNow(t), { hover: L.hover, held: L.held, lit: L.lit, follow: fol });
      if (step === null) return;
      const { office, frames, played, look, labels: shown } = step;
      if (step.reshaped) {
        // another map: the old office fades out over the new one; reduced motion: a cut
        const fx = xfade.current;
        if (drawnMap !== null && drawnMap !== office.map && !reduced && fx !== null) crossfade(r, cv, fx);
        drawnMap = office.map;
        // what he picked (data-want) and what is drawn (data-map: the loft for a map not ready yet)
        if (holder.current !== null) Object.assign(holder.current.dataset, { want: step.want, map: office.map });
        r.setOffice(office, step.agents);
        resize();
      }
      // his plugins' objects: drawn again only when their spots changed (another office, a plugin more)
      if (step.corners !== drawnCorners) {
        drawnCorners = step.corners;
        // drawn at once when their chunk is in, else a frame once it loads (a still office draws none meanwhile)
        void r.setObjects(step.corners).then((late) => {
          if (late) kickRef.current();
        });
      }
      // his drag or pinch: a sky blend due now waits until he lets go (bims m_33030)
      const hold = holder.current?.hasAttribute('data-drag') === true || Date.now() - lastWheel < HOLD_MS;
      const motion = r.frame(t, played, hold ? { ...look, hold } : look);
      if (fallRef.current !== null) {
        const fh = holder.current;
        const fl = fallingLook(r.falling(), fh?.clientWidth ?? 0, fh?.clientHeight ?? 0);
        const sig = fallingSig(fl);
        if (sig !== lastFall) {
          lastFall = sig;
          placeFalling(fallRef.current, fl);
        }
      }
      if (!firstDrawn) {
        firstDrawn = true;
        setDrawn(true);
      }
      // the follow camera, from where the bims stand now: the next frame draws the new view
      const chasing = chase(r, t, reduced);
      const u = look.unlock;
      const said = unlockLabel(u, t, reduced, planOf(mapOf(office.map)).things);
      const at = said !== null && u ? r.pieceOf(u.stage) : null;
      const tag = said !== null && at !== null ? { text: said, x: Math.round(at.x), y: Math.round(at.y) } : null;
      if (JSON.stringify(tag) !== lastNew) {
        lastNew = JSON.stringify(tag);
        setNewTag(tag);
      }
      const pills = objPillsOf(step.corners.placed, (key) => r?.objectTopOf(key) ?? null);
      const pillSig = JSON.stringify(pills);
      if (pillSig !== lastPills) {
        lastPills = pillSig;
        setObjPills(pills);
      }
      const tip = objTipOf(step.corners.placed, L.obj, (key) => {
        const top = r?.objectTopOf(key) ?? null;
        const box = r?.objectBoxOf(key) ?? null;
        return top === null || box === null ? null : { ...top, box };
      });
      const tipSig = JSON.stringify(tip);
      if (tipSig !== lastTip) {
        lastTip = tipSig;
        setObjTip(tip);
      }
      ranks.current = new Map(frames.map((f) => [f.name, tagRank(f.pose.seg.act.k)]));
      heads.current = new Map();
      const box = holder.current;
      for (const [name, l] of shown) {
        const h = r.headOf(name);
        if (h !== null) heads.current.set(name, h);
        else if ((l.pill !== null || l.mode === 'hover') && box !== null) {
          // a '?' / '✗' whose bim is out of the frame stays at its nearest edge, an arrow towards it (bims m_28199);
          // the hover card too, so its head past the top (a close zoom) sends it to the corner (tags.ts hoverCardAt)
          const s = r.spotOf(name);
          if (s === null) continue;
          const e = edgeOf(s.x, s.y, box.clientWidth, box.clientHeight, TAG_INSET);
          heads.current.set(name, { x: e.x, y: e.y, feet: e.y, ...(e.side === null ? {} : { edge: e.side }) });
        }
      }
      layoutTags();
      // the bims walk: the object's tip moves off a bim that comes into its place
      bimRects.current = [
        ...frames.flatMap((f) => {
          const h = r?.headOf(f.name) ?? null;
          return h === null ? [] : [bimBox(h)];
        }),
        // their desks too: one in front of an object hides its edge (ui-review m_52091)
        ...r.deskSpots().map(deskBox),
      ];
      placeTipEl();
      if (pinsRef.current !== null) placePins(pinsRef.current, pinPoints(r, step.agents.find((a) => a.main)?.name ?? null));
      if (zzzRef.current !== null) {
        const zs = new Map<string, { x: number; y: number }>();
        for (const name of sleepers(frames)) {
          const h = r.headOf(name);
          if (h !== null) zs.set(name, h);
        }
        placeZzz(zzzRef.current, zs);
      }
      // the extensions come (extensions.ts): kept per project and map, so a reload never takes one away
      if (step.grown !== null && JSON.stringify(step.grown) !== lastGrown) {
        lastGrown = JSON.stringify(step.grown);
        if (!testMode()) keepExt(projectRef.current, step.grown.map, step.grown.need);
      }
      const sig = labelsSig(shown);
      if (sig !== lastLabels) {
        lastLabels = sig;
        setLabels(shown);
      }
      // a resting skater or climber (pastimes.ts): no frame during its rest, a timer for its next line;
      // a resting bim's next idle move (render/idle.ts) the same way: no frame between its moves
      const next = chasing && !awayNow() ? 0 : nextAfter({ motion, moving: step.moving, labels: shown, away: awayNow(), lost, wake: soonest(soonest(pastimeWake(frames, office), step.wake), r.idleWake()) });
      if (next === 0) raf = requestAnimationFrame(draw);
      else if (next !== null) tick = setTimeout(draw, next);
    };
    // one step of the follow camera (focus.ts): ease towards its bim (or back to the fit); true while it moves
    const chase = (rr: OfficeRenderer, t: number, still: boolean): boolean => {
      const F = followRef.current;
      if (F.target === null) return false;
      let to: Cam | null = FIT;
      if (F.target !== 'fit') {
        const s = rr.spotOf(F.target.name);
        to = s === null ? null : aimAt(s.u, s.v, zoomRef.current);
      }
      if (to === null) return false;
      const dt = F.at === 0 ? 1 / 60 : Math.min(0.1, t - F.at);
      F.at = t;
      const c = easeCam(camRef.current, to, dt, still);
      if (F.target === 'fit' && sameCam(c, FIT)) followRef.current = { target: null, at: 0 };
      if (sameCam(c, camRef.current)) return false;
      camRef.current = c;
      rr.setCam(c);
      setFitted(atFit(c));
      return true;
    };
    const kick = () => {
      if (r === null || lost || gone || awayNow()) return;
      if (raf !== 0) return;
      if (tick !== undefined) clearTimeout(tick);
      tick = undefined;
      raf = requestAnimationFrame(draw);
    };
    kickRef.current = kick;
    applyCam.current = (c) => {
      r?.setCam(c);
      kick();
    };

    // a pinch (Chromium: a wheel with ctrlKey) zooms around the pointer; two fingers move the view
    // while zoomed, and scroll the bar at the fit (nothing to move there)
    const onWheel = (e: WheelEvent) => {
      lastWheel = Date.now();
      const at = box.getBoundingClientRect();
      const c = wheelCam(camRef.current, { dx: e.deltaX, dy: e.deltaY, mode: e.deltaMode, ctrl: e.ctrlKey }, e.clientX - at.left, e.clientY - at.top, at.width, at.height, box.clientHeight);
      if (c === null) return;
      e.preventDefault();
      moveCam(c);
    };
    box.addEventListener('wheel', onWheel, { passive: false });
    wheelRef.current = onWheel;

    // the window away: nothing runs; back: one frame, and the loop if it moves
    // the z's' CSS loop (zzz.ts) pauses on the same flag (architect m_28263)
    const onWindow = () => {
      const isAway = awayNow();
      box.toggleAttribute('data-away', isAway);
      if (isAway) cancel();
      else kick();
    };
    box.toggleAttribute('data-away', awayNow());
    document.addEventListener('visibilitychange', onWindow);
    window.addEventListener('focus', onWindow);
    window.addEventListener('blur', onWindow);
    const motionMq = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
    const schemeMq = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: light)') : null;
    const onLook = () => {
      reduced = reducedMotion();
      kick();
    };
    motionMq?.addEventListener('change', onLook);
    schemeMq?.addEventListener('change', onLook);
    const theme = new MutationObserver(onLook);
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    const sized = new ResizeObserver(() => {
      resize();
      kick();
    });
    sized.observe(box);
    // a still office looks at its rows' clock now and then (a long turn going hot, a trip to the
    // island): it draws only when an activity changed
    const check = setInterval(() => {
      if (r === null || !started || awayNow() || lost) return;
      // the clock's eggs (the eclipse, friday 17:00, midnight) or an activity changed: a frame
      if (driver.stale(clockNow(Date.now() / 1000))) kick();
    }, CHECK_MS);

    parts
      .load()
      .then((make) => {
        if (gone) return;
        const made = make(cv, transparent ? { transparent } : undefined);
        r = made;
        made.onLost(fail);
        pick.current = (x, y) => (lost ? null : made.pick(x, y));
        objAt.current = (x, y) => (lost ? null : made.objectAt(x, y));
        if (snapRef !== undefined)
          snapRef.current = (target) => {
            made.copyTo(target);
          };
        if (headRef !== undefined) headRef.current = (name) => (lost ? null : made.headOf(name));
        made.setCam(camRef.current);
        started = true;
        setState('on');
        bimsView.setLost(false);
        resize();
        kick();
      })
      .catch(fail);

    return () => {
      gone = true;
      cancel();
      clearInterval(check);
      document.removeEventListener('visibilitychange', onWindow);
      window.removeEventListener('focus', onWindow);
      window.removeEventListener('blur', onWindow);
      motionMq?.removeEventListener('change', onLook);
      schemeMq?.removeEventListener('change', onLook);
      theme.disconnect();
      sized.disconnect();
      box.removeEventListener('wheel', onWheel);
      wheelRef.current = () => undefined;
      kickRef.current = () => undefined;
      applyCam.current = () => undefined;
      pick.current = () => null;
      objAt.current = () => null;
      if (snapRef !== undefined) snapRef.current = null;
      if (headRef !== undefined) headRef.current = null;
      r?.dispose();
    };
  }, [parts, driver, layoutTags, placeTipEl, moveCam, snapRef, headRef, transparent, fill]);

  // hover: the bim under the pointer, its line after the tip's delay; off it, at once
  const hoverTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pending = useRef<string | null>(null);
  const [pointer, setPointer] = useState(false);
  useEffect(
    () => () => {
      clearTimeout(hoverTimer.current);
    },
    [],
  );
  // a card that just closed keeps the next one warm: it shows at once (the ds tips' rule)
  const warmUntil = useRef(0);
  const setHover = (name: string | null) => {
    if (live.current.hover === name) return;
    if (name === null && live.current.hover !== null) warmUntil.current = Date.now() + WARM_MS;
    live.current.hover = name;
    kickRef.current();
  };
  // the plugin object under the pointer (no bim there): its name's tip after the tip's delay, at
  // once while warm (one card or tip warms the next), off it at once (Gabriel m_51985)
  const objTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const objPending = useRef<string | null>(null);
  useEffect(
    () => () => {
      clearTimeout(objTimer.current);
    },
    [],
  );
  const setObj = (key: string | null, fade: boolean) => {
    if (live.current.obj === key) return;
    if (key === null && live.current.obj !== null) warmUntil.current = Date.now() + WARM_MS;
    setObjFade(fade);
    live.current.obj = key;
    kickRef.current();
  };
  const hoverObj = (key: string | null) => {
    if (key === objPending.current) return;
    objPending.current = key;
    clearTimeout(objTimer.current);
    if (key === null) setObj(null, false);
    else if (Date.now() < warmUntil.current || live.current.obj !== null) setObj(key, false);
    else
      objTimer.current = setTimeout(() => {
        setObj(key, true);
      }, HOVER_MS);
  };
  const objEvent = (e: { clientX: number; clientY: number }): string | null => {
    const box = holder.current;
    if (box === null) return null;
    const at = box.getBoundingClientRect();
    return objAt.current(e.clientX - at.left, e.clientY - at.top);
  };
  // a one-finger (or mouse) drag moves the view while zoomed; past DRAG_PX it is never a click
  const drag = useRef<{ id: number; x: number; y: number; moved: boolean } | null>(null);
  const dragged = useRef(false);
  const [dragging, setDragging] = useState(false);
  // his pointer on an AGENTS row (focus.ts): the halo under its bim, never its card (Gabriel
  // m_48440, designer m_48444: the card over the tile hid the very bim the row points at; the row
  // already says its name). The card comes only from a hover on the bim itself.
  const lit = useBimsLit();
  const rowLit = lit?.from === 'row' ? lit.name : null;
  useEffect(() => {
    live.current.lit = rowLit;
    kickRef.current();
  }, [rowLit]);
  /** a drag's start (a press while zoomed); the holder leaves out its buttons and pills */
  const dragDown = (e: { readonly button: number; readonly pointerId: number; readonly clientX: number; readonly clientY: number }) => {
    if (e.button !== 0 || atFit(camRef.current)) return;
    drag.current = { id: e.pointerId, x: e.clientX, y: e.clientY, moved: false };
    dragged.current = false;
  };
  /** a drag's step past DRAG_PX: the view moves by the pointer's move (the holder's size), `el` keeps the pointer; false when no drag goes on */
  const dragMove = (e: { readonly pointerId: number; readonly clientX: number; readonly clientY: number }, el: Element): boolean => {
    const d = drag.current;
    const box = holder.current;
    if (d === null || d.id !== e.pointerId || box === null) return false;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (!d.moved && Math.hypot(dx, dy) < DRAG_PX) return true;
    if (!d.moved) {
      d.moved = true;
      el.setPointerCapture(e.pointerId);
      setDragging(true);
      pending.current = null;
      clearTimeout(hoverTimer.current);
      setHover(null);
      hoverObj(null);
    }
    d.x = e.clientX;
    d.y = e.clientY;
    const at = box.getBoundingClientRect();
    moveCam(panBy(camRef.current, dx, dy, at.width, at.height));
    return true;
  };
  const dragUp = () => {
    if (drag.current?.moved === true) dragged.current = true;
    drag.current = null;
    setDragging(false);
  };
  // his camera from another element too (cameraFrom: the right side's grown world, its hit span
  // over the island): the same wheel (a pinch zooms, two fingers move) and the same drag as on
  // the office; a click that ended a drag goes no further (it never opens what is under it)
  const camIn = useRef({ down: dragDown, move: dragMove, up: dragUp });
  useLayoutEffect(() => {
    camIn.current = { down: dragDown, move: dragMove, up: dragUp };
  });
  useEffect(() => {
    const el = cameraFrom;
    if (el == null) return;
    const wheel = (e: WheelEvent) => {
      wheelRef.current(e);
    };
    const down = (e: PointerEvent) => {
      camIn.current.down(e);
    };
    const move = (e: PointerEvent) => {
      camIn.current.move(e, el);
    };
    const up = () => {
      camIn.current.up();
    };
    const click = (e: MouseEvent) => {
      if (!dragged.current) return;
      dragged.current = false;
      e.stopPropagation();
      e.preventDefault();
    };
    el.addEventListener('wheel', wheel, { passive: false });
    el.addEventListener('pointerdown', down);
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
    el.addEventListener('click', click, true);
    return () => {
      el.removeEventListener('wheel', wheel);
      el.removeEventListener('pointerdown', down);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      el.removeEventListener('click', click, true);
    };
  }, [cameraFrom]);
  const pickEvent = (e: { clientX: number; clientY: number }): string | null => {
    const box = holder.current;
    if (box === null) return null;
    const at = box.getBoundingClientRect();
    return pick.current(e.clientX - at.left, e.clientY - at.top);
  };

  if (state === 'lost')
    // its own element: React would reuse the holder's div here, the height resize() set on it by
    // hand included (a 212 px line, bims-qa m_27786); the rows show under it as always
    return (
      <div key="lost" className="bims-lost" data-testid="app.right.bims.lost">
        {bims.lost}
      </div>
    );
  const cardW = Math.min(CARD_W, boxW - 2 * TAG_INSET);
  return (
    <div
      key="office"
      className="bims-office"
      ref={holder}
      // esc goes back to the fit while the office has focus (a click gives it)
      tabIndex={0}
      aria-label={bims.view_bims_tip}
      data-testid="app.right.bims"
      data-state={state}
      data-fill={fill || undefined}
      data-shown={(drawn && grown) || undefined}
      style={state === 'loading' ? ({ '--bims-ratio': String(lastRatio) } as CSSProperties) : undefined}
      data-pointer={pointer || undefined}
      data-zoomed={!fitted || undefined}
      data-stage={growNow.stage}
      data-drag={dragging || undefined}
      onKeyDown={(e) => {
        keys.current = [...keys.current.slice(-9), e.key];
        if (konamiDone(keys.current)) {
          keys.current = [];
          driver.konami(Date.now() / 1000);
          kickRef.current();
        }
        if (e.key !== 'Escape' || fitted) return;
        e.stopPropagation();
        moveCam(FIT);
      }}
      onPointerDown={(e) => {
        if ((e.target as Element).closest('button, .bims-pill') !== null) return;
        dragDown(e);
      }}
      onPointerUp={dragUp}
      onPointerCancel={() => {
        drag.current = null;
        setDragging(false);
      }}
      onDoubleClick={(e) => {
        // on the floor only: a double-click on a bim already opened it (designer m_27920)
        if (pickEvent(e) === null) moveCam(FIT);
      }}
      onPointerMove={(e) => {
        if (dragMove(e, e.currentTarget)) return;
        const name = pickEvent(e);
        setPointer(name !== null);
        // a bim goes first: over one, no object's tip; on an object's pill, the pill's own tip alone (designer m_51998)
        const onPill = (e.target as Element).closest('.bims-objpill') !== null;
        hoverObj(name === null && !onPill ? objEvent(e) : null);
        if (name === pending.current) return;
        pending.current = name;
        clearTimeout(hoverTimer.current);
        bimsLit.set(name === null ? null : { name, from: 'bim' }, 'bim');
        if (name === null) setHover(null);
        else if (Date.now() < warmUntil.current) setHover(name);
        else
          hoverTimer.current = setTimeout(() => {
            setHover(name);
          }, HOVER_MS);
      }}
      onPointerLeave={() => {
        bimsLit.set(null, 'bim');
        pending.current = null;
        clearTimeout(hoverTimer.current);
        setPointer(false);
        setHover(null);
        hoverObj(null);
      }}
      onClick={(e) => {
        if (dragged.current) {
          dragged.current = false;
          return;
        }
        const name = pickEvent(e);
        if (name === null) return;
        // his poke (pokes.ts): the first click opens the agent too, the poke lands as the camera arrives
        driver.poke(name, Date.now() / 1000 + (name === follow ? 0 : OPEN_DELAY_S));
        onOpen(name);
        kickRef.current();
      }}
    >
      <canvas
        ref={canvas}
        // the garden past what the camera fits is cropped; a soft fade at the sides says there is more.
        // Zoomed in, the lawn fills the view: no fade (in dark it read as a black vignette, designer m_47300)
        className={`bims-canvas${fitted && boxW > 0 && cropped(growNow, boxW >= WIDE_FIT_PX) ? ' bims-fade' : ''}`}
        data-testid="app.right.bims.canvas"
      />
      <canvas ref={xfade} className="bims-xfade" aria-hidden="true" data-testid="app.right.bims.xfade" />
      {/* the rain or snow outside the building (falling.ts): a CSS loop, never a WebGL frame */}
      <div ref={fallRef} className="bims-falling" aria-hidden="true" data-testid="app.right.bims.falling" />
      <div className="bims-overlay">
        <svg ref={leaders} className="bims-leaders" aria-hidden="true" />
        <div ref={zzzRef} className="bims-zzzs" data-testid="app.right.bims.zzzs" />
        {pins && (
          <Pins
            boxRef={(el) => {
              pinsRef.current = el;
              kickRef.current();
            }}
          />
        )}
        <div ref={moreRef} className="bims-more" data-testid="app.right.bims.more" />
        {/* the ground's corner: hover says the level, the jobs done, the next piece (the map's own) */}
        <span className="bims-level" data-testid="app.right.bims.level" {...tipAttrs(tipOf(growNow, planOf(map).things))} />
        {objPills.map((p) => (
          <span
            key={p.key}
            className={`bims-pill bims-objpill ${p.pill === '?' ? 'asks' : 'failed'}`}
            role={onOpenPlugin === undefined ? undefined : 'button'}
            data-pill={p.pill === '?' ? 'asks' : 'failed'}
            data-testid={`app.right.bims.objpill.${p.key}`}
            {...tipAttrs(p.tip)}
            style={{ left: `${String(p.x)}px`, top: `${String(p.y)}px` }}
            onClick={(e) => {
              e.stopPropagation();
              onOpenPlugin?.(p.key);
            }}
          >
            {p.pill}
          </span>
        ))}
        {/* the hovered object's plugins, the ds's tip (.b-tip) over its top; placeTipEl places it once its size is known */}
        {objTip !== null && (
          <div
            ref={objTipRef}
            className={`b-tip bims-objtip${objFade ? ' in' : ''}`}
            role="tooltip"
            data-testid="app.right.bims.objtip"
            data-key={objTip.key}
            style={{ visibility: 'hidden' }}
          >
            <span className="w">{objTip.words}</span>
          </div>
        )}
        {newTag !== null && (
          <div className="bims-label bims-new" data-mode="event" style={{ left: `${String(newTag.x)}px`, top: `${String(newTag.y)}px` }} data-testid="app.right.bims.new">
            {newTag.text}
          </div>
        )}
        {/* the map chip, bottom left, a project's office only (the town passes no onPickMap) */}
        {onPickMap !== undefined && (
          <div ref={mapChipRef} className="bims-mapsel" data-testid="app.right.bims.mapsel">
            <MapChip map={map} onPick={onPickMap} />
          </div>
        )}
        {/* '⤢ fit', bottom right, while zoomed or following */}
        {!fitted && (
          <FitChip
            ref={fitRef}
            onFit={() => {
              moveCam(FIT);
            }}
          />
        )}
        {[...labels.values()].map((l) => {
          const tagRef = (el: HTMLDivElement | null) => {
            if (el === null) tags.current.delete(l.name);
            else {
              tags.current.set(l.name, el);
              layoutTags();
            }
          };
          if (l.mode === 'hover')
            return <HoverCard key={l.name} label={l} width={cardW} measure={measure} measureBold={measureBold} tagRef={tagRef} onOpen={onOpen} />;
          const tag = tagOf(l);
          return (
            <div key={l.name} className="bims-tag" data-mode={l.mode} ref={tagRef}>
              {tag.text !== null && (
                <NameLabel name={l.name} mode={l.mode}>
                  {boxW > 0 ? cutWords(tag.text, boxW - 2 * TAG_INSET - CHIP_PAD - (tag.pill === null ? 0 : PILL_ROOM), measure) : tag.text}
                </NameLabel>
              )}
              {tag.pill !== null && <Pill name={l.name} pill={tag.pill} onOpen={onOpen} />}
            </div>
          );
        })}
      </div>
    </div>
  );
}


/**
 * The hover card (Gabriel: multi-line, easy to read; designer m_27920): its name in 600 with its
 * pill, its state dim under it, its title on at most CARD_LINES lines cut on a whole word with one
 * glued '…' (tags.ts cutLines, measured, so the lines are ours, not the browser's). No accent but
 * the pill's. The office's layoutTags places it: over the head, under the feet, or in the corner (tags.ts hoverCardAt).
 */
function HoverCard({ label, width, measure, measureBold, tagRef, onOpen }: {
  label: Label;
  width: number;
  measure: (s: string) => number;
  measureBold: (s: string) => number;
  tagRef: (el: HTMLDivElement | null) => void;
  onOpen: (agent: string) => void;
}) {
  const room = width - 2 * CARD_PAD;
  // main's card says 'bise', as its row (designer m_28987); its test id keeps the agent's name
  const shown = shownName(label.name);
  const name = room > 0 ? cutWords(shown, room - (label.pill === null ? 0 : PILL_ROOM), measureBold) : shown;
  const state = room > 0 ? cutWords(label.words, room, measure) : label.words;
  const title = cutLines(label.title, room, CARD_LINES, measure);
  return (
    <div className="bims-card" data-mode="hover" ref={tagRef} style={width > 0 ? { maxWidth: `${String(width)}px` } : undefined} data-testid={`app.right.bims.label.${label.name}`}>
      <div className="bims-card-head">
        <span className="bims-card-name">{name}</span>
        {label.pill !== null && <Pill name={label.name} pill={label.pill} onOpen={onOpen} />}
      </div>
      {state !== '' && (
        <div className="bims-card-state" data-testid={`app.right.bims.card.state.${label.name}`}>
          {state}
        </div>
      )}
      {title.map((line, i) => (
        <div key={i} data-testid={i === 0 ? `app.right.bims.card.title.${label.name}` : undefined}>
          {line}
        </div>
      ))}
    </div>
  );
}
