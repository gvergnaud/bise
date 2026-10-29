import { describe, expect, it } from 'vitest';
import { activities } from './activity';
import { labelsOf } from './labels';
import { layoutOf } from './maps';
import { newTimeline } from './timeline';
import { agent, mainAgent, office } from './fixtures';
import { growOf, STAGE_AT, UNLOCK_LABEL, UNLOCK_WALK } from './growth';
import { localOf } from './eggs';
import { makeDriver, NO_ATTENTION, officeKey, type Clock } from './office-driver';
import { crewOf, objsOf } from './corners.test';

const parts = { activities, labelsOf, layoutOf, newTimeline };
const T = 1_791_100_000;
const clock = (t: number): Clock => ({ t, local: localOf(new Date(t * 1000)), night: false, reduced: true, forced: null });
const crew = [mainAgent(), agent('ada', { status: 'working' }), agent('bob')];
const snap = { agents: crew, cards: [], places: [] };

describe('office-driver: one office brain', () => {
  it('draws nothing before any rows', () => {
    expect(makeDriver(parts).step(clock(T), NO_ATTENTION)).toBeNull();
  });

  it('lays the office once per shape, every bim posed', () => {
    const d = makeDriver(parts);
    d.rows(snap, crew, 'loft');
    const a = d.step(clock(T), NO_ATTENTION);
    expect(a?.reshaped).toBe(true);
    expect(a?.office.map).toBe('loft');
    expect(new Set(a?.frames.map((f) => f.name))).toEqual(new Set(crew.map((x) => x.name)));
    expect(d.step(clock(T + 1), NO_ATTENTION)?.reshaped).toBe(false);
    // the same rows again: the same shape, no new plan
    d.rows({ ...snap }, crew, 'loft');
    expect(d.step(clock(T + 2), NO_ATTENTION)?.reshaped).toBe(false);
    // another map: a new plan, the map he picked said
    d.rows(snap, crew, 'kyoto');
    const k = d.step(clock(T + 3), NO_ATTENTION);
    expect(k?.reshaped).toBe(true);
    expect(k?.want).toBe('kyoto');
  });

  it('poses the whole crew when the first rows were empty, and a newcomer after it (bim-review m_49981: 9 of 16 in the side tile)', () => {
    const d = makeDriver(parts);
    const moving = (t: number): Clock => ({ ...clock(t), reduced: false });
    const empty = { agents: [], cards: [], places: [] };
    // the app's first rows: none yet (the hub not heard), then the crew
    d.rows(empty, [], 'loft');
    d.step(moving(T), NO_ATTENTION);
    d.rows(office, office.agents, 'loft');
    const names = (t: number): string[] => d.step(moving(t), NO_ATTENTION)?.frames.map((f) => f.name) ?? [];
    expect(new Set(names(T + 0.2))).toEqual(new Set(office.agents.map((a) => a.name)));
    // a newcomer: in by the door, posed on the next step
    const grown = { ...office, agents: [...office.agents, agent('newcomer', { status: 'working' })] };
    d.rows(grown, grown.agents, 'loft');
    expect(names(T + 20)).toContain('newcomer');
    expect(names(T + 20)).toHaveLength(grown.agents.length);
  });

  it("his sketch's extensions: they come with the objects that need them, a kept mark brings them back, and they never go", () => {
    const d = makeDriver(parts);
    d.rows(snap, crew, 'loft');
    expect(d.step(clock(T), NO_ATTENTION)?.grown).toEqual({ map: 'loft', need: { wing: 0, garden: false } });
    // a kept mark (another session's): the wing comes back with no object needing it
    d.seedNeed('loft', { wing: 4, garden: true });
    const a = d.step(clock(T + 1), NO_ATTENTION);
    expect(a?.reshaped).toBe(true);
    expect(a?.office.wing?.z1).toBe(4);
    expect(a?.office.garden).toBeDefined();
    expect(a?.grown?.need).toEqual({ wing: 4, garden: true });
    // a smaller mark never takes it away
    d.seedNeed('loft', { wing: 0, garden: false });
    expect(d.step(clock(T + 2), NO_ATTENTION)?.office.wing?.z1).toBe(4);
    // a map that does not grow: none
    d.rows(snap, crew, 'loft');
    d.seedNeed('kyoto', { wing: 0, garden: false });
    expect(d.step(clock(T + 3), NO_ATTENTION)?.grown).toBeNull();
  });

  it('a stage past the last drawn one is the unlock moment, gone after its walk and label', () => {
    const d = makeDriver(parts);
    d.rows(snap, crew, 'loft');
    d.grow(growOf({ jobs_done: STAGE_AT[2], days_used: 1, artifacts_made: 0 }), T);
    expect(d.step(clock(T), NO_ATTENTION)?.look.unlock).toBeUndefined();
    d.grow(growOf({ jobs_done: STAGE_AT[3], days_used: 1, artifacts_made: 0 }), T + 1);
    const u = d.step(clock(T + 1.5), NO_ATTENTION)?.look.unlock;
    expect(u?.at).toBe(T + 1);
    d.step(clock(T + 2 + UNLOCK_WALK + UNLOCK_LABEL), NO_ATTENTION);
    expect(d.step(clock(T + 3 + UNLOCK_WALK + UNLOCK_LABEL), NO_ATTENTION)?.look.unlock).toBeUndefined();
  });

  it("the row under his pointer gets the row's ring, the followed bim its own; the row wins on the followed one", () => {
    const d = makeDriver(parts);
    d.rows(snap, crew, 'loft');
    const litOf = (s: ReturnType<typeof d.step>) => Object.fromEntries((s?.played ?? []).filter((f) => f.lit !== undefined).map((f) => [f.name, f.lit]));
    expect(litOf(d.step(clock(T), { ...NO_ATTENTION, lit: 'ada', follow: 'bob' }))).toEqual({ ada: 'row', bob: 'follow' });
    expect(litOf(d.step(clock(T + 1), { ...NO_ATTENTION, lit: 'bob', follow: 'bob' }))).toEqual({ bob: 'row' });
  });

  it('stale: only when an activity changes', () => {
    const d = makeDriver(parts);
    d.rows(snap, crew, 'loft');
    d.step(clock(T), NO_ATTENTION);
    expect(d.stale(clock(T + 1))).toBe(false);
    const busy = crew.map((a) => (a.name === 'bob' ? { ...a, status: 'working' as const } : a));
    d.rows({ ...snap, agents: busy }, busy, 'loft');
    expect(d.stale(clock(T + 1))).toBe(true);
  });

  it('a change only the clock brings (a trip ends) is heard at the next step: then stale no more', () => {
    // the row-clock's case (budget (6)): the rows are the same, the activities moved with time
    let later = false;
    const acts: typeof activities = (sn, t) => {
      const m = new Map(activities(sn, t));
      const bob = m.get('bob');
      if (later && bob) m.set('bob', { ...bob, k: bob.k === 'sleep' ? 'nap' : 'sleep' });
      return m;
    };
    const d = makeDriver({ ...parts, activities: acts });
    d.rows(snap, crew, 'loft');
    d.step(clock(T), NO_ATTENTION);
    later = true;
    expect(d.stale(clock(T + 5))).toBe(true);
    d.step(clock(T + 5), NO_ATTENTION);
    expect(d.stale(clock(T + 10)), 'heard: the check is quiet again').toBe(false);
  });

  it('objects heard one by one: every object a fresh office places is drawn (his m_49375: the wing stood empty)', () => {
    for (const n of [8, 14]) {
      const all = objsOf(n);
      const four = crewOf(4);
      const rows = { agents: four, cards: [], places: [] };
      const fresh = makeDriver(parts);
      fresh.rows(rows, four, 'climb');
      fresh.objects(all);
      const f = fresh.step(clock(T), NO_ATTENTION);
      const d = makeDriver(parts);
      d.rows(rows, four, 'climb');
      d.step(clock(T), NO_ATTENTION);
      for (let k = 1; k <= n; k++) {
        d.objects(all.slice(0, k));
        d.step(clock(T + k), NO_ATTENTION);
      }
      const s = d.step(clock(T + n + 1), NO_ATTENTION);
      expect(s?.corners.placed.length, `climb, ${String(n)} objects`).toBeGreaterThanOrEqual(f?.corners.placed.length ?? 0);
    }
  });

  it('officeKey: the names and main, in order, with the map', () => {
    expect(officeKey(crew, 'loft')).toBe('loft|main*,ada,bob');
  });
});
