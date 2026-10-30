// The town's renderer (three; page bims-maps v9, page town-live): every project's office is its own
// office scene (office-scene.ts, the minimap's: its map's look, its growth, its bims, its eggs)
// where the town's plan put it, and the town around them (townscape.ts), merged. From afar an
// office is a bake of its scene posed still (merge.ts: a handful of draw calls), baked again only
// when its still pose changes; close up (his pinch past LIVE_ZOOM, or the dive) the offices in view,
// LIVE_MAX at most, show their scene itself, posed live each frame. One hemisphere light and one sun
// with its soft shadow for the whole town (view.ts), under his clock's sky when frame() gets one
// (sky.ts lightOf, sky-light.ts: a blend, then rest), the office's one light for both themes
// (bims-light, direction A): the day's materials always, offices and townscape, the lamps and glows
// by the sky (baked offices too), the night's warm pools of the offices nearest the view, POOLS_MAX
// at most (town/pools.ts); the backdrop is the theme's page. No sky: the day palette's fixed light.
// The hero's film (TownOpts.themed) keeps the theme's materials and light, as signed. It draws only
// when frame() is called. In test
// mode it exposes window.__biseTown = { frames, calls, ms, offices, merged, live, check } for the
// town's check, and window.__biseTownDrawn() (each office: in view, drawn, where, inside the camera's
// planes) for its camera path. The camera's near and far planes wrap the whole town (town/camera.ts
// townDepth). Owner: maps-e.
import { Box3, type BufferGeometry, Group, type Material, Mesh, type MeshLambertMaterial, type Object3D, PointLight, Vector3, WebGLRenderer } from 'three';
import type { Motion } from '../model';
import type { Step } from '../office-driver';
import { growSig, NO_GROW } from '../growth';
import { BACK, RIGHT, UP, type Fit, toScreen, viewFit } from './camera';
import { FIT, sameCam, type Cam } from '../zoom';
import { lookOf } from './maps';
import { DAY, NIGHT } from './palette';
import { STYLE } from './style';
import { mergeRoot, type Merged } from './merge';
import { looseChairs } from './furniture';
import { APPEAR_S, makeOfficeScene, UP_LAYER, type OfficeScene } from './office-scene';
import { POOF_S } from './poof';
import { makeView } from './view';
import { applyLight } from './sky-light';
import { POOL, blendAt, blending, lightOf, retarget, type Blend, type Light } from '../sky';
import { poolsOf } from './world';
import { POOLS_MAX, townPools } from '../town/pools';
import { pickAt, type Target } from './pick';
import { TOWN_DAY, TOWN_NIGHT, buildTownscape } from './townscape';
import { diveFit, townDepth, townFit } from '../town/camera';
import { siteOf, townSeed } from '../town/ground';
import { beyondOf, type Beyond } from '../town/beyond';
import { crossings, townPlan, type Placed, type TownPlan } from '../town/plan';
import type { MakeTownRenderer, TownRenderer, TownSite } from '../town/types';

/** the town's key light's distance from what it lights (its sun stands 3x the office's) */
const TOWN_KEY_DIST = 48;

/** a sign's anchor: over the back wall's middle, this high */
const SIGN_Y = 2.6;
/** past this zoom over the town's fit, the offices in view are posed live (bims m_29006) */
export const LIVE_ZOOM = 1.6;
/** at most this many live offices at once (architect m_28994: battery) */
export const LIVE_MAX = 4;

interface Probe {
  frames: number;
  calls: number;
  ms: number;
  offices: number;
  merged: { from: number; to: number };
  /** the offices drawn live at the last frame */
  live: string[];
  /** the far offices popping a newcomer in at the last frame (live meanwhile, baked again after) */
  popping: string[];
  /** the last frames' render calls, ms (the zoomed check reads their median: one frame on a busy machine says little) */
  recent: number[];
  /** the plan's road and river points inside an office: 0 and 0 */
  check: { road: number; river: number };
  /** the live offices' bims on the canvas at the last frame, css px (a check hovers and clicks one) */
  heads: { id: string; name: string; x: number; y: number; feet: number }[];
}

interface Drawn {
  id: string;
  inView: boolean;
  shown: boolean;
  live: boolean;
  fp: { x0: number; x1: number; z0: number; z1: number };
  box: { x0: number; x1: number; y0: number; y1: number; z0: number; z1: number } | null;
  /** the box's depth from the camera, and the camera's clipping planes */
  depth: { min: number; max: number; near: number; far: number } | null;
  /** where its dirt path meets it (town/plan.ts Placed.door), css px in the canvas: the path check pinches in there */
  door: { x: number; y: number };
}

/** the world box of what is drawn under `obj`: its visible meshes only (a live office's hidden world left out) */
function drawnBox(obj: Object3D): Box3 | null {
  obj.updateMatrixWorld(true);
  const out = new Box3();
  const one = new Box3();
  obj.traverseVisible((o) => {
    if (!(o instanceof Mesh)) return;
    const g = o.geometry as BufferGeometry;
    if (g.boundingBox === null) g.computeBoundingBox();
    if (g.boundingBox === null) return;
    out.union(one.copy(g.boundingBox).applyMatrix4(o.matrixWorld));
  });
  return out.isEmpty() ? null : out;
}

