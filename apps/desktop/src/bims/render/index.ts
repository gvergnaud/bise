// les bims' renderer (three.js), the only module that imports three: Office.tsx loads it with
// import('./render') so the list view never pays for it (its own chunk, budget in
// electron.vite.config.ts). The minimap is one office scene (office-scene.ts: the world, the bims
// posed by pose.ts, his paper planes, the eggs, the unlock moment) drawn through one view (view.ts:
// the renderer, the iso camera, the office's light) at his camera's zoom (camera.ts viewFit); it
// answers where heads are and what is under a point (pick.ts). It renders only when frame() is
// called and says how the loop must run next (Motion). The look is style.ts. The sky (sky.ts): each
// frame blends the drawn sky to Look.sky (SKY_FADE_S at ~10 fps: Motion 'blend', then rest; reduced
// motion: a cut) and puts its one light (both themes, bims-light direction A) on the lights
// (sky-light.ts), the lamps and their pools, the slab's night lift and the snow (office-scene
// weather); the page under the cube is the theme's own. falling() says what falls and the
// building's outline for the CSS rain.
// In test mode (window.bise.test) it exposes window.__biseBims = { frames, trace, skate, head, at, zoom, cam, sky } for
// the headless checks (bims-qa; skate: each skater's phase per frame, budget.spec (5)).
import { Vector3 } from 'three';
import type { BimFrame, Look, MakeRenderer, Motion, OfficeRenderer } from '../model';
import { FIT, type Cam } from '../zoom';
import { type Fit, boxSite, fitSite, groundAllOf, siteOf, toScreen, viewFit, wideSiteOf } from './camera';
import { NO_GROW } from '../growth';
import { pickAt, pickObjectAt, type ObjTarget, type Target } from './pick';
import type { Rig } from './pose';
import { sinceArrived, skateAt } from '../skate';
import { STYLE } from './style';
import { lightOffice } from './world';
import { applyLight } from './sky-light';
import { blendAt, blending, lightOf, retarget, sameSky, type Blend, type Light } from '../sky';
import { louder } from './pose';
import { makeOfficeScene } from './office-scene';
import { myLook, onMyLook } from '../mine';
import { makeView } from './view';
import { ghostBox } from './ghost';

/** the building's box for the rain's outline: past its walls, up to their tops (the tallest map's) */
const HULL_PAD = 0.3;
const HULL_H = 2.4;

/** the convex hull of points (monotone chain), counter-clockwise */
function hullOf(ps: { x: number; y: number }[]): { x: number; y: number }[] {
  const p = [...ps].sort((a, b) => a.x - b.x || a.y - b.y);
  if (p.length < 3) return p;
  const cross = (o: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }): number => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
  const lower: { x: number; y: number }[] = [];
  for (const q of p) {
    while (lower.length >= 2 && cross(lower[lower.length - 2] as { x: number; y: number }, lower[lower.length - 1] as { x: number; y: number }, q) <= 0) lower.pop();
    lower.push(q);
  }
  const upper: { x: number; y: number }[] = [];
  for (const q of p.reverse()) {
    while (upper.length >= 2 && cross(upper[upper.length - 2] as { x: number; y: number }, upper[upper.length - 1] as { x: number; y: number }, q) <= 0) upper.pop();
    upper.push(q);
  }
  return [...lower.slice(0, -1), ...upper.slice(0, -1)];
}

/** from this width (css px) the camera fits the building and a little ground (wideSiteOf); under it the building */
const WIDE_FIT_PX = 480;

/** a world box's 8 corners on the canvas: the rectangle they span, css px (a plugin object's hit area and foot) */
function screenBox(f: Fit, b: { readonly min: { x: number; y: number; z: number }; readonly max: { x: number; y: number; z: number } }): { left: number; top: number; right: number; bottom: number } {
  const r = { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity };
  for (const x of [b.min.x, b.max.x])
    for (const y of [b.min.y, b.max.y])
      for (const z of [b.min.z, b.max.z]) {
        const p = toScreen(f, { x, y, z });
        r.left = Math.min(r.left, p.x);
        r.right = Math.max(r.right, p.x);
        r.top = Math.min(r.top, p.y);
        r.bottom = Math.max(r.bottom, p.y);
      }
  return r;
}

