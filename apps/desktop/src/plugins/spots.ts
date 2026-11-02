// The plugins place's room (bims-plugins' hero, Gabriel's note n5 on page bims-plugins v10): a slab
// with two low walls, his plugins' objects on its spots, the next free spots dotted. What goes
// where is decided here, pure and three-free (the renderer, render.ts, only draws it): each object
// takes the spot drawn for its own kind of object, else the first free spot it fits, else the
// roomiest free one, scaled down to it. The room holds SPOTS.length objects: the ones on first.
// Also the objects of rows that bims-link's objectOf gives none (a connector he never added: its
// own object, never glass). Owner: plug-desktop.
import { defaultObject, kindOf, objectOf, type Picks, type PluginObject, type PluginRow } from '@/bims/plugin-objects';
import type { ObjId } from '@/bims/render/objects/ids';

/** One spot of the room: where its object stands (room units, the slab centred on 0), how it turns, the object it was drawn for. */
export interface Spot {
  readonly x: number;
  readonly z: number;
  readonly rot: number;
  /** the object this spot was drawn for (its size is the spot's room) */
  readonly for: ObjId;
  /** the spot hangs on the back wall */
  readonly wall?: boolean;
}

/** the slab's floor, room units */
export const ROOM = { w: 9.4, d: 4.8 } as const;

/** bims-plugins' hero spots (scratch entry.ts HERO, signed with plugins-ui), in the order they fill. */
export const SPOTS: readonly Spot[] = [
  { for: 'cushion', x: 0.4, z: 1.2, rot: 0.3 },
  { for: 'library', x: -2.6, z: -1.82, rot: 0 },
  { for: 'arcade', x: -3.0, z: 0.35, rot: 0.35 },
  { for: 'chess', x: -1.05, z: 0.25, rot: 0.12 },
  { for: 'jukebox', x: -0.85, z: -1.85, rot: 0 },
  { for: 'visio', x: 3.15, z: 1.0, rot: -0.45 },
  { for: 'printer', x: 1.7, z: -0.6, rot: -0.2 },
  { for: 'aquarium', x: 1.45, z: -1.95, rot: 0 },
  { for: 'shelf', x: -4.25, z: -0.55, rot: Math.PI / 2 },
  { for: 'rack', x: 3.75, z: -1.8, rot: 0 },
  { for: 'babyfoot', x: 1.55, z: 1.45, rot: 0.18 },
  { for: 'calendar', x: 0.3, z: -2.17, rot: 0, wall: true },
  { for: 'easel', x: -3.55, z: 1.65, rot: 0.55 },
  { for: 'plant', x: 4.15, z: 1.65, rot: 0 },
  { for: 'trophies', x: -4.2, z: 1.0, rot: Math.PI / 2 },
  { for: 'telescope', x: 2.75, z: -0.3, rot: -0.6 },
];

/** An object's floor and height, room units, and whether it hangs on a wall (makeObject's foot). */
export interface Foot {
  readonly w: number;
  readonly d: number;
  readonly h: number;
  readonly wall?: boolean;
}

/** One object in the room: where it stands and how much it shrinks to fit its spot (1: its own size). */
export interface Placed {
  readonly o: PluginObject;
  readonly spot: Spot;
  readonly scale: number;
}

/** What the room shows: the placed objects, then the next free spots (at most 3), dotted. */
export interface RoomPlan {
  readonly placed: readonly Placed[];
  readonly dotted: readonly Spot[];
}

const DOTTED = 3;

/**
 * Where each object goes. `footOf` gives an object's foot (the renderer measures them). The ones on
 * come first, then glass, each by key (officeObjects' order); past SPOTS.length, the rest stay out.
 */
