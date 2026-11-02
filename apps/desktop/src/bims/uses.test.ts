// A working bim walks to the object of the plugin its turn uses (uses.ts): the hub's names matched
// against his plugins rows, the use spot by every map's objects, the trips kept calm.
import { describe, expect, it } from 'vitest';
import type { Agent, Plugin, Uses } from '@/proto';
import { activities } from './activity';
import { labelsOf } from './labels';
import { localOf } from './eggs';
import { makeDriver, NO_ATTENTION, type Step } from './office-driver';
import { newTimeline } from './timeline';
import { agent, mainAgent } from './fixtures';
import { officeObjects } from './plugin-objects';
import { layoutOf } from './maps';
import { MAP_IDS } from './maps/ids';
import { objectSpots } from './corners';
import { crewOf, objsOf } from './corners.test';
import { FLOOR_S, HOLD_S, NO_TRIPS, nextTrips, pluginOfUse, standSpotOf } from './uses';
import type { Box } from './model';

const row = (id: string, o: Partial<Plugin> = {}): Plugin => ({ id, name: id, kind: 'plugin', scope: 'user', title: id, state: 'ready', on: true, ...o }) as Plugin;
const part = (name: string, kind: 'skill' | 'mcp') => ({ id: `${name}#${kind}`, kind, name, on: true }) as NonNullable<Plugin['parts']>[number];

describe('which plugin a call uses', () => {
  const rows = [
    row('mistral:gmail', { name: 'gmail', kind: 'mistral', scope: 'mistral' }),
    row('computer', { parts: [part('computer-use', 'skill'), part('computer', 'mcp')] }),
    row('blender', { parts: [part('blender', 'mcp')] }),
    row('skills', { kind: 'skills', parts: [part('gabriel-s-tone', 'skill')] }),
    row('mistral:linear', { name: 'linear', kind: 'mistral', on: false }),
  ];

  it('matches each kind exactly, against the on rows only', () => {
    expect(pluginOfUse({ kind: 'group', name: 'gmail' }, rows)).toBe('mistral:gmail');
    expect(pluginOfUse({ kind: 'group', name: 'blender' }, rows)).toBe('blender');
    expect(pluginOfUse({ kind: 'skill', plugin: 'computer', name: 'computer-use' }, rows)).toBe('computer');
    expect(pluginOfUse({ kind: 'skill', name: 'gabriel-s-tone' }, rows)).toBe('skills');
    expect(pluginOfUse({ kind: 'server', name: 'blender' }, rows)).toBe('blender');
    // off, unknown, none of his
    expect(pluginOfUse({ kind: 'group', name: 'linear' }, rows)).toBeNull();
    expect(pluginOfUse({ kind: 'group', name: 'slack' }, rows)).toBeNull();
    expect(pluginOfUse({ kind: 'unknown', name: 'gmail' }, rows)).toBeNull();
    expect(pluginOfUse({ kind: 'skill', name: 'computer' }, rows)).toBeNull();
    expect(pluginOfUse({ kind: 'group', name: 'gmail' }, null)).toBeNull();
  });
});

describe('the trips', () => {
  const yes = (): boolean => true;
  const step = (prev = NO_TRIPS, using: [string, string | null][] = [], t = 0, can: (bim: string, key: string) => boolean = yes) => nextTrips(prev, new Map(using), t, can);

  it('a burst of calls to one plugin is one trip; it stays HOLD_S after the last one, then goes back', () => {
    let s = step(NO_TRIPS, [['a', 'gmail']], 0);
    expect(s.now.get('a')).toEqual({ key: 'gmail', began: 0, seen: 0, on: true });
    s = step(s, [['a', null]], 1);
    s = step(s, [['a', 'gmail']], 2);
    expect(s.now.get('a')).toEqual({ key: 'gmail', began: 0, seen: 2, on: true });
    // its call ran until the look that finds it gone (t 3): it stays HOLD_S from then
    s = step(s, [['a', null]], 3);
    expect(s.now.get('a')).toEqual({ key: 'gmail', began: 0, seen: 3, on: false });
    s = step(s, [['a', null]], 3 + HOLD_S - 0.5);
    expect(s.now.get('a')?.key).toBe('gmail');
    s = step(s, [['a', null]], 3 + HOLD_S);
    expect(s.now.has('a')).toBe(false);
  });

  it('the same object at most once per FLOOR_S; another plugin is a new trip at once', () => {
    let s = step(NO_TRIPS, [['a', 'gmail']], 0);
    s = step(s, [['a', 'blender']], 3);
    expect(s.now.get('a')).toEqual({ key: 'blender', began: 3, seen: 3, on: true });
    s = step(s, [['a', 'gmail']], 5);
    // too soon back to gmail: it stays at blender (held), the call shows on its object
    expect(s.now.get('a')?.key).toBe('blender');
    s = step(s, [['a', null]], 20);
    expect(s.now.has('a')).toBe(false);
    s = step(s, [['a', 'gmail']], FLOOR_S + 0.5);
    expect(s.now.get('a')?.key).toBe('gmail');
  });

  it('a state wins: a bim not working any more ends its trip at once; one it cannot go to stays put', () => {
    let s = step(NO_TRIPS, [['a', 'gmail'], ['b', 'gmail']], 0, (bim) => bim === 'a');
    expect([...s.now.keys()]).toEqual(['a']);
    s = step(s, [], 1);
    expect(s.now.size).toBe(0);
  });
});

