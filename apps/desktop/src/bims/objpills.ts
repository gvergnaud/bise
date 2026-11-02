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
import { BODY_PX } from './render/pick';

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
  /** its drawn foot's y (the tip goes under it when its place over the object is a bim's) */
  readonly foot: number;
}

/**
 * The tip of the object under his pointer (Gabriel m_51985: hovering a plugin's object names its
 * plugins); null when none is hovered, the key is no drawn object, or it is off view. Pure:
 * `at` gives an object's drawn top and foot on the canvas.
 */
export function objTipOf(
  placed: readonly Placed[],
  key: string | null,
  at: (key: string) => { readonly x: number; readonly y: number; readonly foot: number } | null,
): ObjTip | null {
  if (key === null) return null;
  const p = placed.find((q) => q.key === key);
  if (p === undefined) return null;
  const s = at(p.key);
  return s === null ? null : { key: p.key, words: namesWords(p.names), pill: p.pill !== null, x: Math.round(s.x), y: Math.round(s.y), foot: Math.round(s.foot) };
}

/** an object's pill's height, css px (office.css .bims-pill: --s-4): a tip over a pill clears it */
export const OBJ_PILL_PX = 16;

/**
 * Where an object's tip goes in the tile (designer m_51998), css px from the tile's top left: a
 * tip's gap over the object's top (over its pill when it wears one), centred on it; under its foot
 * (the same gap) when a bim's head sits in that place or the tile has no room over it; always
 * TIP_GAP inside the tile's edges. `heads`: the bims' heads on the canvas. Pure.
 */
export function placeObjTip(
  tip: ObjTip,
  size: { readonly w: number; readonly h: number },
  tile: { readonly w: number; readonly h: number },
  heads: readonly { readonly x: number; readonly y: number }[],
): { readonly left: number; readonly top: number; readonly side: 'above' | 'below' } {
  const clamp = (v: number, hi: number): number => Math.max(TIP_GAP, Math.min(v, hi - TIP_GAP));
  const left = Math.round(clamp(tip.x - size.w / 2, tile.w - size.w));
  const above = tip.y - TIP_GAP - (tip.pill ? OBJ_PILL_PX : 0) - size.h;
  // a head near the tip's box (a bim's head is about BODY_PX round, under its top point)
  const onHead = heads.some((h) => h.x >= left - BODY_PX && h.x <= left + size.w + BODY_PX && h.y >= above - 2 * BODY_PX && h.y <= above + size.h);
  const side = above >= TIP_GAP && !onHead ? 'above' : 'below';
  const top = side === 'above' ? above : tip.foot + TIP_GAP;
  return { left, top: Math.round(clamp(top, tile.h - size.h)), side };
}