export function planRoom(objs: readonly PluginObject[], footOf: (id: ObjId) => Foot): RoomPlan {
  const order = [...objs].sort((a, b) => Number(a.glass) - Number(b.glass) || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  const free = new Set(SPOTS.map((_, i) => i));
  const placed: Placed[] = [];
  // first pass: an object whose spot was drawn for it takes it
  const mine = new Map<PluginObject, number>();
  for (const o of order) {
    const i = SPOTS.findIndex((s, j) => free.has(j) && s.for === spotFor(o.obj));
    if (i >= 0) {
      free.delete(i);
      mine.set(o, i);
    }
  }
  for (const o of order) {
    const own = mine.get(o);
    if (own !== undefined) {
      placed.push({ o, spot: SPOTS[own] as Spot, scale: 1 });
      continue;
    }
    const f = footOf(o.obj);
    const open = [...free].filter((j) => (SPOTS[j]?.wall ?? false) === (f.wall ?? false));
    if (open.length === 0) continue;
    const room = (j: number): Foot => footOf((SPOTS[j] as Spot).for);
    const fits = open.find((j) => room(j).w >= f.w * 0.9 && room(j).d >= f.d * 0.9);
    const j = fits ?? open.reduce((a, b) => (area(room(b)) > area(room(a)) ? b : a));
    free.delete(j);
    const r = room(j);
    const scale = fits !== undefined ? 1 : Math.min(1, r.w / Math.max(f.w, 0.01), r.d / Math.max(f.d, 0.01));
    placed.push({ o, spot: SPOTS[j] as Spot, scale });
  }
  const dotted = SPOTS.filter((s, j) => free.has(j) && s.wall !== true).slice(0, DOTTED);
  return { placed, dotted };
}

/** bise's cat sleeps on the cushion: its spot is the cushion's. */
function spotFor(obj: ObjId): ObjId {
  return obj === 'cat' ? 'cushion' : obj;
}

const area = (f: Foot): number => f.w * f.d;

/** An icon's object: a plugin's object, and which of its looks (the shelf's book colours). */
export type IconObject = PluginObject & { readonly seed?: number };

/**
 * A row's object for its icon: objectOf's, else (a connector he never added, a marketplace plugin)
 * its default object, never glass, no pill; a connector with no object of its own: none. A row that
 * is on stays lit, a failed server too (its ✗ says it; designer m_40885); a skills folder takes a
 * look of its own (its seed), so two folders side by side differ. `lit`: drawn as on though the row
 * is off (computer, a built-in not set up yet). Null: the row brings none (shadowed): the plain tile.
 */
export function iconObject(row: PluginRow, picks: Picks, lit = false): IconObject | null {
  const o = objectOf(row, picks) ?? (row.state === 'available' ? defaultIcon(row) : null);
  if (o === null) return null;
  const kind = kindOf(row);
  // a connector no object stands for (the kind's plant) is its initials, never a stand-in (designer m_40880)
  if (kind === 'connector' && o.obj === 'plant' && !o.picked) return null;
  const glass = o.glass && row.state !== 'loaded' && !(lit && o.pill !== '✗');
  const seed = o.obj === 'shelf' ? seedOf(o.key) : 0;
  return { ...o, glass, ...(seed === 0 ? {} : { seed }) };
}

/** a look from the key, 0..11 (the shelf's books cycle through their colours) */
export function seedOf(key: string): number {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return h % 12;
}

/** a row he never added: its own object, never glass, no pill */
function defaultIcon(row: PluginRow): PluginObject {
  const d = defaultObject(row);
  return { key: row.id ?? row.name, names: [row.title !== undefined && row.title !== null && row.title !== '' ? row.title : row.name], obj: d.obj, ...(d.n === undefined ? {} : { n: d.n }), glass: false, pill: null, picked: false };
}

/** The cache key of one icon's render: `px` css px square, drawn at `scale` device px per css px. */
export function iconKey(o: Pick<IconObject, 'obj' | 'n' | 'glass' | 'seed'>, px: number, scale = 2): string {
  return `${o.obj}|${o.n ?? ''}|${o.glass ? 'g' : ''}|${o.seed ?? 0}|${px}${scale === 2 ? '' : `@${scale}x`}`;
}

/** How many device px an icon gets per css px on this screen: 2 at least (a 1x screen gets 2x for
 * free, the same PNG as on retina), the screen's ratio rounded up past it (3 on a 3x screen). */
export function iconScale(dpr: number): number {
  return Math.max(2, Math.min(3, Math.ceil(Number.isFinite(dpr) && dpr > 0 ? dpr : 1)));
}
