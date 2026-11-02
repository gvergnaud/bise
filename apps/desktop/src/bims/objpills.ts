// His plugins' objects' '?' / '✗' pills over the office (bims-plugins p4 step 2): a broken plugin's
// object (glass) gets the '✗', one that waits on his login the '?', each on its object's drawn top (the gap a
// bim's pill keeps over its head). An object that stands for plugins not drawn carries the worst of
// their pills, and its pill is that plugin's (Placed.pillOf). Each pill's tip names its plugin and
// its state ('slides failed to start', designer m_43241). Hovering an object names the plugins it
// stands for in the ds's tip over its top, under its foot when a bim's head is there (objTipOf,
// placeObjTip: Gabriel m_51985, designer m_51998). Pure: the office (Office.tsx) gives it the
// canvas's projection. Owner: bims-life.
import { TIP_GAP } from '@/ds';
import { namesWords } from '@/app/tipwords';
import { settings } from '@/proto/words.gen';
import type { Placed } from './corners';

/** a pill over an object: the key of the plugin it is for (its click opens it), its sign, css px on the canvas */
export interface ObjPill {
  readonly key: string;
  readonly pill: '?' | '✗';
  /** its tip: the plugin it is for and its state ('slides failed to start', 'Gmail needs a login'; designer m_43241) */
  readonly tip: string;
  readonly x: number;
  readonly y: number;
}

/**
 * The pills of the objects that have one and are in view, in the placed order: `topOf` gives an
 * object's drawn top on the canvas (the pill sits on it as a bim's on its head: designer m_41417). Pure.
 */
export function objPillsOf(placed: readonly Placed[], topOf: (key: string) => { readonly x: number; readonly y: number } | null): ObjPill[] {
  const out: ObjPill[] = [];
  for (const p of placed) {
    if (p.pill === null) continue;
    const s = topOf(p.key);
    if (s === null) continue;
    const key = p.pillOf ?? p.key;
    const name = p.pillName ?? key;
    out.push({ key, pill: p.pill, tip: p.pill === '✗' ? settings.plugins_waits_error({ name }) : settings.plugins_waits_login({ name }), x: Math.round(s.x), y: Math.round(s.y) });
  }
  return out;
}

/** the name tip of a hovered object: its key, its words, and where it is drawn on the canvas (css px) */
export interface ObjTip {
  readonly key: string;
  /** the plugins it stands for, all of them ('docs, sheets and slides', designer m_51998) */
  readonly words: string;
  /** it wears a '?' / '✗' pill on its top: the tip goes over the pill */
  readonly pill: boolean;
  /** its drawn top's middle (where its pill sits) */
  readonly x: number;
  readonly y: number;
  /** its drawn box on the canvas: its sides and its foot (the tip goes under or beside it) */
  readonly left: number;
  readonly right: number;
  readonly foot: number;
}

