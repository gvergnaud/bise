// The plugins place's three side, loaded only by import('./render') (the entry chunk never holds
// three, electron.vite.config.ts): each plugin's icon as its office object (bims-objects'
// makeObject), and the room, the place's hero (spots.ts plans it). One offscreen WebGL view draws
// every icon once, kept as a data URL by (object, count, glass, size); the room has its own canvas
// and draws a frame only when what it shows changes, or while a new object drops in (~0.4 s; cut
// under reduced motion). With `enter` (Room.tsx: the first time the place opens) its first frames
// play the banners' entrance (bims/render/entrance.ts): the slab, walls and rugs rise in cubes, the
// objects pop on them. Light: the office's day sun from the same corner (-5, 14, 3), its one soft
// contact shadow, a transparent backdrop. Owner: plug-desktop.
import { Box3, Group, Mesh, type Object3D, PlaneGeometry, Raycaster, ShadowMaterial, Vector2, Vector3, type Material, type BufferGeometry } from 'three';
import { bleed, boxSite, RIGHT, UP, viewFit, type Bleed, type Fit, type Site } from '@/bims/render/camera';
import { HOVER_MS, MOVES, hoverAt } from '@/bims/render/objects/hover';
import { FIT, type Cam } from '@/bims/zoom';
import { makeObject, type ObjId } from '@/bims/render/objects';
import { DAY, NIGHT } from '@/bims/render/palette';
import { STYLE } from '@/bims/render/style';
import { makeView, type View } from '@/bims/render/view';
import { lightOffice } from '@/bims/render/world';
import { type Entrance, type GroundPart, makeEntrance, wrapIn } from '@/bims/render/entrance';
import type { PluginObject } from '@/bims/plugin-objects';
import { iconKey, planRoom, ROOM, type Foot, type IconObject, type Placed, type Spot } from './spots';

/** the office's sun, from the room's front left */
const SUN = { x: -5, y: 14, z: 3 } as const;
/** how dark the contact shadow is on the transparent floor */
const SHADOW = 0.22;

type LightBox = Parameters<typeof lightOffice>[3];

function light(view: View, night: boolean, half: { W: number; D: number }): void {
  lightOffice(view.hemi, view.sun, night ? NIGHT : DAY, { x0: -half.W / 2, W: half.W, D: half.D } as unknown as LightBox, STYLE);
  view.sun.position.set(SUN.x, SUN.y, SUN.z);
  view.sun.target.position.set(0, 0, 0);
}

/** a floor that shows only the shadows on it */
function shadowFloor(w: number, d: number): Mesh {
  const m = new Mesh(new PlaneGeometry(w, d), new ShadowMaterial({ opacity: SHADOW }));
  m.rotation.x = -Math.PI / 2;
  m.position.y = 0.001;
  m.receiveShadow = true;
  return m;
}

function dispose(root: Group | Mesh): void {
  root.traverse((x) => {
    if (!(x instanceof Mesh)) return;
    (x.geometry as BufferGeometry).dispose();
    const m = x.material as Material | Material[];
    for (const y of Array.isArray(m) ? m : [m]) y.dispose();
  });
}

/** the box seen whole in a size x size square, its bounds to the square's (ICON_AIR left around it) */
const ICON_AIR = 0.06;
function tightFit(b: Box3, size: number): Fit {
  const target = { x: (b.min.x + b.max.x) / 2, y: (b.min.y + b.max.y) / 2, z: (b.min.z + b.max.z) / 2 };
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const x of [b.min.x, b.max.x]) {
    for (const y of [b.min.y, b.max.y]) {
      for (const z of [b.min.z, b.max.z]) {
        const p = { x: x - target.x, y: y - target.y, z: z - target.z };
        const u = p.x * RIGHT.x + p.y * RIGHT.y + p.z * RIGHT.z;
        const v = p.x * UP.x + p.y * UP.y + p.z * UP.z;
        x0 = Math.min(x0, u); x1 = Math.max(x1, u); y0 = Math.min(y0, v); y1 = Math.max(y1, v);
      }
    }
  }
  const half = (Math.max(x1 - x0, y1 - y0) * (1 + 2 * ICON_AIR)) / 2;
  const cx = (x0 + x1) / 2;
  const cy = (y0 + y1) / 2;
  return { target, left: cx - half, right: cx + half, top: cy + half, bottom: cy - half, scale: (2 * half) / size, widthPx: size, heightPx: size };
}

