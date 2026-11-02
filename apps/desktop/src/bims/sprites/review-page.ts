// The bims review studio's page (review.html): what scripts/bims-review.mts calls through
// window.review (bim-review, Gabriel m_48073). For a row of review-space.ts it draws the real bim
// (makeBim, the app's outfits and rigs: pose.ts rigOf through outfits.testkit posesOf) through one of
// the app's cameras (the office's iso fit, its zoom and his closest pinch, the coin and settings'
// swatch: main's through the app's own headPng and cutPng, an agent's coin the same view; a 3/4
// back), and measures what geometry can say: NaN, a mesh with no material, a part through the floor
// or far from the bim, two faces in one plane (z-fight), hair through the headwear, the skull, the
// eyes or the torso (hair.testkit), arms through the torso (outfits.testkit). Contact sheets put the
// rows side by side with their findings. Studios only, never the app. Owner: bim-review.
import { Box3, Group, Material, Mesh, type Object3D, Vector3 } from 'three';
import { type Bim, makeBim } from '../render/bim';
import { RIGHT, UP, type Fit } from '../render/camera';
import { coplanarClashes } from '../render/coplanar';
import { hairMeasure } from '../render/hair.testkit';
import { HEAD_Y } from '../render/laptop';
import { CUTS, type Cut, DAY, type HeadWear, NIGHT, OWN, type Own, type Kind } from '../render/palette';
import { NO_PROPS, type Rig, rigOf } from '../render/pose';
import { OUTFIT_BIMS, clipperOf, posesOf } from '../render/outfits.testkit';
import { STYLE } from '../render/style';
import { makeView, type View } from '../render/view';
import { castShadows, lightOffice } from '../render/world';
import type { MapId, Pose } from '../model';
import type { Feature } from '../parkpath';
import { FOLLOW_ZOOM } from '../focus';
import { kitOf } from '../render/kit';
import { loft } from '../render/maps/loft';
import { COIN_HEAD, cutPng, headPng } from '@/app/side3d/render';
import { aliveRig, aliveSeed, stanceAt, stands } from '../render/alive';
import { mapShot, parkShot, trickStrip } from './review-skate';
import type { TrickKey } from '../skate';
import { DUR, type IdleMove, idleRig } from '../render/idle';
import type { Row } from './pairwise';
import { type Cam, type Dressed, IDLE_KIND, REVIEW_NAME, dimsOf, dress, rowId, validRow } from './review-space';
import { pairwise } from './pairwise';

/** the loop phase every reviewed bim moves at (the turnaround's) */
const PHASE = 1.3;

const poseTable = new Map<boolean, Map<string, readonly Rig[]>>();
function rigsOf(main: boolean): Map<string, readonly Rig[]> {
  const had = poseTable.get(main);
  if (had) return had;
  const m = new Map<string, readonly Rig[]>();
  for (const p of posesOf(main, PHASE)) m.set(p.id, p.frames.map((f) => ({ ...f.rig, x: 0, z: 0, dir: 0 })));
  poseTable.set(main, m);
  return m;
}

/** standing still, arms down, facing +z (the turnaround's) */
function standRig(): Rig {
  const r = rigsOf(false).get('terrace')?.[0];
  if (!r) throw new Error('no terrace pose');
  return { ...r, x: 0, z: 0, dir: 0, y: 0, body: [0, 0, 0], head: [0, 0, 0], armL: [0, 0, 0.12], armR: [0, 0, -0.12], legL: 0, legR: 0, eyes: 1, mouth: 'smile', props: NO_PROPS, opacity: 1, grey: 0 };
}

/** the coin's standing visitor, turned to the camera (app/side3d/render.ts standing) */
const FACE = Math.PI / 4;
function coinPose(): Pose {
  const spot = { x: 0, z: 0, wz: 0, ax: 0, face: FACE };
  const act = { k: 'visit', at: 'desk' } as const;
  return { x: 0, z: 0, dir: FACE, moving: false, since: 30, seg: { t: 0, act, sig: act.k, spot, path: { pts: [spot], seg: [], len: 0 }, speed: 2.4, from: null } };
}