/** a box on the canvas, css px from the tile's top left */
export interface Rect {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

/**
 * The tip of the object under his pointer (Gabriel m_51985: hovering a plugin's object names its
 * plugins); null when none is hovered, the key is no drawn object, or it is off view. Pure:
 * `at` gives an object's drawn top (its middle) and its drawn box on the canvas.
 */
export function objTipOf(
  placed: readonly Placed[],
  key: string | null,
  at: (key: string) => { readonly x: number; readonly y: number; readonly box: Rect } | null,
): ObjTip | null {
  if (key === null) return null;
  const p = placed.find((q) => q.key === key);
  if (p === undefined) return null;
  const s = at(p.key);
  if (s === null) return null;
  const r = Math.round;
  return { key: p.key, words: namesWords(p.names), pill: p.pill !== null, x: r(s.x), y: r(s.y), left: r(s.box.left), right: r(s.box.right), foot: r(s.box.bottom) };
}

/** an object's pill's height, css px (office.css .bims-pill: --s-4): a tip over a pill clears it */
export const OBJ_PILL_PX = 16;
/** what an object's tip keeps clear of every bim, every chip of the tile and the tile's edge, css px (designer m_52074) */
export const TIP_CLEAR = 4;
/** a bim's half width around its head's middle, css px: its head and body box (headOf: its head's top, its feet) */
export const BIM_HALF_PX = 14;

/** where an object's tip goes: its box's top left in the tile, and the object's side it is on */
export interface TipAt {
  readonly left: number;
  readonly top: number;
  readonly side: 'above' | 'below' | 'left' | 'right';
}

/**
 * Where an object's tip goes in the tile (designer m_52074), css px from its top left: TIP_GAP
 * from the object, first over its top (over its pill when it wears one), else under its foot (both
 * centred on it, slid to stay in the tile), else beside it, left then right (centred on its
 * height). Each place keeps TIP_CLEAR from every bim's head and body box, from the tile's chips
 * and from its edge; none does: null, no tip (the pill or a zoom names it). Pure.
 */
export function placeObjTip(
  tip: ObjTip,
  size: { readonly w: number; readonly h: number },
  tile: { readonly w: number; readonly h: number },
  keep: { readonly bims: readonly Rect[]; readonly chips: readonly Rect[] },
): TipAt | null {
  const slide = (v: number, room: number, len: number): number => Math.max(TIP_CLEAR, Math.min(v, room - TIP_CLEAR - len));
  const across = slide(tip.x - size.w / 2, tile.w, size.w);
  const mid = slide((tip.y + tip.foot) / 2 - size.h / 2, tile.h, size.h);
  const places: readonly TipAt[] = [
    { side: 'above', left: across, top: tip.y - TIP_GAP - (tip.pill ? OBJ_PILL_PX : 0) - size.h },
    { side: 'below', left: across, top: tip.foot + TIP_GAP },
    { side: 'left', left: tip.left - TIP_GAP - size.w, top: mid },
    { side: 'right', left: tip.right + TIP_GAP, top: mid },
  ];
  const inTile = (p: TipAt): boolean => p.left >= TIP_CLEAR && p.top >= TIP_CLEAR && p.left + size.w <= tile.w - TIP_CLEAR && p.top + size.h <= tile.h - TIP_CLEAR;
  const hits = (p: TipAt, r: Rect): boolean =>
    p.left - TIP_CLEAR < r.right && p.left + size.w + TIP_CLEAR > r.left && p.top - TIP_CLEAR < r.bottom && p.top + size.h + TIP_CLEAR > r.top;
  const found = places.find((p) => inTile(p) && !keep.bims.some((r) => hits(p, r)) && !keep.chips.some((r) => hits(p, r)));
  return found === undefined ? null : { ...found, left: Math.round(found.left), top: Math.round(found.top) };
}

/** a bim's half width, arms out at its desk, as a share of its drawn height (head's top to feet) */
export const BIM_HALF_OF_HEIGHT = 0.4;

/**
 * A bim's head and body box on the canvas from its head's top and its feet (OfficeRenderer.headOf):
 * as wide as it is drawn, so a zoom widens it with its arms (ui-review m_52091: pinched, the tip
 * stood over main's hand). Pure.
 */
export function bimBox(h: { readonly x: number; readonly y: number; readonly feet: number }): Rect {
  const half = Math.max(BIM_HALF_PX, BIM_HALF_OF_HEIGHT * (h.feet - h.y));
  return { left: h.x - half, top: h.y, right: h.x + half, bottom: Math.max(h.y, h.feet) };
}

/** a bim's desk on the canvas (its top's middle and its reach, OfficeRenderer.deskSpots): what stands in front of an object hides its edge, the tip keeps off it. Pure. */
export function deskBox(d: { readonly x: number; readonly y: number; readonly r: number }): Rect {
  return { left: d.x - d.r, top: d.y - d.r, right: d.x + d.r, bottom: d.y + d.r };
}
