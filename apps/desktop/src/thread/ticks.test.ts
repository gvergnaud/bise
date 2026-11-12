// msg-dots (ticks.ts): pure rules: which ticks, the one he is in, the bubble's time, ↑/↓.
import { describe, expect, it } from 'vitest';
import type { Entry } from '@/proto';
import { currentDot, dotsOf, dotWhen, shortWords, stepDot } from './ticks';
import { dotsFit } from './MsgDots';

const you = (pos: number, text: string): Entry => ({ pos, at_ms: pos * 1000, kind: 'you', text });
const agent = (pos: number): Entry => ({ pos, at_ms: 0, kind: 'agent', text: 'reply' });

describe('dotsOf', () => {
  it('joins the hub marks and his held messages by pos, newest 20, oldest first', () => {
    const marks = { marks: Array.from({ length: 20 }, (_, i) => ({ pos: (i + 1) * 10, at_ms: 1, text: `m${String(i)}` })), upto: 200 };
    // two of his after `upto` (live), one held mark again, an agent's entry
    const dots = dotsOf(marks, [you(200, 'again'), agent(205), you(210, 'live one'), you(220, '  two\n words ')]);
    expect(dots).toHaveLength(20);
    expect(dots.map((d) => d.pos).slice(-3)).toEqual([200, 210, 220]);
    expect(dots[0]?.pos).toBe(30);
    expect(dots.at(-1)?.text).toBe('two words');
    expect(dots.find((d) => d.pos === 200)?.text).toBe('m19');
  });
  // msg-rail-fix (his m_63243 (3)): a held one's words from its parts, never its model text's markers
  it("a held one of his: its parts and their words; with none, the hub's mark's words", () => {
    const img = { kind: 'image' as const, name: '[Image #1]', source: '/u/a.png', mime: 'image/png', b64_file: '' };
    const marker = 'see <image name="[Image #1]" path="/u/a.png" mime="image/png" b64="">';
    const held: Entry = { ...you(20, marker), parts: [{ kind: 'text', text: 'see ' }, img] };
    const marks = { marks: [{ pos: 10, at_ms: 1, text: 'see', parts: [{ kind: 'text' as const, text: 'see ' }, img] }], upto: 15 };
    const dots = dotsOf(marks, [you(10, marker), held]);
    expect(dots.map((d) => d.text)).toEqual(['see', 'see ▣ [Image #1]']);
    expect(dots.map((d) => d.parts.length)).toEqual([2, 2]);
  });
  it('no marks yet: his held messages alone; none: empty', () => {
    expect(dotsOf(null, [you(3, 'a'), agent(4), undefined]).map((d) => d.pos)).toEqual([3]);
    expect(dotsOf(null, [])).toEqual([]);
  });
});

describe('currentDot', () => {
  const dots = dotsOf(null, [you(10, 'a'), you(20, 'b'), you(30, 'c')]);
  it('the last of his above the line he reads', () => {
    expect(currentDot(dots, 25)).toBe(20);
    expect(currentDot(dots, 30)).toBe(30);
    expect(currentDot(dots, 5)).toBeNull();
  });
  it('nothing read (no row yet): his newest', () => {
    expect(currentDot(dots, null)).toBe(30);
  });
});

describe('stepDot', () => {
  const dots = dotsOf(null, [you(10, 'a'), you(20, 'b'), you(30, 'c')]);
  it('moves one tick, stays at the ends', () => {
    expect(stepDot(dots, 20, -1)).toBe(10);
    expect(stepDot(dots, 20, 1)).toBe(30);
    expect(stepDot(dots, 10, -1)).toBe(10);
    expect(stepDot(dots, 30, 1)).toBe(30);
  });
});

describe('dotWhen', () => {
  const now = Date.UTC(2026, 10, 11, 16, 43);
  it('today a clock, yesterday says so, older its day', () => {
    expect(dotWhen(now - 60_000, now, 'UTC')).toBe('16:42');
    expect(dotWhen(now - 86_400_000, now, 'UTC')).toBe('yesterday 16:43');
    expect(dotWhen(now - 3 * 86_400_000, now, 'UTC')).toBe('8 Nov 16:43');
    expect(dotWhen(0, now, 'UTC')).toBe('');
  });
});

describe('shortWords', () => {
  it('cuts at a word as the hub does', () => {
    const s = shortWords('word '.repeat(100));
    expect(s.endsWith('word…')).toBe(true);
    expect(Array.from(s).length).toBeLessThanOrEqual(201);
  });
});

describe('dotsFit', () => {
  it('ticks that fit in the list, 8 px each, 16 above and under', () => {
    expect(dotsFit(192)).toBe(20);
    expect(dotsFit(100)).toBe(8);
    expect(dotsFit(20)).toBe(0);
  });
});
