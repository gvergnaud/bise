// One office as a scene (page town-live p2, architect m_28994): its world (world.ts, its map's look
// and growth), one bim per BimFrame posed by pose.ts (bim.ts), his paper planes (plane.ts), the
// ring under a lit bim (halo.ts), the office's eggs (eggdecor.ts) and the unlock moment's crew (unlock.ts),
// all in one group in the office's own units. No renderer, no camera, no light: the minimap
// (render/index.ts) draws it through its view, the town places one per project. frame() poses it
// for time t and says how the loop must run next (Motion); the caller renders. The sky's lamps and
// snow cover come from its caller (weather(): sky.ts through render/index.ts or town.ts). Owner: bims-wire.
import { Color, Group, Matrix4, Mesh, MeshBasicMaterial, RingGeometry, Vector3, type ColorRepresentation, type Material, type Object3D } from 'three';
import type { Agent } from '@/proto';
import type { BimFrame, Look, Motion, Office, Pt } from '../model';
import { makeBim, nearAt, type Bim } from './bim';
import { pillOf } from '../labels';
import { siteOf, toScreen, type Fit, type V3 } from './camera';
import { cameraStart, viewPoint, flightAt, glideAt, makePlane, planFlight, planGlide, shadowOf, sizeAt, startSize, type FlightFrame, type FlightPlan, type GlidePlan, type Plane } from './plane';
import { DESK_SPOT, HELD_LEN, ON_DESK, PLANE_CATCH, POCKET_SINK, REACH_S, awayOf, heldAt } from './catch';
import { PLANE_FLIGHT, louder, rigOf, type Rig } from './pose';
import { BURST_PX, idleFrame, idleRig } from './idle';
import { aliveOf, newCrowd } from './alive';
import { restAt, type NapRest } from './nap';
import { skateRig } from './skate';
import { climbRig } from './climb';
import { eggRig } from './egg';
import { makeEggDecor, type EggDecor } from './eggdecor';
import { MAIN_HOODIE, WORLD, hash, outfitOf, paletteOf, type HerHair, type Outfit, type Palette } from './palette';
import { type Look as MyLook, mainOutfit, sameLook } from './look';
import { buildWorld, castShadows, lightDesk, type DeskHandles, type World } from './world';
import { NO_GROW, growSig, type Grow } from '../growth';
import { cozyAt } from '../cozy';
import { lookOf } from './maps';
import type { MapLook } from './maps/look';
import { makeUnlock } from './unlock';
import { seatChair } from './furniture';
import { POOF_S, SPARK, sparkSpread } from './poof';
import { makePoofs, type Poof } from './poofs';
import type { Style } from './style';
import type { ObjectsLayer } from './objects-layer';
import { NO_CORNERS, type Corners } from '../corners';
import { HALO, ROW_RING, ringOf } from './halo';

/** the length on screen (css px) the loop and the zigzag are sized for, its office length for that; the shadow's own length (plane.ts) */
const LOOP_PX = 12;
const LOOP_LEN = 0.42;
const SHADOW_LEN = 0.42;
/** a gliding plane's least shadow alpha (plane.ts SHADOW_ALPHA is 0.2 on the floor) */
const GLIDE_SHADOW = 0.09;
/** a bim's pill, office units up (over its head: its loop keeps clear of it); the plane's shadow just over the floor */
const PILL_Y = 1.7;
const FLOOR_LIFT = 0.02;
const PLANE_FORESHORTEN = 1.25;
/** three's layer the hero's glides fly in when Look.planesUp (town.ts draws it over the page: TownOpts.planesOver) */
export const UP_LAYER = 1;
/**
 * a glide leaving the hero's send button: its world length per css px of its dart (bims m_36638: it
 * takes over at the dart's own size). Measured on the hero: PLANE_FORESHORTEN's 1.25 showed a 30 px
 * dart's plane at 12-15 px (seen from above, banking, its wings' span reads more than its length)
 */
const BUTTON_SEEN = 2.5;
const mixV = (a: V3, b: V3, f: number): V3 => ({ x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f, z: a.z + (b.z - a.z) * f });
const unit = (a: V3): V3 => {
  const l = Math.hypot(a.x, a.y, a.z) || 1;
  return { x: a.x / l, y: a.y / l, z: a.z / l };
};
/** how much of the eclipse's violet goes into the backdrop */
const ECLIPSE_MIX = 0.45;

/** what a frame says of each bim it posed (the headless checks' trace, render/index.ts) */
export type OnRig = (f: BimFrame, rig: Rig, skating: boolean) => void;

