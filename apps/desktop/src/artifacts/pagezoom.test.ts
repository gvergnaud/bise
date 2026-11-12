import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { FIT, MAX_PAGE_ZOOM, atFit, clampView, lookOf, panBy, pinchFactor, roomOf, toPage, toScreen, zoomAt } from './pagezoom';

const box = { w: 600, h: 800 };
const close = (a: { x: number; y: number }, b: { x: number; y: number }) => {
  expect(a.x).toBeCloseTo(b.x, 6);
  expect(a.y).toBeCloseTo(b.y, 6);
};

describe('a web page pinch (pagezoom)', () => {
  it("the most it zooms is Preview's image zoom (plugins/media's zoom.ts MAX_K): 8 screen px per page px", () => {
    const zoom = readFileSync(new URL('../../../../plugins/media/ai.mistral.vibe/views/preview/zoom.ts', import.meta.url), 'utf8');
    const m = /export const MAX_K = (\d+(?:\.\d+)?);/u.exec(zoom);
    expect(m, "Preview's zoom.ts exports MAX_K").not.toBeNull();
    expect(MAX_PAGE_ZOOM).toBe(Number(m?.[1]));
  });

  it('zoom stays in [fit, MAX]: a pinch out past fit is the fit itself, in past MAX stops there', () => {
    expect(zoomAt(FIT, 0.2, 100, 100, box)).toEqual(FIT);
    const deep = zoomAt(FIT, 1000, 100, 100, box);
    expect(deep.z).toBe(MAX_PAGE_ZOOM);
    const back = zoomAt(deep, 1 / 1000, 300, 300, box);
    expect(back).toEqual(FIT);
    expect(atFit(back)).toBe(true);
    expect(clampView({ z: Number.NaN, tx: 5, ty: 5 }, box)).toEqual(FIT);
  });

  it('zooms around the fingers: the page point under them stays under them', () => {
    const at = { x: 420, y: 130 };
    const v = zoomAt(FIT, pinchFactor(-120), at.x, at.y, box);
    expect(v.z).toBeGreaterThan(3);
    close(toScreen(v, at.x, at.y), at);
    // a second pinch elsewhere, from a zoomed view: the page point under the fingers stays
    const fingers = { x: 200, y: 600 };
    const p = toPage(v, fingers.x, fingers.y);
    const w = zoomAt(v, 1.3, p.x, p.y, box);
    close(toScreen(w, p.x, p.y), fingers);
  });

  it('fingers apart zoom in, together zoom out', () => {
    expect(pinchFactor(-10)).toBeGreaterThan(1);
    expect(pinchFactor(10)).toBeLessThan(1);
    expect(pinchFactor(0)).toBe(1);
  });

  it('a pan follows the fingers and never shows past the page edge; at fit there is nothing to pan', () => {
    expect(panBy(FIT, 50, 50, box)).toEqual(FIT);
    const v = zoomAt(FIT, 3, 300, 400, box);
    const down = panBy(v, 0, 30, box);
    expect(down.ty).toBeCloseTo(v.ty - 30, 6);
    const far = panBy(v, 1e6, 1e6, box);
    expect(far.tx).toBeCloseTo(box.w * (1 - v.z), 6);
    expect(far.ty).toBeCloseTo(box.h * (1 - v.z), 6);
    const home = panBy(v, -1e6, -1e6, box);
    expect(home).toEqual({ z: v.z, tx: 0, ty: 0 });
  });

  it('a page point keeps its place on the page through a zoom to 3 and a pan (notes and selections are page points)', () => {
    const note = { x: 120, y: 340 };
    const v = panBy(zoomAt(FIT, 3, 100, 300, box), 40, -25, box);
    expect(v.z).toBeCloseTo(3, 6);
    const shown = toScreen(v, note.x, note.y);
    // the frame draws it at the page point; the window shows it where the zoom puts that point
    expect(shown.x).toBeCloseTo(v.tx + 3 * note.x, 6);
    close(toPage(v, shown.x, shown.y), note);
  });

  it('the room the frame reads: none at fit, then how far each way the view can still go', () => {
    expect(roomOf(FIT, box)).toBeNull();
    const v = zoomAt(FIT, 2, 0, 0, box);
    expect(roomOf(v, box)).toEqual({ l: 0, r: box.w, u: 0, d: box.h });
    const end = panBy(v, 1e6, 1e6, box);
    const r = roomOf(end, box);
    expect(r?.r).toBeCloseTo(0, 6);
    expect(r?.d).toBeCloseTo(0, 6);
    expect(r?.l).toBeCloseTo(box.w, 6);
  });

  it("the frame's look: none at fit; zoomed, a transform and a clip that shows exactly its own box", () => {
    expect(lookOf(FIT, box, 8)).toBeNull();
    const v = { z: 2, tx: -100, ty: -200 };
    const look = lookOf(v, box, 8);
    expect(look?.transform).toBe('translate(-100px, -200px) scale(2)');
    expect(look?.transformOrigin).toBe('0 0');
    // in the frame's own px, the box [0,600]×[0,800] is [50,350]×[100,500]: inset top 100, right 250, bottom 300, left 50
    expect(look?.clipPath).toBe('inset(100px 250px 300px 50px round 4px)');
  });
});
