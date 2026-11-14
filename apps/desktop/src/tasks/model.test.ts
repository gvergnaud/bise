// The right bar's rows and the since line, on plain rows (model.ts, since.ts). Owner: tasks.
import { describe, expect, it } from 'vitest';
import type { WorkRow, WorkStatus } from '@/proto';
import { barRows, cutAtWord, goalsOf, type ProjWork } from './model';
import { sinceLooked } from './since';
import { tasksOf } from './chips';
import { resolveMerged } from './model';
import { originText } from './TaskTab';

const NOW = 1_700_000_000_000;
const H = 3_600_000;

function w(id: string, status: WorkStatus, last: number, extra: Partial<WorkRow> = {}): ProjWork {
  return { project: 'p', projectName: 'p', w: { id, name: id, by: 'main', created_ms: NOW - 48 * H, last_ms: last, status, ...extra } };
}

describe('the bar rows', () => {
  it('lists the open ones in the Board order, newest first inside a status, and folds ongoing and done this week', () => {
    const all = [
      w('a', 'working', NOW - 2 * H),
      w('b', 'needs_you', NOW - 5 * H),
      w('c', 'working', NOW - H),
      w('d', 'paused', NOW),
      w('e', 'to_review', NOW),
      w('f', 'ongoing', NOW),
      w('g', 'done', NOW, { done: { at_ms: NOW - 24 * H, by: 'user' } }),
      w('h', 'done', NOW, { done: { at_ms: NOW - 8 * 24 * H, by: 'user' } }),
    ];
    const r = barRows(all, NOW);
    expect(r.open.map((x) => x.w.id)).toEqual(['b', 'c', 'a', 'e', 'd']);
    expect(r.ongoing.map((x) => x.w.id)).toEqual(['f']);
    expect(r.done.map((x) => x.w.id)).toEqual(['g']);
  });

  it('counts the done-when items ticked of all, none without items', () => {
    expect(goalsOf({ goals: [{ text: 'x', done: true }, { text: 'y' }] })).toEqual({ done: 1, all: 2 });
    expect(goalsOf({})).toBeNull();
  });

  it('cuts a long name at a word', () => {
    expect(cutAtWord('Plugin apps in the gallery and the store', 20)).toBe('Plugin apps in the…');
    expect(cutAtWord('Short', 20)).toBe('Short');
  });
});

describe('the chips of a thing', () => {
  const all = [
    w('a', 'done', NOW - H, { agents: ['perf'], cards: [3], origin: 12 }),
    w('b', 'working', NOW - 2 * H, { agents: ['perf', 'docs'] }),
    w('c', 'paused', NOW, { agents: ['docs'], cards: [3] }),
  ].map((x) => x.w);
  it('reads an agent from the tasks it served, open ones first, then the newest', () => {
    expect(tasksOf(all, { agent: 'perf' }).map((x) => x.id)).toEqual(['b', 'a']);
    expect(tasksOf(all, { agent: 'docs' }).map((x) => x.id)).toEqual(['c', 'b']);
  });
  it('reads a card, his message and an artifact by their own fields', () => {
    expect(tasksOf(all, { card: 3 }).map((x) => x.id)).toEqual(['c', 'a']);
    expect(tasksOf(all, { origin: 12 }).map((x) => x.id)).toEqual(['a']);
    expect(tasksOf(all, { ids: ['b', 'gone'] }).map((x) => x.id)).toEqual(['b']);
  });
});

describe('since you looked', () => {
  it('counts what was done and what moved after his look, and every task that needs him now', () => {
    const since = NOW - 3 * H;
    const all = [
      w('a', 'done', NOW, { done: { at_ms: NOW - H, by: 'user' } }),
      w('b', 'done', NOW - 5 * H, { done: { at_ms: NOW - 5 * H, by: 'user' } }),
      w('c', 'working', NOW - H),
      w('d', 'waiting', NOW - 4 * H),
      w('e', 'needs_you', NOW - 10 * H),
    ];
    expect(sinceLooked(all, since)).toEqual({ done: 1, moving: 1, needs: 1 });
  });
});

describe('a merged task read as the one it went into', () => {
  it('follows the merges to the end, and stops on a loop', () => {
    const tombs = [{ id: 'a', merged_into: 'b' }, { id: 'b', merged_into: 'c' }, { id: 'x', merged_into: null }];
    expect(resolveMerged(tombs, 'a')).toBe('c');
    expect(resolveMerged(tombs, 'x')).toBe('x');
    expect(resolveMerged(tombs, 'live')).toBe('live');
    expect(['p', 'q']).toContain(resolveMerged([{ id: 'p', merged_into: 'q' }, { id: 'q', merged_into: 'p' }], 'p'));
  });
});

describe("his message quoted on the Task tab", () => {
  it('reads his message by its id, none when this window does not hold it', () => {
    const es = [undefined, { kind: 'agent', msg_id: 7, text: 'not his' }, { kind: 'you', msg_id: 7, text: ' make the app feel native ' }] as const;
    expect(originText(es as never, 7)).toBe('make the app feel native');
    expect(originText(es as never, 8)).toBeNull();
  });
});