export interface OfficeScene {
  /** everything of the office, in its own units */
  readonly root: Group;
  /** the office changed (an agent came or left, another map): its world again */
  setOffice(office: Office, agents: readonly Agent[]): void;
  /** his own bim's look (page bim-pick; null: today's main): main's bim made again when it changed */
  setMine(look: MyLook | null): void;
  /**
   * The growth and the theme as `look` says: the world built again when they changed (true: it
   * was, the ground may be wider).
   */
  prepare(look: Look): boolean;
  /** his plugins' objects at their spots (corners.ts objectSpots); the same ones again: nothing rebuilt. Resolves once they are in the scene: true when their chunk had to load first (the caller draws a frame then), false when the next frame drew them anyway */
  setObjects(c: Corners): Promise<boolean>;
  /** pose every bim for t through this fit (planes size to it); null: no world yet */
  frame(t: number, frames: readonly BimFrame[], look: Look, fit: Fit, onRig?: OnRig): Motion | null;
  /** the backdrop behind it: the theme's page (themed: its palette's), violet in an eclipse; null before a world */
  sky(): ColorRepresentation | null;
  /** the sky's lamps (0..1), snow on the ground (0..1) and the slab's night lift (0..1) for the next frames; lamps null: the theme's */
  weather(lamps: number | null, cover: number, lift?: number): void;
  /** its palette (the view's lights), null before a world */
  pal(): Palette | null;
  readonly office: () => Office | null;
  /** a shown bim's head top and feet, office units; null: none or hidden */
  headTop(name: string, out: Vector3): Vector3 | null;
  feet(name: string, out: Vector3): Vector3 | null;
  /** every bim's name, shown or not */
  names(): readonly string[];
  /** s from the last frame's t until a resting bim's next idle move (render/idle.ts idleWake); null: none, or reduced motion */
  idleWake(): number | null;
  /** a bim's body (the hero's entry pops it in), null: no such bim */
  body(name: string): Object3D | null;
  desk(name: string): DeskHandles | undefined;
  /** the inbox's top tray, world units; null: no inbox */
  inboxTop(): { readonly x: number; readonly y: number; readonly z: number } | null;
  /** a plugin object's top as drawn, world units (its pill); null: not drawn */
  objectTop(key: string): { readonly x: number; readonly y: number; readonly z: number } | null;
  /** where a stage's piece stands (the map's look), office units */
  pieceAt(stage: number): { readonly x: number; readonly z: number } | null;
  /** its world alone (no bim, plane, halo): the town bakes its walls while its bims move */
  world(): Group | null;
  dispose(): void;
}

/** a newcomer's pop in, s */
export const APPEAR_S = 0.45;
/** his sketch's new extension's arrival (drop, squash, settle), s; how high it falls from; how much it squashes */
export const ARRIVE_S = 0.4;
const ARRIVE_DROP = 1.2;
const ARRIVE_SQUASH = 0.12;
/** it fades in over the first part of its fall, so the eye catches the drop (designer m_41488) */
const ARRIVE_FADE = 0.4;
/** the garden lands this long after the wing when both come: two growths, not one jolt */
const ARRIVE_STAGGER = 0.15;
/** where it lands (arriveAt's): the dust puffs at its edges */
const ARRIVE_LAND = 0.6;

/**
 * A group's meshes at this opacity: their own transparent copies of their materials while it fades
 * (kit.mat shares one per colour: the room's would fade too), back to the shared ones at 1.
 */
function fadeIn(g: Object3D, kept: Map<Mesh, Material | Material[]>, opacity: number): void {
  if (opacity >= 1) {
    for (const [m, mat] of kept) {
      const copies: Material[] = Array.isArray(m.material) ? m.material : [m.material];
      for (const c of copies) c.dispose();
      m.material = mat;
    }
    kept.clear();
    return;
  }
  g.traverse((o) => {
    if (!(o instanceof Mesh)) return;
    const m = o as Mesh;
    if (!kept.has(m)) {
      const own: Material | Material[] = m.material;
      kept.set(m, own);
      m.material = Array.isArray(own) ? own.map((c) => c.clone()) : own.clone();
    }
    const mats: Material[] = Array.isArray(m.material) ? m.material : [m.material];
    for (const c of mats) {
      c.transparent = true;
      c.opacity = opacity;
    }
  });
}

/** a new extension's opacity at p (0..1) of its arrival. Pure. */
export function arriveFade(p: number): number {
  return Math.max(0, Math.min(1, p / ARRIVE_FADE));
}

/** a new extension's height over its place and its height's scale at p (0..1) of its arrival: it falls, squashes, settles. Pure. */
export function arriveAt(p: number): { y: number; sy: number } {
  if (p >= 1) return { y: 0, sy: 1 };
  if (p < 0.6) return { y: ARRIVE_DROP * (1 - (p / 0.6) ** 2), sy: 1 };
  if (p < 0.85) return { y: 0, sy: 1 - ARRIVE_SQUASH * Math.sin((Math.PI * (p - 0.6)) / 0.25) };
  return { y: 0, sy: 1 + 0.3 * ARRIVE_SQUASH * Math.sin((Math.PI * (p - 0.85)) / 0.15) };
}

/** the pop's scale at p in [0, 1]: an ease-out a little past 1 (back, ~6%), then settled. Pure. */
export function appearOut(p: number): number {
  const c = 1.3;
  const y = Math.min(1, Math.max(0, p)) - 1;
  return 1 + (c + 1) * y ** 3 + c * y * y;
}

/**
 * A bim's outfit with a head set over its name's (the hero's cast: TownOpts.heads); undefined: no
 * override, its name's own. main keeps its own. Pure.
 */
export function outfitWithHead(name: string, main: boolean, head: HerHair | undefined): Outfit | undefined {
  return main || head === undefined ? undefined : { ...outfitOf(name, false), cut: head.cut, hat: head.hat };
}

/**
 * `ground`: its floating grass block (false in the town: its lawn is the ground); `themed`: the theme's
 * materials (the hero film's town); `head`: a bim's cut and hat over its name's (the hero's cast), its outfit otherwise its own
 */