// ---- icons ------------------------------------------------------------------------------------

let iconView: View | null = null;
const icons = new Map<string, string>();

function theIconView(): View {
  if (iconView !== null && !iconView.lost()) return iconView;
  iconView?.dispose();
  const v = makeView(document.createElement('canvas'), { shadowAuto: true, transparent: true });
  v.sky('#000000');
  v.renderer.setPixelRatio(1);
  light(v, false, { W: 2, D: 2 });
  iconView = v;
  return v;
}

/**
 * One object's icon, `px` css px square, drawn at `scale`x (2 by default; spots.ts iconScale): a PNG
 * data URL, the same one for every row with that object, count, glass and size. Throws when there
 * is no WebGL (the caller draws the plain tile).
 */
export function iconOf(o: Pick<IconObject, 'obj' | 'n' | 'glass' | 'seed'>, px: number, scale = 2): string {
  const key = iconKey(o, px, scale);
  const had = icons.get(key);
  if (had !== undefined) return had;
  const v = theIconView();
  const made = makeObject(o.obj, { off: o.glass, n: o.n ?? 5, seed: o.seed ?? 0 });
  // its own box, not its foot: a wall object (the calendar) is flat, so the box fits it closer
  const b = new Box3().setFromObject(made.group);
  const floor = shadowFloor(Math.max(1, (b.max.x - b.min.x) * 2), Math.max(1, (b.max.z - b.min.z) * 2));
  v.scene.add(made.group, floor);
  const fit = tightFit(b, Math.round(px * scale));
  v.renderer.setSize(fit.widthPx, fit.heightPx, false);
  v.look(fit);
  v.render();
  const url = v.renderer.domElement.toDataURL('image/png');
  v.scene.remove(made.group, floor);
  dispose(made.group);
  dispose(floor);
  icons.set(key, url);
  return url;
}

// ---- the room ---------------------------------------------------------------------------------

/** how long a new object takes to drop in and settle, ms */
const DROP_MS = 400;

export interface RoomView {
  /** what the room shows; new keys (after the first call) drop in */
  show(objs: readonly PluginObject[], o: { readonly night: boolean; readonly reduced: boolean }): void;
  /** the canvas's css size, and how far it reaches past the banner's visual box (the room rests fitted in a 2.2:1 box centred in it) */
  resize(widthPx: number, heightPx: number, inset?: Bleed): void;
  /** the plugin key of the object under (x, y), css px from the canvas's top left; null: none */
  pick(x: number, y: number): string | null;
  /** the object under the pointer (null: none): a new one plays its hover move once (render/objects/hover.ts), none under reduced motion */
  hover(key: string | null, reduced: boolean): void;
  /** his pinch and pan (zoom.ts Cam): the view inside the room's fit */
  setCam(c: Cam): void;
  /** an object's box on the canvas at rest, css px from its top left (its hover tip's mark); null: not shown */
  rectOf(key: string): { left: number; top: number; width: number; height: number } | null;
  dispose(): void;
}

const feet = new Map<ObjId, Foot>();
/** an object's foot, measured once (its geometry built then freed) */
function footOf(id: ObjId): Foot {
  const had = feet.get(id);
  if (had !== undefined) return had;
  const m = makeObject(id, {});
  dispose(m.group);
  const f = m.wall === true ? { ...m.foot, wall: true } : m.foot;
  feet.set(id, f);
  return f;
}

