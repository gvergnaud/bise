// His plugins' objects' '?' / '✗' pills over the office (bims-plugins p4 step 2): a broken plugin's
// object (glass) gets the '✗', one that waits on his login the '?', each on its object's drawn top (the gap a
// bim's pill keeps over its head). An object that stands for plugins not drawn carries the worst of
// their pills, and its pill is that plugin's (Placed.pillOf). Each pill's tip names its plugin and
// its state ('slides failed to start', designer m_43241). Hovering an object names its plugin in a
// tip on its top (objTipOf, Gabriel m_51985). Pure: the office (Office.tsx) gives it the canvas's projection.
// Owner: bims-life.
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

/** the name tip over a hovered object: its plugin's key, its name as he reads it, its drawn top in css px */
export interface ObjTip {
  readonly key: string;
  readonly name: string;
  /** the object wears a '?' / '✗' pill on that top: the tip goes over it */
  readonly pill: boolean;
  readonly x: number;
  readonly y: number;
}

/**
 * The tip over the object under his pointer (Gabriel m_51985: hovering a plugin's object names
 * the plugin), on its drawn top as its pill is; null when none is hovered, the key is no drawn
 * object, or it is off view. Pure: `topOf` as objPillsOf's.
 */
export function objTipOf(placed: readonly Placed[], key: string | null, topOf: (key: string) => { readonly x: number; readonly y: number } | null): ObjTip | null {
  if (key === null) return null;
  const p = placed.find((q) => q.key === key);
  if (p === undefined) return null;
  const s = topOf(p.key);
  return s === null ? null : { key: p.key, name: p.name, pill: p.pill !== null, x: Math.round(s.x), y: Math.round(s.y) };
}
