// The title bar's give-way order (designer m_45616, m_45674): the minis never shrink; the crumb's
// fact goes first, then the pill's short form, never a word cut mid-way.
import { describe, expect, it } from 'vitest';
import { barFit, CUT_MIN_PX, cutCrumb, giveWay, rowFit, STATUS_NARROW_PX, type RowKey, type RowPart, type RowRoom } from './giveway';

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

describe("a thread's title row gives way in one order (designer m_67200, his m_67197)", () => {
  // his shot's header (vb-architect, at 1024): each part's width with its gap before it
  const cut = (whole: number, text: number): RowPart => ({ whole, least: whole - Math.max(0, text - CUT_MIN_PX), last: 0 });
  const his: Omit<RowRoom, 'room'> = {
    name: { whole: 80, least: CUT_MIN_PX, last: CUT_MIN_PX },
    pr: { whole: 64, least: 64, last: 0 },
    branch: cut(102, 80),
    page: cut(160, 130),
    chip: { whole: 128, least: 128 - (100 - CUT_MIN_PX), last: 32 },
    sub: cut(460, 452),
    fact: { whole: 100, least: 100, last: 0 },
  };
  // the order designer agreed (m_67200), first to give way first: written here, never read from the module
  const AGREED: readonly RowKey[] = ['sub', 'page', 'branch', 'pr', 'chip', 'fact', 'name'];
  const none: RowPart = { whole: 0, least: 0, last: 0 };
  const total = AGREED.reduce((n, k) => n + his[k].whole, 0);
  const shown = (f: ReturnType<typeof rowFit>) => AGREED.reduce((n, k) => n + f[k].width, 0);
  // the title's room in a window that wide, the sidebar shown: the lead (56 + 240), the title's 24,
  // the right end's three 28 buttons 4 apart and the bar's 12
  const roomAt = (w: number) => w - (56 + 240) - 24 - (3 * 28 + 2 * 4) - 12;

  it('room for all: every part whole', () => {
    const f = rowFit({ ...his, room: total });
    for (const k of AGREED) expect(f[k]).toEqual({ form: 'whole', width: his[k].whole });
  });

  it('at 1440, 1024 and 680 (the window\'s least), the parts laid in a row never pass the room: no two boxes meet', () => {
    for (const w of [1440, 1024, 680]) {
      const room = roomAt(w);
      const f = rowFit({ ...his, room });
      // the row's order on screen: name, PR, branch, page, task, subtitle, state
      let x = 0;
      const boxes = (['name', 'pr', 'branch', 'page', 'chip', 'sub', 'fact'] as const).map((k) => {
        const b = { k, l: x, r: x + f[k].width };
        x = b.r;
        return b;
      });
      for (const a of boxes) for (const b of boxes) if (a !== b && a.r > a.l && b.r > b.l) expect(a.r <= b.l || b.r <= a.l).toBe(true);
      expect(x).toBeLessThanOrEqual(room);
      expect(f.name.width).toBeGreaterThanOrEqual(CUT_MIN_PX);
    }
  });

  it('his 1024: the subtitle gives way, the task chip whole', () => {
    const f = rowFit({ ...his, room: roomAt(1024) });
    expect(f.chip.form).toBe('whole');
    expect(f.sub.form).not.toBe('whole');
  });

  it('gives way in order: a part gives only once every part before it gave all it can; one cut at a time', () => {
    for (let room = 0; room <= total + 10; room += 3) {
      const f = rowFit({ ...his, room });
      const first = AGREED.findIndex((k) => f[k].form !== 'last');
      if (first >= 0) for (const k of AGREED.slice(first + 1)) expect(f[k].form, `${k} at ${String(room)}`).toBe('whole');
      for (const k of AGREED) if (f[k].form === 'cut') expect(f[k].width).toBeGreaterThanOrEqual(his[k].least);
      // the row fits whenever its least forms do (the name at its least, the chip's mark)
      if (room >= his.name.least + his.chip.last) expect(shown(f)).toBeLessThanOrEqual(room);
    }
  });

  it('the order: subtitle, page, branch, PR, task chip (to its mark, never out), state, name', () => {
    const at = (room: number) => rowFit({ ...his, room });
    const lasts = (ks: readonly RowKey[]) => ks.reduce((n, k) => n + his[k].last, 0);
    const wholes = (ks: readonly RowKey[]) => ks.reduce((n, k) => n + his[k].whole, 0);
    for (let i = 0; i < AGREED.length; i += 1) {
      const k = AGREED[i] as RowKey;
      // one px short of k at its least, every part before it gone to its last form
      const room = lasts(AGREED.slice(0, i)) + his[k].least + wholes(AGREED.slice(i + 1)) - 1;
      const f = at(room);
      expect(f[k].form, k).toBe('last');
      for (const later of AGREED.slice(i + 1)) expect(f[later].form, `${later} after ${k}`).not.toBe('last');
    }
    expect(at(0).chip).toEqual({ form: 'last', width: 32 });
    expect(at(0).name).toEqual({ form: 'last', width: CUT_MIN_PX });
  });

  it('a part not there takes nothing and gives nothing (main\'s header: no links, no chip)', () => {
    const f = rowFit({ ...his, pr: none, branch: none, page: none, chip: none, room: 200 });
    expect(f.chip).toEqual({ form: 'whole', width: 0 });
    expect(shown(f)).toBeLessThanOrEqual(200);
  });
});
