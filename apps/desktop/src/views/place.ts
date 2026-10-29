// src/views · the views layer's model (docs/mcp-apps.md risk 2, placement agreed with native-lead
// m_50044): one layer for the whole window keeps every live view's frame and places it over its
// anchor (its card under the tool row, or its panel tab), clipped to what its column shows. A
// frame is never moved in the DOM: an anchor gone (Virtuoso unmounted the row, the tab closed)
// hides it; past MAX_LIVE live frames the one shown longest ago is torn down and comes back from
// its stored result when its anchor shows again. Pure. Owner: mcp-apps-host.

/** Live frames at most; past it the oldest hidden one is torn down. */
export const MAX_LIVE = 8;

/** A thread view sits under the panel's floating sheet (z 2), a tab view over it (z 4); no stacking context of its own. */
export const Z = { thread: 2, tab: 4 } as const;

export type Slot = keyof typeof Z;

export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
}

/** Where a frame goes: its anchor's box, the inset that clips it to its column, its z. */
export interface Place {
  readonly slot: Slot;
  readonly box: Rect;
  /** clip-path: inset(top right bottom left), css px, each ≥ 0 */
  readonly inset: readonly [number, number, number, number];
  readonly z: number;
  /** what shows of it, in window px (Electron main's frames list: permissions, gestures) */
  readonly seen: Rect;
}

/** The part of `a` inside `b` (w or h 0 when they don't meet). Pure. */
export function intersect(a: Rect, b: Rect): Rect {
  const x = Math.max(a.x, b.x);
  const y = Math.max(a.y, b.y);
  const r = Math.min(a.x + a.w, b.x + b.w);
  const bt = Math.min(a.y + a.h, b.y + b.h);
  return { x, y, w: Math.max(0, r - x), h: Math.max(0, bt - y) };
}

/**
 * A frame's place over its anchor's box, clipped by every box that clips it (its scrolling
 * column, the panel's content…): null when nothing of it shows. Pure.
 */
export function placeOf(slot: Slot, box: Rect, clips: readonly Rect[]): Place | null {
  if (box.w <= 0 || box.h <= 0) return null;
  const seen = clips.reduce(intersect, box);
  if (seen.w <= 0 || seen.h <= 0) return null;
  const inset: [number, number, number, number] = [seen.y - box.y, box.x + box.w - (seen.x + seen.w), box.y + box.h - (seen.y + seen.h), seen.x - box.x];
  return { slot, box, inset, z: Z[slot], seen };
}

/** The anchors a view has now, by slot: a box and its clips, or absent. */
export interface Anchors {
  readonly thread?: { readonly box: Rect; readonly clips: readonly Rect[] };
  readonly tab?: { readonly box: Rect; readonly clips: readonly Rect[] };
}

/**
 * Where a view's one frame goes: in its tab when fullscreen and the tab shows, else over its
 * card, else nowhere (hidden, kept). A fullscreen view whose tab is hidden (another tab in front)
 * never falls back to its card: the card says it is in the panel. Pure.
 */
export function pick(mode: 'inline' | 'fullscreen', a: Anchors): Place | null {
  if (mode === 'fullscreen') return a.tab === undefined ? null : placeOf('tab', a.tab.box, a.tab.clips);
  return a.thread === undefined ? null : placeOf('thread', a.thread.box, a.thread.clips);
}

/**
 * Which live views to tear down: past MAX_LIVE, the ones shown longest ago (`seenAt`, ms), never
 * one that shows now. Pure.
 */
export function evict(live: ReadonlyMap<string, number>, showing: ReadonlySet<string>, max: number = MAX_LIVE): string[] {
  if (live.size <= max) return [];
  const hidden = [...live.entries()].filter(([k]) => !showing.has(k)).sort((a, b) => a[1] - b[1]);
  return hidden.slice(0, live.size - max).map(([k]) => k);
}

/** Two places the same (no style to write)? Pure. */
export function samePlace(a: Place | null, b: Place | null): boolean {
  if (a === null || b === null) return a === b;
  return a.slot === b.slot && a.box.x === b.box.x && a.box.y === b.box.y && a.box.w === b.box.w && a.box.h === b.box.h && a.inset.every((v, i) => v === b.inset[i]);
}
