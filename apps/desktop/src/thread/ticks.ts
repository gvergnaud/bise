// His ticks beside a thread (msg-dots, his ask m_60465, designer m_60490): one per message of his,
// his last MARKS. The hub says where his last ones are (`thread/marks`, store.subscribeThread: the
// view holds only the thread's tail); his messages that came since, and any the view holds, join
// them by pos. Which one he is in, and the bubble's time. Pure. Owner: msg-dots.
import type { Entry } from '@/proto';
import { MARKS, type ThreadMarks } from '@/store';
import { whenOf } from './entry';

/** One tick: his message's entry pos, its words cut short, when he sent it. */
export interface Dot {
  readonly pos: number;
  readonly text: string;
  readonly at: number;
}

/** A mark's words at most (chars), as the hub's `thread::marks::TEXT_MAX`. */
const TEXT_MAX = 200;

/** His words on one line, at most TEXT_MAX chars, cut at a word with '…' (the hub's `short`). Pure. */
export function shortWords(text: string): string {
  const one = text.split(/\s+/).filter((w) => w !== '').join(' ');
  // by code points, as the hub's chars() (an emoji of several may be cut: a bubble's words, not a quote)
  const chars = Array.from(one);
  if (chars.length <= TEXT_MAX) return one;
  const cut = chars.slice(0, TEXT_MAX).join('');
  const i = cut.lastIndexOf(' ');
  return `${(i > TEXT_MAX / 2 ? cut.slice(0, i) : cut).trimEnd()}…`;
}

/**
 * The ticks: the hub's marks and his messages the view holds (`held`, entries in the view's
 * order), one per pos, oldest first, the newest `n`. Pure.
 */
export function dotsOf(marks: ThreadMarks | null, held: readonly (Entry | undefined)[], n: number = MARKS): readonly Dot[] {
  const by = new Map<number, Dot>();
  for (const m of marks?.marks ?? []) by.set(m.pos, { pos: m.pos, text: m.text, at: m.at_ms });
  for (const e of held) if (e?.kind === 'you') by.set(e.pos, { pos: e.pos, text: shortWords(e.text), at: e.at_ms });
  const all = [...by.values()].sort((a, b) => a.pos - b.pos);
  return all.slice(Math.max(0, all.length - n));
}

/**
 * The tick he is in: the last of his messages at or above `reading` (the pos of the row in the top
 * third of the list); at the list's end (`reading` null) his newest. None when every tick is below. Pure.
 */
export function currentDot(dots: readonly Dot[], reading: number | null): number | null {
  if (reading === null) return dots[dots.length - 1]?.pos ?? null;
  let cur: number | null = null;
  for (const d of dots) if (d.pos <= reading) cur = d.pos;
  return cur;
}

/** The bubble's time: '16:43' today, 'yesterday 16:43', else '3 Nov 16:43'. Pure (`now`, `tz` given). */
export function dotWhen(ms: number, now: number, tz?: string): string {
  if (ms <= 0) return '';
  const w = whenOf(ms, now, tz);
  const y = whenOf(now - 86_400_000, now, tz);
  // yesterday's day words, as whenOf writes them ('10 Nov 16:43' -> '10 Nov')
  const yDay = y.slice(0, y.lastIndexOf(' '));
  return yDay !== '' && w.startsWith(`${yDay} `) ? `yesterday ${w.slice(yDay.length + 1)}` : w;
}

/** The tick ↑/↓ moves to from `pos` (the first or last at the ends). Pure. */
export function stepDot(dots: readonly Dot[], pos: number, by: -1 | 1): number | null {
  const i = dots.findIndex((d) => d.pos === pos);
  if (i < 0) return dots[0]?.pos ?? null;
  return dots[Math.min(dots.length - 1, Math.max(0, i + by))]?.pos ?? null;
}
