import { describe, expect, it } from 'vitest';
import { BOTTOM, capsuleBounds, CORNER, fitBounds, MENU_GAP, OFF_DISPLAY, onScreen, pinnedBounds, pinOf, TOP, type DisplayBox } from './place';

// a 1512x982 MacBook display: the menu bar 33 px, the Dock 70 px
const mac: DisplayBox = {
  bounds: { x: 0, y: 0, width: 1512, height: 982 },
  workArea: { x: 0, y: 33, width: 1512, height: 879 },
};
const size = { width: 640, height: 120 };

describe('capsuleBounds', () => {
  it('top: centred, its top 170 px under the display top', () => {
    expect(capsuleBounds(size, mac, { at: 'top' })).toEqual({ x: 436, y: TOP, width: 640, height: 120 });
  });

  it('top: grows downward, the top never moves', () => {
    expect(capsuleBounds({ width: 640, height: 400 }, mac, { at: 'top' }).y).toBe(TOP);
  });

  it('bottom: its bottom 96 px above the Dock, grows upward', () => {
    const b = capsuleBounds(size, mac, { at: 'bottom' });
    expect(b.y + b.height).toBe(33 + 879 - BOTTOM);
    const tall = capsuleBounds({ width: 640, height: 300 }, mac, { at: 'bottom' });
    expect(tall.y + tall.height).toBe(33 + 879 - BOTTOM);
  });

  it('menu: the glass 6 px under the tray icon, centred on it', () => {
    const icon = { x: 1200, y: 4, width: 30, height: 24 };
    const b = capsuleBounds({ width: 360, height: 300 }, mac, { at: 'menu', icon });
    expect(b.y).toBe(33 + MENU_GAP);
    expect(b.x + 180).toBe(1215);
  });

  it('menu: an icon near the right edge keeps the box on the display', () => {
    const b = capsuleBounds({ width: 360, height: 300 }, mac, { at: 'menu', icon: { x: 1490, y: 4, width: 20, height: 24 } });
    expect(b.x + b.width).toBe(1512);
  });

  it('a glass inside a transparent margin is what gets placed', () => {
    const glass = { x: 20, y: 16, width: 600, height: 88 };
    const b = capsuleBounds({ width: 640, height: 120 }, mac, { at: 'top' }, glass);
    expect(b.y + glass.y).toBe(TOP);
    expect(b.x + glass.x + glass.width / 2).toBe(756);
  });

  it('never outside the work area, even bigger than it', () => {
    const b = capsuleBounds({ width: 3000, height: 3000 }, mac, { at: 'bottom' });
    expect(b).toEqual({ x: 0, y: 33, width: 1512, height: 879 });
  });

  it("a test run's display is far from every real one", () => {
    const b = capsuleBounds(size, OFF_DISPLAY, { at: 'top' });
    expect(b.x).toBeLessThan(-20000);
    expect(b.y).toBeLessThan(-20000);
  });
});

describe('a shown capsule never moves while he types (lead m_11455)', () => {
  it('the spotlight: 20 sizes as the list filters, its glass top-left stays where it was at show', () => {
    const glass0 = { x: 84, y: 28, width: 700, height: 60 };
    const b0 = capsuleBounds({ width: 868, height: 120 }, mac, { at: 'top' }, glass0);
    const pin = pinOf(b0, { width: 868, height: 120 }, { at: 'top' }, glass0);
    for (let i = 0; i < 20; i++) {
      const h = 60 + ((i * 37) % 400);
      const glass = { x: 84, y: 28, width: 700 + (i % 3) * 80, height: h };
      const b = pinnedBounds({ width: glass.width + 168, height: h + 60 }, pin, glass);
      expect([b.x + glass.x, b.y + glass.y]).toEqual([b0.x + glass0.x, b0.y + glass0.y]);
    }
  });

  it('a list taller than the space below grows past it rather than climbing (clamped once, at show)', () => {
    const glass = { x: 0, y: 0, width: 700, height: 2000 };
    const b0 = capsuleBounds({ width: 700, height: 60 }, mac, { at: 'top' });
    const b = pinnedBounds({ width: 700, height: 2000 }, pinOf(b0, { width: 700, height: 60 }, { at: 'top' }), glass);
    expect(b.y).toBe(b0.y);
    expect(b.x).toBe(b0.x);
  });

  it('what bise brings (the voice view, a card): its left and its bottom stay, it grows upward', () => {
    const g0 = { x: 84, y: 0, width: 700, height: 80 };
    const b0 = capsuleBounds({ width: 868, height: 80 }, mac, { at: 'bottom' }, g0);
    const pin = pinOf(b0, { width: 868, height: 80 }, { at: 'bottom' }, g0);
    const g1 = { x: 84, y: 0, width: 860, height: 300 };
    const b1 = pinnedBounds({ width: 1028, height: 300 }, pin, g1);
    expect(b1.x + g1.x).toBe(b0.x + g0.x);
    expect(b1.y + g1.y + g1.height).toBe(b0.y + g0.y + g0.height);
  });
});