/** what the headless check reads (test mode only) */
interface Probe {
  frames: number;
  /** the slowest frame drawn, ms (the render and the GPU's finish); null: not measured (a check sets 0 to start) */
  slowest: number | null;
  /** each bim's place per frame; y its height off the floor (a skater up a ramp, in the bowl: below 0) */
  trace: { t: number; name: string; x: number; z: number; y: number; op: number }[];
  /** each drawn frame of an arrived skater: its session n (skate.ts skateAt) and whether it rides its line or rests */
  skate: { t: number; name: string; n: number; phase: 'line' | 'rest' }[];
  head(name: string): { x: number; y: number } | null;
  /** a floor point of the office on the canvas, css px (a check pinches in on a corner there); null: no fit yet */
  at(x: number, y: number, z: number): { x: number; y: number } | null;
  /** his camera's zoom now (zoom.ts Cam; 1 = the fit): the follow check reads how close it looks */
  zoom(): number;
  /** his whole camera now (zoom and where it looks): his camera check reads whether a wheel moved it */
  cam(): Cam;
  /** the sky drawn at the last frame (sky.ts lightOf) and whether its blend still runs; null: the theme's fixed light */
  sky(): { light: Light; blending: boolean } | null;
}
const TRACE_MAX = 3000;

const testMode = (): boolean => (window as Partial<Window> & { bise?: { test?: boolean } }).bise?.test === true;

/** the town on bise's thread (render/town.ts): in this chunk too, so three loads once */
export { makeTownRenderer } from './town';

