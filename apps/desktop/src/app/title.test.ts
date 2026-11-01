// The bar's title is what he looks at, its subtitle where it sits (nearest first), each earlier
// part still a way back; the window's title is the bar's.
import { hero } from '@/proto/words_hero.gen';
import { describe, expect, it } from 'vitest';
import { barTitleOf, stateFactOf, windowTitleOf } from './title';

const name = (p: string) => (p === 'acme-1' ? 'acme' : p);

describe('barTitleOf', () => {
  it("an agent's thread: the agent, then its own title and its project (a way back to the project's main)", () => {
    const t = barTitleOf({ screen: 'thread', project: 'acme-1', agent: 'perf' }, name, () => [], 'profiling the hub');
    expect(t.title).toBe('perf');
    expect(t.sub).toEqual([{ c: 'profiling the hub' }, { c: 'acme', go: { screen: 'thread', project: 'acme-1', agent: 'main' } }]);
    expect(windowTitleOf(t)).toBe('perf');
  });

  it('a page is its own word, nothing under it', () => {
    expect(barTitleOf({ screen: 'inbox' }, name)).toEqual({ title: 'inbox', sub: [] });
    expect(barTitleOf({ screen: 'scheduled' }, name).title).toBe('scheduled');
  });

  it("a project's screen: its word, then the project", () => {
    const t = barTitleOf({ screen: 'changes', project: 'acme-1' }, name);
    expect(t.title).toBe(hero.changes_title);
    expect(t.sub.map((p) => p.c)).toEqual(['acme']);
  });
});

describe("the subtitle's one fact on a thread (designer m_43892)", () => {
  const NOW = 1_791_100_000_000;
  const MIN = 60_000;
  const a = (over: Partial<{ status: string; waits: number; since_ms: number; turn_ms: number | null; archived: boolean; waiting_on: { who: string } | null }> = {}) => ({ status: 'idle', waits: 0, since_ms: NOW - 30 * MIN, archived: false, ...over });
  it('what waits on him first, then its work with its turn\'s age, else nothing', () => {
    expect(stateFactOf(a({ status: 'working', waits: 1 }), NOW)).toEqual({ kind: 'waits', text: 'waits for you' });
    expect(stateFactOf(a({ waiting_on: { who: 'you' } }), NOW)?.kind).toBe('waits');
    // turn_ms is the turn's duration (proto rows/agents.rs), never a timestamp
    expect(stateFactOf(a({ status: 'working', turn_ms: 3 * MIN }), NOW)).toEqual({ kind: 'working', text: 'working · 3 min' });
    // no turn_ms yet: the age since its status changed
    expect(stateFactOf(a({ status: 'working', since_ms: NOW - 5 * MIN }), NOW)).toEqual({ kind: 'working', text: 'working · 5 min' });
    expect(stateFactOf(a(), NOW)).toBeNull();
    expect(stateFactOf(a({ status: 'done' }), NOW)).toBeNull();
    expect(stateFactOf(a({ status: 'working', archived: true }), NOW)).toBeNull();
    expect(stateFactOf(undefined, NOW)).toBeNull();
  });
});
