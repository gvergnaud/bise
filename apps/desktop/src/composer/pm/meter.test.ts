import { describe, expect, it } from 'vitest';
import { flat, push, step, timeOf, wave } from './meter';
import { quietStopLine } from './dictation';

describe('the dictation meter (the TUI voice chip)', () => {
  it('a level is one of 8 steps, silence the lowest, full scale the highest', () => {
    expect([0, 0.1, 0.125, 0.5, 0.99, 1, 2].map(step)).toEqual([0, 0, 1, 4, 7, 7, 7]);
    expect([-1, Number.NaN].map(step)).toEqual([0, 0]);
  });

  it('silence: 6 bars at the lowest step, never fewer', () => {
    expect(flat()).toEqual([0, 0, 0, 0, 0, 0]);
  });

  it('each level comes in on the right, the oldest goes left', () => {
    let b = flat();
    b = push(b, 0.7);
    b = push(b, 0.4);
    expect(b).toEqual([0, 0, 0, 0, 5, 3]);
    for (const v of [0.1, 0.9, 0.3, 0.6, 0.2]) b = push(b, v);
    expect(b).toEqual([3, 0, 7, 2, 4, 1]);
  });

  it('transcribing: the wave ▂▃▅▆▅▃ rolls one bar each frame, back at its start after 6', () => {
    expect(wave(0)).toEqual([1, 2, 4, 5, 4, 2]);
    expect(wave(1)).toEqual([2, 4, 5, 4, 2, 1]);
    expect(wave(6)).toEqual(wave(0));
  });

  it('the time shows past the first minute, m:ss, then h:mm:ss past an hour (dictate-nocap)', () => {
    expect([0, 59_999, Number.NaN].map(timeOf)).toEqual(['', '', '']);
    expect(timeOf(60_000)).toBe('1:00');
    expect(timeOf(9 * 60_000 + 5_400)).toBe('9:05');
    expect(timeOf(3_599_999)).toBe('59:59');
    expect(timeOf(3_600_000)).toBe('1:00:00');
    expect(timeOf(2 * 3_600_000 + 61_000)).toBe('2:01:01');
  });

  it('a mic left on stopped by itself says so, with or without his words (designer m_64473)', () => {
    expect(quietStopLine('fix the login')).toBe('the mic stopped after 5 min of silence · your words are in');
    expect(quietStopLine(' ')).toBe('the mic stopped after 5 min of silence · nothing was heard');
  });
});
