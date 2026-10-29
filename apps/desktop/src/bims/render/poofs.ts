// The newcomers' poofs drawn (poof.ts says where each piece is): one instanced mesh of clay puffs and
// one point cloud of gold sparkles under the office's root, so every poof in the office is two draw
// calls at most. A sparkle is a flat 4-point star drawn on the screen (designer m_49932: at the tile's
// fit the 3D stars were under a pixel and toned to white): unlit, never tone mapped, over everything,
// sized in css px from the camera at draw time, with a 1 px warm-ink rim so it shows on the cream floor.
// Owner: bim-pop.
import {
  BufferAttribute, BufferGeometry, Color, type Group, InstancedMesh, type Material, Matrix4, Points, Quaternion, ShaderMaterial, Vector3,
} from 'three';
import { POOF_S, PUFF, SPARK, poofAt, puffBoost, sparkBoost, sparkSpread } from './poof';
import type { Style } from './style';

/** the puffs' clay: warm paper, no glow, in both themes (designer m_39015); the sparkles' gold: the DAY gold (palette.ts), never the accent */
const PUFF_COLOR = '#f2ede2';
const SPARK_COLOR = '#ffd166';
/** the sparkles' rim: the palette's warm ink (DAY.ink), one css px */
const SPARK_RIM = '#1d1a17';
/** the paper-white core of a small star (no rim): the puffs' paper */
const SPARK_CORE = '#fffaf0';
/** the rim's width, css px, on each side of the star */
export const RIM_PX = 1;
/** designer m_49952: a star under this many css px of gold has no rim (the rims merged into a dark clump at the fit) */
export const RIM_FROM_PX = 6;

/**
 * A sparkle on screen: `fill` its gold star's css px across, `rim` its rim's (RIM_PX from RIM_FROM_PX
 * up, else 0 and a paper-white core), `size` the sprite (both). `r` its half length in office units,
 * `scale` office units per css px. Pure.
 */
export function sparkPx(r: number, scale: number): { readonly fill: number; readonly rim: number; readonly size: number } {
  const fill = (2 * r) / scale;
  const rim = fill >= RIM_FROM_PX ? RIM_PX : 0;
  return { fill, rim, size: fill + 2 * rim };
}

const SPARK_VERT = /* glsl */ `
attribute float aSize;
attribute float aRim;
attribute float aSpin;
uniform float uDpr;
varying float vSpin;
varying float vFill;
varying float vRim;
varying float vSize;
void main() {
  vSpin = aSpin;
  vRim = aRim;
  vSize = aSize;
  vFill = (aSize - 2.0 * aRim) / aSize;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = aSize * uDpr;
}`;

// a 4-point star: |x|^k + |y|^k under 1 (k < 1 pulls its sides in), its tips at the fill's edge; a
// rimmed star: the band a pixel past it is the rim; a small one: no rim, a paper-white pixel at its middle
const SPARK_FRAG = /* glsl */ `
uniform vec3 uGold;
uniform vec3 uRim;
uniform vec3 uCore;
varying float vSpin;
varying float vFill;
varying float vRim;
varying float vSize;
void main() {
  vec2 p = gl_PointCoord * 2.0 - 1.0;
  float c = cos(vSpin);
  float s = sin(vSpin);
  vec2 q = vec2(c * p.x - s * p.y, s * p.x + c * p.y) / max(vFill, 0.05);
  float f = pow(abs(q.x), 0.7) + pow(abs(q.y), 0.7);
  float w = fwidth(f);
  vec3 col = uGold;
  if (vRim > 0.5) {
    if (f > 1.0 + 1.5 * w) discard;
    if (f >= 1.0) col = uRim;
  } else {
    if (f > 1.0) discard;
    if (length(p) * vSize * 0.5 < 0.75) col = uCore;
  }
  gl_FragColor = vec4(col, 1.0);
  #include <colorspace_fragment>
}`;
/** at most this many poofs at once (a burst of newcomers: the oldest ones' last frames are dropped) */
const MAX = 6;

export interface Poof {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  /** when it began, the scene's time */
  readonly t0: number;
  readonly seed: number;
}

export interface Poofs {
  /** pose every poof at t, `scale` the camera's office units per css px (puffBoost); false: none left (the loop may stop) */
  draw(t: number, poofs: readonly Poof[], scale: number): boolean;
  dispose(): void;
}