describe("the spotlight's thread pane widens it right, its left part never moves (designer's condition 1)", () => {
  const wa = { x: 0, y: 25, width: 1470, height: 900 };
  const d13: DisplayBox = { bounds: { x: 0, y: 0, width: 1470, height: 956 }, workArea: wa };
  it('room on the right (a wide screen): 640 -> 1120 keeps its left', () => {
    const wide0: DisplayBox = { bounds: { x: 0, y: 0, width: 2560, height: 1440 }, workArea: { x: 0, y: 25, width: 2560, height: 1340 } };
    const b0 = capsuleBounds({ width: 640, height: 400 }, wide0, { at: 'top' });
    const pin = pinOf(b0, { width: 640, height: 400 }, { at: 'top' });
    const wide = onScreen(pinnedBounds({ width: 1120, height: 400 }, pin), pin, wide0.workArea);
    expect(wide.bounds.x).toBe(b0.x);
  });
  it('a 13-inch, centred: 1120 fits, shifted left by what passes the right edge (65 pt), never re-centred', () => {
    const b0 = capsuleBounds({ width: 640, height: 400 }, d13, { at: 'top' });
    const pin = pinOf(b0, { width: 640, height: 400 }, { at: 'top' });
    const wide = onScreen(pinnedBounds({ width: 1120, height: 400 }, pin), pin, wa);
    expect(wide.bounds.x + wide.bounds.width).toBe(wa.x + wa.width);
    expect(b0.x - wide.bounds.x).toBe(65);
  });
  it('the right edge would leave the screen: shifted left just enough, and stays shifted when it shrinks', () => {
    const b0 = { x: 700, y: 195, width: 640, height: 400 };
    const pin = pinOf(b0, { width: 640, height: 400 }, { at: 'top' });
    const wide = onScreen(pinnedBounds({ width: 1120, height: 400 }, pin), pin, wa);
    expect(wide.bounds.x + wide.bounds.width).toBe(wa.x + wa.width);
    expect(wide.bounds.x).toBe(350);
    const back = onScreen(pinnedBounds({ width: 640, height: 400 }, wide.pin), wide.pin, wa);
    expect(back.bounds.x).toBe(350);
  });
  it("the left part at the screen's right edge: the pane grows the window left by its whole width (480)", () => {
    const b0 = { x: wa.x + wa.width - 640, y: 195, width: 640, height: 400 };
    const pin = pinOf(b0, { width: 640, height: 400 }, { at: 'top' });
    const wide = onScreen(pinnedBounds({ width: 1120, height: 400 }, pin), pin, wa);
    expect(b0.x - wide.bounds.x).toBe(480);
    expect(wide.bounds.x + wide.bounds.width).toBe(b0.x + b0.width);
  });
  it('reopened with the pane (fn space within 10 min): its left part where it was, never the 1120 window centred (fix 4)', () => {
    // the page tells main the left part as its glass (Capsule.tsx): the show centres that
    for (const d of [d13, { bounds: { x: 0, y: 0, width: 2560, height: 1440 }, workArea: { x: 0, y: 25, width: 2560, height: 1340 } }]) {
      const b0 = capsuleBounds({ width: 640, height: 400 }, d, { at: 'top' });
      const pin = pinOf(b0, { width: 640, height: 400 }, { at: 'top' });
      const wide = onScreen(pinnedBounds({ width: 1120, height: 400 }, pin), pin, d.workArea);
      const left = { x: 0, y: 0, width: 640, height: 400 };
      const again = capsuleBounds({ width: 1120, height: 400 }, d, { at: 'top' }, left);
      expect(again.x).toBe(wide.bounds.x);
      // the whole window as the glass: centred, the left moved (the red: 240 pt)
      expect(capsuleBounds({ width: 1120, height: 400 }, d, { at: 'top' }).x).not.toBe(wide.bounds.x);
    }
  });
  it('two screens: opened on the second one, the pane keeps the window on the screen of its left part', () => {
    const d2: DisplayBox = { bounds: { x: 1470, y: 0, width: 1920, height: 1080 }, workArea: { x: 1470, y: 25, width: 1920, height: 1000 } };
    const b0 = capsuleBounds({ width: 640, height: 400 }, d2, { at: 'top' });
    const pin = pinOf(b0, { width: 640, height: 400 }, { at: 'top' });
    const wide = onScreen(pinnedBounds({ width: 1120, height: 400 }, pin), pin, d2.workArea);
    expect(wide.bounds.x).toBe(b0.x);
    expect(wide.bounds.x).toBeGreaterThanOrEqual(d2.workArea.x);
    expect(wide.bounds.x + wide.bounds.width).toBeLessThanOrEqual(d2.workArea.x + d2.workArea.width);
  });
  it('a second screen to the right: kept inside its own work area; the corner (right-pinned) never shifts', () => {
    const wa2 = { x: 1470, y: 0, width: 1920, height: 1080 };
    const pin = pinOf({ x: 3000, y: 170, width: 640, height: 300 }, { width: 640, height: 300 }, { at: 'top' });
    const wide = onScreen(pinnedBounds({ width: 1120, height: 300 }, pin), pin, wa2);
    expect(wide.bounds.x + wide.bounds.width).toBe(wa2.x + wa2.width);
    const corner = pinOf({ x: 3000, y: 700, width: 320, height: 100 }, { width: 320, height: 100 }, { at: 'corner' });
    const c = onScreen(pinnedBounds({ width: 900, height: 100 }, corner), corner, wa2);
    expect(c.pin).toBe(corner);
  });
});