/** one office of the town: its scene, its bake from afar (and the still pose it was baked from) */
interface Spot {
  readonly scene: OfficeScene;
  /** what its scene was set to (office-scene setOffice): its plan's shape */
  shape: string;
  /** its bake, and its glows' copies with the scene's own (the sky's lamps reach them: frame()) */
  bake: { readonly sig: string; readonly merged: Merged; readonly holder: Group; readonly glows: readonly (readonly [MeshLambertMaterial, MeshLambertMaterial])[] } | null;
  /** its scene is in the town now, posed live */
  live: boolean;
  /** live: its walls baked (its world's hundreds of meshes in a handful), its bims moving over them */
  walls: { readonly sig: string; readonly merged: Merged } | null;
  /** its bims at its last still (null: never baked yet), and how long it stays live for a newcomer's pop (s, Date.now's) */
  names: ReadonlySet<string> | null;
  popUntil: number;
}

/**
 * a newcomer in a far office pops in live, as the hero's always-live offices show it (his m_49844:
 * « la même animation d'apparition que dans la hero section, le glitter »): its office posed live for
 * the pop and its poof (office-scene APPEAR_S, poof.ts POOF_S) and a beat, then baked again
 */
const POP_LIVE_S = APPEAR_S + POOF_S + 0.3;
/** while its newcomer walks to its spot, the office stays live this much longer each frame */
const POP_TAIL_S = 0.4;
const reducedNow = (): boolean => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

const testMode = (): boolean => (window as Partial<Window> & { bise?: { test?: boolean } }).bise?.test === true;

/**
 * What makes two still offices look the same (bims m_29006): the office, its growth, the theme and
 * each agent's state, never its office life (a snack trip, a nap): a far office is baked again only
 * when an agent's state changes, as the minimap's row-clock check would see it.
 */
function stillSig(s: Step): string {
  const bims = s.agents.map((a) => `${a.name}:${a.status}${a.main ? '*' : ''}`).join(';');
  return `${s.office.map}|${s.office.size}|${[...s.office.desks.keys()].join(',')}|${growSig(s.look.grow ?? NO_GROW)}|${s.look.night ? 'n' : 'd'}|${bims}`;
}

const shapeOf = (s: Step): string => `${s.office.map}|${s.office.size}|${s.agents.map((a) => `${a.name}${a.main ? '*' : ''}`).join(',')}`;

