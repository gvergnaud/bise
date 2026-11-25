import { describe, expect, it } from 'vitest';
import type { Agent } from '@/proto';
import { hub_ev } from '@/proto/fixtures.gen';
import { ago, archivedEnd, archivedOf, dotOf, endOf, heldNumbers, holds, HOLD_MS, lineOf, liveOf, noteJump, numbersOf, rankedOf, subOf } from './model';

const T = 1_791_100_000_000;
const ag = (name: string, over: Partial<Agent> = {}): Agent => ({ name, dir: name, main: false, status: 'idle', archived: false, title: '', purpose: '', since_ms: T, waits: 0, watching: [], ...over });

describe('the agents list', () => {
  it("the fixture's agents: main first, then the hub's order", () => {
    const ev = hub_ev.find((e) => e.ev === 'agents');
    if (ev?.ev !== 'agents') throw new Error('no agents fixture');
    const names = liveOf(ev.agents).map((a) => a.name);
    expect(names[0]).toBe('main');
    expect(new Set(names)).toEqual(new Set(ev.agents.filter((a) => !a.archived).map((a) => a.name)));
  });

  it("main, then the hub's (start) order as the TUI's panel: a status never moves a row; archived apart", () => {
    const list = [
      ag('rest', { since_ms: T + 9 }),
      ag('done', { status: 'done' }),
      ag('work-old', { status: 'working', since_ms: T }),
      ag('work-new', { status: 'working', since_ms: T + 5 }),
      ag('asks', { status: 'working', waits: 2 }),
      ag('stuck', { status: 'blocked' }),
      ag('broke', { status: 'failed' }),
      ag('gone', { archived: true }),
      ag('main', { main: true }),
    ];
    expect(liveOf(list).map((a) => a.name)).toEqual(['main', 'rest', 'done', 'work-old', 'work-new', 'asks', 'stuck', 'broke']);
    expect(archivedOf(list).map((a) => a.name)).toEqual(['gone']);
  });

  it('dots, ends and ages', () => {
    expect(dotOf('working')).toBe('wk');
    expect(dotOf('waiting')).toBe('nd');
    expect(dotOf('failed')).toBe('er');
    expect(endOf(ag('a', { waits: 3 }), T)).toEqual({ text: 'waits', needs: true });
    expect(endOf(ag('a', { status: 'working' }), T + 12 * 60_000)).toEqual({ text: '12 min', needs: false });
    expect(endOf(ag('a', { status: 'done' }), T)).toEqual({ text: 'done', needs: false });
    // wt-disk: its worktree waits for disk: it says so, not its age, and asks nothing of him (designer m_67943)
    expect(endOf(ag('a', { status: 'working', phase: 'waiting_disk' }), T + 12 * 60_000)).toEqual({ text: 'waiting for disk', needs: false });
    // wt-disk: a refused one says 'failed' and why, once, even beside a failed report (designer m_67975, architect m_67978)
    const why = 'refused: 8 GB free, under 10 GB · free some space, then ask bise again';
    const refused = ag('b', { status: 'failed', failure: why, report: { kind: 'failed', text: why, at_ms: T } });
    const s = subOf(refused, T);
    expect(s).toEqual({ end: 'failed', why, report: '' });
    expect(`${s.end} · ${s.why}`.match(/failed/g)).toHaveLength(1);
    // no reason: its failed report's words beside 'failed', never 'failed · failed' (ui-review m_67988); not failed: no why
    const noWhy = subOf(ag('c', { status: 'failed', report: { kind: 'failed', text: 'tests red', at_ms: T } }), T);
    expect(noWhy).toEqual({ end: 'failed', why: '', report: 'tests red' });
    expect(`${noWhy.end} · ${noWhy.report}`.match(/failed/g)).toHaveLength(1);
    expect(subOf(ag('e', { status: 'idle', report: { kind: 'done', text: 'shipped', at_ms: T } }), T).report).toBe('done · shipped');
    expect(subOf(ag('d', { status: 'idle', failure: 'old' }), T).why).toBe('');
    // an archived one on /archived (designer m_19489): how it ended, then how long ago
    expect(archivedEnd(ag('a', { archived: true, status: 'done' }), T + 50 * 3_600_000)).toBe('✓ done · 2 d');
    expect(archivedEnd(ag('a', { archived: true, status: 'idle' }), T + 3 * 3_600_000)).toBe('– stopped · 3 h');
    expect(archivedEnd(ag('a', { archived: true, status: 'failed' }), T)).toBe('– stopped · now');
    expect(ago(T, T + 30_000)).toBe('now');
    expect(ago(T, T + 3 * 3_600_000)).toBe('3 h');
    expect(ago(T, T + 50 * 3_600_000)).toBe('2 d');
    expect(lineOf(ag('a', { purpose: 'make it fast' }))).toBe('make it fast');
    expect(lineOf(ag('a', { title: 'profiling', purpose: 'x' }))).toBe('profiling');
  });

  it("⌥ numbers are the hub's rank (agent-rank): main 0, then by rank; an older hub's rows in start order", () => {
    const main = ag('main', { main: true, rank: 0 });
    const [p99, alerts, docs] = [ag('p99', { rank: 3 }), ag('alerts', { rank: 1 }), ag('docs', { rank: 2 })];
    expect([...rankedOf([p99, main, alerts, docs])]).toEqual([['main', 0], ['alerts', 1], ['docs', 2], ['p99', 3]]);
    expect([...rankedOf([main, ag('x'), ag('y')])]).toEqual([['main', 0], ['x', 1], ['y', 2]]);
  });

  it("⌥ numbers read .rank, plus pins.ts's partition: his pinned agents first, each group in the hub's rank; ⌥N is the Nth row", () => {
    const main = ag('main', { main: true, rank: 0 });
    const [p99, alerts, docs, ci] = [ag('p99', { rank: 3 }), ag('alerts', { rank: 1 }), ag('docs', { rank: 2 }), ag('ci', { rank: 4 })];
    const live = [p99, main, alerts, docs, ci];
    // no pin: the hub's rank itself
    expect(rankedOf(live, [])).toEqual(rankedOf(live));
    // pinned in any order: they lead in the hub's rank (ci after p99), main stays 0, the rest keep theirs
    expect([...rankedOf(live, ['ci', 'p99'])].sort((a, b) => a[1] - b[1])).toEqual([['main', 0], ['p99', 1], ['ci', 2], ['alerts', 3], ['docs', 4]]);
    // a pin of a name no longer live changes nothing
    expect(rankedOf(live, ['gone'])).toEqual(rankedOf(live));
    // main pinned stays ⌥0
    expect(rankedOf(live, ['main']).get('main')).toBe(0);
  });

  it('the numbers hold still while ⌥ is held, and a moment after an ⌥N jump', () => {
    const shown = new Map([['main', 0], ['a', 1], ['b', 2], ['c', 3]]);
    // the hub moved c up, dropped a, brought d: held, b and c keep theirs, d goes last
    const next = new Map([['main', 0], ['c', 1], ['d', 2], ['b', 3]]);
    expect([...heldNumbers(shown, next)]).toEqual([['main', 0], ['b', 2], ['c', 3], ['d', 4]]);
    expect(holds(true, undefined, T)).toBe(true);
    expect(holds(false, T, T + HOLD_MS - 1)).toBe(true);
    expect(holds(false, T, T + HOLD_MS)).toBe(false);
    // through numbersOf: ⌥ held, a rank change from the hub moves nothing
    const main = ag('main', { main: true, rank: 0 });
    const before = numbersOf('p-hold', [main, ag('a', { rank: 1 }), ag('b', { rank: 2 })], false, T);
    const moved = [main, ag('a', { rank: 2 }), ag('b', { rank: 1 })];
    expect(numbersOf('p-hold', moved, true, T)).toEqual(before);
    // an ⌥N jump: still held after ⌥ goes, until HOLD_MS
    noteJump(T);
    expect(numbersOf('p-hold', moved, false, T + 1_000)).toEqual(before);
    expect(numbersOf('p-hold', moved, false, T + HOLD_MS).get('b')).toBe(1);
  });
});

describe('R9/S3: the agents list end says whom it waits on', () => {
  it("'waiting on bise' dim, 'waiting on you' in the accent", () => {
    expect(endOf(ag('a', { status: 'working', waiting_on: { who: 'agent', name: 'main' } }), T)).toEqual({ text: 'waiting on bise', needs: false });
    expect(endOf(ag('a', { waiting_on: { who: 'you' } }), T)).toEqual({ text: 'waiting on you', needs: true });
  });
});