describe('the voice view: placed once for its max height, then its top-left stays (lead m_11532)', () => {
  it('partials and levels grow it down to its max, never past the Dock line, top-left identical', () => {
    const max = 420;
    const g0 = { x: 84, y: 0, width: 700, height: 80 };
    const b0 = fitBounds({ width: 868, height: 80 }, mac, g0, max);
    // its max glass would end BOTTOM above the work area's bottom
    expect(b0.y + g0.y + max).toBe(mac.workArea.y + mac.workArea.height - BOTTOM);
    const pin = pinOf(b0, { width: 868, height: 80 }, { at: 'top' }, g0);
    for (let i = 0; i < 20; i++) {
      const g = { x: 84, y: 0, width: 700, height: 80 + i * 17 };
      const b = pinnedBounds({ width: 868, height: g.height }, pin, g);
      expect([b.x + g.x, b.y + g.y]).toEqual([b0.x + g0.x, b0.y + g0.y]);
      expect(b.y + g.y + g.height).toBeLessThanOrEqual(mac.workArea.y + mac.workArea.height - BOTTOM);
    }
  });

  it('a max taller than the work area still starts inside it', () => {
    const b = fitBounds({ width: 700, height: 80 }, mac, undefined, 5000);
    expect(b.y).toBe(mac.workArea.y);
  });
});

describe('corner: the fn talk, his pick C (designer m_22119)', () => {
  const card = { width: 320, height: 96 };
  const glassBR = (b: { x: number; y: number }, g: { x: number; y: number; width: number; height: number }) => [b.x + g.x + g.width, b.y + g.y + g.height];

  it('Dock at the bottom: its glass 16 px from the right edge and 16 px above the Dock', () => {
    const b = capsuleBounds(card, mac, { at: 'corner' });
    expect([b.x + b.width, b.y + b.height]).toEqual([1512 - CORNER, 33 + 879 - CORNER]);
    expect(b.width).toBe(320);
  });

  it('Dock on the right: 16 px left of it, 16 px from the bottom', () => {
    const right: DisplayBox = { bounds: mac.bounds, workArea: { x: 0, y: 33, width: 1440, height: 949 } };
    const b = capsuleBounds(card, right, { at: 'corner' });
    expect([b.x + b.width, b.y + b.height]).toEqual([1440 - CORNER, 982 - CORNER]);
  });

  it('a hidden Dock (a 4 px strip): 16 px from the display bottom', () => {
    const hidden: DisplayBox = { bounds: mac.bounds, workArea: { x: 0, y: 33, width: 1512, height: 945 } };
    const b = capsuleBounds(card, hidden, { at: 'corner' });
    expect(b.y + b.height).toBe(982 - CORNER);
  });

  it("on a second screen: that screen's corner", () => {
    const ext: DisplayBox = { bounds: { x: 1512, y: -200, width: 2560, height: 1440 }, workArea: { x: 1512, y: -175, width: 2560, height: 1415 } };
    const b = capsuleBounds(card, ext, { at: 'corner' });
    expect([b.x + b.width, b.y + b.height]).toEqual([1512 + 2560 - CORNER, -200 + 1440 - CORNER]);
  });

  it('a glass inside a transparent margin is what sits in the corner', () => {
    const g = { x: 24, y: 24, width: 320, height: 96 };
    const b = capsuleBounds({ width: 368, height: 144 }, mac, { at: 'corner' }, g);
    expect(glassBR(b, g)).toEqual([1512 - CORNER, 33 + 879 - CORNER]);
  });

  it('the answer grows upward to half the screen: its glass keeps its right edge and its bottom', () => {
    const g0 = { x: 24, y: 24, width: 320, height: 96 };
    const b0 = capsuleBounds({ width: 368, height: 144 }, mac, { at: 'corner' }, g0);
    const pin = pinOf(b0, { width: 368, height: 144 }, { at: 'corner' }, g0);
    for (let i = 0; i < 20; i++) {
      const g = { x: 24, y: 24, width: 320, height: 96 + i * 20 };
      const b = pinnedBounds({ width: 368, height: g.height + 48 }, pin, g);
      expect(glassBR(b, g)).toEqual(glassBR(b0, g0));
      expect(b.y + g.y).toBeGreaterThanOrEqual(mac.workArea.y);
    }
  });
});
