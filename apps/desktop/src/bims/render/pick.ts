// What is under a canvas point (pure): the nearest bim's body, else the desk under it; and the plugin object under it (pickObjectAt). index.ts
// projects the heads and desks with camera.toScreen and hands them here.

export interface Target {
  readonly name: string;
  /** the bim's head and feet on the canvas, css px; null when it is off the map (gone, faded out) */
  readonly head: { readonly x: number; readonly y: number } | null;
  readonly feet: { readonly x: number; readonly y: number } | null;
  /** its desk's centre on the canvas and its half size, css px */
  readonly desk: { readonly x: number; readonly y: number; readonly r: number } | null;
}

/** how far from a bim's body a point still picks it, css px */
export const BODY_PX = 14;

/** the distance from point p to the segment a-b */
function toSegment(p: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const l2 = dx * dx + dy * dy;
  const f = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2));
  return Math.hypot(p.x - (a.x + dx * f), p.y - (a.y + dy * f));
}

export function pickAt(targets: readonly Target[], x: number, y: number): string | null {
  const p = { x, y };
  let best: string | null = null;
  let bd = BODY_PX;
  for (const t of targets) {
    if (!t.head || !t.feet) continue;
    const d = toSegment(p, t.head, t.feet);
    if (d < bd) { bd = d; best = t.name; }
  }
  if (best !== null) return best;
  let deskBest: string | null = null;
  let dd = Infinity;
  for (const t of targets) {
    if (!t.desk) continue;
    const d = Math.hypot(x - t.desk.x, y - t.desk.y);
    if (d < t.desk.r && d < dd) { dd = d; deskBest = t.name; }
  }
  return deskBest;
}

/** a plugin's object as drawn on the canvas (its box's corners projected), css px */
export interface ObjTarget {
  readonly key: string;
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

/**
 * The plugin object under a canvas point (his hover names its plugin, Gabriel m_51985): of the
 * drawn boxes that hold it, the front one (its foot lowest on the canvas, nearest the camera);
 * null on none. The bims go first: the office asks this only when pickAt found none.
 */
export function pickObjectAt(targets: readonly ObjTarget[], x: number, y: number): string | null {
  let best: ObjTarget | null = null;
  for (const t of targets) {
    if (x < t.left || x > t.right || y < t.top || y > t.bottom) continue;
    if (best === null || t.bottom > best.bottom) best = t;
  }
  return best?.key ?? null;
}