/** the sparkles' point cloud: SPARK.n × MAX points, their size (css px) and spin per point, drawn over everything */
function sparkCloud(): { pts: Points; geo: BufferGeometry; mat: ShaderMaterial; pos: BufferAttribute; size: BufferAttribute; rim: BufferAttribute; spin: BufferAttribute } {
  const n = SPARK.n * MAX;
  const geo = new BufferGeometry();
  const pos = new BufferAttribute(new Float32Array(n * 3), 3);
  const size = new BufferAttribute(new Float32Array(n), 1);
  const spin = new BufferAttribute(new Float32Array(n), 1);
  geo.setAttribute('position', pos);
  geo.setAttribute('aSize', size);
  geo.setAttribute('aSpin', spin);
  const rim = new BufferAttribute(new Float32Array(n), 1);
  geo.setAttribute('aRim', rim);
  geo.setDrawRange(0, 0);
  const mat = new ShaderMaterial({
    uniforms: { uDpr: { value: 1 }, uGold: { value: new Color(SPARK_COLOR) }, uRim: { value: new Color(SPARK_RIM) }, uCore: { value: new Color(SPARK_CORE) } },
    vertexShader: SPARK_VERT,
    fragmentShader: SPARK_FRAG,
    // over everything, never tone mapped (designer m_49932)
    depthTest: false,
    depthWrite: false,
    transparent: true,
    toneMapped: false,
  });
  const pts = new Points(geo, mat);
  pts.frustumCulled = false;
  pts.visible = false;
  // its size in device px: the renderer's own pixel ratio, read as it draws (the tile, the town's bake, the hero)
  pts.onBeforeRender = (r) => {
    const u = mat.uniforms.uDpr;
    if (u) u.value = r.getPixelRatio();
  };
  return { pts, geo, mat, pos, size, rim, spin };
}

export function makePoofs(root: Group, style: Style): Poofs {
  const puffGeo = style.ball(1);
  // designer m_49905: under the sky's light the paper read grey: the puffs glow half their own paper white
  const puffMat = style.material(PUFF_COLOR, { emissive: PUFF_COLOR, ei: 0.55 });
  const mesh = (g: BufferGeometry, m: Material, count: number): InstancedMesh => {
    const im = new InstancedMesh(g, m, count * MAX);
    im.count = 0;
    im.frustumCulled = false;
    im.visible = false;
    root.add(im);
    return im;
  };
  const puffs = mesh(puffGeo, puffMat, PUFF.n);
  const sparks = sparkCloud();
  // the sparkles drawn last (over the puffs, the bims and the room)
  sparks.pts.renderOrder = 1000;
  root.add(sparks.pts);
  const m4 = new Matrix4();
  const p3 = new Vector3();
  const s3 = new Vector3();
  const q0 = new Quaternion();

  return {
    draw(t, list, scale) {
      const boost = puffBoost(scale);
      const spark = sparkBoost(scale);
      const spread = sparkSpread(scale);
      const live = list.filter((p) => t - p.t0 >= 0 && t - p.t0 < POOF_S).slice(-MAX);
      let ip = 0;
      let is = 0;
      for (const p of live) {
        for (const b of poofAt(p.seed, t - p.t0, boost, spark, spread)) {
          if (b.r <= 1e-3) continue;
          p3.set(p.x + b.x, p.y + b.y, p.z + b.z);
          s3.setScalar(b.r);
          if (b.kind === 'puff') puffs.setMatrixAt(ip++, m4.compose(p3, q0, s3));
          else {
            sparks.pos.setXYZ(is, p3.x, p3.y, p3.z);
            const px = sparkPx(b.r, scale);
            sparks.size.setX(is, px.size);
            sparks.rim.setX(is, px.rim);
            sparks.spin.setX(is, b.spin);
            is++;
          }
        }
      }
      puffs.count = ip;
      puffs.visible = ip > 0;
      puffs.instanceMatrix.needsUpdate = true;
      sparks.geo.setDrawRange(0, is);
      sparks.pts.visible = is > 0;
      sparks.pos.needsUpdate = true;
      sparks.size.needsUpdate = true;
      sparks.rim.needsUpdate = true;
      sparks.spin.needsUpdate = true;
      return live.length > 0;
    },
    dispose() {
      root.remove(puffs, sparks.pts);
      puffGeo.dispose();
      puffMat.dispose();
      sparks.geo.dispose();
      sparks.mat.dispose();
    },
  };
}
