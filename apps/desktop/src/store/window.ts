// A thread's detached window (inbox-open, architect m_26597): the thread's view is its tail (the
// newest page and the older pages above it, one contiguous run) plus at most one window around a
// far entry (his answer weeks back), read with the hub's own `thread/page {before: pos + k}`. The
// positions between them are not held: the gap, `{lo, hi}` (the window's newest pos, the tail's
// oldest), where the list draws its gap row; paging newer from the window (`from`: the transcript
// lines under it are read) fills it until it meets the tail, then they merge. A new jump replaces
// the window, once the window's page on its way is in (the store asks one at a time). Pure: the store keeps the asks and calls these. Owner: inbox-open.
//
// Why the steps are safe: a page reads the 600 transcript lines before `before` and gives its
// newest `limit` entries (bise_proto::thread::page), an entry at most one per line. A newer ask
// reads 150 lines past `from` with limit 200, so every entry in [from, before) comes whole; its
// newest entry may be cut by `before` (an entry spans lines), so it is held back and read again by
// the next ask, unless it is the only one (then `from` moves by the lines read). A jump reads
// JUMP_AFTER lines past the entry, so it is among the newest JUMP_LIMIT.
import type { ThreadOrder } from "./types";

/** The positions not held between the window and the tail: (lo, hi). */
export interface Gap {
  /** the window's newest held pos */
  readonly lo: number;
  /** the tail's oldest held pos */
  readonly hi: number;
  /** the transcript lines before it are read (the next newer page starts there) */
  readonly from: number;
}

/** A page asked for the window, kept until its answer comes. */
export type Ask =
  | { readonly kind: "jump"; readonly pos: number; readonly before: number; readonly hi: number }
  | { readonly kind: "newer"; readonly from: number; readonly before: number; readonly hi: number };

/** Transcript lines read after the entry a jump goes to (its context under it). */
export const JUMP_AFTER = 24;
/** A jump page's entries. */
export const JUMP_LIMIT = 60;
/** Transcript lines a newer page reads past the window (under thread::page's 200 entries). */
export const NEWER_LINES = 150;
/** A newer page's entries (the hub's MAX_LIMIT). */
export const NEWER_LIMIT = 200;
/**
 * An older page's entries, as he scrolls up (the hub's MAX_LIMIT, never its default 40): one page
 * is a round trip to a hub that may be busy (harness's: 140 ms a page, seconds behind its state
 * pushes), and main's 600 lines hold ~240 short entries (agents' messages), so 40 a page made
 * his scroll up wait at 'older messages…' every screen or two (thread-older).
 */
export const OLDER_LIMIT = 200;

/** What a jump to `pos` does. Pure. */
export type JumpPlan =
  /** the entry is held: show it (`pos`: the row that shows it, the next one when it has none) */
  | { readonly kind: "held"; readonly pos: number }
  /** the thread is not loaded yet, or the entry is newer than anything held (it comes live) */
  | { readonly kind: "wait" }
  /** ask this page; the window held before (if any) leaves the view */
  | { readonly kind: "ask"; readonly ask: Ask; readonly limit: number; readonly view: ThreadOrder };

/** The tail's oldest held pos (the gap's top, else the first). */
function tailStart(v: ThreadOrder): number | undefined {
  return v.gap?.hi ?? v.order[0];
}

export function planJump(v: ThreadOrder, pos: number): JumpPlan {
  if (!v.loaded) return { kind: "wait" };
  if (v.order.includes(pos)) return { kind: "held", pos };
  const hi = tailStart(v);
  const first = v.order[0];
  const last = v.order[v.order.length - 1];
  if (hi === undefined || first === undefined || last === undefined || pos > last) return { kind: "wait" };
  // inside a held run but not an entry of it (a line no row shows): the next entry after it
  const inWindow = v.gap !== undefined && pos > first && pos < v.gap.lo;
  if (inWindow || pos > hi) return { kind: "held", pos: v.order.find((p) => p > pos) ?? last };
  const before = Math.min(pos + 1 + JUMP_AFTER, hi);
  // a new jump replaces the window: the view is the tail again, with the tail's own before/more
  const view: ThreadOrder =
    v.gap === undefined
      ? v
      : { order: v.order.filter((p) => p >= hi), before: hi, more: true, loaded: true };
  return { kind: "ask", ask: { kind: "jump", pos, before, hi }, limit: JUMP_LIMIT, view };
}

