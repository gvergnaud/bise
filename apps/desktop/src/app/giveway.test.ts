// The title bar's give-way order (designer m_45616, m_45674): the minis never shrink; the crumb's
// fact goes first, then the pill's short form, never a word cut mid-way.
import { describe, expect, it } from 'vitest';
import { barFit, cutCrumb, giveWay, STATUS_NARROW_PX, SUB_MIN_PX, titleFit } from './giveway';

describe('the title bar gives way', () => {
  it('room for all: nothing gives way', () => {
    expect(giveWay({ slot: 700, fact: 110, pill: 330, minis: 230 })).toEqual({ fact: false, narrow: false });
  });
  it('a little short: the fact goes first, the pill keeps its long words whole', () => {
    expect(giveWay({ slot: 500, fact: 110, pill: 330, minis: 230 })).toEqual({ fact: true, narrow: false });
  });
  it('shorter still: the fact gone and the pill in its short form (1000 with the pill, ui-review m_45670)', () => {
    expect(giveWay({ slot: 340, fact: 110, pill: 330, minis: 0 })).toEqual({ fact: false, narrow: false });
    expect(giveWay({ slot: 300, fact: 110, pill: 330, minis: 230 })).toEqual({ fact: true, narrow: true });
  });
  it('the long words never cut: whenever they don\'t fit whole, the short form', () => {
    for (let slot = 0; slot < 800; slot += 7) {
      const g = giveWay({ slot, fact: 0, pill: 330, minis: 0 });
      if (!g.narrow) expect(330).toBeLessThanOrEqual(slot);
    }
  });
  it('no pill: nothing gives way for the minis (the crumb\'s name cuts last, by the bar\'s flex)', () => {
    expect(giveWay({ slot: 100, fact: 110, pill: 0, minis: 230 })).toEqual({ fact: false, narrow: false });
  });
  it('under 160 px of room for the pill: its short form, as before', () => {
    expect(giveWay({ slot: STATUS_NARROW_PX + 10, fact: 0, pill: 100, minis: 20 }).narrow).toBe(true);
  });
});

describe('the crumb beside the minis (designer m_45727)', () => {
  const room = { name: 60, role: 180, minis: 200, pillShort: 90, pillSign: 24 };
  it('room for all: the role whole and the pill\'s short form', () => {
    expect(cutCrumb({ ...room, avail: 600 })).toEqual({ role: true, pill: 90 });
  });
  it('short: the role goes whole before the pill gives way; the name stays', () => {
    expect(cutCrumb({ ...room, avail: 400 })).toEqual({ role: false, pill: 90 });
  });
  it('shorter: the role out and the pill its sign alone', () => {
    expect(cutCrumb({ ...room, avail: 300 })).toEqual({ role: false, pill: 24 });
  });
  it('never a cut role: at every room, the role shows only when it fits whole', () => {
    for (let avail = 0; avail < 700; avail += 5) {
      const c = cutCrumb({ ...room, avail });
      if (c.role) expect(room.name + room.role + room.minis + c.pill).toBeLessThanOrEqual(avail);
    }
  });
  it('no minis in the bar: nothing changes for the crumb', () => {
    expect(cutCrumb({ ...room, minis: 0, avail: 10 })).toEqual({ role: true, pill: 90 });
  });
});

describe('the minis after the crumb (designer m_45869)', () => {
  const room = { name: 60, role: 180, fact: 110, minis: 200, pillFull: 330, pillShort: 90, pillSign: 24 };
  it('all of it when it holds', () => {
    expect(barFit({ ...room, avail: 900 })).toEqual({ fact: true, role: true, narrow: false, pill: 330 });
  });
  it('the fact goes first, then the pill takes its short form, then the role drops whole, then the pill is its sign', () => {
    expect(barFit({ ...room, avail: 800 })).toMatchObject({ fact: false, role: true, narrow: false });
    expect(barFit({ ...room, avail: 600 })).toMatchObject({ fact: false, role: true, narrow: true, pill: 90 });
    expect(barFit({ ...room, avail: 400 })).toMatchObject({ fact: false, role: false, narrow: true, pill: 90 });
    expect(barFit({ ...room, avail: 300 })).toMatchObject({ fact: false, role: false, pill: 24 });
  });
  it('no pill: the fact still goes before the role, never narrow', () => {
    const none = { ...room, pillFull: 0, pillShort: 0, pillSign: 0 };
    expect(barFit({ ...none, avail: 500 })).toEqual({ fact: false, role: true, narrow: false, pill: 0 });
    expect(barFit({ ...none, avail: 300 })).toEqual({ fact: false, role: false, narrow: false, pill: 0 });
  });
});

describe('a thread\'s title with its links (designer m_57304)', () => {
  // the name and fact 200, the links PR 70, branch 90, page 150, the subtitle 220
  const r = (room: number, sub = 220) => titleFit({ room, fixed: 200, links: [70, 90, 150], sub });

  it('room for all: all whole', () => {
    expect(r(730)).toEqual({ sub: 'whole', links: 3 });
  });

  it('the subtitle gives way first: cut, never under its least, then out whole; the links stay', () => {
    expect(r(729)).toEqual({ sub: 'cut', links: 3 });
    expect(r(510 + SUB_MIN_PX)).toEqual({ sub: 'cut', links: 3 });
    expect(r(510 + SUB_MIN_PX - 1)).toEqual({ sub: 'out', links: 3 });
  });

  it('then the links, whole, last first: the page, the branch, the PR last; the name and fact stay', () => {
    expect(r(509)).toEqual({ sub: 'out', links: 2 });
    expect(r(359)).toEqual({ sub: 'out', links: 1 });
    expect(r(269)).toEqual({ sub: 'out', links: 0 });
    expect(r(100)).toEqual({ sub: 'out', links: 0 });
  });

  it('a subtitle no wider than its least is never cut: whole or out', () => {
    expect(r(200 + 310 + 30, 30)).toEqual({ sub: 'whole', links: 3 });
    expect(r(200 + 310 + 29, 30)).toEqual({ sub: 'out', links: 3 });
  });
});