/** the room's slab, its two low walls and two rugs (scratch renderHero, signed with plugins-ui); the entrance's ground (the floor's planks a plain colour) */
function slab(view: View): GroundPart[] {
  const { w: W, d: D } = ROOM;
  const out: GroundPart[] = [];
  const add = (w: number, h: number, d: number, c: string, x: number, y: number, z: number, r?: number, cast = true): void => {
    const m = new Mesh(STYLE.box(w, h, d, r), STYLE.material(c));
    m.position.set(x, y, z);
    m.castShadow = cast;
    m.receiveShadow = true;
    out.push({ mesh: m });
  };
  add(W + 0.3, 0.5, D + 0.3, DAY.dirt, 0, -0.35, 0, 0.12);
  const fl = new Mesh(STYLE.box(W, 0.12, D, 0.04), STYLE.floor(DAY.floor, W, D));
  fl.position.set(0, -0.06, 0);
  fl.receiveShadow = true;
  out.push({ mesh: fl, plain: DAY.floor });
  add(W, 1.25, 0.16, DAY.wall, 0, 0.62, -D / 2 - 0.02, 0.04);
  add(W + 0.02, 0.05, 0.18, DAY.wallTop, 0, 1.27, -D / 2 - 0.02, 0.02);
  add(0.16, 1.25, D, DAY.wall, -W / 2 - 0.02, 0.62, 0, 0.04);
  add(0.18, 0.05, D + 0.02, DAY.wallTop, -W / 2 - 0.02, 1.27, 0, 0.02);
  add(3.2, 0.02, 2.2, '#bfe1f2', -2.0, 0.012, 0.55, 0.2, false);
  add(2.6, 0.02, 2.0, '#ffd166', 1.5, 0.012, 0.9, 0.2, false);
  for (const m of out) view.scene.add(m.mesh);
  return out;
}

/** a free spot: a dotted square on the floor, one group */
function dots(s: Spot, night: boolean): Group {
  const mat = STYLE.material(night ? '#f3ede4' : '#6b645a', { opacity: 0.7 });
  const out = new Group();
  const S = 0.9, step = 0.22, len = 0.11;
  for (let t = -S / 2; t < S / 2; t += step) {
    for (const [dx, dz, along] of [[t, -S / 2, true], [t, S / 2, true], [-S / 2, t, false], [S / 2, t, false]] as const) {
      const m = new Mesh(STYLE.box(along ? len : 0.05, 0.02, along ? 0.05 : len), mat);
      m.position.set(s.x + dx + (along ? len / 2 : 0), 0.03, s.z + dz + (along ? 0 : len / 2));
      out.add(m);
    }
  }
  return out;
}

/** a drop's lift and squash at t (0..1): it falls, lands, squashes a little and stands */
export function dropAt(t: number): { lift: number; squash: number } {
  if (t >= 1) return { lift: 0, squash: 1 };
  if (t < 0.7) {
    const k = t / 0.7;
    return { lift: 2.5 * (1 - k * k), squash: 1 };
  }
  return { lift: 0, squash: 0.9 + 0.1 * ((t - 0.7) / 0.3) };
}