export const makeRenderer: MakeRenderer = (canvas, opts) => {
  // the office's shadow follows its bims: drawn again every frame (a still office draws none)
  const view = makeView(canvas, { shadowAuto: true, transparent: opts?.transparent === true });
  const office = makeOfficeScene(STYLE);
  view.scene.add(office.root);
  // his bim's look (page bim-pick): main's bim in it now and at every new pick
  office.setMine(myLook());
  const stopMine = onMyLook(() => {
    office.setMine(myLook());
  });
  /** the whole site fitted to the canvas, and what shows through his camera (fit = base at FIT) */
  let base: Fit | null = null;
  let fit: Fit | null = null;
  let cam: Cam = FIT;
  let size = { w: 0, maxH: 0, box: false };
  const v = new Vector3();
  /** the sky's blend (sky.ts): from the drawn sky to the slot's, then rest; null: no sky (the theme's light) */
  let blend: Blend | null = null;
  let lastT = 0;
  let lastLook: Look | null = null;
  let drawnSky: Light | null = null;
  let skyMoving = false;
  /** main's name in the office drawn (pointOf's tray) */
  let lead: string | null = null;

  const probe: Probe | null = testMode()
    ? { frames: 0, slowest: null, trace: [], skate: [], head: (name) => headOf(name), at: (x, y, z) => (fit ? toScreen(fit, { x, y, z }) : null), zoom: () => cam.zoom, cam: () => cam, sky: () => (drawnSky ? { light: drawnSky, blending: skyMoving } : null) }
    : null;
  if (probe) (window as unknown as { __biseBims?: Probe }).__biseBims = probe;

  /** the office's light and backdrop, after its world was built: the theme's, and the sky's on top at t */
  function lightUp(t: number, look: Look | null): void {
    const o = office.office();
    const pal = office.pal();
    if (!o || !pal) return;
    lightOffice(view.hemi, view.sun, pal, o, STYLE);
    const paper = office.sky();
    skyMoving = false;
    if (look?.sky === undefined) {
      blend = null;
      drawnSky = null;
      office.weather(null, 0);
      if (paper !== null) view.sky(paper);
      return;
    }
    // his drag or pinch holds a new blend until he lets go (bims m_33030)
    blend = retarget(blend, look.sky, t, look.reduced, look.hold === true);
    const sky = blendAt(blend, t);
    // one light for both themes (direction A); the page under the cube is the theme's, untouched
    const l = lightOf(sky);
    applyLight(view.hemi, view.sun, l, { x: o.x0 + o.W / 2, y: 0, z: o.D / 2 });
    office.weather(l.lamps, sky.cover, l.lift);
    if (paper !== null) view.sky(paper);
    drawnSky = l;
    // a blend his gesture holds: the loop's rate until he lets go, then it starts
    skyMoving = blending(blend, t) || (look.hold === true && !sameSky(blend.to, look.sky));
  }

  function applyFit(): void {
    const o = office.office();
    if (!o || size.w <= 0) return;
    // 320: the building; wide: a little ground past it. The garden past that is cropped (Office.tsx's
    // soft fade), so the office never gets smaller as the project grows (bims m_28178)
    const site = size.w >= WIDE_FIT_PX ? wideSiteOf(o) : siteOf(o);
    base = size.box ? boxSite(site, size.w, size.maxH) : fitSite(site, size.w, size.maxH);
    applyView();
    view.size(base);
  }

  // his pinch and pan: the fit seen closer (camera.ts viewFit), the canvas's size unchanged
  function applyView(): void {
    if (!base) return;
    fit = viewFit(base, cam);
    view.look(fit);
  }

  function headOf(name: string): { x: number; y: number; feet: number } | null {
    if (!fit) return null;
    const top = office.headTop(name, v);
    if (!top) return null;
    const p = toScreen(fit, top);
    if (p.x < 0 || p.y < -20 || p.x > fit.widthPx || p.y > fit.heightPx) return null;
    const feet = office.feet(name, v);
    return { ...p, feet: feet ? toScreen(fit, feet).y : p.y };
  }

  /** the headless checks' trace of each posed bim (test mode) */
  const traced = (t: number) => (f: BimFrame, rig: Rig, skating: boolean): void => {
    if (!probe) return;
    probe.trace.push({ t, name: f.name, x: rig.x, z: rig.z, y: rig.lift ?? 0, op: rig.opacity });
    if (probe.trace.length > TRACE_MAX) probe.trace.splice(0, probe.trace.length - TRACE_MAX);
    const p = f.pose.seg.pastime;
    const park = office.office()?.park;
    if (skating && p?.kind === 'skate' && park) {
      const now = skateAt(f.name, p, sinceArrived(f.pose), park);
      probe.skate.push({ t, name: f.name, n: now.n, phase: now.move === null ? 'rest' : 'line' });
      if (probe.skate.length > TRACE_MAX) probe.skate.splice(0, probe.skate.length - TRACE_MAX);
    }
  };

  const api: OfficeRenderer = {
    setOffice(o, as) {
      lead = as.find((a) => a.main)?.name ?? null;
      office.setOffice(o, as);
      lightUp(lastT, lastLook);
      applyFit();
    },
    setObjects(c) {
      return office.setObjects(c);
    },
    setSize(widthPx, maxHeightPx, box = false) {
      size = { w: Math.round(widthPx), maxH: Math.round(maxHeightPx), box };
      applyFit();
      return base?.heightPx ?? 0;
    },
    setCam(c) {
      cam = c;
      applyView();
    },
    frame(t, frames, look: Look): Motion {
      if (view.lost() || !office.office()) return 'still';
      // the growth moved: the world again, the ground may be wider; the theme: the world and its light again
      const grew = office.prepare(look);
      lastT = t;
      lastLook = look;
      lightUp(t, look);
      if (grew) applyFit();
      if (!fit) return 'still';
      const posed = office.frame(t, frames, look, fit, probe ? traced(t) : undefined);
      if (posed === null) return 'still';
      // the sky's blend: ~10 fps over its 30 s (reduced motion: a cut, never a blend), then rest
      const motion = skyMoving ? louder(posed, 'blend') : posed;
      const t0 = probe ? performance.now() : 0;
      view.render();
      if (probe) {
        probe.frames++;
        // the slowest frame's cost, the GPU's work included, only while a check measures (budget.spec (7): the lamp pools)
        if (probe.slowest !== null) {
          view.renderer.getContext().finish();
          probe.slowest = Math.max(probe.slowest, performance.now() - t0);
        }
      }
      return motion;
    },
    idleWake: () => (view.lost() ? null : office.idleWake()),
    headOf,
    spotOf(name) {
      if (!fit || !base) return null;
      const top = office.headTop(name, v);
      if (!top) return null;
      const head = toScreen(fit, top);
      const feet = office.feet(name, v);
      if (!feet) return null;
      const mid = toScreen(base, { x: feet.x, y: 0.5, z: feet.z });
      return { x: head.x, y: head.y, u: mid.x / base.widthPx, v: mid.y / base.heightPx };
    },
    objectTopOf(key) {
      const top = office.objectTop(key);
      if (!fit || !top) return null;
      const p = toScreen(fit, top);
      return p.x < 0 || p.y < 0 || p.x > fit.widthPx || p.y > fit.heightPx ? null : p;
    },
    pieceOf(stage) {
      const at = office.pieceAt(stage);
      if (!fit || !at) return null;
      // a piece out of the frame (the garden cropped at 320): the frame's edge nearest it (bims m_28178)
      const p = toScreen(fit, { ...at, y: 1.2 });
      const m = 12;
      return { x: Math.min(fit.widthPx - m, Math.max(m, p.x)), y: Math.min(fit.heightPx - m, Math.max(m * 2, p.y)) };
    },
    pointOf(what) {
      const o = office.office();
      if (!fit || !o) return null;
      let at: { x: number; y: number; z: number } | null = null;
      if (what === 'tray') {
        // the inbox stand by the door; on a plan with none, the tray of main's desk (or its top)
        const d = lead === null ? undefined : office.desk(lead);
        at = office.inboxTop() ?? (d ? (d.tray ?? d.top) : null);
      } else {
        const s = o.ghosts?.[what.ghost];
        if (s) {
          const b = ghostBox(o.furniture, s);
          at = { x: (b.x0 + b.x1) / 2, y: 0, z: (b.z0 + b.z1) / 2 };
        }
      }
      if (!at) return null;
      const p = toScreen(fit, at);
      return p.x < 0 || p.y < 0 || p.x > fit.widthPx || p.y > fit.heightPx ? null : { x: p.x, y: p.y };
    },
    pick(x, y) {
      if (!fit || !office.pal()) return null;
      const f = fit;
      const targets: Target[] = [];
      const deskR = 0.8 / f.scale;
      for (const name of office.names()) {
        const d = office.desk(name);
        const head = office.headTop(name, v);
        const headPx = head ? toScreen(f, head) : null;
        const feet = office.feet(name, v);
        targets.push({
          name,
          head: headPx,
          feet: feet ? toScreen(f, feet) : null,
          desk: d ? { ...toScreen(f, d.top), r: deskR } : null,
        });
      }
      return pickAt(targets, x, y);
    },
    objectAt(x, y) {
      if (!fit) return null;
      const f = fit;
      const targets: ObjTarget[] = [...office.objectBoxes()].map(([key, b]) => ({ key, ...screenBox(f, b) }));
      return pickObjectAt(targets, x, y);
    },
    objectFootOf(key) {
      const b = office.objectBoxes().get(key);
      return fit && b ? screenBox(fit, b).bottom : null;
    },
    copyTo(target) {
      view.copyTo(target);
    },
    falling() {
      const o = office.office();
      if (!fit || !o || !blend || !lastLook?.sky) return null;
      const s = blendAt(blend, lastT);
      const pts: { x: number; y: number }[] = [];
      for (const x of [o.x0 - HULL_PAD, o.x0 + o.W + HULL_PAD])
        for (const z of [-HULL_PAD, o.D + HULL_PAD])
          for (const y of [-0.2, HULL_H]) pts.push(toScreen(fit, { x, y, z }));
      // where it falls: over the cube only (bims m_34321), its top's outline raised by the room's height
      const g = groundAllOf(o, lastLook.grow ?? NO_GROW);
      const cube: { x: number; y: number }[] = [];
      for (const x of [g.x0, g.x1]) for (const z of [g.z0, g.z1]) for (const y of [0, HULL_H]) cube.push(toScreen(fit, { x, y, z }));
      return { rain: s.rain, snow: s.snow, hull: hullOf(pts), cube: hullOf(cube) };
    },
    onLost(cb) {
      view.onLost(cb);
    },
    dispose() {
      stopMine();
      view.scene.remove(office.root);
      office.dispose();
      view.dispose();
      if (probe) delete (window as unknown as { __biseBims?: Probe }).__biseBims;
    },
  };
  return api;
};