describe('where a bim stands to use an object', () => {
  // its head (render/laptop.ts HEAD_R) and a little air clear of every box, which may stand at head height (a mailbox, a canvas)
  const inside = (p: { x: number; z: number }, b: Box, m = 0.245 + 0.15): boolean => p.x > b.x0 - m && p.x < b.x1 + m && p.z > b.z0 - m && p.z < b.z1 + m;

  it('on every map: clear of every box, near the network, facing the object; most objects have one', () => {
    const out: string[] = [];
    let placed = 0;
    let spotted = 0;
    for (const map of MAP_IDS) {
      for (const n of [3, 20]) {
        const o = layoutOf(crewOf(n), map);
        const c = objectSpots(o, objsOf(20));
        const boxes: Box[] = [...o.furniture, ...(o.props ?? []), ...c.placed.map((p) => p.box)];
        for (const p of c.placed) {
          placed++;
          const s = standSpotOf(o, p, c.placed);
          if (s === null) continue;
          spotted++;
          const hit = boxes.find((b) => inside(s, b));
          if (hit) out.push(`${map} ${String(n)} ${p.obj}: its spot ${s.x.toFixed(2)},${s.z.toFixed(2)} in a box`);
          const toward = Math.atan2((p.box.x0 + p.box.x1) / 2 - s.x, (p.box.z0 + p.box.z1) / 2 - s.z);
          if (Math.abs(Math.atan2(Math.sin(toward - s.face), Math.cos(toward - s.face))) > 1e-6) out.push(`${map} ${p.obj}: not facing it`);
        }
      }
    }
    expect(out.slice(0, 12)).toEqual([]);
    // ~88%: an object packed in a cluster (another object nearer every free spot round it) or past
    // a wall from the walk has none; its bim stays at its desk and the object plays its move
    expect(spotted / placed).toBeGreaterThan(0.85);
  }, 60_000);
});

describe('the office driver: a working bim walks to the object its call uses', () => {
  const t0 = 1_791_100_000;
  const clock = (t: number) => ({ t, local: localOf(new Date(t * 1000)), night: false, reduced: true, forced: null });
  const rows = [row('mistral:gmail', { name: 'gmail', kind: 'mistral', scope: 'mistral', state: 'loaded' })];
  const busy = (uses?: Uses) => agent('ada', { status: 'working', doing: { kind: 'tool', since_ms: 0, ...(uses ? { uses } : {}) } });
  const drive = (ada: Agent, t: number, d = makeDriver({ activities, labelsOf, layoutOf, newTimeline })) => {
    const crew = [mainAgent(), ada, agent('bob')];
    d.rows({ agents: crew, cards: [], places: [] }, crew, 'loft');
    d.objects(officeObjects(rows, new Map()));
    d.plugins(rows);
    return { d, s: d.step(clock(t), NO_ATTENTION) };
  };
  const at = (s: Step | null, name: string) => s?.frames.find((f) => f.name === name)?.pose;

  it('a call to gmail: it stands at the object, facing it, and the object is in use', () => {
    const { d } = drive(busy({ kind: 'group', name: 'gmail' }), t0);
    const s = d.step(clock(t0 + 60), NO_ATTENTION);
    const p = at(s, 'ada');
    const obj = s?.corners.placed.find((q) => q.key === 'mistral:gmail');
    expect(obj).toBeDefined();
    expect(p?.seg.act.at).toBe('object');
    const spot = p?.seg.act.use?.spot;
    expect(spot).toBeDefined();
    if (p === undefined || spot === undefined) return;
    expect(Math.hypot(p.x - spot.x, p.z - spot.z)).toBeLessThan(1e-3);
    expect([...(s?.using ?? [])]).toEqual(['mistral:gmail']);
    expect(s?.look.using?.has('mistral:gmail')).toBe(true);
  });

  it('a call of no plugin of his: at its desk, nothing in use', () => {
    const { d } = drive(busy({ kind: 'group', name: 'nobody' }), t0);
    const s = d.step(clock(t0 + 60), NO_ATTENTION);
    expect(at(s, 'ada')?.seg.act.at).toBe('desk');
    expect(s?.using.size).toBe(0);
    expect(s?.look.using).toBeUndefined();
  });

  it('no plugins rows heard: no trip', () => {
    const d = makeDriver({ activities, labelsOf, layoutOf, newTimeline });
    const crew = [mainAgent(), busy({ kind: 'group', name: 'gmail' })];
    d.rows({ agents: crew, cards: [], places: [] }, crew, 'loft');
    d.objects(officeObjects(rows, new Map()));
    d.step(clock(t0), NO_ATTENTION);
    expect(at(d.step(clock(t0 + 60), NO_ATTENTION), 'ada')?.seg.act.at).toBe('desk');
  });

  it('it stays HOLD_S after its last call there, then walks back to its desk', () => {
    const { d } = drive(busy({ kind: 'group', name: 'gmail' }), t0);
    d.step(clock(t0 + 20), NO_ATTENTION);
    // its next call names nothing: a fresh snapshot
    drive(busy(), t0 + 21, d);
    expect(at(d.step(clock(t0 + 21 + HOLD_S / 2), NO_ATTENTION), 'ada')?.seg.act.at).toBe('object');
    drive(busy(), t0 + 21 + HOLD_S + 1, d);
    expect(at(d.step(clock(t0 + 21 + HOLD_S + 60), NO_ATTENTION), 'ada')?.seg.act.at).toBe('desk');
  });

  it('a state wins: asked a question, it leaves the object at once', () => {
    const { d } = drive(busy({ kind: 'group', name: 'gmail' }), t0);
    d.step(clock(t0 + 20), NO_ATTENTION);
    drive(agent('ada', { status: 'blocked', doing: { kind: 'tool', since_ms: 0, uses: { kind: 'group', name: 'gmail' } } }), t0 + 21, d);
    expect(at(d.step(clock(t0 + 22), NO_ATTENTION), 'ada')?.seg.act.at).not.toBe('object');
  });
});