/** The newer page to ask, while there is a gap. Pure. */
export function planNewer(v: ThreadOrder): { readonly ask: Ask; readonly limit: number } | null {
  const g = v.gap;
  if (g === undefined) return null;
  const before = Math.min(g.from + NEWER_LINES, g.hi);
  return { ask: { kind: "newer", from: g.from, before, hi: g.hi }, limit: NEWER_LIMIT };
}

/**
 * Whether a page answers the ask: a jump's holds its entry's run (first ≤ pos ≤ last, or empty),
 * a newer one reaches into the window (an older page of the window stays under its first), and
 * neither reaches the tail (the tail's own page, a reconnection's). Pure.
 */
export function answers(ask: Ask, v: ThreadOrder, pos: readonly number[]): boolean {
  const first = pos[0];
  const last = pos[pos.length - 1];
  if (first === undefined || last === undefined) return true;
  if (last >= ask.hi) return false;
  if (ask.kind === "jump") return first <= ask.pos && ask.pos <= last;
  const top = v.order[0];
  return top !== undefined && last >= top;
}

/**
 * The view after a page: its positions merged in, the gap moved. `ask`: the page answers it.
 * An older page's before/more replace the view's (it is the oldest); a page that reaches the
 * tail from inside the gap moves the gap's top down. Pure.
 */
export function merged(v: ThreadOrder, pos: readonly number[], before: number | null, more: boolean, ask: Ask | null): ThreadOrder {
  let keep = pos;
  let gap = v.gap;
  if (ask?.kind === "jump") {
    if (ask.before < ask.hi) {
      // its newest entry may be cut by `before` (never the one it went to): read again by the next newer page
      const last = pos[pos.length - 1];
      const cut = last !== undefined && last > ask.pos && pos.length > 1 ? last : null;
      keep = cut !== null ? pos.slice(0, -1) : pos;
      const lo = keep[keep.length - 1] ?? ask.pos;
      gap = { lo, hi: ask.hi, from: cut ?? ask.before };
    } else gap = undefined;
  } else if (ask?.kind === "newer" && gap !== undefined) {
    if (ask.before >= ask.hi) gap = undefined;
    else {
      const fresh = pos.filter((p) => p >= ask.from);
      const last = fresh[fresh.length - 1];
      const cut = last !== undefined && fresh.length > 1;
      if (cut) keep = pos.filter((p) => p !== last);
      const lo = Math.max(gap.lo, ...keep.filter((p) => p < ask.hi));
      gap = { lo, hi: gap.hi, from: cut ? last : ask.before };
    }
  } else if (gap !== undefined) {
    // not an answer (the tail's page, an older page of the window, a reconnection's): what falls
    // in the gap is held only when the page reaches the tail, whose run then starts lower
    const g = gap;
    const reaches = pos.some((p) => p >= g.hi);
    if (reaches) {
      const low = pos.filter((p) => p > g.lo && p < g.hi);
      const start = low[0];
      if (start !== undefined) gap = { ...g, hi: start };
    } else keep = pos.filter((p) => p <= g.lo || p >= g.hi);
  }
  const all = new Set(v.order);
  for (const p of keep) all.add(p);
  const order = [...all].sort((a, b) => a - b);
  const oldest = pos.length === 0 || (v.order.length > 0 && (pos[0] ?? 0) < (v.order[0] ?? 0));
  const fresh = !v.loaded;
  return {
    order,
    before: fresh || oldest ? before : v.before,
    more: fresh || oldest ? more : v.more,
    loaded: true,
    ...(gap === undefined ? {} : { gap }),
  };
}

/** A live entry the view doesn't hold yet: listed, unless it falls in the gap. Pure. */
export function listsLive(v: ThreadOrder, pos: number): boolean {
  return v.gap === undefined || pos <= v.gap.lo || pos >= v.gap.hi;
}