/** the rig a row draws: its pose at its frame (0, 1/3, 2/3 of the loop), or standing; alive: bim-alive's layer on it */
export function rigOfRow(r: Row, main: boolean): Rig {
  const posed = r.life === 'alive' && !main ? lifeOn(poseRig(r, main), Number(r.frame ?? 0)) : poseRig(r, main);
  return r.idle !== undefined && r.idle !== 'none' && !main ? idleOn(posed, r) : posed;
}

/** bim-idle's move on a resting bim (render/idle.ts idleRig): frame 0 at its peak, 1 on its way, 2 near its end */
function idleOn(rig: Rig, r: Row): Rig {
  const kind = IDLE_KIND[r.pose ?? ''];
  if (kind === undefined) return rig;
  const move = r.idle as IdleMove;
  const u = [0.5, 0.25, 0.95][Number(r.frame ?? 0)] ?? 0.5;
  return idleRig(rig, { kind, move: { move, start: 0, dur: DUR[move], side: 1 }, u, hold: 0 });
}

function poseRig(r: Row, main: boolean): Rig {
  if (r.pose === 'stand' || r.pose === undefined) return standRig();
  const frames = rigsOf(main).get(r.pose);
  if (!frames || frames.length === 0) throw new Error(`no pose ${r.pose}`);
  const i = Math.min(frames.length - 1, Math.floor((Number(r.frame ?? 0) * frames.length) / 3));
  return frames[i] as Rig;
}

/** bim-alive's layer at its widest (render/alive.ts aliveRig): frame n holds the name's stance n, the breath out, half, in; only on a bim that stands */
function lifeOn(rig: Rig, n: number): Rig {
  if (!stands(rig)) return rig;
  const st = stanceAt(aliveSeed(REVIEW_NAME), n + 1);
  return aliveRig(rig, { ...st, breath: n / 2, k: 1, shifting: false }, true);
}

function bimOf(d: Dressed, merge = true): Bim {
  return makeBim(d.main ? 'main' : 'bim-3', d.main, STYLE, d.outfit, undefined, merge);
}

function headOf(bim: Bim): Object3D | undefined {
  let head: Object3D | undefined;
  bim.group.traverse((o) => {
    if (head === undefined && o.children.length > 0 && Math.abs(o.position.y - HEAD_Y) < 1e-6) head = o;
  });
  return head;
}

/** a square fit around a box, in the office's iso axes; `px` the side, `air` the margin share, `low` the box on the square's bottom */
function isoFit(b: Box3, px: number, air: number, low: boolean, scale?: number): Fit {
  const target = { x: (b.min.x + b.max.x) / 2, y: (b.min.y + b.max.y) / 2, z: (b.min.z + b.max.z) / 2 };
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const x of [b.min.x, b.max.x]) for (const y of [b.min.y, b.max.y]) for (const z of [b.min.z, b.max.z]) {
    const p = { x: x - target.x, y: y - target.y, z: z - target.z };
    const u = p.x * RIGHT.x + p.y * RIGHT.y + p.z * RIGHT.z;
    const v = p.x * UP.x + p.y * UP.y + p.z * UP.z;
    x0 = Math.min(x0, u); x1 = Math.max(x1, u); y0 = Math.min(y0, v); y1 = Math.max(y1, v);
  }
  const half = scale !== undefined ? (scale * px) / 2 : (Math.max(x1 - x0, y1 - y0) * (1 + 2 * air)) / 2;
  const cx = (x0 + x1) / 2;
  const cy = low ? y0 + half : (y0 + y1) / 2;
  return { target, left: cx - half, right: cx + half, top: cy + half, bottom: cy - half, scale: (2 * half) / px, widthPx: px, heightPx: px };
}

/** the near front view (the turnaround's): the bim turned by `turn` */
function frontView(v: View, size: number, half: number, ty: number, tx = 0): void {
  const { camera } = v;
  const target = new Vector3(tx, ty, 0);
  camera.position.set(tx, ty + Math.sin(0.2) * 10, Math.cos(0.2) * 10);
  camera.up.set(0, 1, 0);
  camera.lookAt(target);
  camera.left = -half; camera.right = half; camera.top = half; camera.bottom = -half;
  camera.near = 0.1; camera.far = 30;
  camera.updateProjectionMatrix();
  v.renderer.setSize(size, size, false);
}