/** `enter`: its first frames play the entrance (bims/render/entrance.ts): no drop, no hover move until its end */
export function mountRoom(canvas: HTMLCanvasElement, o0: { readonly enter?: boolean } = {}): RoomView {
  const view = makeView(canvas, { shadowAuto: true, transparent: true });
  view.sky('#000000');
  const base = slab(view);
  // each object in its wrapper (identity at rest: what the entrance moves)
  let shown: { key: string; g: Group; w: Group; p: Placed }[] = [];
  let marks: Group[] = [];
  // the entrance while it plays, and its start (the first frame's)
  let ent: Entrance | null = null;
  let t0 = -1;
  let first = true;
  let night = false;
  let width = 0;
  let sized = '';
  let raf = 0;
  const drops = new Map<string, number>();
  // the fit of the whole room, and his camera inside it
  let fit: Fit | null = null;
  let cam: Cam = FIT;
  // the object under the pointer and when its hover move began (0: none playing)
  let hovered: string | null = null;
  let hoverT0 = 0;
  const ray = new Raycaster();

  const draw = (now: number): void => {
    raf = 0;
    let moving = false;
    if (ent !== null) {
      if (t0 < 0) t0 = now;
      if (ent.at(now - t0)) moving = true;
      else {
        ent.dispose();
        ent = null;
      }
    }
    for (const s of shown) {
      const t0 = drops.get(s.key);
      const { lift, squash } = dropAt(t0 === undefined ? 1 : (now - t0) / DROP_MS);
      if (t0 !== undefined && now - t0 >= DROP_MS) drops.delete(s.key);
      else if (t0 !== undefined) moving = true;
      const playing = hoverT0 > 0 && s.key === hovered;
      const ht = playing ? (now - hoverT0) / HOVER_MS : 1;
      if (playing && ht < 1) moving = true;
      else if (playing) hoverT0 = 0;
      const h = hoverAt(MOVES[s.p.o.obj], ht);
      const k = s.p.scale;
      const rot = s.p.spot.rot;
      s.g.position.set(s.p.spot.x + h.dx * Math.cos(rot), lift + h.lift, s.p.spot.z - h.dx * Math.sin(rot));
      s.g.rotation.set(h.pitch, rot + h.yaw, h.roll);
      s.g.scale.set((k * h.sx) / Math.sqrt(squash), k * h.sy * squash, (k * h.sz) / Math.sqrt(squash));
    }
    view.render();
    if (moving) raf = requestAnimationFrame(draw);
  };
  const frame = (): void => {
    if (raf === 0 && width > 0) raf = requestAnimationFrame(draw);
  };

  return {
    show(objs, o) {
      if (o.night !== night || first) {
        night = o.night;
        view.tint(night ? NIGHT : DAY);
        light(view, night, { W: ROOM.w, D: ROOM.d });
      }
      const plan = planRoom(objs, footOf);
      const sig = (p: Placed): string => `${iconKey(p.o, 0)}@${p.spot.x},${p.spot.z}*${p.scale}`;
      const keep = new Map(shown.map((s) => [`${s.key}#${sig(s.p)}`, s]));
      const next: typeof shown = [];
      for (const p of plan.placed) {
        const id = `${p.o.key}#${sig(p)}`;
        const had = keep.get(id);
        if (had !== undefined) {
          keep.delete(id);
          next.push(had);
          continue;
        }
        const made = makeObject(p.o.obj, { off: p.o.glass, night: night ? 1 : 0, n: p.o.n ?? 5 });
        made.group.rotation.y = p.spot.rot;
        if (p.o.obj === 'cat') {
          // bise's cat sleeps on its cushion
          made.group.add(makeObject('cushion', { off: p.o.glass }).group);
        }
        // at its spot now (the draw keeps it there): the entrance reads its foot
        made.group.position.set(p.spot.x, 0, p.spot.z);
        made.group.scale.setScalar(p.scale);
        view.scene.add(made.group);
        const w = wrapIn(made.group);
        const wasHere = shown.some((s) => s.key === p.o.key);
        // while the entrance plays a new one comes in with it, never drops
        if (ent !== null) ent.add(w);
        else if (!first && !wasHere && !o.reduced) drops.set(p.o.key, performanceNow());
        next.push({ key: p.o.key, g: made.group, w, p });
      }
      for (const s of keep.values()) {
        ent?.remove(s.w);
        view.scene.remove(s.w);
        dispose(s.g);
        drops.delete(s.key);
      }
      shown = next;
      for (const m of marks) {
        ent?.remove(m);
        view.scene.remove(m);
        dispose(m);
      }
      marks = plan.dotted.map((s) => dots(s, night));
      for (const m of marks) {
        view.scene.add(m);
        ent?.add(m);
      }
      if (first && o0.enter === true && !o.reduced) {
        ent = makeEntrance({ parent: view.scene, ground: base, things: [...shown.map((s) => s.w), ...marks], bims: [] });
      }
      first = false;
      frame();
    },
    resize(w, h, e = { l: 0, t: 0, r: 0, b: 0 }) {
      const px = Math.max(1, Math.round(w));
      const py = Math.max(1, Math.round(h));
      const sig = `${String(px)}x${String(py)}:${String(e.l)},${String(e.t)},${String(e.r)},${String(e.b)}`;
      // the same size: the canvas may have lost its frame (a window resize, the room scrolled
      // away and back), so draw it again; one frame, no loop
      if (sig === sized) {
        frame();
        return;
      }
      sized = sig;
      width = px;
      const P = 0.75;
      const site: Site = { x0: -ROOM.w / 2 - P, x1: ROOM.w / 2 + P, z0: -ROOM.d / 2 - P, z1: ROOM.d / 2 + P, yLow: -0.6, yHigh: 2.2 };
      // the 2.2:1 box in the visual box, centred; the canvas past it on each side
      const iw = Math.max(1, px - e.l - e.r);
      const ih = Math.max(1, py - e.t - e.b);
      const lw = Math.round(Math.min(iw, ih * 2.2));
      const lh = Math.round(lw / 2.2);
      const dx = (iw - lw) / 2;
      const dy = (ih - lh) / 2;
      fit = bleed(boxSite(site, lw, lh), { l: e.l + dx, r: px - lw - e.l - dx, t: e.t + dy, b: py - lh - e.t - dy });
      view.size(fit);
      view.look(viewFit(fit, cam));
      frame();
    },
    pick(x, y) {
      const el = view.renderer.domElement;
      const w = el.clientWidth;
      const h = el.clientHeight;
      if (w <= 0 || h <= 0 || shown.length === 0) return null;
      ray.setFromCamera(new Vector2((x / w) * 2 - 1, 1 - (y / h) * 2), view.camera);
      const hit = ray.intersectObjects(shown.map((s) => s.g), true)[0];
      if (hit === undefined) return null;
      // the shown group the hit mesh is in
      let o: Object3D | null = hit.object;
      while (o !== null && !shown.some((s) => s.g === o)) o = o.parent;
      return shown.find((s) => s.g === o)?.key ?? null;
    },
    hover(key, reduced) {
      if (key === hovered || ent !== null) return;
      hovered = key;
      hoverT0 = key !== null && !reduced ? performanceNow() : 0;
      frame();
    },
    setCam(c) {
      cam = c;
      if (fit !== null) view.look(viewFit(fit, c));
      frame();
    },
    rectOf(key) {
      const s = shown.find((x) => x.key === key);
      const el = view.renderer.domElement;
      if (s === undefined || el.clientWidth <= 0) return null;
      view.camera.updateMatrixWorld();
      const b = new Box3().setFromObject(s.g);
      let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
      for (const x of [b.min.x, b.max.x]) for (const y of [b.min.y, b.max.y]) for (const z of [b.min.z, b.max.z]) {
        const p = new Vector3(x, y, z).project(view.camera);
        const px = ((p.x + 1) / 2) * el.clientWidth;
        const py = ((1 - p.y) / 2) * el.clientHeight;
        x0 = Math.min(x0, px); x1 = Math.max(x1, px); y0 = Math.min(y0, py); y1 = Math.max(y1, py);
      }
      return { left: x0, top: y0, width: x1 - x0, height: y1 - y0 };
    },
    dispose() {
      if (raf !== 0) cancelAnimationFrame(raf);
      ent?.dispose();
      for (const s of shown) dispose(s.g);
      for (const m of [...base.map((b) => b.mesh), ...marks]) dispose(m);
      view.dispose();
    },
  };
}

/** the frame clock (requestAnimationFrame's), read when a drop starts */
const performanceNow = (): number => performance.now();
