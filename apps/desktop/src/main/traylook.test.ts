import { describe, expect, it } from 'vitest';
import { armAngles, armCount, markDistance, rasterMark } from '../shared/mark';
import { KISSES, newWait, stepTurn, toBgra, trayFrame, trayLook, trayTip, TRAY_PT } from './traylook';

describe("the ':*' geometry", () => {
  it('the star is the team: one branch per running agent, none is the plain three, capped at 6', () => {
    expect(armCount(0)).toBe(3);
    expect(armCount(1)).toBe(1);
    expect(armCount(2)).toBe(2);
    expect(armCount(4)).toBe(4);
    expect(armCount(11)).toBe(6);
    expect(armAngles(1)).toEqual([0]);
    expect(armAngles(2)).toEqual([0, 90]);
    expect(armAngles(3)).toEqual([0, 60, 120]);
  });

  it('the eyes and the star where the SVG draws them', () => {
    expect(markDistance(28.5, 36, 7).eyes).toBeLessThan(0);
    expect(markDistance(28.5, 66, 7).eyes).toBeLessThan(0);
    expect(markDistance(64, 40, 7).star).toBeLessThan(0); // the upright branch
    expect(markDistance(90, 90, 7).star).toBeGreaterThan(0);
    // a beat grows the star about its centre
    expect(markDistance(64, 33, 7, { beat: 1.25 }).star).toBeLessThan(0);
    expect(markDistance(64, 33, 7).star).toBeGreaterThan(0);
  });

  it('rasterizes in the colours given, transparent elsewhere', () => {
    const px = rasterMark(30, [10, 20, 30, 255], [200, 100, 50, 255], { size: 15 });
    const at = (x: number, y: number): number[] => Array.from(px.slice((y * 30 + x) * 4, (y * 30 + x) * 4 + 4));
    expect(at(8, 10)).toEqual([10, 20, 30, 255]); // an eye (28.5, 36 → 8.5, 10.8)
    expect(at(19, 16)).toEqual([200, 100, 50, 255]); // the star's centre (64, 56 → 19.2, 16.8)
    expect(at(1, 1)[3]).toBe(0);
  });
});

describe('the tray look', () => {
  it('rest, work, wait over everything (the plain star: no team at 15 px)', () => {
    expect(trayLook({ waiting: 0, working: 0 })).toBe('rest');
    expect(trayLook({ waiting: 0, working: 2 })).toBe('work');
    expect(trayLook({ waiting: 1, working: 4 })).toBe('wait');
  });

  it('the two kisses only for a new wait', () => {
    expect(newWait({ waiting: 0, working: 0 }, { waiting: 1, working: 0 })).toBe(true);
    expect(newWait({ waiting: 1, working: 0 }, { waiting: 2, working: 0 })).toBe(true);
    expect(newWait({ waiting: 2, working: 0 }, { waiting: 1, working: 3 })).toBe(false);
    // two beats up to 1.25, back to 1 at the end
    expect(KISSES.filter((b) => b === 1.25)).toHaveLength(2);
    expect(KISSES.at(-1)).toBe(1);
  });

  it('a working step turns a third of the star symmetry', () => {
    expect([0, 1, 2, 3].map(stepTurn)).toEqual([0, 20, 40, 0]);
  });

  it('its tip says the state (law 6)', () => {
    expect(trayTip({ waiting: 0, working: 0 })).toBe('bise: all quiet');
    expect(trayTip({ waiting: 0, working: 3 })).toBe('bise: working with 3 agents');
    expect(trayTip({ waiting: 1, working: 3 })).toBe('bise: 1 thing waits for you');
    expect(trayTip({ waiting: 2, working: 0 })).toBe('bise: 2 things wait for you');
  });

  it('frames: black (template) at rest, all one pink on a wait whatever the bar, 1x and 2x', () => {
    const rest = trayFrame('rest', 2);
    expect(rest.px).toBe(TRAY_PT * 2);
    const opaque = (f: Uint8Array): number[][] => {
      const out: number[][] = [];
      for (let o = 0; o < f.length; o += 4) if (f[o + 3] === 255) out.push(Array.from(f.slice(o, o + 3)));
      return out;
    };
    expect(opaque(rest.bgra).every(([b, g, r]) => b === 0 && g === 0 && r === 0)).toBe(true);
    // BGRA: #d03a72, eyes and star alike (a dark bar or a light one: the image can't know)
    const wait = opaque(trayFrame('wait', 2).bgra);
    expect(wait.length).toBeGreaterThan(0);
    expect(wait.every(([b, g, r]) => b === 114 && g === 58 && r === 208)).toBe(true);
    expect(trayFrame('wait', 1).px).toBe(TRAY_PT);
  });

  it('BGRA premultiplied', () => {
    expect(Array.from(toBgra(new Uint8Array([200, 100, 50, 128])))).toEqual([25, 50, 100, 128]);
  });
});