/** a data URL drawn onto a canvas of its size */
async function canvasOf(url: string, size: number): Promise<HTMLCanvasElement> {
  const img = new Image();
  img.src = url;
  await img.decode();
  const c = document.createElement('canvas');
  c.width = size;
  c.height = size;
  c.getContext('2d')?.drawImage(img, 0, 0, size, size);
  return c;
}

/** one row through its camera, `size` px square, on a transparent ground; main's coin and swatch are the app's own (headPng, cutPng: drawn at 2x of `px`) */
export async function renderRow(r: Row, size: number): Promise<HTMLCanvasElement> {
  const d = dress(r);
  if (d.main && (r.cam === 'coin' || r.cam === 'swatch')) {
    const night = r.light === 'night';
    const px = Math.round(size / 2);
    return canvasOf(r.cam === 'coin' ? headPng('main', true, px, night, d.look) : cutPng('main', true, px, night, d.look), size);
  }
  return r.desk !== undefined ? drawAtDesk(r, size) : drawRow(r, size);
}

/**
 * A row's bim seated at its desk: the loft's desk with its laptop, lamp and chair (maps/loft.ts desk,
 * the spot at the origin facing +z, as the office lays it), the bim in its pose, seen through the
 * office's iso camera framed on the two (his closest pinch), the whole turned by `desk` degrees so
 * the hands show from the side or behind. A typing check (bim-3d m_49255: the hands through the
 * desk's edge in the hero film); the bim alone never shows it.
 */
function drawAtDesk(r: Row, size: number): HTMLCanvasElement {
  const d = dress(r);
  const night = r.light === 'night';
  const pal = night ? NIGHT : DAY;
  const v = sharedView();
  camReset();
  const root = new Group();
  root.rotation.y = (Number(r.desk ?? 0) * Math.PI) / 180;
  v.scene.add(root);
  const k = kitOf(STYLE, pal);
  const spot = { x: 0, z: 0, wz: 0, ax: 0, face: 0 };
  const build = { k, root, o: { furniture: [] } } as unknown as Parameters<typeof loft.desk>[0];
  loft.desk(build, spot, REVIEW_NAME);
  const bim = bimOf(d);
  bim.daylight(night ? 0 : 1);
  bim.apply(rigOfRow(r, d.main));
  bim.detail(true);
  castShadows(root);
  root.add(bim.group);
  lightOffice(v.hemi, v.sun, pal, { x0: -1, W: 2, D: 2 } as unknown as Parameters<typeof lightOffice>[3], STYLE);
  v.sun.position.set(-5, 14, 3);
  v.sun.target.position.set(0, 0, 0);
  root.updateMatrixWorld(true);
  const fit = isoFit(new Box3().setFromObject(root, true), size, 0.03, false);
  v.renderer.setSize(size, size, false);
  v.look(fit);
  v.render();
  const out = document.createElement('canvas');
  out.width = size;
  out.height = size;
  v.copyTo(out);
  v.scene.remove(root);
  bim.dispose();
  return out;
}