export function makeOfficeScene(style: Style, o: { readonly ground?: boolean; readonly themed?: boolean; readonly head?: (name: string) => HerHair | undefined } = {}): OfficeScene {
  const ground = o.ground ?? true;
  // the hero film's town keeps the theme's materials (TownOpts.themed, bims m_34678); the app's town the day's
  const themed = o.themed ?? false;
  const root = new Group();
  let office: Office | null = null;
  let agents: readonly Agent[] = [];
  let world: World | null = null;
  let night: boolean | null = null;
  /** the sky's lamps (sky.ts lightOf) and snow cover; lamps null: the theme's */
  let lamps: number | null = null;
  /** the slab's night lift (sky.ts Light.lift) */
  let lift = 0;
  let cover = 0;
  let eclipsed = false;
  let eggDecor: EggDecor | null = null;
  let april = false;
  /** his bim's look (setMine): main's outfit; null today's */
  let mine: MyLook | null = null;
  // the project's growth as drawn now (growth.ts), and the unlock moment's crew
  let grow: Grow = NO_GROW;
  let growKey = growSig(NO_GROW);
  const unlock = makeUnlock(root, style);
  // his plugins' objects (objects-layer.ts, in its own chunk with the objects' makers: the bims
  // chunk keeps its budget): loaded on the first objects, over the world, rebuilt with the theme
  let objects: ObjectsLayer | null = null;
  let loading: Promise<void> | null = null;
  let corners: Corners = NO_CORNERS;
  const loadObjects = (): Promise<void> => {
    loading ??= import('./objects-layer').then((m) => {
      objects = m.makeObjectsLayer(style);
      root.add(objects.root);
    });
    return loading;
  };
  const bims = new Map<string, Bim>();
  const rigs = new Map<string, Rig>();
  /** s from the last frame until the next resting bim's idle move (render/idle.ts); null: none */
  let wakeIdle: number | null = null;
  /** the standing bims' stances (render/alive.ts): no two start a change at once */
  const crowd = newCrowd();
  /** his planes in flight, by bim and when it left ('name@12.30'): the mesh, its plan; each ends on its own catch */
  /** each flight's plan: the app's (zigzag and loop) or the film's paper glide (pose.plane.glide) */
  const planes = new Map<string, { mesh: Plane; leftAt: string; plan: FlightPlan | GlidePlan; glide: boolean; big: number }>();
  /** the sheets laid on desks after a catch, by bim (Pose.letter) */
  const letters = new Map<string, Plane>();
  const tmp = new Vector3();
  const basis = new Matrix4();
  const axis = [new Vector3(), new Vector3(), new Vector3()] as const;
  // the ring under a lit bim (focus.ts, halo.ts): flat on the floor under its feet; the followed
  // bim's soft one, a hovered row's line that reads at any zoom (sized to the fit each frame)
  const haloGeo = new RingGeometry(HALO.inner, HALO.outer, 40);
  haloGeo.rotateX(-Math.PI / 2);
  const haloMat = new MeshBasicMaterial({ color: HALO.day, transparent: true, opacity: HALO.alpha, depthWrite: false });
  const rowMat = new MeshBasicMaterial({ color: HALO.day, transparent: true, opacity: ROW_RING.alpha, depthWrite: false });
  let rowGeo: { inner: number; outer: number; geo: RingGeometry } = { inner: HALO.inner, outer: HALO.outer, geo: new RingGeometry(HALO.inner, HALO.outer, 48) };
  const halos = new Map<string, Mesh>();
  // a bim that comes in while the office is open pops in (Gabriel's review, main m_31507: the hero and
  // the app): from the scale of nothing, a little past its size, settled in APPEAR_S. The ones there
  // when the office opens are simply there.
  const shownLast = new Set<string>();
  const appearing = new Map<string, number>();
  /** his sketch's extensions arriving (null: its first frame starts it), and the ones the office on screen had */
  const arriving = new Map<'wing' | 'garden', { t0: number | null; delay: number; landed: boolean; fades: Map<Mesh, Material | Material[]> }>();
  let extHad: { map: string; wing: boolean; garden: boolean } | null = null;
  let primed = false;
  // and a soft poof as it does (poof.ts, main m_38961): seen, never kitsch; none under reduced motion
  const poofs = makePoofs(root, style);
  const poofList: Poof[] = [];

  /** the office's map's look (the loft before an office) */
  function mapLook(): MapLook {
    return lookOf(office?.map ?? 'loft');
  }

  function mainName(): string {
    return agents.find((a) => a.main)?.name ?? 'main';
  }

  function rebuildWorld(isNight: boolean): void {
    if (world) { root.remove(world.root); world.dispose(); world = null; }
    night = isNight;
    if (!office) return;
    const look = lookOf(office.map);
    eclipsed = false;
    // the office and the app's town keep their day materials in both themes (bims-light, direction A); the hero's town its theme's
    world = buildWorld(office, mainName(), style, look, themed && isNight, grow, ground);
    world.cover(cover);
    world.page(isNight);
    world.lift(lift);
    if (lamps !== null) world.lit(lamps);
    root.add(world.root);
    if (eggDecor) { root.remove(eggDecor.root); eggDecor.dispose(); }
    eggDecor = makeEggDecor(office, style, grow.stage);
    root.add(eggDecor.root);
    objects?.set(corners, isNight);
  }

  /** each desk's screen and lamp as its bim's last pose says, at the sky's lamps (a still office's too: the town's bake follows) */
  function lightDesks(): void {
    if (!world) return;
    for (const [name, d] of world.desks) {
      const rig = rigs.get(name);
      // from stage 9 every desk's lamp is lit at night, its bim there or not (cozy.ts cozyAt: the desk lamps); a stopped bim's stays off
      const allLit = cozyAt(grow.stage).deskLamps && (rig?.grey ?? 0) < 1;
      lightDesk(d, world.pal, rig?.screen ?? 0, (rig ? rig.lamp : name !== mainName()) || allLit, lamps ?? undefined);
    }
  }

  // s3, april fools: each bim in the next one's outfit (hats shuffled), the first one in main's hoodie
  function aprilOutfit(name: string, main: boolean): Outfit | undefined {
    if (!april || main) return undefined;
    const others = agents.filter((a) => !a.main).map((a) => a.name);
    const i = others.indexOf(name);
    const next = others[(i + 1) % others.length] ?? name;
    const o = outfitOf(next, false);
    return i === 0 ? { ...o, hoodie: MAIN_HOODIE } : o;
  }
  function dropBims(): void {
    for (const b of bims.values()) { root.remove(b.group); b.dispose(); }
    bims.clear();
    rigs.clear();
  }

  function syncBims(): void {
    const keep = new Set(agents.map((a) => a.name));
    for (const [name, b] of bims) {
      if (!keep.has(name)) { root.remove(b.group); b.dispose(); bims.delete(name); rigs.delete(name); }
    }
    for (const a of agents) ensureBim(a.name, a.main);
  }

  function ensureBim(name: string, main: boolean): Bim {
    let b = bims.get(name);
    if (!b) {
      const look = mapLook();
      const outfit = main && mine !== null ? mainOutfit(mine) : (aprilOutfit(name, main) ?? outfitWithHead(name, main, o.head?.(name)));
      b = makeBim(name, main, style, outfit, main ? undefined : (w) => look.wear?.(w));
      if (style.shadows) castShadows(b.group);
      b.group.visible = false;
      bims.set(name, b);
      root.add(b.group);
    }
    return b;
  }

  /** a plane's mesh at p, its nose and top, scaled (x, y along its width and height, z its length) */
  function place(mesh: Plane, p: V3, nose: V3, up: V3, sx: number, sz: number): void {
    const g = mesh.group;
    g.visible = true;
    g.scale.set(sx, sx, sz);
    g.position.set(p.x, p.y, p.z);
    // its nose along +z, its top +y (plane.ts): upside down over the loop's top, banked in the turns
    basis.makeBasis(
      axis[0].set(up.y * nose.z - up.z * nose.y, up.z * nose.x - up.x * nose.z, up.x * nose.y - up.y * nose.x),
      axis[1].set(up.x, up.y, up.z),
      axis[2].set(nose.x, nose.y, nose.z),
    );
    g.quaternion.setFromRotationMatrix(basis);
  }

  /** the bim's right hand in the office's units (bim.ts hand, its pose applied this frame) */
  function handOf(name: string): V3 | null {
    const b = bims.get(name);
    if (!b) return null;
    b.hand(tmp);
    root.worldToLocal(tmp);
    return { x: tmp.x, y: tmp.y, z: tmp.z };
  }

  /** where a sheet lies on its bim's desk (catch.ts DESK_SPOT, from its desk spot, standing or sitting), office units; null: no desk */
  function deskSpot(name: string, spot: { x: number; z: number; face: number }): V3 | null {
    const d = world?.desks.get(name);
    if (!d) return null;
    const c = Math.cos(spot.face);
    const sn = Math.sin(spot.face);
    return { x: spot.x + DESK_SPOT.x * c + DESK_SPOT.z * sn, y: d.top.y + 0.004, z: spot.z - DESK_SPOT.x * sn + DESK_SPOT.z * c };
  }

  /**
   * His planes: each from his side of the screen to the bim's hand (its own flight, the size shrinking
   * to the bim's), then held, read and put away (catch.ts), one mesh all the way, never swapped; the
   * sheets left on desks (Pose.letter). Reduced motion: no flight, the open sheet in its hand, then on
   * its desk. True while one moves.
   */
  /**
   * a point of the view (world units) in this office's own (the plane flies in its root): in the town
   * the office stands at its place, and his button's point taken as the office's flew from that far off
   */
  /**
   * A check's probe (test mode only): each newcomer's pop, where it stands on the canvas (css px from
   * its top left: its feet, its head, and the stars' ring round it), so a check finds it in any layout
   * and sees what covers it (designer m_49975: window.__bisePoofs, the newest last, at most 20)
   */
  function probePoof(name: string, rig: Rig, fit: Fit): void {
    const w = window as { bise?: { test?: boolean }; __bisePoofs?: unknown[] };
    if (w.bise?.test !== true) return;
    root.updateWorldMatrix(true, false);
    const at = (y: number): { x: number; y: number } => {
      const v = root.localToWorld(new Vector3(rig.x, y, rig.z));
      const s = toScreen(fit, { x: v.x, y: v.y, z: v.z });
      return { x: Math.round(s.x * 10) / 10, y: Math.round(s.y * 10) / 10 };
    };
    const list = (w.__bisePoofs ??= []);
    list.push({ name, at: Date.now(), feet: at(0), head: at(1.1), stars: at(SPARK.y), ringPx: Math.round(sparkSpread(fit.scale) * 0.6 / fit.scale) });
    if (list.length > 20) list.shift();
  }

  const local = (w: V3): V3 => {
    root.updateWorldMatrix(true, false);
    const v = root.worldToLocal(new Vector3(w.x, w.y, w.z));
    return { x: v.x, y: v.y, z: v.z };
  };
  /** a plane's meshes on one layer (three's layers are each object's own, never its children's) */
  const layerOf = (g: Object3D, n: number): void => {
    if (g.layers.isEnabled(n) && g.layers.mask === 1 << n) return;
    g.traverse((o) => {
      o.layers.set(n);
    });
  };
  function drawPlanes(t: number, frames: readonly BimFrame[], fit: Fit, light: boolean, reduced: boolean, planeFrom?: Look['planeFrom'], overPage = false): boolean {
    const flying = new Set<string>();
    const site = office ? siteOf(office) : null;
    let moving = false;
    /** the bims whose desk shows a sheet this frame (one each) */
    const laid = new Set<string>();
    // every plane on its way, each its own flight: a newer one never cuts an older (designer m_32877)
    const flights = frames.flatMap((f) => {
      const own = [...(f.pose.aloft ?? []), ...(f.pose.plane ? [f.pose.plane] : [])];
      // a newer plane's reach cuts this one's catch short (catch.ts heldAt cut), in this one's catch time
      return own.map((fp, i) => {
        const next = own[i + 1];
        const cut = next === undefined ? Infinity : fp.since - (fp.dur ?? PLANE_FLIGHT) + ((next.dur ?? PLANE_FLIGHT) - REACH_S - next.since);
        return { f, fp, cut };
      });
    });
    for (const { f, fp, cut } of flights) {
      const s = fp.since;
      const dur = fp.dur ?? PLANE_FLIGHT;
      if (s < 0 || s >= dur + PLANE_CATCH) continue;
      const rig = rigs.get(f.name);
      const hand = handOf(f.name);
      if (!rig || !hand) continue;
      const desk = deskSpot(f.name, f.pose.seg.spot);
      const away = desk !== null ? awayOf(f.pose.seg.act) : 'pocket';
      const held = heldAt(s - dur, away, reduced, cut);
      if (reduced && held === null) continue;
      // one plan per flight, made when it leaves (his pan or zoom mid-flight moves no plane); keyed by
      // its bim and when it left, so two to one bim fly side by side
      const leftAt = (t - s).toFixed(2);
      const key = `${f.name}@${leftAt}`;
      flying.add(key);
      let p = planes.get(key);
      if (!p) {
        // the loop and the zigzag size with LOOP_PX on screen (designer via bims-qa m_27793)
        const unit = Math.max(1, (LOOP_PX * PLANE_FORESHORTEN * fit.scale) / LOOP_LEN);
        // the newest is the one the egg may loop (eggs.ts reads pose.plane)
        const loops = fp.loops ?? (fp === f.pose.plane && f.egg?.kind === 'loop' ? 2 : 1);
        const avoid = pillHeads(frames, f.name);
        const from = planeFrom ? local(viewPoint(fit, planeFrom)) : cameraStart(fit, fp.startX);
        const seed = hash(key);
        const glide = fp.glide === true;
        const plan = glide ? planGlide({ from, to: hand, seed, dur, px: fit.scale }) : planFlight({ from, to: hand, seed, loops, unit, avoid, dur });
        // from the hero's send button: its dart's length there (the same plane, never a jump in size); else big near his eye
        const big = planeFrom?.px !== undefined ? (planeFrom.px * BUTTON_SEEN * fit.scale) / HELD_LEN : startSize(fit, HELD_LEN);
        p = { mesh: makePlane(style), leftAt, plan, glide, big };
        planes.set(key, p);
        root.add(p.mesh.group, p.mesh.shadow);
      }
      p.mesh.setInk(light);
      // a glide from the view's point (the hero's sent bubble) leaves from it as the camera moves
      const from = p.glide && planeFrom ? local(viewPoint(fit, planeFrom)) : undefined;
      const at = (x: number): FlightFrame => (p.glide ? glideAt(p.plan as GlidePlan, x, hand, from) : flightAt(p.plan as FlightPlan, x, hand));
      if (held === null) {
        // flying: to the hand's real point, from big near his eye to the bim's own size
        moving = true;
        const fl = at(s);
        const size = HELD_LEN * sizeAt(fl.u, p.big);
        p.mesh.setFold(0);
        place(p.mesh, fl.p, fl.nose, fl.up, size, size);
        // the hero's glide in the layer over its page (its chat): from the button to the hand, one plane
        layerOf(p.mesh.group, p.glide && overPage ? UP_LAYER : 0);
        // its soft shadow on the floor once it is over the slab, fainter and wider as it climbs
        // a glide (the film's paper plane) keeps a faint one however high, so the eye finds it (designer m_32270)
        const raw = shadowOf(fl.p, 0);
        const sh = p.glide ? { ...raw, alpha: Math.max(raw.alpha, GLIDE_SHADOW) } : raw;
        const over = site !== null && fl.p.x > site.x0 && fl.p.x < site.x1 && fl.p.z > site.z0 && fl.p.z < site.z1;
        p.mesh.shadow.visible = over && sh.alpha > 0.01;
        if (p.mesh.shadow.visible) {
          p.mesh.shadow.position.set(fl.p.x, FLOOR_LIFT, fl.p.z);
          p.mesh.shadow.rotation.z = Math.atan2(fl.nose.x, fl.nose.z);
          p.mesh.shadow.scale.set(0.8 * size / SHADOW_LEN, 1.4 * size / SHADOW_LEN, 1);
          (p.mesh.shadow.material as MeshBasicMaterial).opacity = sh.alpha;
        }
        continue;
      }
      // in its hand: where catch.ts says, in the bim's frame, from the flight's last orientation (back in the town's layer)
      layerOf(p.mesh.group, 0);
      moving = moving || !reduced;
      p.mesh.shadow.visible = false;
      if (!held.shown) { p.mesh.group.visible = false; continue; }
      const toOffice = (v: V3): V3 => {
        const c = Math.cos(rig.dir);
        const sn = Math.sin(rig.dir);
        return { x: v.x * c + v.z * sn, y: v.y, z: -v.x * sn + v.z * c };
      };
      const end = reduced ? null : at(dur);
      const nose = toOffice(held.nose);
      const up = toOffice(held.up);
      let o = end === null ? { nose, up } : blendTurn(end, { nose, up }, held.hold);
      if (away === 'desk' && held.put > 0) {
        // laid as its desk lies (the letter after it: never a turn at the handoff)
        const fc = Math.cos(f.pose.seg.spot.face);
        const fs = Math.sin(f.pose.seg.spot.face);
        const byDesk = (v: V3): V3 => ({ x: v.x * fc + v.z * fs, y: v.y, z: -v.x * fs + v.z * fc });
        o = blendTurn(o, { nose: byDesk(ON_DESK.nose), up: byDesk(ON_DESK.up) }, held.put);
      }
      const side = HELD_LEN * (1 - 0.5 * held.quarter);
      // the hand grips it at `grip` along its length
      let c = { x: hand.x - o.nose.x * held.grip * side, y: hand.y - o.nose.y * held.grip * side, z: hand.z - o.nose.z * held.grip * side };
      if (away === 'desk' && desk !== null) c = mixV(c, desk, held.put);
      else c = mixV(c, { x: c.x + toOffice(POCKET_SINK).x, y: c.y, z: c.z + toOffice(POCKET_SINK).z }, held.put);
      if (away === 'desk' && held.put > 0) laid.add(f.name);
      p.mesh.setFold(held.fold);
      place(p.mesh, c, o.nose, o.up, side, side);
    }
    for (const [key, p] of planes) {
      if (!flying.has(key)) { root.remove(p.mesh.group, p.mesh.shadow); p.mesh.dispose(); planes.delete(key); }
    }
    // the sheets on desks: read, laid there, kept while the bim keeps its state (timeline.ts Pose.letter)
    const kept = new Set<string>();
    for (const f of frames) {
      if (f.pose.letter === undefined || laid.has(f.name) || awayOf(f.pose.seg.act) !== 'desk') continue;
      // its own catch still going (that plane's sheet is in its hand): none yet
      const spot = deskSpot(f.name, f.pose.seg.spot);
      if (!spot) continue;
      let m = letters.get(f.name);
      if (!m) {
        m = makePlane(style);
        m.setFold(1);
        letters.set(f.name, m);
        root.add(m.group);
      }
      m.setInk(light);
      kept.add(f.name);
      const c = Math.cos(f.pose.seg.spot.face);
      const sn = Math.sin(f.pose.seg.spot.face);
      const turn = (v: V3): V3 => ({ x: v.x * c + v.z * sn, y: v.y, z: -v.x * sn + v.z * c });
      place(m, spot, turn(ON_DESK.nose), turn(ON_DESK.up), HELD_LEN, HELD_LEN);
    }
    for (const [name, m] of letters) {
      if (!kept.has(name)) { root.remove(m.group); m.dispose(); letters.delete(name); }
    }
    return moving;
  }

  /** the way between two orientations (nlerp, the top squared to the nose again) */
  function blendTurn(a: { nose: V3; up: V3 }, b: { nose: V3; up: V3 }, f: number): { nose: V3; up: V3 } {
    const nose = unit(mixV(a.nose, b.nose, f));
    const u = mixV(a.up, b.up, f);
    const d = u.x * nose.x + u.y * nose.y + u.z * nose.z;
    return { nose, up: unit({ x: u.x - nose.x * d, y: u.y - nose.y * d, z: u.z - nose.z * d }) };
  }

  /** where the '?' / '✗' pills are (labels.ts pillOf: over the asking and the failed bims' heads), but the target's */
  function pillHeads(frames: readonly BimFrame[], target: string): V3[] {
    const out: V3[] = [];
    for (const f of frames) {
      if (f.name === target || pillOf(f.pose.seg.act) === null) continue;
      const rig = rigs.get(f.name);
      if (rig) out.push({ x: rig.x, y: PILL_Y, z: rig.z });
    }
    return out;
  }

  /** a hovered row's ring at `scale` (halo.ts ringOf): made again only when the zoom changes its size */
  function rowGeoAt(scale: number): RingGeometry {
    const r = ringOf('row', scale);
    if (rowGeo.inner === r.inner && rowGeo.outer === r.outer) return rowGeo.geo;
    rowGeo.geo.dispose();
    const geo = new RingGeometry(r.inner, r.outer, 48);
    geo.rotateX(-Math.PI / 2);
    rowGeo = { inner: r.inner, outer: r.outer, geo };
    return geo;
  }

  function drawHalos(frames: readonly BimFrame[], isNight: boolean, scale: number): void {
    const ink = isNight ? HALO.night : HALO.day;
    haloMat.color.set(ink);
    rowMat.color.set(ink);
    const lit = new Set<string>();
    for (const f of frames) {
      const rig = rigs.get(f.name);
      if (f.lit === undefined || !rig) continue;
      lit.add(f.name);
      const geo = f.lit === 'row' ? rowGeoAt(scale) : haloGeo;
      const mat = f.lit === 'row' ? rowMat : haloMat;
      let h = halos.get(f.name);
      if (!h) { h = new Mesh(geo, mat); h.renderOrder = 1; halos.set(f.name, h); root.add(h); }
      h.geometry = geo;
      h.material = mat;
      h.position.set(rig.x, 0.02, rig.z);
      h.visible = rig.opacity > 0.05;
    }
    for (const [name, h] of halos) {
      if (!lit.has(name)) { root.remove(h); halos.delete(name); }
    }
  }

  const shown = (name: string): Bim | null => {
    const b = bims.get(name);
    return b && b.group.visible ? b : null;
  };

  return {
    root,
    setObjects(c) {
      corners = c;
      if (objects !== null) {
        objects.set(c, night === true);
        return Promise.resolve(false);
      }
      // none yet and none to draw: no chunk to load
      if (c.placed.length === 0 && c.rugs.length === 0) return Promise.resolve(false);
      return loadObjects().then(() => {
        objects?.set(corners, night === true);
        return true;
      });
    },
    setOffice(o, as) {
      // another map: its bims dress for it (the look's wear)
      if (office && office.map !== o.map) dropBims();
      // a new extension on the same map (extensions.ts): it arrives like an object (frame: drop, squash, settle)
      if (extHad !== null && extHad.map === o.map) {
        const wing = o.wing !== undefined && !extHad.wing;
        if (wing) arriving.set('wing', { t0: null, delay: 0, landed: false, fades: new Map() });
        if (o.garden !== undefined && !extHad.garden) arriving.set('garden', { t0: null, delay: wing ? ARRIVE_STAGGER : 0, landed: false, fades: new Map() });
      }
      extHad = { map: o.map, wing: o.wing !== undefined, garden: o.garden !== undefined };
      office = o;
      agents = as;
      rebuildWorld(night ?? false);
      syncBims();
    },
    prepare(look) {
      if (!office) return false;
      // the growth moved (a new stage, garden, frames, cones): the world again, the ground may be wider
      const g = look.grow ?? NO_GROW;
      let grew = false;
      if (growSig(g) !== growKey) {
        grow = g;
        growKey = growSig(g);
        rebuildWorld(look.night);
        grew = true;
      } else if (night !== look.night) {
        // the theme: the hero's town's world again; the others' only changes the page under it
        if (themed) rebuildWorld(look.night);
        else {
          night = look.night;
          world?.page(look.night);
          objects?.set(corners, look.night);
        }
      }
      // t4: the eclipse turns the sky behind the office violet
      eclipsed = look.eggs?.eclipse === true;
      const fool = look.eggs?.april === true;
      if (fool !== april) { april = fool; dropBims(); }
      return grew;
    },
    frame(t, frames, look, fit, onRig) {
      if (!office || !world) return null;
      // his open items as sheets in the inbox's top tray (render/inbox.ts): shown or hidden, never moving
      world.inbox?.sheets(look.inbox ?? 0);
      // each napper on its bed's rest as built (render/nap.ts)
      const o = office;
      const naps = world.naps;
      const bedAt = (p: Pt): NapRest | undefined => restAt(o, naps, p);
      let motion: Motion = 'still';
      const seen = new Set<string>();
      rigs.clear();
      // the bims' small details only when they are big enough on screen (bim.ts nearAt); their clay's warmth by day (claylight.ts)
      const near = nearAt(fit.scale);
      const day = 1 - (lamps ?? (world.pal.pendants ? 1 : 0));
      // the resting bims' idle moves (render/idle.ts): none under reduced motion or drawn too small
      const { plan: idle, wake } = idleFrame(frames, t, { reduced: look.reduced, scale: fit.scale, bedAt });
      wakeIdle = wake;
      crowd.begin(t);
      for (const f of frames) {
        const b = ensureBim(f.name, f.main);
        b.detail(near);
        b.daylight(day);
        // his easter egg on top of its pose (eggs.ts picks it, egg.ts plays it)
        // a skater in the park (skate.ts): its line's pose once it is there; on the way, the walk's
        const park = office.park;
        const gym = office.gym;
        // or a climber on the gym's wall (climb.ts), the same way
        const skater = look.reduced ? null : park ? skateRig(f.name, f.pose, park, b.outfit.phase) : gym ? climbRig(f.name, f.pose, gym) : null;
        const still = idle.get(f.name);
        // a standing bim breathes and shifts its weight (render/alive.ts), under its rare idle moves
        const posed = aliveOf(crowd, f, rigOf({ pose: f.pose, t, main: f.main, phase: b.outfit.phase, reduced: look.reduced, bedAt }), t, { reduced: look.reduced, scale: fit.scale, skater: skater !== null });
        const rig = eggRig(skater ?? (still ? idleRig(posed, still, 1 / fit.scale >= BURST_PX) : posed), f.egg ?? null);
        b.apply(rig);
        if (primed && !look.reduced && !shownLast.has(f.name) && !appearing.has(f.name)) {
          appearing.set(f.name, t);
          poofList.push({ x: rig.x, y: 0, z: rig.z, t0: t, seed: hash(f.name) });
          probePoof(f.name, rig, fit);
        }
        const since = appearing.get(f.name);
        if (since !== undefined) {
          const p = (t - since) / APPEAR_S;
          b.group.scale.setScalar(p >= 1 ? 1 : Math.max(1e-3, appearOut(p)));
          if (p >= 1) appearing.delete(f.name);
        }
        rigs.set(f.name, rig);
        seen.add(f.name);
        motion = louder(motion, rig.motion);
        onRig?.(f, rig, skater !== null);
      }
      crowd.end();
      for (const [name, b] of bims) if (!seen.has(name)) b.group.visible = false;
      shownLast.clear();
      for (const name of seen) shownLast.add(name);
      if (frames.length > 0) primed = true;
      if (appearing.size > 0) motion = 'walk';
      while (poofList.length > 0 && (poofList[0]?.t0 ?? t) + POOF_S <= t) poofList.shift();
      if (poofs.draw(t, poofList, fit.scale)) motion = 'walk';
      // the halo reads on the floor as lit: the night's (light) when the lamps are on, in the office
      drawHalos(frames, themed ? look.night : (lamps ?? 0) >= 0.5, fit.scale);
      lightDesks();
      for (const [name, d] of world.desks) seatChair(d.chair, rigs.get(name)?.seat);
      if (drawPlanes(t, frames, fit, !look.night, look.reduced, look.planeFrom, look.planesUp === true)) motion = 'walk';
      // the office's own eggs: the ghost crossing asks for frames; april's outfits rebuild the bims once
      if (eggDecor?.update(look.eggs) === true) motion = 'walk';
      // a new extension arriving: it falls in, squashes, settles; reduced motion: there at once
      for (const [kind, ar] of arriving) {
        const g = world.ext.get(kind);
        if (ar.t0 === null) ar.t0 = t + ar.delay;
        const p = look.reduced || g === undefined ? 1 : Math.max(0, (t - ar.t0) / ARRIVE_S);
        const a = arriveAt(p);
        g?.position.setY(a.y);
        g?.scale.setY(a.sy);
        if (g !== undefined) fadeIn(g, ar.fades, p >= 1 ? 1 : arriveFade(p));
        // at the landing, a puff of dust at its slab's front corners (the bims' poof), then nothing
        const box = kind === 'wing' ? office.wing : office.garden;
        if (!ar.landed && p >= ARRIVE_LAND && p < 1 && box !== undefined) {
          ar.landed = true;
          for (const [x, z] of [[box.x0, box.z1], [box.x1, box.z1], [box.x1, box.z0], [(box.x0 + box.x1) / 2, box.z1]] as const) poofList.push({ x, y: 0, z, t0: t, seed: hash(`${kind}${String(x)}`) });
        }
        if (p >= 1) arriving.delete(kind);
        else motion = 'walk';
      }
      // the unlock moment: two bims bring the new piece in from the door
      if (unlock.update(t, office, look.unlock, look.unlock ? world.pieces.get(look.unlock.stage) : undefined, look.reduced, mapLook())) motion = 'walk';
      return motion;
    },
    sky() {
      if (!world) return null;
      // a sky event, not the bar changing colour (bims m_28109): the violet blended into the backdrop
      // the theme's own page (the office's world is the day's in both themes)
      const paper = themed ? world.pal.bg : paletteOf(night === true).bg;
      return eclipsed ? new Color(paper).lerp(new Color(WORLD.eclipse), ECLIPSE_MIX) : paper;
    },
    weather(l, c, up = 0) {
      // the desk lamps fade with the sky between frames too (a baked office in the town is never posed again)
      const relight = l !== lamps;
      lamps = l;
      cover = c;
      lift = up;
      world?.cover(c);
      world?.lift(up);
      world?.lit(l ?? (world.pal.pendants ? 1 : 0));
      if (relight) lightDesks();
    },
    pal: () => world?.pal ?? null,
    office: () => office,
    headTop(name, out) {
      return shown(name)?.headTop(out) ?? null;
    },
    feet(name, out) {
      return shown(name)?.feet(out) ?? null;
    },
    setMine(look) {
      if (sameLook(look, mine)) return;
      mine = look;
      // main's bim goes; the next frame's ensureBim makes it again in the new look, where it was
      for (const a of agents) {
        const b = a.main ? bims.get(a.name) : undefined;
        if (b) { root.remove(b.group); b.dispose(); bims.delete(a.name); }
      }
    },
    names: () => [...bims.keys()],
    idleWake: () => wakeIdle,
    body: (name) => bims.get(name)?.group ?? null,
    desk: (name) => world?.desks.get(name),
    inboxTop: () => world?.inbox?.top ?? null,
    objectTop: (key) => objects?.topOf(key) ?? null,
    pieceAt(stage) {
      return office ? mapLook().pieceAt(office, stage) : null;
    },
    world: () => world?.root ?? null,
    dispose() {
      objects?.dispose();
      if (world) { root.remove(world.root); world.dispose(); world = null; }
      if (eggDecor) { eggDecor.dispose(); eggDecor = null; }
      unlock.dispose();
      poofs.dispose();
      for (const b of bims.values()) b.dispose();
      bims.clear();
      for (const p of planes.values()) p.mesh.dispose();
      planes.clear();
      for (const h of halos.values()) root.remove(h);
      halos.clear();
      haloGeo.dispose();
      haloMat.dispose();
      rowGeo.geo.dispose();
      rowMat.dispose();
    },
  };
}