export const makeTownRenderer: MakeTownRenderer = (canvas, opts = {}) => {
  // the town's shadow is drawn again only when something in it moved (a bake, a live office's frame)
  const v3 = makeView(canvas, { shadowAuto: false });
  const { renderer, scene, sun } = v3;
  // the hero's planes over its page (TownOpts.planesOver): the same scene and camera, only UP_LAYER, lit as the town
  const over = opts.planesOver ? new WebGLRenderer({ canvas: opts.planesOver, alpha: true, antialias: STYLE.antialias, powerPreference: 'low-power' }) : null;
  over?.setClearColor(0x000000, 0);
  if (over) for (const l of [v3.hemi, sun]) l.layers.enable(UP_LAYER);

  let plan: TownPlan | null = null;
  let planKey = '';
  /** the app's endless town (opts.endless): the grass, road, river and trees past the lawn (town/beyond.ts) */
  let beyond: Beyond | null = null;
  let sites: readonly TownSite[] = [];
  let night: boolean | null = null;
  let size = { w: 0, maxH: 0 };
  let base: Fit | null = null;
  let view: Fit | null = null;
  let cam: Cam = FIT;
  let shadowsDirty = true;
  /** where the sun and its shadow's box were aimed last (aimSun): a pan moves them only when it changes */
  let sunKey = '';
  /** the sky's blend and the light drawn (sky.ts); null: the theme's fixed sun (the hero) */
  let blend: Blend | null = null;
  let skyLight: Light | null = null;
  /** the hero's: the theme's materials and fixed light (TownOpts.themed); else the day's, under the sky */
  const themed = opts.themed === true;
  /** the night's lamp pools (town/pools.ts): a fixed count, lit by the sky's lamps; none in the hero's film */
  const pools = themed ? [] : Array.from({ length: POOLS_MAX }, () => new PointLight(POOL.color, 0, POOL.distance, POOL.decay));
  for (const l of pools) scene.add(l);
  /** each pool's full intensity (0: unused), and where they were placed last */
  let poolI: number[] = pools.map(() => 0);
  let poolKey = '';
  /** where the sun aims now (aimSun): the pools go to the offices nearest it */
  let aimAt = { x: 0, z: 0 };
  const spots = new Map<string, Spot>();
  /** the townscape merged (its lawn and road keep their own materials: lift() lights them at night) */
  let scape: { sig: string; merged: Merged; loose: Group | null; lift: (level: number) => void } | null = null;
  /** the hero's entry build holds the town loose (TownOpts.entry) until settle() */
  let held = opts.entry === true;
  /** the lawn's holes the townscape was built with last (the entry leaves no cube there) */
  let lastPits: readonly { x0: number; x1: number; z0: number; z1: number }[] = [];
  const v = new Vector3();
  const probe: Probe | null = testMode() ? { frames: 0, calls: 0, ms: 0, offices: 0, merged: { from: 0, to: 0 }, live: [], popping: [], recent: [], check: { road: 0, river: 0 }, heads: [] } : null;
  if (probe) (window as unknown as { __biseTown?: Probe }).__biseTown = probe;

  const placedOf = (id: string): Placed | undefined => plan?.offices.find((p) => p.id === id);

  function spotOf(id: string): Spot {
    let s = spots.get(id);
    if (!s) {
      // no grass block of its own (bims m_28807): the town's lawn is its ground
      const heads = opts.heads;
      const head = heads ? (name: string) => heads(id, name) : undefined;
      s = { scene: makeOfficeScene(STYLE, { ground: false, themed, ...(head ? { head } : {}) }), shape: '', bake: null, live: false, walls: null, names: null, popUntil: 0 };
      spots.set(id, s);
    }
    return s;
  }

  function dropBake(s: Spot): void {
    if (!s.bake) return;
    scene.remove(s.bake.holder);
    s.bake.merged.dispose();
    s.bake = null;
  }

  /**
   * Its world's desk chairs out of any bake, drawn on their own (furniture.ts looseChairs): their
   * slides as a bim sits and gets up show (his m_50679). An old world's holders go with it.
   */
  function syncChairs(s: Spot): void {
    const w = s.scene.world();
    const root = s.scene.root;
    for (const c of [...root.children]) if (c.userData.chairsOf !== undefined && c.userData.chairsOf !== w?.id) root.remove(c);
    if (w && !root.children.some((c) => c.userData.chairsOf === w.id)) looseChairs(w, root);
  }

  function dropWalls(s: Spot): void {
    if (!s.walls) return;
    s.scene.root.remove(s.walls.merged.group);
    s.walls.merged.dispose();
    s.walls = null;
    const w = s.scene.world();
    if (w) w.visible = true;
  }

  /**
   * A live office's walls baked (page town-live: 4 live offices in 6 ms, not 14), baked again when its
   * world was built again (growth, theme, shape), never when a desk lights (its screen and lamp stay
   * live materials, merge.ts: a bake is ~100 ms, the hero's film lit one every few seconds); its world
   * hidden under them.
   */
  function walls(s: Spot, step: Step): void {
    const w = s.scene.world();
    if (!w) return;
    // a desk lit or dimmed needs no new bake: its screen and lamp stay the world's own materials (below)
    const sig = `${s.shape}|${growSig(step.look.grow ?? NO_GROW)}|${step.look.night ? 'n' : 'd'}|${String(w.id)}`;
    if (s.walls?.sig === sig) return;
    dropWalls(s);
    const at = s.scene.root.position.clone();
    s.scene.root.position.set(0, 0, 0);
    // mergeRoot(w) refreshes w's matrices from its parent's: the root's own, at 0 now, first (else a
    // walls bake made after the office was drawn carries its place twice: the walls off their bims)
    s.scene.root.updateMatrixWorld(true);
    w.visible = true;
    // its chairs out of the walls: they slide under their bims
    syncChairs(s);
    const merged = mergeRoot(w, STYLE, STYLE.shadows);
    s.scene.root.position.copy(at);
    s.scene.root.updateMatrixWorld(true);
    for (const c of merged.group.children) {
      const m = (c as unknown as { material: Material }).material;
      // the desks' screens and lamps stay shared: lightDesk lights the walls' copy too (dropped with this world)
      if (m !== merged.baked && m.userData.live !== true) (c as unknown as { material: Material }).material = m.clone();
    }
    // the entry build grows the loose world in: its bake waits, hidden, for settle()
    w.visible = held;
    merged.group.visible = !held;
    s.scene.root.add(merged.group);
    s.walls = { sig, merged };
  }

  function dropSpot(id: string): void {
    const s = spots.get(id);
    if (!s) return;
    dropWalls(s);
    dropBake(s);
    scene.remove(s.scene.root);
    s.scene.dispose();
    spots.delete(id);
  }

  /** its scene told of its office, posed for this step (the town's frame: planes sized to the view) */
  function posed(s: Spot, step: Step, t: number): Motion | null {
    if (step.reshaped || s.shape !== shapeOf(step)) {
      s.shape = shapeOf(step);
      s.scene.setOffice(step.office, step.agents);
    }
    s.scene.prepare(step.look);
    const f = shownFit();
    return f ? s.scene.frame(t, step.played, over ? { ...step.look, planesUp: true } : step.look, f) : null;
  }

  /** where each office stands, its bake or its live scene shown */
  function place(): void {
    for (const [id, s] of spots) {
      const p = placedOf(id);
      if (!p) continue;
      s.scene.root.position.set(p.x, 0, p.z);
      if (s.bake) s.bake.holder.position.set(p.x, 0, p.z);
      if (s.bake) s.bake.holder.visible = !s.live;
    }
    shadowsDirty = true;
  }

  function disposeGeometries(root: Object3D): void {
    root.traverse((o) => {
      const g = (o as unknown as { geometry?: { dispose(): void } }).geometry;
      g?.dispose();
    });
  }

  /** the entry's loose town drawn (held: the build, the fold), else its merged copies; both kept */
  function loose(on: boolean): void {
    if (scape?.loose) {
      scape.loose.visible = on;
      scape.merged.group.visible = !on;
    }
    for (const s of spots.values()) {
      if (!s.walls) continue;
      const w = s.scene.world();
      if (w) w.visible = on;
      s.walls.merged.group.visible = !on;
    }
    shadowsDirty = true;
  }

  /** the entry's loose townscape out of the scene (its merged copy is drawn from now on) */
  function dropLoose(s: { loose: Group | null; merged: Merged }): void {
    if (!s.loose) return;
    scene.remove(s.loose);
    disposeGeometries(s.loose);
    s.loose = null;
    s.merged.group.visible = true;
  }

  function syncScape(isNight: boolean): void {
    if (!plan) return;
    // where an office carves under its floor (California's bowl), in the town's frame: the lawn leaves a hole there
    const pits = sites.flatMap((t) => {
      const p = placedOf(t.id);
      const own = lookOf(t.office.map).pits?.(t.office, t.grow.stage) ?? [];
      return p ? own.map((b) => ({ x0: b.x0 + p.x, x1: b.x1 + p.x, z0: b.z0 + p.z, z1: b.z1 + p.z })) : [];
    });
    lastPits = pits;
    // the day's lawn and road in both themes (the sky lights them); the hero's film its theme's
    const dark = themed && isNight;
    const sSig = `${String(plan.seed)}|${JSON.stringify(plan.lawn)}|${String(plan.road.length)}|${String(plan.trees.length)}|${String(plan.offices.length)}|${dark ? 'n' : 'd'}|${JSON.stringify(pits)}|${beyond === null ? '' : String(beyond.trees.length)}`;
    if (scape?.sig === sSig) return;
    if (scape) {
      scene.remove(scape.merged.group);
      scape.merged.dispose();
      dropLoose(scape);
    }
    const built = buildTownscape(plan, STYLE, dark ? NIGHT : DAY, dark ? TOWN_NIGHT : TOWN_DAY, pits, beyond);
    const { root, lift } = built;
    const merged = mergeRoot(root, STYLE, STYLE.shadows);
    scene.add(merged.group);
    if (opts.entry === true) {
      // the entry build grows the loose things in: the merged copy waits, hidden, for settle(); the
      // loose ones stay for the next hold() (the hero's fold and its next loop's wave)
      merged.group.visible = !held;
      root.visible = held;
      scene.add(root);
      scape = { sig: sSig, merged, loose: root, lift };
    } else {
      disposeGeometries(root);
      scape = { sig: sSig, merged, loose: null, lift };
    }
    lightUp(isNight);
    shadowsDirty = true;
  }

  function count(): void {
    if (!probe) return;
    probe.offices = spots.size;
    let from = scape?.merged.from ?? 0;
    let to = scape?.merged.to ?? 0;
    for (const s of spots.values()) {
      if (!s.bake) continue;
      from += s.bake.merged.from;
      to += s.bake.merged.to;
    }
    probe.merged = { from, to };
  }

  /**
   * The one light of the town, the sun's shadow over the whole lawn: the sky's (sky.ts lightOf, set
   * by aimSun), else the day palette's fixed one (the hero's: its theme's); the backdrop the theme's
   * page, untouched (as the office's).
   */
  function lightUp(isNight: boolean): void {
    if (!plan) return;
    const pal = themed && isNight ? NIGHT : DAY;
    v3.tint(pal);
    if (themed && isNight && opts.nightLift !== undefined) {
      v3.hemi.intensity *= opts.nightLift;
      sun.intensity *= opts.nightLift;
    }
    v3.sky(opts.sky ? (isNight ? opts.sky.night : opts.sky.day) : (isNight ? NIGHT : DAY).bg);
    const lamps = skyLight?.lamps ?? 0;
    pools.forEach((l, i) => {
      l.intensity = (poolI[i] ?? 0) * lamps;
    });
    // the night's lift on the day lawn and road (townscape.ts NIGHT_GLOW)
    scape?.lift(skyLight?.lift ?? 0);
    if (STYLE.shadows) {
      sun.castShadow = true;
      sun.shadow.mapSize.set(2048, 2048);
      sun.shadow.radius = 4;
      sun.shadow.bias = -0.0008;
    }
    sunKey = '';
    aimSun(shownFit());
  }

  /**
   * The sun and its shadow over the ground in view (page town-live p4): the whole lawn at the fit,
   * only what shows when he zooms in, so the shadow's 2048 px land where he looks and stay sharp
   * close up. Never wider than the lawn's.
   */
  function aimSun(f: Fit | null): void {
    if (!plan) return;
    const { lawn } = plan;
    const lawnHalf = Math.max(lawn.x1 - lawn.x0, lawn.z1 - lawn.z0) * 0.62 + 2;
    let cx = (lawn.x0 + lawn.x1) / 2;
    let cz = (lawn.z0 + lawn.z1) / 2;
    let half = lawnHalf;
    if (f && f !== base && base) {
      const corners = [groundAt(f, 0, 0, 0), groundAt(f, f.widthPx, 0, 0), groundAt(f, 0, f.heightPx, 0), groundAt(f, f.widthPx, f.heightPx, 0)];
      const xs = corners.map((c) => c.x);
      const zs = corners.map((c) => c.z);
      const x0 = Math.min(...xs);
      const x1 = Math.max(...xs);
      const z0 = Math.min(...zs);
      const z1 = Math.max(...zs);
      // the offices' walls stand up out of the view's ground: a margin for them and their shadows
      const near = Math.max(x1 - x0, z1 - z0) * 0.62 + 4;
      if (near < lawnHalf) {
        cx = (x0 + x1) / 2;
        cz = (z0 + z1) / 2;
        half = near;
      }
    }
    const key = `${cx.toFixed(2)}|${cz.toFixed(2)}|${half.toFixed(2)}`;
    if (key === sunKey) return;
    sunKey = key;
    aimAt = { x: cx, z: cz };
    placePools();
    // the office's sun: up and a little to the west of the room, as world.ts puts it; under a sky,
    // from the sun's or the moon's side (sky-light.ts), as far
    if (skyLight) applyLight(v3.hemi, sun, skyLight, { x: cx, y: 0, z: cz }, TOWN_KEY_DIST);
    else {
      sun.position.set(cx - 5 * 3, 14 * 3, cz - 1 * 3);
      sun.target.position.set(cx, 0, cz);
      sun.target.updateMatrixWorld();
    }
    if (STYLE.shadows) {
      const cam = sun.shadow.camera;
      cam.left = -half;
      cam.right = half;
      cam.top = half;
      cam.bottom = -half;
      cam.near = 1;
      cam.far = 140;
      cam.updateProjectionMatrix();
      shadowsDirty = true;
    }
  }

  /** the night's pools to the offices nearest where the sun aims (town/pools.ts), lit by the sky's lamps */
  function placePools(): void {
    if (pools.length === 0 || !plan) return;
    const offices = plan.offices.flatMap((p) => {
      const o = spots.get(p.id)?.scene.office();
      return o ? [{ x: p.x, z: p.z, pools: poolsOf(o) }] : [];
    });
    const placed = townPools(offices, aimAt, pools.length);
    const key = JSON.stringify(placed);
    if (key === poolKey) return;
    poolKey = key;
    poolI = pools.map((l, i) => {
      const p = placed[i];
      if (p) l.position.set(p.x, POOL.y, p.z);
      return p?.i ?? 0;
    });
    const lamps = skyLight?.lamps ?? 0;
    pools.forEach((l, i) => {
      l.intensity = (poolI[i] ?? 0) * lamps;
    });
  }

  function applyFit(): void {
    if (!plan || size.w <= 0) return;
    base = townFit(plan, size.w, size.maxH, opts.box === true, opts.pad ?? 0);
    applyView();
  }

  /** what is drawn: the dive's way, else the town's fit through his camera */
  const shownFit = (): Fit | null => view ?? (base && !sameCam(cam, FIT) ? viewFit(base, cam) : base);

  /**
   * The offices posed live at this view: `near`, the ones in view once it is close enough (at most
   * LIVE_MAX, the nearest the middle first), and `popping`, the ones whose newcomer pops in (still()),
   * live a moment wherever the camera is
   */
  function liveSets(): { near: readonly string[]; popping: readonly string[] } {
    const f = shownFit();
    if (!f || !base || !plan) return { near: [], popping: [] };
    // the hero's few offices: all of them, always (an office back in view would bake its walls again: ~100 ms)
    if (opts.allLive === true) return { near: plan.offices.map((p) => p.id), popping: [] };
    const now = Date.now() / 1000;
    const popping = plan.offices.filter((p) => (spots.get(p.id)?.popUntil ?? 0) > now).map((p) => p.id);
    // close enough: the fit's width shows LIVE_ZOOM times less of the town
    if ((base.right - base.left) / (f.right - f.left) < LIVE_ZOOM) return { near: [], popping };
    const cx = f.widthPx / 2;
    const cy = f.heightPx / 2;
    const near = plan.offices
      .filter((p) => inView(f, p))
      .map((p) => {
        const c = toScreen(f, { x: (p.fp.x0 + p.fp.x1) / 2, y: 0, z: (p.fp.z0 + p.fp.z1) / 2 });
        return { id: p.id, d: Math.hypot(c.x - cx, c.y - cy) };
      })
      .sort((a, b) => a.d - b.d)
      .slice(0, LIVE_MAX)
      .map((x) => x.id);
    return { near, popping };
  }

  function applyView(): void {
    const f = shownFit();
    if (!f) return;
    v3.look(f);
    if (plan) {
      // the clipping planes round the whole town (town/camera.ts townDepth): view.ts's fixed near plane cut its front offices off
      const { near, far } = townDepth(plan, f.target, beyond?.far);
      v3.camera.near = near;
      v3.camera.far = far;
      v3.camera.updateProjectionMatrix();
    }
    v3.size(f);
    if (over) {
      over.setPixelRatio(Math.min(window.devicePixelRatio || 1, STYLE.maxPixelRatio));
      over.setSize(f.widthPx, f.heightPx, false);
    }
    aimSun(f);
  }

  /** the canvas point (css px) seen at height y on the ground (the iso view's ray down) */
  function groundAt(f: Fit, x: number, y: number, h: number): { x: number; z: number } {
    const a = x * f.scale + f.left;
    const b = f.top - y * f.scale;
    const t = (h - f.target.y - b * UP.y) / BACK.y;
    return { x: f.target.x + a * RIGHT.x + b * UP.x + t * BACK.x, z: f.target.z + a * RIGHT.z + b * UP.z + t * BACK.z };
  }

  /** an office's ground on the canvas: whether any of it shows in this view */
  function inView(f: Fit, p: Placed): boolean {
    const corners = [
      { x: p.fp.x0, z: p.fp.z0 }, { x: p.fp.x1, z: p.fp.z0 }, { x: p.fp.x0, z: p.fp.z1 }, { x: p.fp.x1, z: p.fp.z1 },
    ].map((c) => toScreen(f, { x: c.x, y: 0, z: c.z }));
    const x0 = Math.min(...corners.map((c) => c.x));
    const x1 = Math.max(...corners.map((c) => c.x));
    const y0 = Math.min(...corners.map((c) => c.y)) - 3 / f.scale;
    const y1 = Math.max(...corners.map((c) => c.y));
    return x1 > 0 && x0 < f.widthPx && y1 > 0 && y0 < f.heightPx;
  }

  /**
   * test mode, asked by the camera-path check (window.__biseTownDrawn, never per frame): each office now,
   * whether its ground is in view, whether something of it is drawn, where (its visible meshes' world
   * box) and that box's depth against the camera's near and far
   */
  function drawnNow(): Drawn[] {
    const f = shownFit();
    if (!f || !plan) return [];
    const cam = v3.camera;
    cam.updateMatrixWorld();
    const q = new Vector3();
    return plan.offices.map((p): Drawn => {
      const s = spots.get(p.id);
      const obj = s?.live === true ? s.scene.root : s?.bake?.holder.visible === true ? s.bake.holder : null;
      const shown = obj !== null && obj.parent === scene;
      const b = shown ? drawnBox(obj) : null;
      let depth: Drawn['depth'] = null;
      if (b) {
        let min = Infinity;
        let max = -Infinity;
        for (const x of [b.min.x, b.max.x]) for (const y of [b.min.y, b.max.y]) for (const z of [b.min.z, b.max.z]) {
          const d = -q.set(x, y, z).applyMatrix4(cam.matrixWorldInverse).z;
          min = Math.min(min, d);
          max = Math.max(max, d);
        }
        depth = { min, max, near: cam.near, far: cam.far };
      }
      return {
        id: p.id, inView: inView(f, p), shown, live: s?.live === true, fp: { ...p.fp },
        box: b ? { x0: b.min.x, x1: b.max.x, y0: b.min.y, y1: b.max.y, z0: b.min.z, z1: b.max.z } : null,
        depth,
        door: toScreen(f, { x: p.door.x, y: 0, z: p.door.z }),
      };
    });
  }

  if (probe) (window as unknown as { __biseTownDrawn?: () => Drawn[] }).__biseTownDrawn = drawnNow;

  const api: TownRenderer = {
    setTown(os) {
      // what makes two towns the same layout: each office's ground (its map, its desks, its growth), not its bims
      const key = os.map((o) => `${o.id}:${o.office.map}:${o.office.size}:${String(o.grow.stage)}:${String(o.grow.garden)}:${o.home === true ? 'h' : ''}`).join('|');
      if (plan === null || key !== planKey) {
        planKey = key;
        plan = townPlan(os.map((o) => siteOf(o.id, o.office, o.grow, o.home === true)), townSeed(os.map((o) => o.id)));
        beyond = opts.endless === true ? beyondOf(plan) : null;
        if (probe) probe.check = { ...crossings(plan) };
      }
      sites = os;
      const keep = new Set(os.map((o) => o.id));
      for (const id of [...spots.keys()]) if (!keep.has(id)) dropSpot(id);
      for (const o of os) spotOf(o.id);
      syncScape(night ?? false);
      place();
      applyFit();
      count();
    },
    still(id, step) {
      const p = placedOf(id);
      if (!p) return false;
      const s = spotOf(id);
      const sig = stillSig(step);
      if (s.bake?.sig === sig) return false;
      // a bim new since its last still: the office goes live for its pop and its glitter (liveIds), baked after
      const names = new Set(step.frames.map((f) => f.name));
      const was = s.names;
      s.names = names;
      if (was !== null && !reducedNow() && [...names].some((n) => !was.has(n))) {
        s.popUntil = Date.now() / 1000 + POP_LIVE_S;
        return true;
      }
      if (Date.now() / 1000 < s.popUntil) return false;
      if (posed(s, step, step.frames[0]?.pose.since ?? 0) === null && !s.scene.office()) return false;
      // baked where it stands in its own units, then placed: a new plan moves the bake, never bakes it again
      s.scene.root.position.set(0, 0, 0);
      syncChairs(s);
      const merged = mergeRoot(s.scene.root, STYLE, STYLE.shadows);
      const glows: [MeshLambertMaterial, MeshLambertMaterial][] = [];
      for (const c of merged.group.children) {
        // the materials it kept (a lit screen, the glass) are its scene's: its own copies, so a rebuilt scene can't take them
        const m = (c as unknown as { material: Material }).material;
        if (m === merged.baked) continue;
        const copy = m.clone();
        (c as unknown as { material: Material }).material = copy;
        // a glow (kit.ts: a lantern, a shade) and a desk's lamp and screen follow the sky's lamps on its scene's own
        if (m.userData.live === true && 'emissiveIntensity' in m) glows.push([copy as MeshLambertMaterial, m as MeshLambertMaterial]);
      }
      s.scene.root.position.set(p.x, 0, p.z);
      dropBake(s);
      const holder = new Group();
      holder.add(merged.group);
      holder.position.set(p.x, 0, p.z);
      holder.visible = !s.live;
      scene.add(holder);
      s.bake = { sig, merged, holder, glows };
      // the bake's pose is no longer the live one: a live office poses itself at its next frame
      shadowsDirty = true;
      count();
      return true;
    },
    live(id, step, t) {
      const s = spots.get(id);
      if (!s) return 'still';
      // a popping office stays live while its newcomer still walks: baked again only with it at its spot (no visible switch)
      const now = Date.now() / 1000;
      if (s.popUntil > now && step.moving) s.popUntil = Math.max(s.popUntil, now + POP_TAIL_S);
      const m = posed(s, step, t);
      walls(s, step);
      shadowsDirty = true;
      return m ?? 'still';
    },
    popWake() {
      const now = Date.now() / 1000;
      let left: number | null = null;
      for (const s of spots.values()) if (s.popUntil > now) left = Math.max(left ?? 0, s.popUntil - now);
      return left;
    },
    idleWake() {
      let soon: number | null = null;
      for (const s of spots.values()) {
        const w = s.live ? s.scene.idleWake() : null;
        if (w !== null) soon = soon === null ? w : Math.min(soon, w);
      }
      return soon;
    },
    liveIds() {
      const { near, popping } = liveSets();
      return [...near, ...popping.filter((id) => !near.includes(id))];
    },
    popOnlyIds() {
      const { near, popping } = liveSets();
      return popping.filter((id) => !near.includes(id));
    },
    setSize(widthPx, maxHeightPx) {
      size = { w: Math.round(widthPx), maxH: Math.round(maxHeightPx) };
      applyFit();
      return base?.heightPx ?? 0;
    },
    townFit: () => base,
    shownFit,
    diveFit(id, widthPx) {
      const p = placedOf(id);
      const o = spots.get(id)?.scene.office();
      // a bound box (opts.box): the office whole in that box, as its filled Office opens
      return p && o ? diveFit(p, o, widthPx, opts.box === true ? size.maxH : 0) : null;
    },
    setView(f) {
      view = f;
      applyView();
    },
    setCam(c) {
      cam = c;
      applyView();
    },
    frame(isNight, sky) {
      if (v3.lost() || !plan) return false;
      // the sky (sky.ts): a new target blends from the drawn light; each blend frame lights again
      let moving = false;
      let relit = false;
      if (sky) {
        const was = blend;
        blend = retarget(blend, sky.sky, sky.t, sky.reduced);
        moving = blending(blend, sky.t);
        if (moving || blend !== was) {
          // one light for both themes (direction A), the office's
          skyLight = themed ? null : lightOf(blendAt(blend, sky.t));
          relit = true;
        }
      } else if (blend !== null) {
        blend = null;
        skyLight = null;
        relit = true;
      }
      if (night !== isNight) {
        night = isNight;
        syncScape(isNight);
        lightUp(isNight);
      } else if (relit) {
        lightUp(isNight);
        shadowsDirty = true;
      }
      // the offices' lamps, screens and glows fade with the sky (never pop), the baked ones' too (their live
      // materials' copies follow their scene's: emissive only, no light); no snow in the town yet (its lawn)
      for (const s of spots.values()) {
        s.scene.weather(skyLight ? skyLight.lamps : null, 0);
        for (const [copy, own] of s.bake?.glows ?? []) {
          copy.emissiveIntensity = own.emissiveIntensity;
          copy.color.copy(own.color);
        }
      }
      placePools();
      // the live offices' scenes in the town, the others' bakes
      const live = new Set(api.liveIds());
      for (const [id, s] of spots) {
        const on = live.has(id) && s.scene.office() !== null;
        if (on === s.live) continue;
        s.live = on;
        if (on) scene.add(s.scene.root);
        else {
          scene.remove(s.scene.root);
          dropWalls(s);
        }
        if (s.bake) s.bake.holder.visible = !on;
        shadowsDirty = true;
      }
      const t0 = performance.now();
      // the entry build moves the loose things every frame: their shadow with them
      if (shadowsDirty || held) {
        renderer.shadowMap.needsUpdate = true;
        shadowsDirty = false;
      }
      v3.render();
      if (over) {
        v3.camera.layers.set(UP_LAYER);
        over.render(scene, v3.camera);
        v3.camera.layers.set(0);
      }
      if (probe) {
        probe.frames++;
        probe.ms = performance.now() - t0;
        probe.recent = [...probe.recent.slice(-29), probe.ms];
        probe.calls = renderer.info.render.calls;
        probe.live = [...spots].filter(([, s]) => s.live).map(([id]) => id);
        probe.popping = [...spots].filter(([, s]) => s.popUntil > Date.now() / 1000).map(([id]) => id);
        probe.heads = probe.live.flatMap((id) =>
          (spots.get(id)?.scene.names() ?? []).flatMap((name) => {
            const h = api.headOf(id, name);
            return h ? [{ id, name, ...h }] : [];
          }),
        );
      }
      return moving;
    },
    signAt(id) {
      const f = shownFit();
      const p = placedOf(id);
      const o = spots.get(id)?.scene.office() ?? sites.find((x) => x.id === id)?.office;
      if (!f || !p || !o) return null;
      v.set(p.x + o.x0 + o.W / 2, SIGN_Y, p.z - 0.3);
      const s = toScreen(f, v);
      if (s.x < 0 || s.x > f.widthPx || s.y < 0 || s.y > f.heightPx) return null;
      return s;
    },
    pick(x, y) {
      const f = shownFit();
      if (!f || !plan) return null;
      // from the roofs down: the first office the ray meets
      for (let h = 2.6; h >= 0; h -= 0.2) {
        const g = groundAt(f, x, y, h);
        const hit = plan.offices.find((p) => g.x > p.fp.x0 && g.x < p.fp.x1 && g.z > p.fp.z0 && g.z < p.fp.z1);
        if (hit) return hit.id;
      }
      return null;
    },
    headOf(id, name) {
      const f = shownFit();
      const s = spots.get(id);
      if (!f || !s?.live) return null;
      const top = s.scene.headTop(name, v);
      if (!top) return null;
      const p = toScreen(f, top);
      if (p.x < 0 || p.y < -20 || p.x > f.widthPx || p.y > f.heightPx) return null;
      const feet = s.scene.feet(name, v);
      return { ...p, feet: feet ? toScreen(f, feet).y : p.y };
    },
    placedOf: (id) => placedOf(id) ?? null,
    pickBim(x, y) {
      const f = shownFit();
      if (!f) return null;
      for (const [id, s] of spots) {
        if (!s.live) continue;
        const targets: Target[] = s.scene.names().map((name) => {
          const head = s.scene.headTop(name, v);
          const headPx = head ? toScreen(f, head) : null;
          const feet = s.scene.feet(name, v);
          return { name, head: headPx, feet: feet ? toScreen(f, feet) : null, desk: null };
        });
        const name = pickAt(targets, x, y);
        if (name !== null) return { id, name };
      }
      return null;
    },
    copyTo(target) {
      v3.copyTo(target);
    },
    onLost(cb) {
      v3.onLost(cb);
    },
    entry: {
      parts() {
        if (!plan) return null;
        const { lawn } = plan;
        return {
          scape: scape?.loose ?? null,
          centre: { x: (lawn.x0 + lawn.x1) / 2, z: (lawn.z0 + lawn.z1) / 2 },
          lawn: { x0: lawn.x0, x1: lawn.x1, z0: lawn.z0, z1: lawn.z1 },
          pits: lastPits,
          offices: [...spots].flatMap(([id, s]) => {
            const o = s.scene.office();
            if (!o || !s.live) return [];
            const world = held ? s.scene.world() : null;
            const bims = s.scene.names().flatMap((name) => {
              const body = s.scene.body(name);
              return body ? [{ name, body }] : [];
            });
            return [{ id, root: s.scene.root, world, width: o.W, bims }];
          }),
        };
      },
      warm(on) {
        // the unseen warm frame draws the merged copies too, so settle() uploads and compiles nothing
        if (!held) return;
        if (scape) scape.merged.group.visible = on;
        for (const s of spots.values()) if (s.walls) s.walls.merged.group.visible = on;
        shadowsDirty = true;
      },
      settle() {
        if (!held) return;
        held = false;
        loose(false);
      },
      hold() {
        if (held) return;
        held = true;
        loose(true);
      },
    },
    dispose() {
      for (const id of [...spots.keys()]) dropSpot(id);
      if (scape) {
        scene.remove(scape.merged.group);
        scape.merged.dispose();
        dropLoose(scape);
        scape = null;
      }
      v3.dispose();
      over?.dispose();
      if (probe) delete (window as unknown as { __biseTown?: Probe }).__biseTown;
      if (probe) delete (window as unknown as { __biseTownDrawn?: unknown }).__biseTownDrawn;
    },
  };
  return api;
};