function drawRow(r: Row, size: number): HTMLCanvasElement {
  const d = dress(r);
  const cam = (r.cam ?? 'office') as Cam;
  const night = r.light === 'night';
  const pal = night ? NIGHT : DAY;
  const v = sharedView();
  camReset();
  const bim = bimOf(d);
  bim.daylight(night ? 0 : 1);
  castShadows(bim.group);
  const root = new Group();
  v.scene.add(root);
  root.add(bim.group);
  if (cam === 'office' || cam === 'zoom' || cam === 'pinch' || cam === 'coin' || cam === 'swatch') {
    lightOffice(v.hemi, v.sun, pal, { x0: -1, W: 2, D: 2 } as unknown as Parameters<typeof lightOffice>[3], STYLE);
    v.sun.position.set(-5, 14, 3);
    v.sun.target.position.set(0, 0, 0);
  } else {
    v.tint(pal);
    v.sun.position.set(-2, 6, 5);
    v.sun.target.position.set(0, 0, 0);
  }
  if (cam === 'office' || cam === 'zoom') {
    bim.apply(rigOfRow(r, d.main));
    // the office at 320 is ~12 css px a unit, far detail; the follow camera (focus.ts FOLLOW_ZOOM) that many times closer, near
    const cssPx = cam === 'office' ? 12.2 : 12.2 * FOLLOW_ZOOM;
    bim.detail(cam === 'zoom');
    bim.group.updateMatrixWorld(true);
    const box = new Box3().setFromObject(bim.group, true);
    // css px drawn at 2x, then the cell shows that many pixels
    const fit = isoFit(box, size, 0, false, 1 / (cssPx * 2));
    v.renderer.setSize(size, size, false);
    v.look(fit);
  } else if (cam === 'pinch') {
    bim.apply(rigOfRow(r, d.main));
    bim.detail(true);
    bim.group.updateMatrixWorld(true);
    const fit = isoFit(new Box3().setFromObject(bim.group, true), size, 0.04, false);
    v.renderer.setSize(size, size, false);
    v.look(fit);
  } else if (cam === 'coin' || cam === 'swatch') {
    // the coin's head looks up (app/side3d/render.ts COIN_HEAD, bim-3d's f1 fix): a cap's peak leaves the eyes
    bim.apply({ ...rigOf({ pose: coinPose(), t: 30, main: d.main, phase: 0, reduced: true }), head: COIN_HEAD });
    bim.detail(true);
    bim.group.updateMatrixWorld(true);
    const head = headOf(bim);
    const box = new Box3().setFromObject(head ?? bim.group, true);
    if (cam === 'coin') box.min.y -= (box.max.y - box.min.y) * 0.55;
    const fit = isoFit(box, size, cam === 'coin' ? 0.06 : 0.12, cam === 'coin');
    v.renderer.setSize(size, size, false);
    v.look(fit);
  } else {
    bim.apply(rigOfRow(r, d.main));
    bim.detail(true);
    bim.group.rotation.y = -2.35;
    // framed on the posed bim (a gag's arms and props reach wide), never under the turnaround's 1 unit
    bim.group.updateMatrixWorld(true);
    const box = new Box3().setFromObject(bim.group, true);
    const half = Math.max(1.0, (Math.max(box.max.x - box.min.x, box.max.y - box.min.y, box.max.z - box.min.z) / 2) * 1.1);
    frontView(v, size, half, (box.min.y + box.max.y) / 2, (box.min.x + box.max.x) / 2);
  }
  v.render();
  const out = document.createElement('canvas');
  out.width = size;
  out.height = size;
  v.copyTo(out);
  v.scene.remove(root);
  bim.dispose();
  return out;
}

/**
 * One view for every row (as app/side3d/render.ts keeps one): a view a row would open one WebGL
 * context each, and a long sheet runs past the browser's ~16, losing the oldest (main's coins
 * came out empty in a 1,000-row pass, and drew alone).
 */
let shared: View | null = null;
function sharedView(): View {
  if (shared !== null && !shared.lost()) return shared;
  shared?.dispose();
  const v = makeView(document.createElement('canvas'), { shadowAuto: true, transparent: true });
  v.renderer.setPixelRatio(1);
  v.sky('#000000');
  const { near, far } = v.camera;
  camReset = (): void => {
    // the back view moves the camera in close (near 0.1, far 30): the iso views' look() keeps its own depth
    v.camera.near = near;
    v.camera.far = far;
    v.camera.up.set(0, 1, 0);
    v.camera.updateProjectionMatrix();
  };
  shared = v;
  return v;
}
let camReset = (): void => undefined;

/** one finding of the automatic checks on a row */
export interface Issue {
  readonly check: 'nan' | 'material' | 'floor' | 'stray' | 'zfight' | 'hair-headwear' | 'hair-skull' | 'hair-eyes' | 'hair-torso' | 'arm-torso' | 'empty' | 'cropped';
  readonly n: number;
  readonly note: string;
}

const geoCache = new Map<string, Issue[]>();
const clippers = new Map<string, ReturnType<typeof clipperOf>>();
const hairs = new Map<string, ReturnType<typeof hairMeasure>>();

/** whether a mesh and all that holds it are shown */
function visibleIn(o: Object3D, root: Object3D): boolean {
  for (let p: Object3D | null = o; p && p !== root; p = p.parent) if (!p.visible) return false;
  return true;
}

/** names each unnamed mesh by its part (the nearest named group), its shape and its colour: the findings read them */
function labelMeshes(root: Object3D): void {
  root.traverse((x) => {
    if (!(x instanceof Mesh) || x.name) return;
    const o = x as Mesh;
    let part = '';
    for (let p: Object3D | null = o.parent; p && !part; p = p.parent) part = p.name;
    const geo = o.geometry.type.replace('Geometry', '').replace('Buffer', '') || 'mesh';
    const mat: Material | undefined = Array.isArray(o.material) ? o.material[0] : o.material;
    const col = mat && 'color' in mat ? `#${(mat as { color: { getHexString(): string } }).color.getHexString()}` : '';
    const tag = o.userData.hair === true ? 'hair ' : o.userData.worn === true ? 'worn ' : '';
    o.name = `${part || 'bim'}/${tag}${geo}${col}`;
  });
}

/** what geometry says of a row: the same for every colour, camera and light, so kept by its shape */
export function checkRow(r: Row): Issue[] {
  const d = dress(r);
  const shape = [r.head, r.kind, r.pose, r.frame, r.life, r.idle, d.main ? 'main' : 'agent'].join('|');
  const had = geoCache.get(shape);
  if (had) return had;
  const out: Issue[] = [];
  const rig = rigOfRow(r, d.main);
  const bim = bimOf(d, false);
  bim.apply(rig);
  bim.detail(true);
  bim.group.updateMatrixWorld(true);
  // NaN anywhere, a mesh with no material
  let nan = 0;
  let noMat = 0;
  let below = 0;
  const v = new Vector3();
  const feet = bim.feet(new Vector3());
  const strays: string[] = [];
  const lows = new Map<string, number>();
  labelMeshes(bim.group);
  bim.group.traverse((o) => {
    if (!(o instanceof Mesh) || !visibleIn(o, bim.group)) return;
    const m = o as Mesh;
    if (m.matrixWorld.elements.some((x) => !Number.isFinite(x))) nan++;
    const mats: readonly unknown[] = Array.isArray(m.material) ? m.material : [m.material];
    if (mats.length === 0 || mats.some((x) => !(x instanceof Material))) noMat++;
    if (!m.geometry.hasAttribute('position')) return;
    const pos = m.geometry.getAttribute('position');
    let far = 0;
    for (let i = 0; i < pos.count; i++) {
      v.fromBufferAttribute(pos, i);
      if (!Number.isFinite(v.x + v.y + v.z)) {
        nan++;
        continue;
      }
      v.applyMatrix4(m.matrixWorld);
      if (v.y < feet.y - 0.03) {
        below++;
        lows.set(m.name, (lows.get(m.name) ?? 0) + 1);
      }
      if (Math.hypot(v.x - feet.x, v.z - feet.z) > 1.8 || v.y - feet.y > 2.2) far++;
    }
    // the props at the bim's root (hot's steam, the failed cloud) float above it by design
    if (far > 0 && !m.name.startsWith('bim/')) strays.push(`${m.name}: ${String(far)}`);
  });
  if (nan > 0) out.push({ check: 'nan', n: nan, note: `${String(nan)} NaN values` });
  if (noMat > 0) out.push({ check: 'material', n: noMat, note: `${String(noMat)} meshes with no material` });
  // a hair's breadth under its own feet: through the floor
  if (below > 0) out.push({ check: 'floor', n: below, note: `${String(below)} vertices under its feet (y=${feet.y.toFixed(3)}): ${[...lows].map(([k, n]) => `${k} ${String(n)}`).join(', ')}` });
  if (strays.length > 0) out.push({ check: 'stray', n: strays.length, note: `vertices far from the bim: ${strays.join(', ')}` });
  // two faces in one plane over each other: they draw in turns
  // only what is drawn: the hidden props (sweat, a mug, fruit) are not there to fight
  const hidden: Mesh[] = [];
  bim.group.traverse((o) => {
    if (o instanceof Mesh && !visibleIn(o, bim.group)) hidden.push(o as Mesh);
  });
  for (const m of hidden) m.removeFromParent();
  // two faces of one colour in one plane flicker unseen: only two colours fight
  const colourOf = (n: string): string => n.slice(n.lastIndexOf('#'));
  // and the hair's nape over the collar ring is a moving tangent contact (bim-3d m_48623: 206 frames, any
  // radius moves it to another): facets lining up, never a flicker; not counted
  const nape = (a: string, b: string): boolean => a.startsWith('head/hair') && b.startsWith('body/worn Torus');
  const clashes = coplanarClashes(bim.group, 0.002, 4e-5, 'same').filter((c) => colourOf(c.a) !== colourOf(c.b) && !nape(c.a, c.b) && !nape(c.b, c.a));
  if (clashes.length > 0) {
    const area = clashes.reduce((s, c) => s + c.area, 0);
    out.push({ check: 'zfight', n: clashes.length, note: `coplanar: ${clashes.slice(0, 4).map((c) => `${c.a} x ${c.b} (${(c.area * 1e4).toFixed(1)} cm²)`).join(', ')}${clashes.length > 4 ? ` +${String(clashes.length - 4)}` : ''} · ${(area * 1e4).toFixed(1)} cm²` });
  }
  bim.dispose();
  // hair: the women's cuts and his own (hair.testkit); a short look's hat is checked by the man's hat rows
  const cutOf = (): { cut: Cut | Own; hat: HeadWear } | null => {
    const o = d.outfit;
    const cut = o.cut ?? o.own ?? null;
    if (cut === null) return null;
    if (!(CUTS as readonly string[]).includes(cut) && !(OWN as readonly string[]).includes(cut)) return null;
    const hat = (['bare', 'cap', 'capback', 'beanie', 'headphones'] as const).includes(o.hat as HeadWear) ? (o.hat as HeadWear) : 'bare';
    return { cut, hat };
  };
  const c = cutOf();
  if (c !== null) {
    const kind: Kind = d.outfit.kind;
    const hk = `${c.cut}|${c.hat}|${kind}`;
    let hm = hairs.get(hk);
    if (!hm) {
      hm = hairMeasure(c.cut, c.hat, kind);
      hairs.set(hk, hm);
    }
    if (hm.headwear > 0) out.push({ check: 'hair-headwear', n: hm.headwear, note: `${String(hm.headwear)} hair vertices through the ${c.hat}` });
    if (hm.skull > 0) out.push({ check: 'hair-skull', n: hm.skull, note: `${String(hm.skull)} skull vertices through the hair` });
    if (hm.eyes > 0) out.push({ check: 'hair-eyes', n: hm.eyes, note: `${String(hm.eyes)} hair vertices over the eyes` });
    const torso = hm.torsoAt(rig.head);
    if (torso > 0) out.push({ check: 'hair-torso', n: torso, note: `${String(torso)} hair vertices in the torso at head ${rig.head.map((x) => x.toFixed(2)).join(',')}` });
  }
  // arms through the torso (outfits.testkit's measure, its bim for this outfit)
  const ob = d.main ? OUTFIT_BIMS.find((b) => b.outfit === 'main') : OUTFIT_BIMS.find((b) => b.outfit === d.outfit.kind);
  if (ob) {
    let cl = clippers.get(ob.name);
    if (!cl) {
      cl = clipperOf(ob.name, ob.main);
      clippers.set(ob.name, cl);
    }
    const n = cl.clipsAt(rig);
    if (n > 0) out.push({ check: 'arm-torso', n, note: `${String(n)} arm or hand vertices through the ${d.main ? 'hoodie' : d.outfit.kind}` });
  }
  geoCache.set(shape, out);
  return out;
}

/** what the drawn pixels say: nothing drawn, or the bim cut by the cell's edge where it should not be */
function pixelIssues(r: Row, c: HTMLCanvasElement): Issue[] {
  const ctx = c.getContext('2d');
  if (!ctx) return [];
  const { data } = ctx.getImageData(0, 0, c.width, c.height);
  let ink = 0;
  let edge = 0;
  for (let y = 0; y < c.height; y++) {
    for (let x = 0; x < c.width; x++) {
      const a = data[(y * c.width + x) * 4 + 3] ?? 0;
      if (a < 24) continue;
      ink++;
      const onEdge = x === 0 || y === 0 || x === c.width - 1 || (y === c.height - 1 && r.cam !== 'coin' && r.cam !== 'swatch');
      if (onEdge) edge++;
    }
  }
  const out: Issue[] = [];
  if (ink === 0) out.push({ check: 'empty', n: 0, note: 'nothing drawn' });
  // the coin cuts its bust at the bottom on purpose; the office and zoom fit the bim to its own box
  else if (edge > 2 && r.cam !== 'office' && r.cam !== 'zoom' && r.cam !== 'pinch') out.push({ check: 'cropped', n: edge, note: `${String(edge)} px on the frame's edge` });
  return out;
}

export interface Cell {
  readonly id: string;
  readonly row: Row;
  readonly issues: readonly Issue[];
}

/** a contact sheet of rows, `cols` across, cells `size` px, their labels and findings under each */
async function sheet(rows: readonly Row[], title: string, size: number, cols: number): Promise<{ png: string; cells: Cell[] }> {
  const LABEL = 46;
  const n = rows.length;
  const nr = Math.ceil(n / cols);
  const c = document.createElement('canvas');
  c.width = cols * size;
  c.height = 44 + nr * (size + LABEL);
  const ctx = c.getContext('2d');
  if (!ctx) throw new Error('no 2d');
  ctx.fillStyle = '#efe9df';
  ctx.fillRect(0, 0, c.width, c.height);
  ctx.fillStyle = '#1d1a17';
  ctx.font = '600 22px -apple-system, Helvetica, sans-serif';
  ctx.fillText(title, 10, 30);
  const cells: Cell[] = [];
  for (const [i, r] of rows.entries()) {
    const x = (i % cols) * size;
    const y = 44 + Math.floor(i / cols) * (size + LABEL);
    if (r.light === 'night') {
      ctx.fillStyle = '#2a2d3a';
      ctx.fillRect(x + 1, y + 1, size - 2, size - 2);
    }
    let issues: Issue[];
    try {
      const drawn = await renderRow(r, r.cam === 'office' ? Math.round(size / 2) : size);
      ctx.imageSmoothingEnabled = r.cam !== 'office';
      ctx.drawImage(drawn, x, y, size, size);
      ctx.imageSmoothingEnabled = true;
      issues = [...checkRow(r), ...pixelIssues(r, drawn)];
    } catch (e) {
      issues = [{ check: 'empty', n: 0, note: `threw: ${e instanceof Error ? e.message : String(e)}` }];
    }
    const bad = issues.filter((x) => x.check !== 'zfight');
    ctx.strokeStyle = bad.length > 0 ? '#d1495b' : issues.length > 0 ? '#e0a030' : '#d8d0c4';
    ctx.lineWidth = bad.length > 0 ? 3 : 1;
    ctx.strokeRect(x + 1, y + 1, size - 2, size - 2);
    ctx.fillStyle = '#1d1a17';
    ctx.font = '11px -apple-system, Helvetica, sans-serif';
    const head = (r.head ?? '').replace(/^(m|w|me):/, '');
    ctx.fillText(`${String(i + 1)} ${head} · ${r.pose ?? ''}${r.frame && r.frame !== '0' ? `@${r.frame}` : ''}`, x + 4, y + size + 13);
    ctx.fillStyle = '#7a736b';
    ctx.fillText(`${r.cam ?? ''} · ${r.kind ?? ''} · s${r.skin ?? ''} h${r.hair ?? ''} ${r.hatc ?? ''} · ${r.light ?? ''}${r.life === 'alive' ? ' · alive' : ''}${r.idle && r.idle !== 'none' ? ` · ${r.idle}` : ''}${r.desk !== undefined ? ` · desk ${r.desk}°` : ''}`, x + 4, y + size + 27);
    if (issues.length > 0) {
      ctx.fillStyle = bad.length > 0 ? '#d1495b' : '#b07a10';
      ctx.fillText(issues.map((x) => x.check).join(' '), x + 4, y + size + 41);
    }
    cells.push({ id: rowId(r), row: r, issues });
  }
  return { png: c.toDataURL('image/png'), cells };
}

declare global {
  interface Window {
    review: {
      /** the poses the rigs draw (outfits.testkit posesOf's ids) */
      poses(): string[];
      /** the pairwise rows for a seed */
      rows(seed: number): { rows: Row[]; unreachable: string[] };
      sheet(rows: Row[], title: string, size: number, cols: number): Promise<{ png: string; cells: Cell[] }>;
      /** one row, big: a finding's shot */
      cell(r: Row, size: number): Promise<string>;
      check(r: Row): Issue[];
      /** California's park at a growth stage, pinched in on its slab (review-skate.ts) */
      park(stage: number, night: boolean, size: number): string;
      /** any map's office of n agents at a stage, framed round stage pieces (none: whole) (review-skate.ts mapShot) */
      map(id: string, stage: number, night: boolean, n: number, size: number, pieces?: number[]): string;
      /** one trick on its piece: `frames` shots from its start to its end in one strip, labelled */
      trick(trick: string, stage: number, night: boolean, size: number, frames: number): string | null;
    };
  }
}

window.review = {
  poses: () => posesOf(false, PHASE).map((p) => p.id),
  rows(seed) {
    const dims = dimsOf(posesOf(false, PHASE).map((p) => p.id));
    const p = pairwise(dims, validRow, seed);
    return { rows: p.rows.map((r) => ({ ...r })), unreachable: [...p.unreachable] };
  },
  sheet: (rows, title, size, cols) => sheet(rows, title, size, cols),
  async cell(r, size) {
    const c = document.createElement('canvas');
    c.width = size;
    c.height = size;
    const ctx = c.getContext('2d');
    if (!ctx) throw new Error('no 2d');
    ctx.fillStyle = r.light === 'night' ? '#2a2d3a' : '#efe9df';
    ctx.fillRect(0, 0, size, size);
    const drawn = await renderRow(r, r.cam === 'office' ? Math.round(size / 4) : size);
    ctx.imageSmoothingEnabled = r.cam !== 'office';
    ctx.drawImage(drawn, 0, 0, size, size);
    return c.toDataURL('image/png');
  },
  check: (r) => checkRow(r),
  park(stage, night, size) {
    const v = sharedView();
    camReset();
    return parkShot(v, stage, night, size).toDataURL('image/png');
  },
  map(id, stage, night, n, size, pieces) {
    const v = sharedView();
    camReset();
    return mapShot(v, id as MapId, stage, night, n, size, pieces).toDataURL('image/png');
  },
  trick(trick, stage, night, size, frames) {
    const v = sharedView();
    camReset();
    // `trick@piece`: that trick on that piece only (nose_stall@vert, coping_grind@bowl)
    const [k, on] = trick.split('@');
    const st = trickStrip(v, k as TrickKey, stage, night, size, frames, on as Feature | undefined);
    if (!st) return null;
    const c = document.createElement('canvas');
    c.width = size * st.shots.length;
    c.height = size + 26;
    const ctx = c.getContext('2d');
    if (!ctx) return null;
    ctx.fillStyle = night ? '#2a2d3a' : '#efe9df';
    ctx.fillRect(0, 0, c.width, c.height);
    st.shots.forEach((sh, i) => {
      ctx.drawImage(sh, i * size, 26);
    });
    ctx.fillStyle = night ? '#efe9df' : '#1d1a17';
    ctx.font = '600 15px -apple-system, Helvetica, sans-serif';
    ctx.fillText(`${trick} · ${st.name} · stage ${String(stage)} · ${night ? 'night' : 'day'} · ${String(st.shots.length)} frames over the trick`, 8, 18);
    return c.toDataURL('image/png');
  },
};

